// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

pub mod d3dkmt;
pub mod device;
pub mod memory;
pub mod queue;
pub mod sync;

use std::sync::Arc;

use windows_sys::Wdk::Graphics::Direct3D::D3DKMT_SEGMENTGROUPSIZEINFO;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaMemoryType;
use crate::device::PhysicalDevice;
use crate::error::Result;
use crate::sys::windows::wddm::d3dkmt::D3dkmtHandle;
use crate::traits::BackendPhysicalDevice;

pub trait VendorPrivateData: Send + Sync {
    fn createallocation_pdata(&self) -> Vec<u32> {
        Vec::new()
    }

    fn allocationinfo2_pdata(
        &self,
        _create_info: &MagmaCreateBufferInfo,
        _mem_types: &[MagmaMemoryType],
    ) -> Vec<u32> {
        Vec::new()
    }
}

pub trait WindowsDevice {
    fn as_wddm_handle(&self) -> D3dkmtHandle {
        0
    }

    fn vendor_private_data(&self) -> Option<&dyn VendorPrivateData> {
        None
    }
}

pub trait WindowsPhysicalDevice {
    fn as_wddm_handle(&self) -> D3dkmtHandle {
        0
    }

    #[allow(dead_code)]
    fn segment_group_size(&self) -> D3DKMT_SEGMENTGROUPSIZEINFO {
        Default::default()
    }
}

pub fn enumerate_devices() -> Result<Vec<PhysicalDevice>> {
    let mut devices: Vec<PhysicalDevice> = Vec::new();
    let adapters = device::enumerate_adapters()?;

    for (adapter, info) in adapters {
        let physical_device: Arc<dyn BackendPhysicalDevice> = Arc::new(adapter);
        devices.push(PhysicalDevice::new(physical_device, info));
    }

    Ok(devices)
}
