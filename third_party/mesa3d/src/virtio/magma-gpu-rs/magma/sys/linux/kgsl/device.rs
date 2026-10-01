// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::os::fd::AsFd;
use std::sync::Arc;

use log::error;

use magma_gpu::log_status;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MemoryMapping;
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
use crate::defines::MAGMA_HEAP_CPU_VISIBLE_BIT;
use crate::defines::MAGMA_HEAP_DEVICE_LOCAL_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
use crate::error::Error;
use crate::error::Result;
use crate::sys::linux::kgsl::ioctl;
use crate::sys::linux::kgsl::memory::KgslAddressSpace;
use crate::sys::linux::kgsl::memory::KgslBuffer;
use crate::sys::linux::kgsl::queue::KgslQueue;
use crate::sys::linux::kgsl::sync::KgslSyncObject;
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

#[derive(Debug)]
#[allow(dead_code)]
pub struct KgslPhysicalDevice {
    descriptor: OwnedDescriptor,
    chip_id: u64,
    gpu_id: u32,
    gmem_size: u64,
}

pub struct Kgsl {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    mem_types: Vec<MagmaMemoryType>,
    mem_heaps: Vec<MagmaHeap>,
}

#[allow(dead_code)]
impl KgslPhysicalDevice {
    pub fn new(descriptor: OwnedDescriptor) -> KgslPhysicalDevice {
        let (chip_id, gpu_id, gmem_size) = if let Ok(info) = ioctl::get_devinfo(descriptor.as_fd())
        {
            let cid = info.chip_id as u64;
            let gid = if info.gpu_id != 0 {
                info.gpu_id
            } else {
                (((cid >> 24) & 0xff) * 100 + ((cid >> 16) & 0xff) * 10 + ((cid >> 8) & 0xff))
                    as u32
            };
            (cid, gid, info.gmem_sizebytes)
        } else {
            (0, 0, 0)
        };

        KgslPhysicalDevice {
            descriptor,
            chip_id,
            gpu_id,
            gmem_size,
        }
    }

    pub fn chip_id(&self) -> u64 {
        self.chip_id
    }

    pub fn gpu_id(&self) -> u32 {
        self.gpu_id
    }

    pub fn gmem_size(&self) -> u64 {
        self.gmem_size
    }
}

impl PlatformPhysicalDevice for KgslPhysicalDevice {
    fn as_descriptor(&self) -> Option<&OwnedDescriptor> {
        Some(&self.descriptor)
    }

    fn cpu_map(&self, offset: u64, size: usize) -> Result<MemoryMapping> {
        let desc = self.as_descriptor().ok_or(Error::Unimplemented)?;
        Ok(MemoryMapping::from_offset(desc, offset.try_into()?, size)?)
    }

    fn export(&self, _gem_handle: u32) -> Result<MagmaGpuHandle> {
        Err(Error::Unimplemented)
    }

    fn import(&self, handle: MagmaGpuHandle) -> Result<u32> {
        let fd = self.as_fd().unwrap();
        ioctl::gpuobj_import_dmabuf(fd, &handle)
    }

    fn close(&self, gem_handle: u32) {
        let result = ioctl::gpumem_free_id(self.as_fd().unwrap(), gem_handle);
        log_status!(result);
    }
}

impl AsVirtGpu for KgslPhysicalDevice {}
impl BackendPhysicalDevice for KgslPhysicalDevice {}

impl GenericPhysicalDevice for KgslPhysicalDevice {
    fn create_device(
        self: Arc<KgslPhysicalDevice>,
        _info: &MagmaPhysicalDeviceInfo,
    ) -> Result<Arc<dyn BackendDevice>> {
        Ok(Arc::new(Kgsl::new(self)?))
    }

    fn get_memory_types(&self) -> Result<Vec<MagmaMemoryType>> {
        Ok(vec![
            MagmaMemoryType {
                property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT,
                heap_idx: 0,
            },
            MagmaMemoryType {
                property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT,
                heap_idx: 0,
            },
        ])
    }

    fn get_memory_heaps(&self) -> Result<Vec<MagmaHeap>> {
        let heap_size = 4 * 1024 * 1024 * 1024; // 4 GiB default unified memory heap
        Ok(vec![MagmaHeap {
            heap_size,
            heap_flags: MAGMA_HEAP_DEVICE_LOCAL_BIT | MAGMA_HEAP_CPU_VISIBLE_BIT,
        }])
    }

    fn get_queue_family_properties(&self) -> Result<Vec<MagmaQueueFamilyProperties>> {
        Ok(vec![MagmaQueueFamilyProperties::new(
            MagmaQueueFlags::Graphics | MagmaQueueFlags::Compute,
            1,
        )])
    }
}

impl Kgsl {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> Result<Kgsl> {
        let mem_types = physical_device.get_memory_types()?;
        let mem_heaps = physical_device.get_memory_heaps()?;
        Ok(Kgsl {
            physical_device,
            mem_types,
            mem_heaps,
        })
    }
}

impl GenericDevice for Kgsl {
    fn get_memory_budget(&self, heap_idx: u32) -> Result<MagmaHeapBudget> {
        if let Some(heap) = self.mem_heaps.get(heap_idx as usize) {
            Ok(MagmaHeapBudget {
                budget: heap.heap_size,
                usage: 0,
            })
        } else {
            Err(Error::InvalidArgs)
        }
    }

    fn create_address_space(self: Arc<Kgsl>) -> Result<Arc<dyn BackendAddressSpace>> {
        Ok(Arc::new(KgslAddressSpace::new(
            self.physical_device.clone(),
        )?))
    }

    fn create_queue(
        self: Arc<Kgsl>,
        _address_space: &Arc<dyn BackendAddressSpace>,
        _info: &MagmaCreateQueueInfo,
    ) -> Result<Arc<dyn BackendQueue>> {
        Ok(Arc::new(KgslQueue::new(self.physical_device.clone())?))
    }

    fn create_buffer(
        self: Arc<Kgsl>,
        create_info: &MagmaCreateBufferInfo,
    ) -> Result<Arc<dyn BackendBuffer>> {
        let buf = KgslBuffer::new(self.physical_device.clone(), create_info, &self.mem_types)?;
        Ok(Arc::new(buf))
    }

    fn import(self: Arc<Kgsl>, info: MagmaImportHandleInfo) -> Result<Arc<dyn BackendBuffer>> {
        let id = self.physical_device.import(info.handle)?;
        let memory_type = self
            .mem_types
            .get(info.memory_type_idx as usize)
            .ok_or(Error::InvalidArgs)?;
        let buf = KgslBuffer::from_existing(
            self.physical_device.clone(),
            id,
            info.size.try_into()?,
            memory_type.get_map_info(),
        )?;
        Ok(Arc::new(buf))
    }

    fn create_sync_obj(
        self: Arc<Kgsl>,
        _info: &MagmaCreateSyncObjInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        Ok(Arc::new(KgslSyncObject::new(self.physical_device.clone())?))
    }

    fn import_sync_obj(
        self: Arc<Kgsl>,
        info: MagmaImportHandleInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        Ok(Arc::new(KgslSyncObject::from_import(
            self.physical_device.clone(),
            &info,
        )?))
    }
}

impl PlatformDevice for Kgsl {}
impl BackendDevice for Kgsl {}
