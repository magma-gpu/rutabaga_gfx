// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use magma_gpu::util::MappedRegion;

use crate::defines::MagmaStructureType;
use crate::defines::MagmaStructureTypeHeader;
use crate::defines::MagmaSubmitInfo;
use crate::encoder::Encoder;
use crate::error::Result;
use crate::protocol::MagmaCommandHeader;
use crate::protocol::MagmaSubmitAddressSpaceInfo;
use crate::protocol::MagmaWireSubmitBufferInfo;
use crate::protocol::MagmaWireSubmitInfo;
use crate::protocol::MagmaWireSubmitSyncInfo;
use crate::protocol::SubmitCommand;
use crate::protocol::MAGMA_MAX_SYNCOBJS;
use crate::protocol::MAGMA_OPCODE_SUBMIT_COMMAND;
use crate::ring::MagmaRingBuffer;
use crate::traits::BackendQueue;
use crate::traits::GenericQueue;
use crate::traits::VirtioSyncOp;
use crate::virtgpu_shared::device::VirtioGpu;
use crate::virtgpu_shared::VirtioGpuTransport;

pub struct VirtioGpuQueue<T: VirtioGpuTransport> {
    pub device: Arc<VirtioGpu<T>>,
    pub object_id: u32,
    pub ring_idx: u32,
    pub ring: MagmaRingBuffer,
    pub _bo_handle: u32,
    pub _mapping: Arc<dyn MappedRegion>,
}

impl<T: VirtioGpuTransport> GenericQueue for VirtioGpuQueue<T> {
    fn submit_command(&self, submit_info: &MagmaSubmitInfo) -> Result<()> {
        let wait_seqno = self.device.cpu_ring.submitted_seqno();

        let mut wire_sync_info = MagmaWireSubmitSyncInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SubmitSyncInfo as u32,
                size: std::mem::size_of::<MagmaWireSubmitSyncInfo>() as u32,
                p_next: 0,
            },
            num_wait_sync_objs: 0,
            num_signal_sync_objs: 0,
            wait_sync_objs: [0u32; MAGMA_MAX_SYNCOBJS],
            signal_sync_objs: [0u32; MAGMA_MAX_SYNCOBJS],
            wait_points: [0u64; MAGMA_MAX_SYNCOBJS],
            signal_points: [0u64; MAGMA_MAX_SYNCOBJS],
        };

        for (sync_obj, wait_point) in submit_info.sync_info.wait_sync_objs_with_points() {
            if let Some(virtio_sync) = sync_obj.as_virtio_syncobj() {
                if virtio_sync.has_imported_fence() {
                    sync_obj.wait(u64::MAX)?;
                } else {
                    let idx = wire_sync_info.num_wait_sync_objs as usize;
                    if idx < MAGMA_MAX_SYNCOBJS {
                        wire_sync_info.wait_sync_objs[idx] = virtio_sync.host_object_id();
                        wire_sync_info.wait_points[idx] = wait_point;
                        wire_sync_info.num_wait_sync_objs += 1;
                    }
                }
            }
        }

        for (sync_obj, signal_point) in submit_info.sync_info.signal_sync_objs_with_points() {
            if let Some(virtio_sync) = sync_obj.as_virtio_syncobj() {
                let idx = wire_sync_info.num_signal_sync_objs as usize;
                if idx < MAGMA_MAX_SYNCOBJS {
                    wire_sync_info.signal_sync_objs[idx] = virtio_sync.host_object_id();
                    wire_sync_info.signal_points[idx] = signal_point;
                    wire_sync_info.num_signal_sync_objs += 1;
                }
            }
        }

        let mut wire_submit_info = MagmaWireSubmitInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SubmitInfo as u32,
                size: std::mem::size_of::<MagmaWireSubmitInfo>() as u32,
                p_next: 0,
            },
            flags: submit_info.flags,
            _pad0: 0,
            sync_info: wire_sync_info,
        };

        let mut wire_as_info;
        let wire_buf_info;
        if let Some(as_info) = submit_info.address_space_info() {
            wire_as_info = *as_info;
            wire_as_info.header.p_next = 0;
            wire_submit_info.header.p_next = &wire_as_info as *const _ as u64;
        } else if let Some(buf_info) = submit_info.buffer_info() {
            wire_buf_info = MagmaWireSubmitBufferInfo {
                header: MagmaStructureTypeHeader {
                    stype: MagmaStructureType::SubmitBufferInfo as u32,
                    size: std::mem::size_of::<MagmaWireSubmitBufferInfo>() as u32,
                    p_next: 0,
                },
                command_buffer: buf_info.command_buffer,
                _pad0: 0,
                start_offset: buf_info.start_offset,
                length: buf_info.length,
            };
            wire_submit_info.header.p_next = &wire_buf_info as *const _ as u64;
        }

        let cmd = SubmitCommand {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_SUBMIT_COMMAND,
                size: std::mem::size_of::<SubmitCommand>() as u32,
            },
            queue: self.object_id,
            _pad0: wait_seqno,
            submit_info: wire_submit_info,
        };
        const MAX_SUBMIT_SIZE: usize = std::mem::size_of::<SubmitCommand>()
            + std::mem::size_of::<MagmaSubmitAddressSpaceInfo>();
        let mut buf = [0u8; MAX_SUBMIT_SIZE];
        let mut encoder = Encoder::from_slice(&mut buf);
        encoder.encode_submit_command(&cmd)?;
        let written = encoder.bytes_written();
        self.ring.write_and_ping(&buf[..written], || {
            self.device.ping_ring(self.ring_idx, self.ring_idx)
        })?;

        let submitted_seqno = self.ring.submitted_seqno();
        for (sync_obj, _) in submit_info.sync_info.wait_sync_objs_with_points() {
            if let Some(virtio_sync) = sync_obj.as_virtio_syncobj() {
                if !virtio_sync.has_imported_fence() {
                    virtio_sync.record_submission(VirtioSyncOp::Wait {
                        ring_idx: self.ring_idx,
                        seqno: submitted_seqno,
                    });
                }
            }
        }
        for (sync_obj, signal_point) in submit_info.sync_info.signal_sync_objs_with_points() {
            if let Some(virtio_sync) = sync_obj.as_virtio_syncobj() {
                virtio_sync.record_submission(VirtioSyncOp::Signal {
                    ring_idx: self.ring_idx,
                    seqno: submitted_seqno,
                    point: signal_point,
                });
            }
        }

        Ok(())
    }
}

impl<T: VirtioGpuTransport> BackendQueue for VirtioGpuQueue<T> {}
