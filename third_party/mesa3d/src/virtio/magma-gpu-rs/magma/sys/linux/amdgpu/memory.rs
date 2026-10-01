// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaMappedMemoryRange;
use crate::defines::MagmaMemoryType;
use crate::defines::MAGMA_BUFFER_FLAG_AMD_GDS;
use crate::defines::MAGMA_BUFFER_FLAG_AMD_OA;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaGpuMapFlags;
use crate::sys::linux::amdgpu::ioctl::amdgpu_gem_create;
use crate::sys::linux::amdgpu::ioctl::amdgpu_gem_mmap;
use crate::sys::linux::amdgpu::ioctl::amdgpu_gem_va;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_GEM_CREATE_CPU_GTT_USWC;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_GEM_CREATE_DISCARDABLE;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_GEM_CREATE_ENCRYPTED;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_GEM_CREATE_EXPLICIT_SYNC;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_GEM_CREATE_NO_CPU_ACCESS;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_GEM_DOMAIN_GDS;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_GEM_DOMAIN_GTT;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_GEM_DOMAIN_OA;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_GEM_DOMAIN_VRAM;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_VA_OP_CLEAR;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_VA_OP_MAP;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_VM_PAGE_EXECUTABLE;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_VM_PAGE_READABLE;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_VM_PAGE_WRITEABLE;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendPhysicalDevice;
use crate::traits::GenericAddressSpace;
use crate::traits::GenericBuffer;

pub struct AmdGpuAddressSpace {
    pub physical_device: Arc<dyn BackendPhysicalDevice>,
}

pub struct AmdGpuBuffer {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    gem_handle: u32,
    size: usize,
    map_info: Option<u32>,
}

impl GenericAddressSpace for AmdGpuAddressSpace {
    fn map_buffer_gpu(
        &self,
        buffer: &Arc<dyn BackendBuffer>,
        buffer_offset: u64,
        gpu_va: u64,
        size: u64,
        flags: MagmaGpuMapFlags,
    ) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        let gem_handle = buffer.as_gem_handle().ok_or(Error::Unimplemented)?;
        let mut map_flags = 0u32;
        if flags.contains(MagmaGpuMapFlags::Read) {
            map_flags |= AMDGPU_VM_PAGE_READABLE;
        }
        if flags.contains(MagmaGpuMapFlags::Write) {
            map_flags |= AMDGPU_VM_PAGE_WRITEABLE;
        }
        if flags.contains(MagmaGpuMapFlags::Execute) {
            map_flags |= AMDGPU_VM_PAGE_EXECUTABLE;
        }
        if map_flags == 0 {
            map_flags =
                AMDGPU_VM_PAGE_READABLE | AMDGPU_VM_PAGE_WRITEABLE | AMDGPU_VM_PAGE_EXECUTABLE;
        }

        amdgpu_gem_va(
            fd,
            gem_handle,
            AMDGPU_VA_OP_MAP,
            map_flags,
            gpu_va,
            buffer_offset,
            size,
        )
    }

    fn unmap_buffer_gpu(&self, gpu_va: u64, size: u64) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        amdgpu_gem_va(fd, 0, AMDGPU_VA_OP_CLEAR, 0, gpu_va, 0, size)
    }
}

impl BackendAddressSpace for AmdGpuAddressSpace {}

impl AmdGpuBuffer {
    pub fn new(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        create_info: &MagmaCreateBufferInfo,
        mem_types: &[MagmaMemoryType],
    ) -> Result<AmdGpuBuffer> {
        let fd = physical_device.as_fd().unwrap();
        let memory_type = mem_types
            .get(create_info.memory_type_idx as usize)
            .ok_or(Error::InvalidArgs)?;

        let mut domain_flags =
            (AMDGPU_GEM_CREATE_EXPLICIT_SYNC | AMDGPU_GEM_CREATE_DISCARDABLE) as u64;

        if memory_type.is_coherent() {
            domain_flags |= AMDGPU_GEM_CREATE_CPU_GTT_USWC as u64;
        } else {
            domain_flags |= AMDGPU_GEM_CREATE_NO_CPU_ACCESS as u64;
        }

        if memory_type.is_protected() {
            domain_flags |= AMDGPU_GEM_CREATE_ENCRYPTED as u64;
        }

        let domains = if create_info.vendor_flags & MAGMA_BUFFER_FLAG_AMD_OA != 0 {
            AMDGPU_GEM_DOMAIN_OA as u64
        } else if create_info.vendor_flags & MAGMA_BUFFER_FLAG_AMD_GDS != 0 {
            AMDGPU_GEM_DOMAIN_GDS as u64
        } else if memory_type.is_device_local() {
            AMDGPU_GEM_DOMAIN_VRAM as u64
        } else {
            AMDGPU_GEM_DOMAIN_GTT as u64
        };

        let gem_handle = amdgpu_gem_create(
            fd,
            create_info.size,
            create_info.alignment as u64,
            domains,
            domain_flags,
        )?;

        Ok(AmdGpuBuffer {
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
    ) -> Result<AmdGpuBuffer> {
        Ok(AmdGpuBuffer {
            physical_device,
            gem_handle,
            size,
            map_info,
        })
    }
}

impl GenericBuffer for AmdGpuBuffer {
    fn map(self: Arc<AmdGpuBuffer>) -> Result<Arc<dyn MappedRegion>> {
        let fd = self.physical_device.as_fd().unwrap();
        let offset = amdgpu_gem_mmap(fd, self.gem_handle)?;
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

    fn get_map_info(&self) -> Option<u32> {
        self.map_info
    }

    fn as_gem_handle(&self) -> Option<u32> {
        Some(self.gem_handle)
    }
}

impl Drop for AmdGpuBuffer {
    fn drop(&mut self) {}
}

impl BackendBuffer for AmdGpuBuffer {}
