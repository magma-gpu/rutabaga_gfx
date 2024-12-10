// Copyright 2025 Android Open Source Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use mesa3d_util::MesaError;
use mesa3d_util::MesaResult;
use virtgpu_kumquat::VirtGpuKumquat;

use crate::magma::MagmaPhysicalDevice;
use crate::magma_defines::MagmaCreateBufferInfo;
use crate::magma_defines::MagmaHeapBudget;
use crate::magma_defines::MagmaImportHandleInfo;
use crate::magma_defines::MagmaMemoryProperties;
use crate::magma_defines::MagmaPciBusInfo;
use crate::magma_defines::MagmaPciInfo;
use crate::sys::platform::PlatformPhysicalDevice;
use crate::traits::AsVirtGpu;
use crate::traits::Buffer;
use crate::traits::Context;
use crate::traits::Device;
use crate::traits::GenericDevice;
use crate::traits::GenericPhysicalDevice;
use crate::traits::PhysicalDevice;

pub struct MagmaKumquat {
    virtgpu: VirtGpuKumquat,
}

impl MagmaKumquat {
    pub fn new() -> MesaResult<MagmaKumquat> {
        Ok(MagmaKumquat {
            virtgpu: VirtGpuKumquat::new("/tmp/kumquat-gpu-0")?,
        })
    }
}

impl AsVirtGpu for MagmaKumquat {
    fn as_virtgpu(&self) -> Option<&VirtGpuKumquat> {
        Some(&self.virtgpu)
    }
}

impl PlatformPhysicalDevice for MagmaKumquat {}
impl PhysicalDevice for MagmaKumquat {}

impl GenericPhysicalDevice for MagmaKumquat {
    fn create_device(
        &self,
        physical_device: &Arc<dyn PhysicalDevice>,
        _pci_info: &MagmaPciInfo,
    ) -> MesaResult<Arc<dyn Device>> {
        let _virtgpu = physical_device.as_virtgpu().unwrap();
        Err(MesaError::Unsupported)
    }
}

impl GenericDevice for MagmaKumquat {
    fn get_memory_properties(&self) -> MesaResult<MagmaMemoryProperties> {
        Err(MesaError::Unsupported)
    }

    fn get_memory_budget(&self, _heap_idx: u32) -> MesaResult<MagmaHeapBudget> {
        Err(MesaError::Unsupported)
    }

    fn create_context(&self, _device: &Arc<dyn Device>) -> MesaResult<Arc<dyn Context>> {
        Err(MesaError::Unsupported)
    }

    fn create_buffer(
        &self,
        _device: &Arc<dyn Device>,
        _create_info: &MagmaCreateBufferInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        Err(MesaError::Unsupported)
    }

    fn import(
        &self,
        _device: &Arc<dyn Device>,
        _info: MagmaImportHandleInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        Err(MesaError::Unsupported)
    }
}

pub fn enumerate_devices() -> MesaResult<Vec<MagmaPhysicalDevice>> {
    let pci_info: MagmaPciInfo = Default::default();
    let pci_bus_info: MagmaPciBusInfo = Default::default();
    let mut devices: Vec<MagmaPhysicalDevice> = Vec::new();

    let enc = MagmaKumquat::new()?;
    // TODO): Get data from the server

    devices.push(MagmaPhysicalDevice::new(
        Arc::new(enc),
        pci_info,
        pci_bus_info,
    ));

    Ok(devices)
}
