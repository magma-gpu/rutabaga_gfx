// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaMappedMemoryRange;
use crate::defines::MagmaMemoryType;
use crate::error::Error;
use crate::error::Result;
use crate::sys::linux::i915::ioctl;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendPhysicalDevice;
use crate::traits::GenericAddressSpace;
use crate::traits::GenericBuffer;

pub struct I915AddressSpace {
    _physical_device: Arc<dyn BackendPhysicalDevice>,
}

pub struct I915Buffer {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    gem_handle: u32,
    size: usize,
    map_info: Option<u32>,
}

impl I915AddressSpace {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> I915AddressSpace {
        I915AddressSpace {
            _physical_device: physical_device,
        }
    }
}

impl GenericAddressSpace for I915AddressSpace {}
impl BackendAddressSpace for I915AddressSpace {}

impl I915Buffer {
    pub fn new(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        create_info: &MagmaCreateBufferInfo,
        mem_types: &[MagmaMemoryType],
    ) -> Result<I915Buffer> {
        let memory_type = mem_types
            .get(create_info.memory_type_idx as usize)
            .ok_or(Error::InvalidArgs)?;
        let gem_handle = ioctl::gem_create(physical_device.as_fd().unwrap(), create_info.size)?;

        Ok(I915Buffer {
            physical_device,
            gem_handle,
            size: create_info.size.try_into()?,
            map_info: memory_type.get_map_info(),
        })
    }

    pub fn from_existing(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        gem_handle: u32,
        size: usize,
        map_info: Option<u32>,
    ) -> Result<I915Buffer> {
        Ok(I915Buffer {
            physical_device,
            gem_handle,
            size,
            map_info,
        })
    }
}

impl GenericBuffer for I915Buffer {
    fn map(self: Arc<I915Buffer>) -> Result<Arc<dyn MappedRegion>> {
        let offset =
            ioctl::gem_mmap_offset(self.physical_device.as_fd().unwrap(), self.gem_handle)?;
        let mapping = self.physical_device.cpu_map(offset, self.size)?;
        Ok(Arc::new(mapping))
    }

    fn export(&self) -> Result<MagmaGpuHandle> {
        self.physical_device.export(self.gem_handle)
    }

    fn invalidate(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        Err(Error::Unimplemented)
    }

    fn flush(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        Err(Error::Unimplemented)
    }

    fn as_gem_handle(&self) -> Option<u32> {
        Some(self.gem_handle)
    }

    fn get_map_info(&self) -> Option<u32> {
        self.map_info
    }
}

impl Drop for I915Buffer {
    fn drop(&mut self) {
        self.physical_device.close(self.gem_handle);
    }
}

impl BackendBuffer for I915Buffer {}
