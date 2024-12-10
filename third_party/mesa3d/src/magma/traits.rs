// Copyright 2025 Android Open Source Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use mesa3d_util::MappedRegion;
use mesa3d_util::MesaHandle;
use mesa3d_util::MesaResult;
use virtgpu_kumquat::VirtGpuKumquat;

use crate::magma_defines::MagmaCreateBufferInfo;
use crate::magma_defines::MagmaHeapBudget;
use crate::magma_defines::MagmaImportHandleInfo;
use crate::magma_defines::MagmaMappedMemoryRange;
use crate::magma_defines::MagmaMemoryProperties;
use crate::magma_defines::MagmaPciInfo;
use crate::sys::platform::PlatformDevice;
use crate::sys::platform::PlatformPhysicalDevice;

pub trait AsVirtGpu {
    fn as_virtgpu(&self) -> Option<&VirtGpuKumquat> {
        None
    }
}

pub trait GenericPhysicalDevice {
    fn create_device(
        &self,
        physical_device: &Arc<dyn PhysicalDevice>,
        pci_info: &MagmaPciInfo,
    ) -> MesaResult<Arc<dyn Device>>;
}

pub trait GenericDevice {
    fn get_memory_properties(&self) -> MesaResult<MagmaMemoryProperties>;

    fn get_memory_budget(&self, _heap_idx: u32) -> MesaResult<MagmaHeapBudget>;

    fn create_context(&self, device: &Arc<dyn Device>) -> MesaResult<Arc<dyn Context>>;

    fn create_buffer(
        &self,
        device: &Arc<dyn Device>,
        create_info: &MagmaCreateBufferInfo,
    ) -> MesaResult<Arc<dyn Buffer>>;

    fn import(
        &self,
        _device: &Arc<dyn Device>,
        _info: MagmaImportHandleInfo,
    ) -> MesaResult<Arc<dyn Buffer>>;
}

pub trait GenericBuffer {
    fn map(&self, buffer: &Arc<dyn Buffer>) -> MesaResult<Arc<dyn MappedRegion>>;

    fn export(&self) -> MesaResult<MesaHandle>;

    fn invalidate(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> MesaResult<()>;

    fn flush(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> MesaResult<()>;
}

pub trait PhysicalDevice: PlatformPhysicalDevice + AsVirtGpu + GenericPhysicalDevice {}
pub trait Device: GenericDevice + PlatformDevice {}
pub trait Context {}
pub trait Buffer: GenericBuffer {}
