// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::magma::device_state::MagmaDeviceState;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaResult;
use magma_gpu::util::Reader;
use magma_gpu_magma::protocol::MagmaProtocol;
use magma_gpu_magma::protocol::MagmaStructureTypeHeader;
use magma_gpu_magma::protocol::MagmaSubmitAddressSpaceInfo;
use magma_gpu_magma::protocol::MagmaWireSubmitBufferInfo;
use magma_gpu_magma::protocol::MAGMA_STRUCTURE_TYPE_SUBMIT_ADDRESS_SPACE_INFO;
use magma_gpu_magma::protocol::MAGMA_STRUCTURE_TYPE_SUBMIT_BUFFER_INFO;
use magma_gpu_magma::protocol::MAGMA_STRUCTURE_TYPE_SUBMIT_SYNC_INFO;
use magma_gpu_magma::MagmaSubmitBufferInfo;
use magma_gpu_magma::MagmaSubmitInfo;
use magma_gpu_magma::MagmaSubmitSyncInfo;

pub struct MagmaDispatcher {
    state: Arc<MagmaDeviceState>,
    default_queue_id: u32,
}

impl MagmaDispatcher {
    pub fn new(state: Arc<MagmaDeviceState>, default_queue_id: u32) -> Self {
        Self {
            state,
            default_queue_id,
        }
    }

