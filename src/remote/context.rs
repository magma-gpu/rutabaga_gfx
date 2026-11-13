// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap as Map;
use std::sync::Arc;
use std::sync::Mutex;

use log::error;
use magma_gpu::protocols::broccolini_protocol::*;
use magma_gpu::protocols::ipc::Message;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::create_event_pair;
use magma_gpu::util::AsBorrowedDescriptor;
use magma_gpu::util::Error as MagmaGpuError;
use magma_gpu::util::EventSignaler;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::ReadPipe;
use magma_gpu::util::WaitContext;
use magma_gpu::util::MAGMA_GPU_HANDLE_TYPE_MEM_SHM;

use crate::handle::RutabagaHandle;
use crate::resource::RutabagaResource;
use crate::rutabaga_core::RutabagaContext;
use crate::rutabaga_utils::DeviceId as RutabagaDeviceId;
use crate::rutabaga_utils::Resource3DInfo;
use crate::rutabaga_utils::ResourceCreateBlob;
use crate::rutabaga_utils::RutabagaComponentType;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaFence;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaIovec;
use crate::rutabaga_utils::RutabagaResult;
use crate::rutabaga_utils::VulkanInfo as RutabagaVulkanInfo;
use crate::rutabaga_utils::RUTABAGA_BLOB_MEM_GUEST;
use crate::rutabaga_utils::RUTABAGA_FLAG_FENCE_HOST_SHAREABLE;
use crate::rutabaga_utils::RUTABAGA_IMPORT_FLAG_3D_INFO;
use crate::rutabaga_utils::RUTABAGA_IMPORT_FLAG_VULKAN_INFO;
use crate::rutabaga_utils::RUTABAGA_MAP_ACCESS_RW;
use crate::rutabaga_utils::RUTABAGA_MAP_CACHE_CACHED;

pub(crate) struct FenceState {
    pub(crate) read_pipe: ReadPipe,
    pub(crate) fence_handler: RutabagaFenceHandler,
    pub(crate) pending: Map<u64, (RutabagaFence, Option<EventSignaler>)>,
    pub(crate) worker_exited: bool,
}

pub(crate) struct RemoteContext {
    ctx_id: u32,
    stream: Stream<BroccoliniProtocol>,
    fence_handler: RutabagaFenceHandler,
    component_type: RutabagaComponentType,
    fence_state: Arc<Mutex<FenceState>>,
    fence_contexts: Arc<Mutex<Map<u32, Arc<Mutex<FenceState>>>>>,
    wait_ctx: Arc<WaitContext>,
}

impl RemoteContext {
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new(
        ctx_id: u32,
        stream: Stream<BroccoliniProtocol>,
        component_type: RutabagaComponentType,
        fence_handler: RutabagaFenceHandler,
        fence_state: Arc<Mutex<FenceState>>,
        fence_contexts: Arc<Mutex<Map<u32, Arc<Mutex<FenceState>>>>>,
        wait_ctx: Arc<WaitContext>,
    ) -> Box<dyn RutabagaContext> {
        Box::new(RemoteContext {
            ctx_id,
            stream,
            fence_handler,
            component_type,
            fence_state,
            fence_contexts,
            wait_ctx,
        })
    }
}

