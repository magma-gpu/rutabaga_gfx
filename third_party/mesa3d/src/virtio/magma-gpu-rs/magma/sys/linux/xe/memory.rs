// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use log::error;
use magma_gpu::log_status;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;
use magma_gpu::util::MAGMA_MAP_ACCESS_RW;
use magma_gpu::util::MAGMA_MAP_CACHE_WC;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaHeap;
use crate::defines::MagmaMappedMemoryRange;
use crate::defines::MagmaMemoryType;
use crate::defines::MAGMA_BUFFER_FLAG_SCANOUT;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaGpuMapFlags;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_GEM_CPU_CACHING_WB;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_GEM_CPU_CACHING_WC;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_GEM_CREATE_FLAG_NEEDS_VISIBLE_VRAM;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_GEM_CREATE_FLAG_SCANOUT;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_VM_BIND_OP_MAP;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_VM_BIND_OP_UNMAP;
use crate::sys::linux::bindings::xe_bindings::DRM_XE_VM_CREATE_FLAG_SCRATCH_PAGE;
use crate::sys::linux::xe::ioctl::xe_gem_create;
use crate::sys::linux::xe::ioctl::xe_gem_mmap_offset;
use crate::sys::linux::xe::ioctl::xe_vm_bind_single;
use crate::sys::linux::xe::ioctl::xe_vm_create;
use crate::sys::linux::xe::ioctl::xe_vm_destroy;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendPhysicalDevice;
use crate::traits::GenericAddressSpace;
use crate::traits::GenericBuffer;

pub struct XeAddressSpace {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    vm_id: u32,
}

pub struct XeBuffer {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    gem_handle: u32,
    size: usize,
    map_info: Option<u32>,
}

impl XeAddressSpace {
    pub fn new(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        _priority: i32,
    ) -> Result<XeAddressSpace> {
        let fd = physical_device.as_fd().unwrap();
        let vm_id = xe_vm_create(fd, DRM_XE_VM_CREATE_FLAG_SCRATCH_PAGE)?;

        Ok(XeAddressSpace {
            physical_device,
            vm_id,
        })
    }
}

impl Drop for XeAddressSpace {
    fn drop(&mut self) {
        let result = xe_vm_destroy(self.physical_device.as_fd().unwrap(), self.vm_id);
        log_status!(result);
    }
}

impl GenericAddressSpace for XeAddressSpace {
    fn as_vm_id(&self) -> Option<u32> {
        Some(self.vm_id)
    }

    fn map_buffer_gpu(
        &self,
        buffer: &Arc<dyn BackendBuffer>,
        buffer_offset: u64,
        gpu_va: u64,
        size: u64,
        _flags: MagmaGpuMapFlags,
    ) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        let gem_handle = buffer.as_gem_handle().ok_or(Error::Unimplemented)?;
        xe_vm_bind_single(
            fd,
            self.vm_id,
            DRM_XE_VM_BIND_OP_MAP,
            gem_handle,
            buffer_offset,
            gpu_va,
            size,
        )
    }

    fn unmap_buffer_gpu(&self, gpu_va: u64, size: u64) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        xe_vm_bind_single(fd, self.vm_id, DRM_XE_VM_BIND_OP_UNMAP, 0, 0, gpu_va, size)
    }
}

impl BackendAddressSpace for XeAddressSpace {}

impl XeBuffer {
    pub fn new(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        create_info: &MagmaCreateBufferInfo,
        mem_types: &[MagmaMemoryType],
        mem_heaps: &[MagmaHeap],
        sysmem_instance: u16,
        vram_instance: u16,
    ) -> Result<XeBuffer> {
        let fd = physical_device.as_fd().unwrap();
        let memory_type = mem_types
            .get(create_info.memory_type_idx as usize)
            .ok_or(Error::InvalidArgs)?;
        let memory_heap = mem_heaps
            .get(memory_type.heap_idx as usize)
            .ok_or(Error::InvalidArgs)?;

        let mut flags = 0;
        let is_scanout = (create_info.common_flags & MAGMA_BUFFER_FLAG_SCANOUT) != 0;
        let (cpu_caching, map_info) = if is_scanout {
            flags |= DRM_XE_GEM_CREATE_FLAG_SCANOUT;
            let map_info = if memory_type.is_host_visible() {
                Some(MAGMA_MAP_CACHE_WC | MAGMA_MAP_ACCESS_RW)
            } else {
                None
            };
            (DRM_XE_GEM_CPU_CACHING_WC as u16, map_info)
        } else if memory_type.is_cached() {
            (DRM_XE_GEM_CPU_CACHING_WB as u16, memory_type.get_map_info())
        } else {
            (DRM_XE_GEM_CPU_CACHING_WC as u16, memory_type.get_map_info())
        };

        let mut placement = 0;
        if memory_heap.is_cpu_visible() && memory_heap.is_device_local() {
            flags |= DRM_XE_GEM_CREATE_FLAG_NEEDS_VISIBLE_VRAM;
            placement |= 1 << sysmem_instance;
            placement |= 1 << vram_instance;
        } else if memory_heap.is_device_local() {
            placement |= 1 << vram_instance;
        } else if memory_heap.is_cpu_visible() {
            placement |= 1 << sysmem_instance;
        }

        let gem_handle = xe_gem_create(
            fd,
            create_info.size,
            flags,
            cpu_caching,
            placement,
            memory_type.is_protected(),
        )?;

        Ok(XeBuffer {
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
    ) -> Result<XeBuffer> {
        Ok(XeBuffer {
            physical_device,
            gem_handle,
            size,
            map_info,
        })
    }
}

impl GenericBuffer for XeBuffer {
    fn map(self: Arc<XeBuffer>) -> Result<Arc<dyn MappedRegion>> {
        let fd = self.physical_device.as_fd().unwrap();
        let offset = xe_gem_mmap_offset(fd, self.gem_handle)?;
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

impl Drop for XeBuffer {
    fn drop(&mut self) {
        self.physical_device.close(self.gem_handle)
    }
}

impl BackendBuffer for XeBuffer {}
