// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use log::error;

use magma_gpu::log_status;

use crate::defines::MagmaCreateQueueInfo;
use crate::defines::MagmaQueueFlags;
use crate::defines::MagmaSubmitInfo;
use crate::error::Result;
use crate::sys::linux::panthor::device::PanthorPhysicalDevice;
use crate::sys::linux::panthor::ioctl;
use crate::sys::linux::PlatformPhysicalDevice;
use crate::traits::BackendQueue;
use crate::traits::GenericQueue;

pub struct PanthorQueue {
    physical_device: Arc<PanthorPhysicalDevice>,
    group_handle: u32,
    vm_id: u32,
    is_bind_queue: bool,
}

impl PanthorQueue {
    pub fn new(
        physical_device: Arc<PanthorPhysicalDevice>,
        vm_id: u32,
        info: &MagmaCreateQueueInfo,
    ) -> Result<PanthorQueue> {
        let is_bind_queue =
            (info.flags & MagmaQueueFlags::SparseBinding.bits()) != 0 || info.queue_family_idx == 1;

        if is_bind_queue {
            return Ok(PanthorQueue {
                physical_device,
                group_handle: 0,
                vm_id,
                is_bind_queue: true,
            });
        }

        let group_handle = ioctl::group_create(
            physical_device.as_fd().unwrap(),
            physical_device.gpu_info(),
            vm_id,
            info,
        )?;

        Ok(PanthorQueue {
            physical_device,
            group_handle,
            vm_id,
            is_bind_queue: false,
        })
    }
}

impl Drop for PanthorQueue {
    fn drop(&mut self) {
        if !self.is_bind_queue && self.group_handle != 0 {
            let result =
                ioctl::group_destroy(self.physical_device.as_fd().unwrap(), self.group_handle);
            log_status!(result);
        }
    }
}

impl GenericQueue for PanthorQueue {
    fn submit_command(&self, submit_info: &MagmaSubmitInfo) -> Result<()> {
        ioctl::queue_submit(
            self.physical_device.as_fd().unwrap(),
            self.is_bind_queue,
            self.vm_id,
            self.group_handle,
            submit_info,
        )
    }

    fn check_status(&self) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        ioctl::check_queue_status(fd, self.is_bind_queue, self.vm_id, self.group_handle)
    }
}

impl BackendQueue for PanthorQueue {}
