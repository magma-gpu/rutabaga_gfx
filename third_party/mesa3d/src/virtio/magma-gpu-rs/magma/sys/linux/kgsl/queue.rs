// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::defines::MagmaSubmitInfo;
use crate::error::Result;
use crate::sys::linux::kgsl::ioctl;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendQueue;
use crate::traits::GenericQueue;

pub struct KgslQueue {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    context_id: u32,
}

impl KgslQueue {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> Result<KgslQueue> {
        let context_id = ioctl::drawctxt_create(physical_device.as_fd().unwrap())?;
        Ok(KgslQueue {
            physical_device,
            context_id,
        })
    }
}

impl Drop for KgslQueue {
    fn drop(&mut self) {
        ioctl::drawctxt_destroy(self.physical_device.as_fd().unwrap(), self.context_id);
    }
}

impl GenericQueue for KgslQueue {
    fn submit_command(&self, submit_info: &MagmaSubmitInfo) -> Result<()> {
        ioctl::gpu_command_submit(
            self.physical_device.as_fd().unwrap(),
            self.context_id,
            submit_info,
        )
    }
}

impl BackendQueue for KgslQueue {}
