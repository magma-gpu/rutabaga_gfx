// Copyright 2025 Android Open Source Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;
use magma_gpu::util::Result as MagmaGpuResult;
use magma_gpu::virtgpu_kumquat::VirtGpuKumquat;

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
    ) -> MagmaGpuResult<Arc<dyn Device>>;
}

pub trait GenericDevice {
    fn get_memory_properties(&self) -> MagmaGpuResult<MagmaMemoryProperties>;

    fn get_memory_budget(&self, _heap_idx: u32) -> MagmaGpuResult<MagmaHeapBudget>;

    fn create_context(&self, device: &Arc<dyn Device>) -> MagmaGpuResult<Arc<dyn Context>>;

    fn create_buffer(
        &self,
        device: &Arc<dyn Device>,
        create_info: &MagmaCreateBufferInfo,
    ) -> MagmaGpuResult<Arc<dyn Buffer>>;

    fn import(
        &self,
        _device: &Arc<dyn Device>,
        _info: MagmaImportHandleInfo,
    ) -> MagmaGpuResult<Arc<dyn Buffer>>;
}

pub trait GenericBuffer {
    fn map(&self, buffer: &Arc<dyn Buffer>) -> MagmaGpuResult<Arc<dyn MappedRegion>>;

    fn export(&self) -> MagmaGpuResult<MagmaGpuHandle>;

    fn invalidate(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> MagmaGpuResult<()>;

    fn flush(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> MagmaGpuResult<()>;
}

pub trait PhysicalDevice: PlatformPhysicalDevice + AsVirtGpu + GenericPhysicalDevice {}
pub trait Device: GenericDevice + PlatformDevice {}
pub trait Context {}
pub trait Buffer: GenericBuffer {}
