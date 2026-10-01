// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use log::error;
use magma_gpu::log_status;

use crate::defines::MagmaCreateQueueInfo;
use crate::defines::MagmaQueueFlags;
use crate::defines::MagmaSubmitInfo;
use crate::error::Error;
use crate::error::Result;
use crate::sys::linux::bindings::xe_bindings::drm_xe_sync;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_ENGINE_CLASS_COMPUTE;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_ENGINE_CLASS_COPY;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_ENGINE_CLASS_RENDER;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_ENGINE_CLASS_VM_BIND;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_EXEC_QUEUE_GET_PROPERTY_BAN;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_SYNC_FLAG_SIGNAL;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_SYNC_TYPE_SYNCOBJ;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_SYNC_TYPE_TIMELINE_SYNCOBJ;
use crate::sys::linux::xe::ioctl::xe_exec;
use crate::sys::linux::xe::ioctl::xe_exec_queue_create;
use crate::sys::linux::xe::ioctl::xe_exec_queue_destroy;
use crate::sys::linux::xe::ioctl::xe_exec_queue_get_property;
use crate::sys::linux::xe::ioctl::xe_vm_bind_syncs;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendQueue;
use crate::traits::GenericQueue;

pub struct XeQueue {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    exec_queue_id: u32,
    vm_id: u32,
    is_bind_queue: bool,
}

impl XeQueue {
    pub fn new(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        address_space: &Arc<dyn BackendAddressSpace>,
        info: &MagmaCreateQueueInfo,
    ) -> Result<XeQueue> {
        let fd = physical_device.as_fd().unwrap();
        let vm_id = address_space.as_vm_id().ok_or(Error::Unimplemented)?;
        let is_bind_queue = (info.flags & MagmaQueueFlags::SparseBinding.bits()) != 0;

        let engine_class = if is_bind_queue {
            DRM_XE_ENGINE_CLASS_VM_BIND as u16
        } else if info.queue_family_idx == 1 {
            DRM_XE_ENGINE_CLASS_COMPUTE as u16
        } else if info.queue_family_idx == 2 {
            DRM_XE_ENGINE_CLASS_COPY as u16
        } else {
            DRM_XE_ENGINE_CLASS_RENDER as u16
        };

        let exec_queue_id = xe_exec_queue_create(fd, vm_id, engine_class)?;
        Ok(XeQueue {
            physical_device,
            exec_queue_id,
            vm_id,
            is_bind_queue,
        })
    }
}

impl Drop for XeQueue {
    fn drop(&mut self) {
        let result =
            xe_exec_queue_destroy(self.physical_device.as_fd().unwrap(), self.exec_queue_id);
        log_status!(result);
    }
}

impl GenericQueue for XeQueue {
    fn submit_command(&self, submit_info: &MagmaSubmitInfo) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();

        let mut syncs: Vec<drm_xe_sync> = Vec::new();
        for (sync_obj, wait_point) in submit_info.sync_info.wait_sync_objs_with_points() {
            if let Some(handle) = sync_obj.as_raw_handle() {
                let is_timeline = sync_obj.is_timeline() && wait_point > 0;
                let mut s = drm_xe_sync {
                    type_: if is_timeline {
                        DRM_XE_SYNC_TYPE_TIMELINE_SYNCOBJ
                    } else {
                        DRM_XE_SYNC_TYPE_SYNCOBJ
                    },
                    timeline_value: if is_timeline { wait_point } else { 0 },
                    ..Default::default()
                };
                s.__bindgen_anon_1.handle = handle;
                syncs.push(s);
            }
        }
        for (sync_obj, signal_point) in submit_info.sync_info.signal_sync_objs_with_points() {
            if let Some(handle) = sync_obj.as_raw_handle() {
                let is_timeline = sync_obj.is_timeline() && signal_point > 0;
                let mut s = drm_xe_sync {
                    type_: if is_timeline {
                        DRM_XE_SYNC_TYPE_TIMELINE_SYNCOBJ
                    } else {
                        DRM_XE_SYNC_TYPE_SYNCOBJ
                    },
                    flags: DRM_XE_SYNC_FLAG_SIGNAL,
                    timeline_value: if is_timeline { signal_point } else { 0 },
                    ..Default::default()
                };
                s.__bindgen_anon_1.handle = handle;
                syncs.push(s);
            }
        }

        if self.is_bind_queue {
            xe_vm_bind_syncs(fd, self.vm_id, self.exec_queue_id, &syncs)?;
        } else {
            let as_info = submit_info
                .address_space_info()
                .ok_or(Error::Unimplemented)?;

            let num_batch_buffer = if as_info.command_va != 0 && as_info.length != 0 {
                1
            } else {
                0
            };
            let address = if num_batch_buffer > 0 {
                as_info.command_va
            } else {
                0
            };

            xe_exec(fd, self.exec_queue_id, num_batch_buffer, address, &syncs)?;
        }
        Ok(())
    }

    fn check_status(&self) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        let ban_value =
            xe_exec_queue_get_property(fd, self.exec_queue_id, DRM_XE_EXEC_QUEUE_GET_PROPERTY_BAN)
                .map_err(|_| Error::InternalError)?;

        if ban_value != 0 {
            return Err(Error::ContextKilled);
        }
        Ok(())
    }
}

impl BackendQueue for XeQueue {}
