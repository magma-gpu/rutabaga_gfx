// Copyright 2026 Magma-GPU project
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::sync::Arc;

use log::error;
use magma_gpu::util::AsBorrowedDescriptor;
use magma_gpu::util::AsRawDescriptor;
use magma_gpu::util::DescriptorType;
use magma_gpu::util::Error as MagmaGpuError;
use magma_gpu::util::Event;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::WaitContext;
use magma_gpu::util::WaitTimeout;
use magma_gpu::util::WritePipe;
use magma_gpu::util::MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD;

use crate::cross_domain::common::add_item;
use crate::cross_domain::common::CrossDomainItem;
use crate::cross_domain::common::CrossDomainItemState;
use crate::cross_domain::common::CrossDomainJob;
use crate::cross_domain::common::CrossDomainState;
use crate::cross_domain::common::RingWrite;
use crate::cross_domain::common::CROSS_DOMAIN_CONTEXT_CHANNEL_ID;
use crate::cross_domain::common::CROSS_DOMAIN_KILL_ID;
use crate::cross_domain::common::CROSS_DOMAIN_RESAMPLE_ID;
use crate::cross_domain::cross_domain_protocol::CrossDomainReadWrite;
use crate::cross_domain::cross_domain_protocol::CrossDomainSendReceive;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_ATOMIC_MEMORY_SENTINEL_START;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_READ;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_CMD_RECEIVE;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_ID_TYPE_EVENT;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_ID_TYPE_SOCKET;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_ID_TYPE_VIRTGPU_BLOB;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_ID_TYPE_WRITE_PIPE;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_PIPE_READ_START;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaFence;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaResult;

const CROSS_DOMAIN_DEFAULT_BUFFER_SIZE: usize = 4096;
const CROSS_DOMAIN_MAX_SEND_RECV_SIZE: usize =
    CROSS_DOMAIN_DEFAULT_BUFFER_SIZE - size_of::<CrossDomainSendReceive>();

pub struct CrossDomainWorker {
    wait_ctx: WaitContext,
    state: Arc<CrossDomainState>,
    item_state: CrossDomainItemState,
    fence_handler: RutabagaFenceHandler,
}

impl CrossDomainWorker {
    pub fn new(
        wait_ctx: WaitContext,
        state: Arc<CrossDomainState>,
        item_state: CrossDomainItemState,
        fence_handler: RutabagaFenceHandler,
    ) -> CrossDomainWorker {
        CrossDomainWorker {
            wait_ctx,
            state,
            item_state,
            fence_handler,
        }
    }