impl RutabagaContext for RemoteContext {
    fn context_create_blob(
        &mut self,
        resource_id: u32,
        resource_create_blob: ResourceCreateBlob,
        iovec_opt: Option<Vec<RutabagaIovec>>,
        handle_opt: Option<RutabagaHandle>,
    ) -> RutabagaResult<RutabagaResource> {
        if resource_create_blob.blob_mem == RUTABAGA_BLOB_MEM_GUEST {
            return Ok(RutabagaResource {
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
            });
        }

        let input_handle = handle_opt.map(MagmaGpuHandle::try_from).transpose()?;
        let cmd = broccolini_resource_create_blob {
            hdr: broccolini_ctrl_hdr {
                type_: BROCCOLINI_CMD_RESOURCE_CREATE_BLOB,
                payload: input_handle.as_ref().map_or(0, |h| h.handle_type),
            },
            resource_id,
            blob_mem: resource_create_blob.blob_mem,
            blob_flags: resource_create_blob.blob_flags,
            padding: 0,
            blob_id: resource_create_blob.blob_id,
            size: resource_create_blob.size,
        };
        if let Some(handle) = input_handle {
            self.stream.write(Message::CmdWithHandle(cmd, handle))?;
        } else {
            self.stream.write(Message::Cmd(cmd))?;
        }

        let protocols = self.stream.read()?;
        for protocol in protocols {
            if let BroccoliniProtocol::RespResourceCreate(resp, handle) = protocol {
                let rutabaga_vk_info = RutabagaVulkanInfo {
                    memory_idx: resp.vulkan_info.memory_idx,
                    device_id: RutabagaDeviceId {
                        device_uuid: resp.vulkan_info.device_id.device_uuid,
                        driver_uuid: resp.vulkan_info.device_id.driver_uuid,
                    },
                };
                let vulkan_info = if resp.flags & BROCCOLINI_RESOURCE_FLAG_VULKAN_INFO == 0
                    || handle.handle_type == MAGMA_GPU_HANDLE_TYPE_MEM_SHM
                    || rutabaga_vk_info == RutabagaVulkanInfo::default()
                {
                    None
                } else {
                    Some(rutabaga_vk_info)
                };

                let map_info =
                    if resp.flags & BROCCOLINI_RESOURCE_FLAG_MAP_INFO != 0 && resp.map_info != 0 {
                        resp.map_info | RUTABAGA_MAP_ACCESS_RW
                    } else {
                        RUTABAGA_MAP_CACHE_CACHED | RUTABAGA_MAP_ACCESS_RW
                    };

                let info_3d = (resp.flags & BROCCOLINI_RESOURCE_FLAG_3D_INFO != 0).then_some(
                    Resource3DInfo {
                        width: resp.width,
                        height: resp.height,
                        drm_fourcc: resp.drm_fourcc,
                        strides: resp.strides,
                        offsets: resp.offsets,
                        modifier: resp.modifier,
                    },
                );

                return Ok(RutabagaResource {
                    resource_id,
                    handle: Some(Arc::new(handle.into())),
                    blob: true,
                    blob_mem: resource_create_blob.blob_mem,
                    blob_flags: resource_create_blob.blob_flags,
                    map_info: Some(map_info),
                    info_2d: None,
                    info_3d,
                    vulkan_info,
                    backing_iovecs: iovec_opt,
                    component_mask: 1 << (self.component_type as u8),
                    size: resource_create_blob.size,
                    mapping: None,
                });
            }
        }

        Err(RutabagaError::InvalidResourceId)
    }

    fn submit_cmd(
        &mut self,
        commands: &mut [u8],
        fence_ids: &[u64],
        shareable_fences: Vec<MagmaGpuHandle>,
    ) -> RutabagaResult<()> {
        if shareable_fences.len() > 4 {
            return Err(MagmaGpuError::Unsupported.into());
        }
        let mut in_fence_handle_types = [0u32; 4];
        for (i, handle) in shareable_fences.iter().enumerate() {
            in_fence_handle_types[i] = handle.handle_type;
        }
        let cmd = broccolini_cmd_submit_3d {
            hdr: broccolini_ctrl_hdr {
                type_: BROCCOLINI_CMD_SUBMIT_3D,
                payload: shareable_fences.len() as u32,
            },
            cmd_size: commands.len() as u32,
            num_in_fences: fence_ids.len() as u32,
            in_fence_handle_types,
        };
        let mut payload = Vec::with_capacity(fence_ids.len() * 8 + commands.len());
        for &fence_id in fence_ids {
            payload.extend_from_slice(&fence_id.to_ne_bytes());
        }
        payload.extend_from_slice(commands);

        if let Err(e) = self.stream.write(Message::CmdWithDataAndHandles(
            cmd,
            &payload,
            shareable_fences,
        )) {
            error!(
                "rutabaga subprocess for ctx_id={} failed during submit_cmd: {:?}",
                self.ctx_id, e
            );
        }
        Ok(())
    }

