// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::defines::MagmaSubmitInfo;
use crate::error::Result;
use crate::sys::windows::wddm::d3dkmt;
use crate::sys::windows::wddm::d3dkmt::D3dkmtHandle;
use crate::traits::BackendDevice;
use crate::traits::BackendQueue;
use crate::traits::GenericQueue;

pub struct WddmQueue {
    handle: D3dkmtHandle,
    _device: Arc<dyn BackendDevice>,
}

impl WddmQueue {
    pub fn new(device: Arc<dyn BackendDevice>) -> Result<WddmQueue> {
        let handle = d3dkmt::create_context_virtual(device.as_wddm_handle())?;
        Ok(WddmQueue {
            handle,
            _device: device,
        })
    }
}

impl Drop for WddmQueue {
    fn drop(&mut self) {
        d3dkmt::destroy_context(self.handle);
    }
}

impl GenericQueue for WddmQueue {
    fn submit_command(&self, _submit_info: &MagmaSubmitInfo) -> Result<()> {
        Ok(())
    }
}

impl BackendQueue for WddmQueue {}