    // Handles the fence according the the token according to the event token.  On success, a
    // boolean value indicating whether the worker thread should be stopped is returned.
    fn handle_fence(
        &mut self,
        fence: RutabagaFence,
        thread_resample_evt: &mut Event,
        receive_buf: &mut [u8],
    ) -> RutabagaResult<()> {
        let events = self.wait_ctx.wait(WaitTimeout::NoTimeout)?;

        // The worker thread must:
        //
        // (1) Poll the ContextChannel (usually Wayland)
        // (2) Poll a number of WaylandReadPipes
        // (3) handle jobs from the virtio-gpu thread.
        //
        // We can only process one event at a time, because each `handle_fence` call is associated
        // with a guest virtio-gpu fence.  Signaling the fence means it's okay for the guest to
        // access ring data.  If two events are available at the same time (say a ContextChannel
        // event and a WaylandReadPipe event), and we write responses for both using the same guest
        // fence data, that will break the expected order of events.  We need the guest to generate
        // a new fence before we can resume polling.
        //
        // The CrossDomainJob queue guarantees a new fence has been generated before polling is
        // resumed.
        if let Some(event) = events.first() {
            match event.connection_id {
                CROSS_DOMAIN_CONTEXT_CHANNEL_ID => {
                    self.process_receive(fence, receive_buf)?;
                }
                CROSS_DOMAIN_RESAMPLE_ID => {
                    // The resample event is triggered when the job queue is in the following state:
                    //
                    // [CrossDomain::AddReadPipe(..)] -> END
                    //
                    // After this event, the job queue will be the following state:
                    //
                    // [CrossDomain::AddReadPipe(..)] -> [CrossDomain::HandleFence(..)] -> END
                    //
                    // Fence handling is tied to some new data transfer across a pollable
                    // descriptor.  When we're adding new descriptors, we stop polling.
                    thread_resample_evt.wait()?;
                    self.state.add_job(CrossDomainJob::HandleFence(fence));
                }
                CROSS_DOMAIN_KILL_ID => {
                    self.fence_handler.call(fence);
                }
                id if id >= CROSS_DOMAIN_ATOMIC_MEMORY_SENTINEL_START as u64
                    && id < CROSS_DOMAIN_PIPE_READ_START as u64 =>
                {
                    let memory_watcher_id: u32 =
                        id.try_into().map_err(MagmaGpuError::TryFromIntError)?;
                    let mut manager = self.state.sentinel_manager.lock().unwrap();
                    let mut remove = false;
                    let mut fence_opt = Some(fence);

                    if manager.is_shutdown(memory_watcher_id) {
                        if let Some(evt) = manager.get_event(memory_watcher_id) {
                            self.wait_ctx.delete(evt.as_borrowed_descriptor())?;
                        }
                        remove = true;
                    } else if let Some(cmd_memory_watcher) =
                        manager.handle_event(memory_watcher_id)?
                    {
                        self.state.write_to_ring(
                            RingWrite::Write(cmd_memory_watcher, None),
                            self.state.channel_ring_id,
                        )?;
                        self.fence_handler.call(fence_opt.take().unwrap());
                    }

                    if let Some(fence) = fence_opt {
                        self.state.add_job(CrossDomainJob::HandleFence(fence));
                    }

                    if remove {
                        manager.remove_watcher(memory_watcher_id);
                    }
                }
                _ => {
                    let mut items = self.item_state.lock().unwrap();
                    let mut cmd_read: CrossDomainReadWrite = Default::default();
                    let item_id: u32 = event
                        .connection_id
                        .try_into()
                        .map_err(MagmaGpuError::TryFromIntError)?;
                    let bytes_read;

                    cmd_read.hdr.cmd = CROSS_DOMAIN_CMD_READ;
                    cmd_read.identifier = item_id;

                    let item = items
                        .table
                        .get_mut(&item_id)
                        .ok_or(RutabagaError::InvalidCrossDomainItemId)?;

                    match item {
                        CrossDomainItem::WaylandReadPipe(ref mut readpipe) => {
                            let ring_write =
                                RingWrite::WriteFromPipe(cmd_read, readpipe, event.readable);
                            bytes_read = self.state.write_to_ring::<CrossDomainReadWrite>(
                                ring_write,
                                self.state.channel_ring_id,
                            )?;

                            // Zero bytes read indicates end-of-file on POSIX.
                            if event.hung_up && bytes_read == 0 {
                                self.wait_ctx.delete(readpipe.as_borrowed_descriptor())?;
                            }
                        }
                        CrossDomainItem::Event(ref mut evt) => {
                            let ring_write =
                                RingWrite::WriteFromEvent(cmd_read, evt, event.readable);
                            bytes_read = self.state.write_to_ring::<CrossDomainReadWrite>(
                                ring_write,
                                self.state.channel_ring_id,
                            )?;
                        }
                        _ => return Err(RutabagaError::InvalidCrossDomainItemType),
                    }

                    if event.hung_up && bytes_read == 0 {
                        items.table.remove(&item_id);
                    }

                    self.fence_handler.call(fence);
                }
            }
        }

        Ok(())
    }