    fn attach(&mut self, resource: &mut RutabagaResource) {
        if !resource.blob {
            error!(
                "RemoteContext (ctx_id={}) cannot attach non-blob 3D resource {}; remote contexts require blob resources",
                self.ctx_id, resource.resource_id
            );
            return;
        }
        let Some(ref handle) = resource.handle else {
            error!(
                "RemoteContext (ctx_id={}) cannot attach blob resource {} without an OS handle",
                self.ctx_id, resource.resource_id
            );
            return;
        };
        let Some(mesa_handle) = handle.as_mesa_handle() else {
            return;
        };
        let Ok(cloned_handle) = mesa_handle.try_clone() else {
            return;
        };

        let mut flags = 0u32;
        let info_3d = if let Some(info) = resource.info_3d {
            flags |= RUTABAGA_IMPORT_FLAG_3D_INFO;
            info
        } else {
            Resource3DInfo {
                width: 0,
                height: 0,
                drm_fourcc: 0,
                strides: [0; 4],
                offsets: [0; 4],
                modifier: 0,
            }
        };
        let vulkan_info = if let Some(vk_info) = resource.vulkan_info {
            flags |= RUTABAGA_IMPORT_FLAG_VULKAN_INFO;
            VulkanInfo {
                memory_idx: vk_info.memory_idx,
                device_id: DeviceId {
                    device_uuid: vk_info.device_id.device_uuid,
                    driver_uuid: vk_info.device_id.driver_uuid,
                },
            }
        } else {
            VulkanInfo::default()
        };
        let attach_cmd = broccolini_ctx_attach_resource {
            hdr: broccolini_ctrl_hdr {
                type_: BROCCOLINI_CMD_CTX_ATTACH_RESOURCE,
                payload: 0,
            },
            resource_id: resource.resource_id,
            handle_type: cloned_handle.handle_type,
            flags,
            width: info_3d.width,
            height: info_3d.height,
            drm_fourcc: info_3d.drm_fourcc,
            strides: info_3d.strides,
            offsets: info_3d.offsets,
            modifier: info_3d.modifier,
            size: resource.size,
            vulkan_info,
            padding: 0,
        };
        if self
            .stream
            .write(Message::CmdWithHandle(attach_cmd, cloned_handle))
            .is_ok()
        {
            let _ = self.stream.read();
        }
    }

    fn detach(&mut self, resource: &RutabagaResource) {
        let _ = self.stream.write(Message::Cmd(broccolini_ctrl_hdr {
            type_: BROCCOLINI_CMD_CTX_DETACH_RESOURCE,
            payload: resource.resource_id,
        }));
    }

    fn context_create_fence(
        &mut self,
        fence: RutabagaFence,
    ) -> RutabagaResult<Option<MagmaGpuHandle>> {
        if self.component_type != RutabagaComponentType::CrossDomain && fence.ring_idx as u32 == 1 {
            self.fence_handler.call(fence);
            return Ok(None);
        }

        let is_shareable = fence.flags & RUTABAGA_FLAG_FENCE_HOST_SHAREABLE != 0;

        {
            let mut state = self.fence_state.lock().unwrap();
            if state.worker_exited {
                drop(state);
                self.fence_handler.call(fence);
                if is_shareable {
                    let (_signaler, waiter) = create_event_pair()?;
                    if let Ok(fallback) = waiter.signaler() {
                        let _ = fallback.signal();
                    }
                    return Ok(Some(waiter.into()));
                }
                return Ok(None);
            }
            state.pending.insert(fence.fence_id, (fence, None));
        }

        let cmd = broccolini_create_fence {
            hdr: broccolini_ctrl_hdr {
                type_: BROCCOLINI_CMD_CREATE_FENCE,
                payload: 0,
            },
            flags: fence.flags,
            ring_idx: fence.ring_idx,
            padding: [0; 3],
            fence_id: fence.fence_id,
        };

        if self.stream.write(Message::Cmd(cmd)).is_err() {
            let removed = self
                .fence_state
                .lock()
                .unwrap()
                .pending
                .remove(&fence.fence_id);
            if let Some((f, signaler_opt)) = removed {
                if let Some(signaler) = signaler_opt {
                    let _ = signaler.signal();
                }
                self.fence_handler.call(f);
            }
            if is_shareable {
                let (_signaler, waiter) = create_event_pair()?;
                if let Ok(fallback) = waiter.signaler() {
                    let _ = fallback.signal();
                }
                return Ok(Some(waiter.into()));
            }
            return Ok(None);
        }

        if is_shareable {
            let protocols = self.stream.read()?;
            for protocol in protocols {
                if let BroccoliniProtocol::RespCreateFence(_fence_id, handle) = protocol {
                    return Ok(Some(handle));
                }
            }
            return Err(RutabagaError::InvalidContextId);
        }

        Ok(None)
    }

    fn component_type(&self) -> RutabagaComponentType {
        self.component_type
    }
}

impl Drop for RemoteContext {
    fn drop(&mut self) {
        let _ = self.stream.write(Message::Cmd(broccolini_ctrl_hdr {
            type_: BROCCOLINI_CMD_CTX_DESTROY,
            payload: 0,
        }));
        self.fence_contexts.lock().unwrap().remove(&self.ctx_id);
        let orphaned_fences = if let Ok(mut state) = self.fence_state.lock() {
            if !state.worker_exited {
                let _ = self
                    .wait_ctx
                    .delete(state.read_pipe.as_borrowed_descriptor());
                state.worker_exited = true;
            }
            std::mem::take(&mut state.pending)
                .into_values()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        for (fence, signaler_opt) in orphaned_fences {
            if let Some(signaler) = signaler_opt {
                let _ = signaler.signal();
            }
            self.fence_handler.call(fence);
        }
    }
}
