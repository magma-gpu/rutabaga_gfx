// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap as Map;
use std::io::IoSlice;
use std::io::IoSliceMut;
use std::mem::size_of;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use log::error;
use magma_gpu::protocols::broccolini_protocol::broccolini_create_fence;
use magma_gpu::util::create_event_pair;
use magma_gpu::util::create_pipe;
use magma_gpu::util::AsBorrowedDescriptor;
use magma_gpu::util::Error as MagmaGpuError;
use magma_gpu::util::EventSignaler;
use magma_gpu::util::EventWaiter;
use magma_gpu::util::Reader;
use magma_gpu::util::WaitContext;
use magma_gpu::util::WaitTimeout;

use super::context::FenceState;
use super::context::RemoteContext;
use super::ZygoteChannel;
use crate::handle::RutabagaHandle;
use crate::resource::RutabagaResource;
use crate::rutabaga_core::RutabagaComponent;
use crate::rutabaga_core::RutabagaContext;
use crate::rutabaga_utils::ResourceCreate3D;
use crate::rutabaga_utils::ResourceCreateBlob;
use crate::rutabaga_utils::RutabagaComponentType;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaFence;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaIovec;
use crate::rutabaga_utils::RutabagaResult;
use crate::rutabaga_utils::Transfer3D;
use crate::rutabaga_utils::RUTABAGA_BLOB_MEM_GUEST;
use crate::rutabaga_utils::RUTABAGA_CONTEXT_INIT_CAPSET_ID_MASK;

const REMOTE_FENCE_SHUTDOWN_ID: u64 = 0;

pub struct RemoteComponent {
    zygote: Arc<ZygoteChannel>,
    remote_capset_mask: u64,
    component_type: RutabagaComponentType,
    fence_handler: RutabagaFenceHandler,
    wait_ctx: Arc<WaitContext>,
    fence_contexts: Arc<Mutex<Map<u32, Arc<Mutex<FenceState>>>>>,
    shutdown_signaler: EventSignaler,
    fence_thread: Option<thread::JoinHandle<()>>,
}

fn run_fence_wait_loop(
    wait_ctx: Arc<WaitContext>,
    fence_contexts: Arc<Mutex<Map<u32, Arc<Mutex<FenceState>>>>>,
    _shutdown_waiter: EventWaiter,
) {
    let mut buf = [0u8; size_of::<broccolini_create_fence>()];

    while let Ok(events) = wait_ctx.wait(WaitTimeout::NoTimeout) {
        let mut should_exit = false;
        for event in events {
            if event.connection_id == REMOTE_FENCE_SHUTDOWN_ID {
                should_exit = true;
                break;
            }

            let ctx_id = event.connection_id as u32;
            let ctx_fence_state = fence_contexts.lock().unwrap().get(&ctx_id).cloned();
            let Some(ctx_fence_state) = ctx_fence_state else {
                continue;
            };

            let mut completed_fence: Option<(RutabagaFenceHandler, RutabagaFence)> = None;
            let mut worker_eof = false;

            if event.readable || event.hung_up {
                let mut state = ctx_fence_state.lock().unwrap();
                if state.worker_exited {
                    continue;
                }
                match state.read_pipe.read(&mut buf) {
                    Ok(0) => {
                        worker_eof = true;
                    }
                    Ok(bytes_read) if bytes_read == buf.len() => {
                        let mut reader = Reader::new(&buf);
                        if let Ok(cmd) = reader.read_obj::<broccolini_create_fence>() {
                            state.pending.remove(&cmd.fence_id);
                            completed_fence = Some((
                                state.fence_handler.clone(),
                                RutabagaFence {
                                    flags: cmd.flags,
                                    fence_id: cmd.fence_id,
                                    ctx_id: cmd.hdr.payload,
                                    ring_idx: cmd.ring_idx,
                                },
                            ));
                        }
                    }
                    Ok(_) => {
                        worker_eof = true;
                    }
                    Err(MagmaGpuError::IoError(ref e))
                        if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => {
                        worker_eof = true;
                    }
                }
            }

            if let Some((handler, fence)) = completed_fence {
                handler.call(fence);
            }

            if worker_eof {
                fence_contexts.lock().unwrap().remove(&ctx_id);
                let (orphaned_fences, handler) = {
                    let mut state = ctx_fence_state.lock().unwrap();
                    if !state.worker_exited {
                        let _ = wait_ctx.delete(state.read_pipe.as_borrowed_descriptor());
                        state.worker_exited = true;
                    }
                    (
                        std::mem::take(&mut state.pending)
                            .into_values()
                            .collect::<Vec<_>>(),
                        state.fence_handler.clone(),
                    )
                };

                for (fence, signaler_opt) in orphaned_fences {
                    if let Some(signaler) = signaler_opt {
                        let _ = signaler.signal();
                    }
                    handler.call(fence);
                }
            }
        }

        if should_exit {
            break;
        }
    }
}

