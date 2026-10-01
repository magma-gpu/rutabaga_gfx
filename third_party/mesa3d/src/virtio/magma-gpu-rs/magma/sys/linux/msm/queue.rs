// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::defines::MagmaSubmitInfo;
use crate::error::Result;
use crate::sys::linux::msm::ioctl;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendQueue;
use crate::traits::GenericQueue;

pub struct MsmQueue {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    submit_queue_id: u32,
}

impl MsmQueue {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>, priority: u32) -> Result<MsmQueue> {
        let submit_queue_id =
            ioctl::submitqueue_create(physical_device.as_fd().unwrap(), priority)?;
        Ok(MsmQueue {
            physical_device,
            submit_queue_id,
        })
    }
}

impl Drop for MsmQueue {
    fn drop(&mut self) {
        ioctl::submitqueue_destroy(self.physical_device.as_fd().unwrap(), self.submit_queue_id);
    }
}

impl GenericQueue for MsmQueue {
    fn submit_command(&self, submit_info: &MagmaSubmitInfo) -> Result<()> {
        ioctl::gem_submit(
            self.physical_device.as_fd().unwrap(),
            self.submit_queue_id,
            submit_info,
        )
    }
}

impl BackendQueue for MsmQueue {}
