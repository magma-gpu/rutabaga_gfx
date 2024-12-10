// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use mesa3d_util::MesaResult;
use std::sync::Arc;

use crate::magma::MagmaPhysicalDevice;
use crate::magma_defines::MagmaCreateBufferInfo;
use crate::magma_defines::MagmaMemoryProperties;
use crate::sys::windows::d3dkmt_common;

pub trait VendorPrivateData {
    fn createallocation_pdata(&self) -> Vec<u32> {
        Vec::new()
    }

    fn allocationinfo2_pdata(
        &self,
        _create_info: &MagmaCreateBufferInfo,
        _mem_props: &MagmaMemoryProperties,
    ) -> Vec<u32> {
        Vec::new()
    }
}

pub fn enumerate_devices() -> MesaResult<Vec<MagmaPhysicalDevice>> {
    let mut devices: Vec<MagmaPhysicalDevice> = Vec::new();
    let adapters = d3dkmt_common::enumerate_adapters()?;

    for (adapter, pci_info, pci_bus_info) in adapters {
        devices.push(MagmaPhysicalDevice::new(
            Arc::new(adapter),
            pci_info,
            pci_bus_info,
        ));
    }

    Ok(devices)
}
