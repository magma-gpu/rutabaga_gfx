// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::os::fd::AsFd;
use std::sync::Arc;

use magma_gpu::util::OwnedDescriptor;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaCreateQueueInfo;
use crate::defines::MagmaCreateSyncObjInfo;
use crate::defines::MagmaHeap;
use crate::defines::MagmaHeapBudget;
use crate::defines::MagmaImportHandleInfo;
use crate::defines::MagmaMemoryType;
use crate::defines::MagmaPhysicalDeviceInfo;
use crate::defines::MagmaQueueFamilyProperties;
use crate::defines::MagmaQueueFlags;
use crate::defines::MagmaSyncType;
use crate::defines::MAGMA_HEAP_CPU_VISIBLE_BIT;
use crate::defines::MAGMA_HEAP_DEVICE_LOCAL_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
use crate::error::Error;
use crate::error::Result;
use crate::sys::linux::amdgpu::ioctl::amdgpu_info_gtt_usage;
use crate::sys::linux::amdgpu::ioctl::amdgpu_info_hw_ip;
use crate::sys::linux::amdgpu::ioctl::amdgpu_info_memory;
use crate::sys::linux::amdgpu::ioctl::amdgpu_info_vis_vram_usage;
use crate::sys::linux::amdgpu::ioctl::amdgpu_info_vram_gtt;
use crate::sys::linux::amdgpu::ioctl::amdgpu_info_vram_usage;
use crate::sys::linux::amdgpu::memory::AmdGpuAddressSpace;
use crate::sys::linux::amdgpu::memory::AmdGpuBuffer;
use crate::sys::linux::amdgpu::queue::AmdGpuQueue;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_HW_IP_COMPUTE;
use crate::sys::linux::bindings::amdgpu_bindings::AMDGPU_HW_IP_GFX;
use crate::sys::linux::drm::DrmSyncObject;
use crate::sys::linux::PlatformDevice;
use crate::sys::linux::PlatformPhysicalDevice;
use crate::traits::AsVirtGpu;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendDevice;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendQueue;
use crate::traits::BackendSyncObject;
use crate::traits::GenericDevice;
use crate::traits::GenericPhysicalDevice;
use crate::traits::GenericSyncObject;

#[derive(Debug)]
pub struct AmdGpuPhysicalDevice {
    descriptor: OwnedDescriptor,
}

pub struct AmdGpu {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    mem_types: Vec<MagmaMemoryType>,
    mem_heaps: Vec<MagmaHeap>,
}

impl AmdGpuPhysicalDevice {
    pub fn new(descriptor: OwnedDescriptor) -> AmdGpuPhysicalDevice {
        AmdGpuPhysicalDevice { descriptor }
    }

    fn query_memory(&self) -> Result<(Vec<MagmaMemoryType>, Vec<MagmaHeap>)> {
        let memory_info = amdgpu_info_memory(self.descriptor.as_fd())?;

        let mut heaps = Vec::new();
        let mut types = Vec::new();
        let mut heap_idx = 0u32;

        if memory_info.gtt.total_heap_size > 0 {
            heaps.push(MagmaHeap {
                heap_size: memory_info.gtt.total_heap_size,
                heap_flags: MAGMA_HEAP_CPU_VISIBLE_BIT,
            });
            types.push(MagmaMemoryType {
                property_flags: MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
                heap_idx,
            });
            types.push(MagmaMemoryType {
                property_flags: MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT,
                heap_idx,
            });
            heap_idx += 1;
        }

        if memory_info.vram.total_heap_size > 0 {
            heaps.push(MagmaHeap {
                heap_size: memory_info.vram.total_heap_size,
                heap_flags: MAGMA_HEAP_DEVICE_LOCAL_BIT,
            });
            types.push(MagmaMemoryType {
                property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
                heap_idx,
            });
            heap_idx += 1;
        }

        if memory_info.cpu_accessible_vram.total_heap_size > 0 {
            heaps.push(MagmaHeap {
                heap_size: memory_info.cpu_accessible_vram.total_heap_size,
                heap_flags: MAGMA_HEAP_DEVICE_LOCAL_BIT | MAGMA_HEAP_CPU_VISIBLE_BIT,
            });
            types.push(MagmaMemoryType {
                property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
                heap_idx,
            });
        }

        Ok((types, heaps))
    }
}

impl PlatformPhysicalDevice for AmdGpuPhysicalDevice {
    fn as_descriptor(&self) -> Option<&OwnedDescriptor> {
        Some(&self.descriptor)
    }
}

impl AsVirtGpu for AmdGpuPhysicalDevice {}
impl BackendPhysicalDevice for AmdGpuPhysicalDevice {}

impl GenericPhysicalDevice for AmdGpuPhysicalDevice {
    fn create_device(
        self: Arc<AmdGpuPhysicalDevice>,
        _info: &MagmaPhysicalDeviceInfo,
    ) -> Result<Arc<dyn BackendDevice>> {
        Ok(Arc::new(AmdGpu::new(self)?))
    }

