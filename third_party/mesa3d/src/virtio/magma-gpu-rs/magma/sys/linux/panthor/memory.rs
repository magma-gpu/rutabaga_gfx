// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use log::error;

use magma_gpu::log_status;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaMappedMemoryRange;
use crate::defines::MagmaMemoryType;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaGpuMapFlags;
use crate::sys::linux::panthor::device::PanthorPhysicalDevice;
use crate::sys::linux::panthor::ioctl;
use crate::sys::linux::PlatformPhysicalDevice;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::GenericAddressSpace;
use crate::traits::GenericBuffer;

pub struct PanthorAddressSpace {
    physical_device: Arc<PanthorPhysicalDevice>,
    vm_id: u32,
}

pub struct PanthorBuffer {
    physical_device: Arc<PanthorPhysicalDevice>,
    gem_handle: u32,
    size: usize,
    map_info: Option<u32>,
}

impl PanthorAddressSpace {
    pub fn new(physical_device: Arc<PanthorPhysicalDevice>) -> Result<PanthorAddressSpace> {
        let vm_id = ioctl::vm_create(physical_device.as_fd().unwrap())?;
        Ok(PanthorAddressSpace {
            physical_device,
            vm_id,
        })
    }
}

impl Drop for PanthorAddressSpace {
    fn drop(&mut self) {
        let result = ioctl::vm_destroy(self.physical_device.as_fd().unwrap(), self.vm_id);
        log_status!(result);
    }
}

impl GenericAddressSpace for PanthorAddressSpace {
    fn as_vm_id(&self) -> Option<u32> {
        Some(self.vm_id)
    }

    fn map_buffer_gpu(
        &self,
        buffer: &Arc<dyn BackendBuffer>,
        buffer_offset: u64,
        gpu_va: u64,
        size: u64,
        flags: MagmaGpuMapFlags,
    ) -> Result<()> {
        let gem_handle = buffer.as_gem_handle().ok_or(Error::Unimplemented)?;
        ioctl::vm_map(
            self.physical_device.as_fd().unwrap(),
            self.vm_id,
            gem_handle,
            buffer_offset,
            gpu_va,
            size,
            flags,
        )
    }

    fn unmap_buffer_gpu(&self, gpu_va: u64, size: u64) -> Result<()> {
        ioctl::vm_unmap(
            self.physical_device.as_fd().unwrap(),
            self.vm_id,
            gpu_va,
            size,
        )
    }
}

impl BackendAddressSpace for PanthorAddressSpace {}

impl PanthorBuffer {
    pub fn new(
        physical_device: Arc<PanthorPhysicalDevice>,
        create_info: &MagmaCreateBufferInfo,
        mem_types: &[MagmaMemoryType],
    ) -> Result<PanthorBuffer> {
        let mem_type = mem_types
            .get(create_info.memory_type_idx as usize)
            .ok_or(Error::InvalidArgs)?;
        let (gem_handle, size) = ioctl::bo_create(
            physical_device.as_fd().unwrap(),
            create_info.size,
            mem_type.is_cached(),
        )?;

        Ok(PanthorBuffer {
            physical_device,
            gem_handle,
            size,
            map_info: mem_type.get_map_info(),
        })
    }

    pub fn from_existing(
        physical_device: Arc<PanthorPhysicalDevice>,
        gem_handle: u32,
        size: usize,
        map_info: Option<u32>,
    ) -> Result<PanthorBuffer> {
        Ok(PanthorBuffer {
            physical_device,
            gem_handle,
            size,
            map_info,
        })
    }
}

impl Drop for PanthorBuffer {
    fn drop(&mut self) {
        self.physical_device.close(self.gem_handle);
    }
}

impl GenericBuffer for PanthorBuffer {
    fn map(self: Arc<PanthorBuffer>) -> Result<Arc<dyn MappedRegion>> {
        let offset = ioctl::bo_mmap_offset(self.physical_device.as_fd().unwrap(), self.gem_handle)?;
        let mapping = self.physical_device.cpu_map(offset, self.size)?;
        Ok(Arc::new(mapping))
    }

    fn export(&self) -> Result<MagmaGpuHandle> {
        self.physical_device.export(self.gem_handle)
    }

    fn invalidate(&self, _sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        ioctl::bo_invalidate(
            self.physical_device.as_fd().unwrap(),
            self.gem_handle,
            self.size as u64,
            ranges,
        )
    }

    fn flush(&self, _sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        ioctl::bo_flush(
            self.physical_device.as_fd().unwrap(),
            self.gem_handle,
            self.size as u64,
            ranges,
        )
    }

    fn as_gem_handle(&self) -> Option<u32> {
        Some(self.gem_handle)
    }

    fn get_map_info(&self) -> Option<u32> {
        self.map_info
    }
}

impl BackendBuffer for PanthorBuffer {}
