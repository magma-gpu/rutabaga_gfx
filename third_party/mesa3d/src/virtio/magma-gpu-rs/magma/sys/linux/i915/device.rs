// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::os::fd::AsFd;
use std::sync::Arc;

use magma_gpu::util::OwnedDescriptor;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaCreateQueueInfo;
use crate::defines::MagmaHeap;
use crate::defines::MagmaHeapBudget;
use crate::defines::MagmaImportHandleInfo;
use crate::defines::MagmaMemoryType;
use crate::defines::MagmaPhysicalDeviceInfo;
use crate::defines::MagmaQueueFamilyProperties;
use crate::defines::MagmaQueueFlags;
use crate::error::Error;
use crate::error::Result;
use crate::sys::linux::i915::ioctl;
use crate::sys::linux::i915::memory::I915AddressSpace;
use crate::sys::linux::i915::memory::I915Buffer;
use crate::sys::linux::i915::queue::I915Queue;
use crate::sys::linux::PlatformDevice;
use crate::sys::linux::PlatformPhysicalDevice;
use crate::traits::AsVirtGpu;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendDevice;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendQueue;
use crate::traits::GenericDevice;
use crate::traits::GenericPhysicalDevice;

#[derive(Debug)]
pub struct I915PhysicalDevice {
    descriptor: OwnedDescriptor,
}

pub struct I915 {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    mem_types: Vec<MagmaMemoryType>,
    mem_heaps: Vec<MagmaHeap>,
}

impl I915PhysicalDevice {
    pub fn new(descriptor: OwnedDescriptor) -> I915PhysicalDevice {
        I915PhysicalDevice { descriptor }
    }
}

impl PlatformPhysicalDevice for I915PhysicalDevice {
    fn as_descriptor(&self) -> Option<&OwnedDescriptor> {
        Some(&self.descriptor)
    }
}

impl AsVirtGpu for I915PhysicalDevice {}
impl BackendPhysicalDevice for I915PhysicalDevice {}

impl GenericPhysicalDevice for I915PhysicalDevice {
    fn create_device(
        self: Arc<I915PhysicalDevice>,
        _info: &MagmaPhysicalDeviceInfo,
    ) -> Result<Arc<dyn BackendDevice>> {
        Ok(Arc::new(I915::new(self)?))
    }

    fn get_memory_types(&self) -> Result<Vec<MagmaMemoryType>> {
        ioctl::query_memory(self.descriptor.as_fd()).map(|(types, _)| types)
    }

    fn get_memory_heaps(&self) -> Result<Vec<MagmaHeap>> {
        ioctl::query_memory(self.descriptor.as_fd()).map(|(_, heaps)| heaps)
    }

    fn get_queue_family_properties(&self) -> Result<Vec<MagmaQueueFamilyProperties>> {
        Ok(vec![MagmaQueueFamilyProperties::new(
            MagmaQueueFlags::Graphics | MagmaQueueFlags::Compute,
            1,
        )])
    }
}

impl I915 {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> Result<I915> {
        let fd = physical_device.as_fd().unwrap();
        ioctl::check_aliasing_ppgtt(fd)?;
        let (mem_types, mem_heaps) = ioctl::query_memory(fd)?;

        Ok(I915 {
            physical_device,
            mem_types,
            mem_heaps,
        })
    }
}

impl GenericDevice for I915 {
    fn get_memory_budget(&self, heap_idx: u32) -> Result<MagmaHeapBudget> {
        if heap_idx >= self.mem_heaps.len() as u32 {
            return Err(Error::InvalidArgs);
        }

        let fd = self.physical_device.as_fd().unwrap();
        let mem_info = ioctl::query_memory_regions(fd)?;
        let heap = &self.mem_heaps[heap_idx as usize];

        let (budget, free) = if heap.is_cpu_visible() && !heap.is_device_local() {
            (mem_info.sysmem_total, mem_info.sysmem_free)
        } else if heap.is_cpu_visible() && heap.is_device_local() {
            (mem_info.vram_mappable_total, mem_info.vram_mappable_free)
        } else if !heap.is_cpu_visible() && heap.is_device_local() {
            (
                mem_info.vram_unmappable_total,
                mem_info.vram_unmappable_free,
            )
        } else {
            return Err(Error::Unimplemented);
        };

        Ok(MagmaHeapBudget {
            budget,
            usage: budget - free,
        })
    }

    fn create_address_space(self: Arc<I915>) -> Result<Arc<dyn BackendAddressSpace>> {
        Ok(Arc::new(I915AddressSpace::new(
            self.physical_device.clone(),
        )))
    }

    fn create_queue(
        self: Arc<I915>,
        _address_space: &Arc<dyn BackendAddressSpace>,
        _info: &MagmaCreateQueueInfo,
    ) -> Result<Arc<dyn BackendQueue>> {
        let queue = I915Queue::new(self.physical_device.clone())?;
        Ok(Arc::new(queue))
    }

    fn create_buffer(
        self: Arc<I915>,
        create_info: &MagmaCreateBufferInfo,
    ) -> Result<Arc<dyn BackendBuffer>> {
        let buf = I915Buffer::new(self.physical_device.clone(), create_info, &self.mem_types)?;
        Ok(Arc::new(buf))
    }

    fn import(self: Arc<I915>, info: MagmaImportHandleInfo) -> Result<Arc<dyn BackendBuffer>> {
        let gem_handle = self.physical_device.import(info.handle)?;
        let memory_type = self
            .mem_types
            .get(info.memory_type_idx as usize)
            .ok_or(Error::InvalidArgs)?;
        let buf = I915Buffer::from_existing(
            self.physical_device.clone(),
            gem_handle,
            info.size.try_into()?,
            memory_type.get_map_info(),
        )?;
        Ok(Arc::new(buf))
    }
}

impl BackendDevice for I915 {}
impl PlatformDevice for I915 {}