    fn get_memory_types(&self) -> Result<Vec<MagmaMemoryType>> {
        self.query_memory().map(|(types, _)| types)
    }

    fn get_memory_heaps(&self) -> Result<Vec<MagmaHeap>> {
        self.query_memory().map(|(_, heaps)| heaps)
    }

    fn get_queue_family_properties(&self) -> Result<Vec<MagmaQueueFamilyProperties>> {
        let mut queue_families: Vec<MagmaQueueFamilyProperties> = Vec::new();
        let fd = self.descriptor.as_fd();

        let gfx_hw_ip = amdgpu_info_hw_ip(fd, AMDGPU_HW_IP_GFX, 0)?;
        if gfx_hw_ip.available_rings > 0 {
            queue_families.push(MagmaQueueFamilyProperties::new(
                MagmaQueueFlags::Graphics | MagmaQueueFlags::Compute,
                1,
            ));
        }

        if let Ok(compute_hw_ip) = amdgpu_info_hw_ip(fd, AMDGPU_HW_IP_COMPUTE, 0) {
            if compute_hw_ip.available_rings > 0 {
                queue_families.push(MagmaQueueFamilyProperties::new(
                    MagmaQueueFlags::Compute,
                    compute_hw_ip.available_rings.count_ones(),
                ));
            }
        }

        if queue_families.is_empty() {
            return Err(Error::InternalError);
        }

        Ok(queue_families)
    }
}

impl AmdGpu {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> Result<AmdGpu> {
        let mem_types = physical_device.get_memory_types()?;
        let mem_heaps = physical_device.get_memory_heaps()?;

        Ok(AmdGpu {
            physical_device,
            mem_types,
            mem_heaps,
        })
    }
}

impl GenericDevice for AmdGpu {
    fn get_memory_budget(&self, heap_idx: u32) -> Result<MagmaHeapBudget> {
        let heap = self
            .mem_heaps
            .get(heap_idx as usize)
            .ok_or(Error::InvalidArgs)?;
        let fd = self.physical_device.as_fd().unwrap();

        let vram_gtt = amdgpu_info_vram_gtt(fd)?;

        let (budget, usage) = if heap.is_device_local() && heap.is_cpu_visible() {
            (
                vram_gtt.vram_cpu_accessible_size,
                amdgpu_info_vis_vram_usage(fd)?,
            )
        } else if heap.is_device_local() {
            (vram_gtt.vram_size, amdgpu_info_vram_usage(fd)?)
        } else if heap.is_cpu_visible() {
            (vram_gtt.gtt_size, amdgpu_info_gtt_usage(fd)?)
        } else {
            return Err(Error::Unimplemented);
        };

        Ok(MagmaHeapBudget { budget, usage })
    }

    fn create_address_space(self: Arc<AmdGpu>) -> Result<Arc<dyn BackendAddressSpace>> {
        Ok(Arc::new(AmdGpuAddressSpace {
            physical_device: self.physical_device.clone(),
        }))
    }

    fn create_queue(
        self: Arc<AmdGpu>,
        _address_space: &Arc<dyn BackendAddressSpace>,
        info: &MagmaCreateQueueInfo,
    ) -> Result<Arc<dyn BackendQueue>> {
        let queue = AmdGpuQueue::new(self.physical_device.clone(), info.priority as i32)?;
        Ok(Arc::new(queue))
    }

    fn create_buffer(
        self: Arc<AmdGpu>,
        create_info: &MagmaCreateBufferInfo,
    ) -> Result<Arc<dyn BackendBuffer>> {
        let buf = AmdGpuBuffer::new(self.physical_device.clone(), create_info, &self.mem_types)?;
        Ok(Arc::new(buf))
    }

    fn import(self: Arc<AmdGpu>, info: MagmaImportHandleInfo) -> Result<Arc<dyn BackendBuffer>> {
        let map_info = self
            .mem_types
            .get(info.memory_type_idx as usize)
            .and_then(|mt| mt.get_map_info());
        let gem_handle = self.physical_device.import(info.handle)?;
        let buf = AmdGpuBuffer::from_existing(
            self.physical_device.clone(),
            gem_handle,
            info.size.try_into()?,
            map_info,
        )?;
        Ok(Arc::new(buf))
    }

    fn create_sync_obj(
        self: Arc<AmdGpu>,
        info: &MagmaCreateSyncObjInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        let sync_obj = DrmSyncObject::new_from_info(self.physical_device.clone(), info)?;
        Ok(Arc::new(sync_obj))
    }

    fn import_sync_obj(
        self: Arc<AmdGpu>,
        info: MagmaImportHandleInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        let sync_obj = DrmSyncObject::new(self.physical_device.clone(), MagmaSyncType::Binary)?;
        sync_obj.import(info.handle)?;
        Ok(Arc::new(sync_obj))
    }
}

impl BackendDevice for AmdGpu {}
impl PlatformDevice for AmdGpu {}