impl RemoteComponent {
    #[allow(dead_code)]
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        zygote: Arc<ZygoteChannel>,
        remote_capset_mask: u64,
        component_type: RutabagaComponentType,
        fence_handler: RutabagaFenceHandler,
    ) -> RutabagaResult<Box<dyn RutabagaComponent>> {
        let wait_ctx = Arc::new(WaitContext::new()?);
        let (shutdown_signaler, shutdown_waiter) = create_event_pair()?;
        wait_ctx.add(
            REMOTE_FENCE_SHUTDOWN_ID,
            shutdown_waiter.as_borrowed_descriptor(),
        )?;

        let fence_contexts: Arc<Mutex<Map<u32, Arc<Mutex<FenceState>>>>> =
            Arc::new(Mutex::new(Map::new()));
        let thread_wait_ctx = wait_ctx.clone();
        let thread_fence_contexts = fence_contexts.clone();

        let fence_thread = thread::Builder::new()
            .name("rutabaga_remote_fence".to_string())
            .spawn(move || {
                run_fence_wait_loop(thread_wait_ctx, thread_fence_contexts, shutdown_waiter)
            })
            .map_err(MagmaGpuError::IoError)?;

        Ok(Box::new(RemoteComponent {
            zygote,
            remote_capset_mask,
            component_type,
            fence_handler,
            wait_ctx,
            fence_contexts,
            shutdown_signaler,
            fence_thread: Some(fence_thread),
        }))
    }

    fn is_remote_capset(&self, capset_id: u32) -> bool {
        capset_id < 64 && (self.remote_capset_mask & (1u64 << capset_id)) != 0
    }
}

impl Drop for RemoteComponent {
    fn drop(&mut self) {
        let _ = self.shutdown_signaler.signal();
        if let Some(handle) = self.fence_thread.take() {
            let _ = handle.join();
        }
    }
}

impl RutabagaComponent for RemoteComponent {
    fn get_capset_info(&self, capset_id: u32) -> (u32, u32) {
        if self.is_remote_capset(capset_id) {
            return self.zygote.get_capset_info(capset_id);
        }
        (0, 0)
    }

    fn get_capset(&self, capset_id: u32, version: u32) -> Vec<u8> {
        if self.is_remote_capset(capset_id) {
            return self.zygote.get_capset(capset_id, version);
        }
        Vec::new()
    }

    fn create_fence(&mut self, fence: RutabagaFence) -> RutabagaResult<()> {
        self.fence_handler.call(fence);
        Ok(())
    }

    fn create_3d(
        &self,
        resource_id: u32,
        resource_create_3d: ResourceCreate3D,
    ) -> RutabagaResult<RutabagaResource> {
        RutabagaResource::new_2d(resource_id, resource_create_3d, self.component_type)
    }

