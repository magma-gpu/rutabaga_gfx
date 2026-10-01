// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaMappedMemoryRange;
use crate::defines::MagmaMemoryType;
use crate::error::Result;
use crate::sys::linux::msm::ioctl;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendPhysicalDevice;
use crate::traits::GenericAddressSpace;
use crate::traits::GenericBuffer;

pub struct MsmAddressSpace {
    _physical_device: Arc<dyn BackendPhysicalDevice>,
}

pub struct MsmBuffer {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    gem_handle: u32,
    size: usize,
    map_info: Option<u32>,
}

impl MsmAddressSpace {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> MsmAddressSpace {
        MsmAddressSpace {
            _physical_device: physical_device,
        }
    }
}

impl GenericAddressSpace for MsmAddressSpace {}
impl BackendAddressSpace for MsmAddressSpace {}

impl MsmBuffer {
    pub fn new(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        create_info: &MagmaCreateBufferInfo,
        mem_types: &[MagmaMemoryType],
    ) -> Result<MsmBuffer> {
        let gem_handle = ioctl::gem_new(physical_device.as_fd().unwrap(), create_info.size)?;
        let map_info = mem_types
            .get(create_info.memory_type_idx as usize)
            .and_then(|mt| mt.get_map_info());

        Ok(MsmBuffer {
            physical_device,
            gem_handle,
            size: create_info.size.try_into()?,
            map_info,
        })
    }

    pub fn from_existing(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        gem_handle: u32,
        size: usize,
        map_info: Option<u32>,
    ) -> Result<MsmBuffer> {
        Ok(MsmBuffer {
            physical_device,
            gem_handle,
            size,
            map_info,
        })
    }
}

impl GenericBuffer for MsmBuffer {
    fn map(self: Arc<MsmBuffer>) -> Result<Arc<dyn MappedRegion>> {
        let offset =
            ioctl::gem_info_offset(self.physical_device.as_fd().unwrap(), self.gem_handle)?;
        let mapping = self.physical_device.cpu_map(offset, self.size)?;
        Ok(Arc::new(mapping))
    }

    fn export(&self) -> Result<MagmaGpuHandle> {
        self.physical_device.export(self.gem_handle)
    }

    fn invalidate(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        ioctl::gem_cpu_prep(self.physical_device.as_fd().unwrap(), self.gem_handle)
    }

    fn flush(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        ioctl::gem_cpu_fini(self.physical_device.as_fd().unwrap(), self.gem_handle)
    }

    fn as_gem_handle(&self) -> Option<u32> {
        Some(self.gem_handle)
    }

    fn get_map_info(&self) -> Option<u32> {
        self.map_info
    }
}

impl Drop for MsmBuffer {
    fn drop(&mut self) {
        // GEM close
    }
}

impl BackendBuffer for MsmBuffer {}
