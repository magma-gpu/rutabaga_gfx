// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::sync::Arc;

use log::error;
use magma_gpu::log_status;

use crate::defines::MagmaSubmitInfo;
use crate::error::Error;
use crate::error::Result;
use crate::sys::linux::amdgpu::ioctl::amdgpu_cs_submit;
use crate::sys::linux::amdgpu::ioctl::amdgpu_ctx_alloc;
use crate::sys::linux::amdgpu::ioctl::amdgpu_ctx_free;
use crate::sys::linux::bindings::amdgpu_bindings::drm_amdgpu_cs_chunk;
use crate::sys::linux::bindings::amdgpu_bindings::drm_amdgpu_cs_chunk_ib;
use crate::sys::linux::bindings::amdgpu_bindings::drm_amdgpu_cs_chunk_syncobj;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_CHUNK_ID_IB;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_CHUNK_ID_SYNCOBJ_TIMELINE_SIGNAL;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_CHUNK_ID_SYNCOBJ_TIMELINE_WAIT;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_HW_IP_GFX;
use crate::sys::linux::DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendQueue;
use crate::traits::GenericQueue;

pub struct AmdGpuQueue {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    context_id: u32,
}

impl AmdGpuQueue {
    pub fn new(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        _priority: i32,
    ) -> Result<AmdGpuQueue> {
        let fd = physical_device.as_fd().unwrap();
        let context_id = amdgpu_ctx_alloc(fd)?;

        Ok(AmdGpuQueue {
            physical_device,
            context_id,
        })
    }
}

impl Drop for AmdGpuQueue {
    fn drop(&mut self) {
        let result = amdgpu_ctx_free(self.physical_device.as_fd().unwrap(), self.context_id);
        log_status!(result);
    }
}

impl GenericQueue for AmdGpuQueue {
    fn submit_command(&self, submit_info: &MagmaSubmitInfo) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        let as_info = submit_info
            .address_space_info()
            .ok_or(Error::Unimplemented)?;

        let mut chunks: Vec<drm_amdgpu_cs_chunk> = Vec::new();

        let ib = drm_amdgpu_cs_chunk_ib {
            va_start: as_info.command_va,
            ib_bytes: as_info.length as u32,
            ip_type: AMDGPU_HW_IP_GFX,
            ..Default::default()
        };

        chunks.push(drm_amdgpu_cs_chunk {
            chunk_id: AMDGPU_CHUNK_ID_IB,
            length_dw: (size_of::<drm_amdgpu_cs_chunk_ib>() / 4) as u32,
            chunk_data: &ib as *const _ as u64,
        });

        let mut in_syncs: Vec<drm_amdgpu_cs_chunk_syncobj> = Vec::new();
        let mut out_syncs: Vec<drm_amdgpu_cs_chunk_syncobj> = Vec::new();

        for (sync_obj, wait_point) in submit_info.sync_info.wait_sync_objs_with_points() {
            if let Some(handle) = sync_obj.as_raw_handle() {
                let is_timeline = sync_obj.is_timeline() && wait_point > 0;
                in_syncs.push(drm_amdgpu_cs_chunk_syncobj {
                    handle,
                    flags: if is_timeline {
                        DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT
                    } else {
                        0
                    },
                    point: if is_timeline { wait_point } else { 0 },
                });
            }
        }
        for (sync_obj, signal_point) in submit_info.sync_info.signal_sync_objs_with_points() {
            if let Some(handle) = sync_obj.as_raw_handle() {
                let is_timeline = sync_obj.is_timeline() && signal_point > 0;
                out_syncs.push(drm_amdgpu_cs_chunk_syncobj {
                    handle,
                    flags: 0,
                    point: if is_timeline { signal_point } else { 0 },
                });
            }
        }

        if !in_syncs.is_empty() {
            chunks.push(drm_amdgpu_cs_chunk {
                chunk_id: AMDGPU_CHUNK_ID_SYNCOBJ_TIMELINE_WAIT,
                length_dw: (in_syncs.len() * size_of::<drm_amdgpu_cs_chunk_syncobj>() / 4) as u32,
                chunk_data: in_syncs.as_ptr() as u64,
            });
        }

        if !out_syncs.is_empty() {
            chunks.push(drm_amdgpu_cs_chunk {
                chunk_id: AMDGPU_CHUNK_ID_SYNCOBJ_TIMELINE_SIGNAL,
                length_dw: (out_syncs.len() * size_of::<drm_amdgpu_cs_chunk_syncobj>() / 4) as u32,
                chunk_data: out_syncs.as_ptr() as u64,
            });
        }

        amdgpu_cs_submit(fd, self.context_id, &chunks)
    }
}

impl BackendQueue for AmdGpuQueue {}