    fn transfer_write(
        &self,
        ctx_id: u32,
        resource: &mut RutabagaResource,
        transfer: Transfer3D,
        buf: Option<IoSlice>,
    ) -> RutabagaResult<()> {
        if ctx_id != 0 || resource.blob {
            error!(
                "RemoteComponent does not support 3D or blob transfer_write (ctx_id={}, resource_id={}); use blob mappings",
                ctx_id, resource.resource_id
            );
            return Err(MagmaGpuError::WithContext(
                "3D context transfers are not supported by RemoteComponent; use blob resources",
            )
            .into());
        }
        resource.transfer_write_2d(transfer, buf)
    }

    fn transfer_read(
        &self,
        ctx_id: u32,
        resource: &mut RutabagaResource,
        transfer: Transfer3D,
        buf: Option<IoSliceMut>,
    ) -> RutabagaResult<()> {
        if ctx_id != 0 {
            error!(
                "RemoteComponent does not support 3D context transfer_read (ctx_id={}, resource_id={}); use blob resources",
                ctx_id, resource.resource_id
            );
            return Err(MagmaGpuError::WithContext(
                "3D context transfers are not supported by RemoteComponent; use blob resources",
            )
            .into());
        }
        resource.transfer_read_2d(transfer, buf)
    }

    fn resource_flush(&self, _resource: &mut RutabagaResource) -> RutabagaResult<()> {
        Ok(())
    }

    fn create_blob(
        &mut self,
        _ctx_id: u32,
        resource_id: u32,
        resource_create_blob: ResourceCreateBlob,
        iovec_opt: Option<Vec<RutabagaIovec>>,
        handle_opt: Option<RutabagaHandle>,
    ) -> RutabagaResult<RutabagaResource> {
        if resource_create_blob.blob_mem != RUTABAGA_BLOB_MEM_GUEST && handle_opt.is_none() {
            return Err(MagmaGpuError::Unsupported.into());
        }
        Ok(RutabagaResource {
            resource_id,
            handle: handle_opt.map(Arc::new),
            blob: true,
            blob_mem: resource_create_blob.blob_mem,
            blob_flags: resource_create_blob.blob_flags,
            map_info: None,
            info_2d: None,
            info_3d: None,
            vulkan_info: None,
            backing_iovecs: iovec_opt,
            component_mask: 1 << (self.component_type as u8),
            size: resource_create_blob.size,
            mapping: None,
        })
    }

    fn create_context(
        &self,
        ctx_id: u32,
        context_init: u32,
        context_name: Option<&str>,
        fence_handler: RutabagaFenceHandler,
    ) -> RutabagaResult<Box<dyn RutabagaContext>> {
        let capset_id = context_init & RUTABAGA_CONTEXT_INIT_CAPSET_ID_MASK;
        if !self.is_remote_capset(capset_id) {
            return Err(RutabagaError::InvalidComponent);
        }

        let (fence_read_pipe, fence_write_pipe) = create_pipe()?;
        let fence_write_desc = fence_write_pipe
            .as_borrowed_descriptor()
            .try_clone()
            .map_err(MagmaGpuError::IoError)?;
        drop(fence_write_pipe);

        let stream =
            self.zygote
                .spawn_context(ctx_id, context_init, context_name, fence_write_desc)?;

        let fence_state = Arc::new(Mutex::new(FenceState {
            read_pipe: fence_read_pipe,
            fence_handler: fence_handler.clone(),
            pending: Map::new(),
            worker_exited: false,
        }));

        self.fence_contexts
            .lock()
            .unwrap()
            .insert(ctx_id, fence_state.clone());

        if let Err(e) = self.wait_ctx.add(
            ctx_id as u64,
            fence_state
                .lock()
                .unwrap()
                .read_pipe
                .as_borrowed_descriptor(),
        ) {
            self.fence_contexts.lock().unwrap().remove(&ctx_id);
            return Err(e.into());
        }

        Ok(RemoteContext::new(
            ctx_id,
            stream,
            self.component_type,
            fence_handler,
            fence_state,
            self.fence_contexts.clone(),
            self.wait_ctx.clone(),
        ))
    }
}