    pub fn dispatch(&self, msg: MagmaProtocol, reader: &mut Reader) -> RutabagaResult<()> {
        match msg {
            MagmaProtocol::CreateDevice(cmd) => {
                let pdev_idx = cmd.physical_device as usize;

                if pdev_idx >= self.state.physical_devices.len() {
                    return Err(RutabagaError::InvalidResourceId);
                }
                let pdev = &self.state.physical_devices[pdev_idx];
                let dev = pdev
                    .create_device()
                    .map_err(|_| RutabagaError::InvalidResourceId)?;
                self.state.set_device(dev);
                Ok(())
            }
            MagmaProtocol::CreateAddressSpace(cmd) => {
                let dev = self
                    .state
                    .get_device()
                    .ok_or(RutabagaError::InvalidResourceId)?;
                let addr_space = dev
                    .create_address_space()
                    .map_err(|_| RutabagaError::InvalidResourceId)?;
                self.state
                    .address_spaces
                    .write()
                    .unwrap()
                    .insert(cmd.address_space, Arc::new(addr_space));
                Ok(())
            }
            MagmaProtocol::CreateQueue(cmd) => {
                let dev = self
                    .state
                    .get_device()
                    .ok_or(RutabagaError::InvalidResourceId)?;
                let as_map = self.state.address_spaces.read().unwrap();
                let addr_space = as_map
                    .get(&cmd.address_space)
                    .ok_or(RutabagaError::InvalidResourceId)?;
                let queue = dev
                    .create_queue(addr_space, &cmd.info)
                    .map_err(|_| RutabagaError::InvalidResourceId)?;
                self.state
                    .queues
                    .write()
                    .unwrap()
                    .insert(cmd.queue, Arc::new(queue));
                Ok(())
            }
            MagmaProtocol::CreateBuffer(cmd) => {
                let dev = self
                    .state
                    .get_device()
                    .ok_or(RutabagaError::InvalidResourceId)?;
                let info = cmd.info;
                let buffer = dev
                    .create_buffer(&info)
                    .map_err(|_| RutabagaError::InvalidResourceId)?;
                self.state
                    .buffers
                    .write()
                    .unwrap()
                    .insert(cmd.buffer_out, Arc::new(buffer));
                Ok(())
            }
            MagmaProtocol::BufferClose(cmd) => {
                let mut buffers = self.state.buffers.write().unwrap();
                buffers.remove(&cmd.buffer);
                Ok(())
            }
            MagmaProtocol::MapBufferGpu(cmd) => {
                let as_map = self.state.address_spaces.read().unwrap();
                let addr_space = as_map
                    .get(&cmd.address_space)
                    .ok_or(RutabagaError::InvalidResourceId)?;
                let buf_map = self.state.buffers.read().unwrap();
                let buffer = buf_map
                    .get(&cmd.buffer)
                    .ok_or(RutabagaError::InvalidResourceId)?;
                addr_space
                    .map_buffer_gpu(buffer, cmd.buffer_offset, cmd.gpu_va, cmd.size, cmd.flags)
                    .map_err(|_| RutabagaError::InvalidResourceId)?;
                Ok(())
            }
            MagmaProtocol::UnmapBufferGpu(cmd) => {
                let as_map = self.state.address_spaces.read().unwrap();
                let addr_space = as_map
                    .get(&cmd.address_space)
                    .ok_or(RutabagaError::InvalidResourceId)?;
                addr_space
                    .unmap_buffer_gpu(cmd.gpu_va, cmd.size)
                    .map_err(|_| RutabagaError::InvalidResourceId)?;
                Ok(())
            }
            MagmaProtocol::SubmitCommand(cmd) => {
                // TODO: Formalize wait_seqno in Gorgonzola instead of using _pad0.
                if cmd._pad0 > 0 {
                    self.state.wait_for_cpu_seqno(cmd._pad0 as u64);
                }

                let queue_id = if cmd.queue != 0 {
                    cmd.queue
                } else {
                    self.default_queue_id
                };
                let queues = self.state.queues.read().unwrap();
                let queue = queues
                    .get(&queue_id)
                    .ok_or(RutabagaError::InvalidResourceId)?;

                let mut next_ptr = cmd.submit_info.header.p_next;
                let mut as_info_storage = None;
                let mut buf_info_storage = None;

                while next_ptr != 0 {
                    let hdr = reader
                        .peek_obj::<MagmaStructureTypeHeader>()
                        .map_err(|_| RutabagaError::InvalidCommandBuffer)?;
                    match hdr.stype {
                        MAGMA_STRUCTURE_TYPE_SUBMIT_ADDRESS_SPACE_INFO => {
                            let ext: MagmaSubmitAddressSpaceInfo = reader
                                .read_try_obj()
                                .map_err(|_| RutabagaError::InvalidCommandBuffer)?;
                            next_ptr = ext.header.p_next;
                            as_info_storage = Some(ext);
                        }
                        MAGMA_STRUCTURE_TYPE_SUBMIT_BUFFER_INFO => {
                            let ext: MagmaWireSubmitBufferInfo = reader
                                .read_try_obj()
                                .map_err(|_| RutabagaError::InvalidCommandBuffer)?;
                            next_ptr = ext.header.p_next;
                            buf_info_storage = Some(MagmaSubmitBufferInfo {
                                header: MagmaStructureTypeHeader {
                                    stype: ext.header.stype,
                                    size: std::mem::size_of::<MagmaSubmitBufferInfo>() as u32,
                                    p_next: 0,
                                },
                                command_buffer: ext.command_buffer,
                                start_offset: ext.start_offset,
                                length: ext.length,
                            });
                        }
                        _ => return Err(RutabagaError::InvalidCommandBuffer),
                    }
                }

                let mut submit_info = MagmaSubmitInfo {
                    header: MagmaStructureTypeHeader {
                        stype: cmd.submit_info.header.stype,
                        size: std::mem::size_of::<MagmaSubmitInfo>() as u32,
                        p_next: 0,
                    },
                    flags: cmd.submit_info.flags,
                    sync_info: Default::default(),
                };

                if let Some(ext) = as_info_storage.as_ref() {
                    submit_info.header.p_next = ext as *const _ as u64;
                } else if let Some(ext) = buf_info_storage.as_ref() {
                    submit_info.header.p_next = ext as *const _ as u64;
                }

                let sync_objs = self.state.sync_objs.read().unwrap();
                let wire_sync = &cmd.submit_info.sync_info;
                if wire_sync.header.stype == MAGMA_STRUCTURE_TYPE_SUBMIT_SYNC_INFO {
                    let mut host_sync_info = MagmaSubmitSyncInfo {
                        header: MagmaStructureTypeHeader {
                            stype: wire_sync.header.stype,
                            size: std::mem::size_of::<MagmaSubmitSyncInfo>() as u32,
                            p_next: 0,
                        },
                        num_wait_sync_objs: wire_sync.num_wait_sync_objs,
                        num_signal_sync_objs: wire_sync.num_signal_sync_objs,
                        wait_sync_objs: Default::default(),
                        signal_sync_objs: Default::default(),
                        wait_points: wire_sync.wait_points,
                        signal_points: wire_sync.signal_points,
                    };
                    for i in 0..wire_sync.num_wait_sync_objs as usize {
                        let guest_id = wire_sync.wait_sync_objs[i];
                        if let Some(so) = sync_objs.get(&guest_id) {
                            host_sync_info.wait_sync_objs[i] = Some(so.as_ref().clone());
                        }
                    }
                    for i in 0..wire_sync.num_signal_sync_objs as usize {
                        let guest_id = wire_sync.signal_sync_objs[i];
                        if let Some(so) = sync_objs.get(&guest_id) {
                            host_sync_info.signal_sync_objs[i] = Some(so.as_ref().clone());
                        }
                    }
                    submit_info.set_sync_info(host_sync_info);
                }

                queue
                    .submit_command(&submit_info)
                    .map_err(|_| RutabagaError::InvalidCommandBuffer)?;

                Ok(())
            }
            MagmaProtocol::CreateSyncObj(cmd) => {
                let dev = self
                    .state
                    .get_device()
                    .ok_or(RutabagaError::InvalidResourceId)?;
                let info = cmd.info;
                let sync_obj = dev
                    .create_sync_obj(&info)
                    .map_err(|_| RutabagaError::InvalidResourceId)?;
                self.state
                    .sync_objs
                    .write()
                    .unwrap()
                    .insert(cmd.sync_obj, Arc::new(sync_obj));
                Ok(())
            }

            MagmaProtocol::SyncObjSignal(cmd) => {
                let sync_objs = self.state.sync_objs.read().unwrap();
                let sync_obj = sync_objs
                    .get(&cmd.sync_obj)
                    .ok_or(RutabagaError::InvalidResourceId)?;
                sync_obj
                    .signal()
                    .map_err(|_| RutabagaError::InvalidResourceId)?;
                Ok(())
            }
            MagmaProtocol::SyncObjTimelineSignal(cmd) => {
                let sync_objs = self.state.sync_objs.read().unwrap();
                let sync_obj = sync_objs
                    .get(&cmd.sync_obj)
                    .ok_or(RutabagaError::InvalidResourceId)?;
                sync_obj
                    .timeline_signal(cmd.point)
                    .map_err(|_| RutabagaError::InvalidResourceId)?;
                Ok(())
            }
            MagmaProtocol::SyncObjClose(_cmd) => Ok(()),
            MagmaProtocol::VirtioSyncObjClose(cmd) => {
                if cmd.ring_idx > 0 && cmd.wait_seqno > 0 {
                    self.state
                        .wait_for_ring_seqno(cmd.ring_idx, cmd.wait_seqno as u64);
                }
                let mut sync_objs = self.state.sync_objs.write().unwrap();
                sync_objs.remove(&cmd.sync_obj);
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