    fn process_receive(
        &mut self,
        fence: RutabagaFence,
        receive_buf: &mut [u8],
    ) -> RutabagaResult<()> {
        let (len, files) = self.state.receive_msg(receive_buf)?;
        let mut cmd_receive: CrossDomainSendReceive = Default::default();

        let num_files = files.len();
        cmd_receive.hdr.cmd = CROSS_DOMAIN_CMD_RECEIVE;
        cmd_receive.num_identifiers = files
            .len()
            .try_into()
            .map_err(MagmaGpuError::TryFromIntError)?;
        cmd_receive.opaque_data_size = len.try_into().map_err(MagmaGpuError::TryFromIntError)?;

        let iter = cmd_receive
            .identifiers
            .iter_mut()
            .zip(cmd_receive.identifier_types.iter_mut())
            .zip(cmd_receive.identifier_sizes.iter_mut())
            .map(|((i, it), is)| (i, it, is))
            .zip(files)
            .take(num_files);

        for ((identifier, identifier_type, identifier_size), file) in iter {
            {
                // Determine the descriptor type using the platform abstraction
                let desc_type = file
                    .determine_type()
                    .map_err(|e| RutabagaError::MagmaGpuError(e.into()))?;

                match desc_type {
                    DescriptorType::Event => {
                        *identifier_type = CROSS_DOMAIN_ID_TYPE_EVENT;
                        *identifier_size = 0;
                        let event = Event::try_from(MagmaGpuHandle {
                            os_handle: file,
                            handle_type: MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD,
                        })?;
                        *identifier = add_item(&self.item_state, CrossDomainItem::Event(event));
                    }
                    DescriptorType::Memory(size, handle_type) => {
                        *identifier_type = CROSS_DOMAIN_ID_TYPE_VIRTGPU_BLOB;
                        *identifier_size = size;

                        let mesa_handle = MagmaGpuHandle {
                            os_handle: file,
                            handle_type,
                        };
                        *identifier =
                            add_item(&self.item_state, CrossDomainItem::Blob(mesa_handle));
                    }
                    DescriptorType::WritePipe => {
                        *identifier_type = CROSS_DOMAIN_ID_TYPE_WRITE_PIPE;
                        *identifier_size = 0;
                        let write_pipe = WritePipe::new(file.as_raw_descriptor());
                        std::mem::forget(file); // WritePipe now owns the descriptor
                        *identifier = add_item(
                            &self.item_state,
                            CrossDomainItem::WaylandWritePipe(write_pipe),
                        );
                    }
                    DescriptorType::Socket(_) => {
                        *identifier_type = CROSS_DOMAIN_ID_TYPE_SOCKET;
                        *identifier_size = 0;
                        *identifier = add_item(&self.item_state, CrossDomainItem::Socket(file));
                    }
                    _ => return Err(RutabagaError::InvalidCrossDomainItemType),
                }
            }
        }

        self.state.write_to_ring(
            RingWrite::Write(cmd_receive, Some(&receive_buf[0..len])),
            self.state.channel_ring_id,
        )?;
        self.fence_handler.call(fence);
        Ok(())
    }

    pub fn run(
        &mut self,
        thread_kill_evt: Event,
        mut thread_resample_evt: Event,
    ) -> RutabagaResult<()> {
        self.wait_ctx.add(
            CROSS_DOMAIN_RESAMPLE_ID,
            thread_resample_evt.as_borrowed_descriptor(),
        )?;
        self.wait_ctx.add(
            CROSS_DOMAIN_KILL_ID,
            thread_kill_evt.as_borrowed_descriptor(),
        )?;
        let mut receive_buf: Vec<u8> = vec![0; CROSS_DOMAIN_MAX_SEND_RECV_SIZE];

        while let Some(job) = self.state.wait_for_job() {
            match job {
                CrossDomainJob::HandleFence(fence) => {
                    match self.handle_fence(fence, &mut thread_resample_evt, &mut receive_buf) {
                        Ok(()) => (),
                        Err(e) => {
                            error!("Worker halting due to: {e}");
                            return Err(e);
                        }
                    }
                }
                CrossDomainJob::AddReadPipe(read_pipe_id) => {
                    let items = self.item_state.lock().unwrap();
                    let item = items
                        .table
                        .get(&read_pipe_id)
                        .ok_or(RutabagaError::InvalidCrossDomainItemId)?;

                    match item {
                        CrossDomainItem::WaylandReadPipe(read_pipe) => self
                            .wait_ctx
                            .add(read_pipe_id as u64, read_pipe.as_borrowed_descriptor())?,
                        _ => return Err(RutabagaError::InvalidCrossDomainItemType),
                    }
                }
                CrossDomainJob::AddReadEvent(efd_id) => {
                    let items = self.item_state.lock().unwrap();
                    let item = items
                        .table
                        .get(&efd_id)
                        .ok_or(RutabagaError::InvalidCrossDomainItemId)?;

                    match item {
                        CrossDomainItem::Event(event) => self
                            .wait_ctx
                            .add(efd_id as u64, event.as_borrowed_descriptor())?,
                        _ => return Err(RutabagaError::InvalidCrossDomainItemType),
                    }
                }
                CrossDomainJob::AddAtomicMemorySentinel(id, recv) => {
                    self.wait_ctx
                        .add(id as u64, recv.as_borrowed_descriptor())?;
                }
                CrossDomainJob::Finish => return Ok(()),
            }
        }

        Ok(())
    }
}
