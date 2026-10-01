// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use log::error;
use magma_gpu::log_status;

use crate::defines::MagmaSubmitInfo;
use crate::error::Result;
use crate::sys::linux::i915::ioctl;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendQueue;
use crate::traits::GenericQueue;

pub struct I915Queue {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    context_id: u32,
}

impl I915Queue {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> Result<I915Queue> {
        let context_id = ioctl::gem_context_create(physical_device.as_fd().unwrap())?;
        Ok(I915Queue {
            physical_device,
            context_id,
        })
    }
}

impl Drop for I915Queue {
    fn drop(&mut self) {
        let result =
            ioctl::gem_context_destroy(self.physical_device.as_fd().unwrap(), self.context_id);
        log_status!(result);
    }
}

impl GenericQueue for I915Queue {
    fn submit_command(&self, submit_info: &MagmaSubmitInfo) -> Result<()> {
        ioctl::gem_execbuffer2(
            self.physical_device.as_fd().unwrap(),
            self.context_id,
            submit_info,
        )
    }
}

impl BackendQueue for I915Queue {}
