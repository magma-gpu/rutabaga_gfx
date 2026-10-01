// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::sys::windows::wddm::d3dkmt::D3dkmtHandle;
use crate::traits::AsVirtioSyncObject;
use crate::traits::BackendDevice;
use crate::traits::BackendSyncObject;
use crate::traits::GenericSyncObject;

pub struct WddmSyncObject {
    handle: D3dkmtHandle,
    _device: Arc<dyn BackendDevice>,
}

impl GenericSyncObject for WddmSyncObject {
    fn as_raw_handle(&self) -> Option<u32> {
        Some(self.handle)
    }
}

impl AsVirtioSyncObject for WddmSyncObject {}
impl BackendSyncObject for WddmSyncObject {}
