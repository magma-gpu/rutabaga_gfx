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
use crate::protocol::MagmaGpuMapFlags;
use crate::sys::linux::kgsl::ioctl;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendPhysicalDevice;
use crate::traits::GenericAddressSpace;
use crate::traits::GenericBuffer;

pub struct KgslAddressSpace {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    vbo_id: u32,
    vbo_base: u64,
    vbo_size: u64,
}

pub struct KgslBuffer {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    id: u32,
    #[allow(dead_code)]
    gpuaddr: u64,
    size: usize,
    map_info: Option<u32>,
}

impl KgslAddressSpace {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> Result<KgslAddressSpace> {
        let fd = physical_device.as_fd().unwrap();
        let (vbo_id, vbo_base, vbo_size) = ioctl::vbo_alloc(fd);

        Ok(KgslAddressSpace {
            physical_device,
            vbo_id,
            vbo_base,
            vbo_size,
        })
    }

    pub fn compute_target_offset(&self, gpu_va: u64) -> u64 {
        if gpu_va >= self.vbo_base {
            gpu_va - self.vbo_base
        } else {
            gpu_va
        }
    }
}

impl Drop for KgslAddressSpace {
    fn drop(&mut self) {
        if self.vbo_id != 0 {
            self.physical_device.close(self.vbo_id);
        }
    }
}

impl GenericAddressSpace for KgslAddressSpace {
    fn as_vm_id(&self) -> Option<u32> {
        if self.vbo_id != 0 {
            Some(self.vbo_id)
        } else {
            None
        }
    }

    fn map_buffer_gpu(
        &self,
        buffer: &Arc<dyn BackendBuffer>,
        buffer_offset: u64,
        gpu_va: u64,
        size: u64,
        _flags: MagmaGpuMapFlags,
    ) -> Result<()> {
        let child_id = buffer.as_gem_handle().ok_or(Error::Unimplemented)?;

        if self.vbo_id == 0 {
            return Ok(());
        }

        let target_offset = self.compute_target_offset(gpu_va);
        if target_offset
            .checked_add(size)
            .is_none_or(|end| end > self.vbo_size)
        {
            return Err(Error::Unimplemented);
        }

        ioctl::gpumem_bind_range(
            self.physical_device.as_fd().unwrap(),
            self.vbo_id,
            child_id,
            buffer_offset,
            target_offset,
            size,
        )
    }

    fn unmap_buffer_gpu(&self, gpu_va: u64, size: u64) -> Result<()> {
        if self.vbo_id == 0 {
            return Ok(());
        }

        let target_offset = self.compute_target_offset(gpu_va);
        ioctl::gpumem_unbind_range(
            self.physical_device.as_fd().unwrap(),
            self.vbo_id,
            target_offset,
            size,
        )
    }
}

impl BackendAddressSpace for KgslAddressSpace {}

impl KgslBuffer {
    pub fn new(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        create_info: &MagmaCreateBufferInfo,
        mem_types: &[MagmaMemoryType],
    ) -> Result<KgslBuffer> {
        let memory_type = mem_types
            .get(create_info.memory_type_idx as usize)
            .ok_or(Error::InvalidArgs)?;
        let (id, gpuaddr) = ioctl::gpumem_alloc_id(
            physical_device.as_fd().unwrap(),
            create_info.size,
            memory_type.is_cached(),
        )?;

        Ok(KgslBuffer {
            physical_device,
            id,
            gpuaddr,
            size: create_info.size.try_into()?,
            map_info: memory_type.get_map_info(),
        })
    }

    pub fn from_existing(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        id: u32,
        size: usize,
        map_info: Option<u32>,
    ) -> Result<KgslBuffer> {
        let gpuaddr = ioctl::gpuobj_info_gpuaddr(physical_device.as_fd().unwrap(), id);

        Ok(KgslBuffer {
            physical_device,
            id,
            gpuaddr,
            size,
            map_info,
        })
    }
}

impl GenericBuffer for KgslBuffer {
    fn map(self: Arc<KgslBuffer>) -> Result<Arc<dyn MappedRegion>> {
        let offset = (self.id as u64) << 12;
        let mapping = self.physical_device.cpu_map(offset, self.size)?;
        Ok(Arc::new(mapping))
    }

    fn export(&self) -> Result<MagmaGpuHandle> {
        self.physical_device.export(self.id)
    }

    fn invalidate(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        Ok(())
    }

    fn flush(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        Ok(())
    }

    fn as_gem_handle(&self) -> Option<u32> {
        Some(self.id)
    }

    fn get_map_info(&self) -> Option<u32> {
        self.map_info
    }
}

impl Drop for KgslBuffer {
    fn drop(&mut self) {
        self.physical_device.close(self.id);
    }
}

impl BackendBuffer for KgslBuffer {}
