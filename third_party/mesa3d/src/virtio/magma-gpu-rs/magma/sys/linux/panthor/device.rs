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
use crate::error::Error;
use crate::error::Result;
use crate::sys::linux::bindings::panthor_bindings::drm_panthor_csif_info;
use crate::sys::linux::bindings::panthor_bindings::drm_panthor_gpu_info;
use crate::sys::linux::drm::DrmSyncObject;
use crate::sys::linux::panthor::ioctl;
use crate::sys::linux::panthor::memory::PanthorAddressSpace;
use crate::sys::linux::panthor::memory::PanthorBuffer;
use crate::sys::linux::panthor::queue::PanthorQueue;
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

#[allow(dead_code)]
#[derive(Debug)]
pub struct PanthorPhysicalDevice {
    descriptor: OwnedDescriptor,
    gpu_info: drm_panthor_gpu_info,
    csif_info: drm_panthor_csif_info,
}

pub struct Panthor {
    physical_device: Arc<PanthorPhysicalDevice>,
    mem_types: Vec<MagmaMemoryType>,
    mem_heaps: Vec<MagmaHeap>,
}

#[allow(dead_code)]
impl PanthorPhysicalDevice {
    pub fn new(descriptor: OwnedDescriptor) -> PanthorPhysicalDevice {
        let gpu_info = ioctl::query_gpu_info(descriptor.as_fd()).unwrap_or_default();
        let csif_info = ioctl::query_csif_info(descriptor.as_fd()).unwrap_or_default();
        PanthorPhysicalDevice {
            descriptor,
            gpu_info,
            csif_info,
        }
    }

    pub fn gpu_info(&self) -> &drm_panthor_gpu_info {
        &self.gpu_info
    }

    pub fn csif_info(&self) -> &drm_panthor_csif_info {
        &self.csif_info
    }

    pub fn gpu_id(&self) -> u32 {
        self.gpu_info.gpu_id
    }

    pub fn device_id(&self) -> u16 {
        (self.gpu_info.gpu_id >> 12) as u16
    }
}

impl PlatformPhysicalDevice for PanthorPhysicalDevice {
    fn as_descriptor(&self) -> Option<&OwnedDescriptor> {
        Some(&self.descriptor)
    }
}

impl AsVirtGpu for PanthorPhysicalDevice {}
impl BackendPhysicalDevice for PanthorPhysicalDevice {}

impl GenericPhysicalDevice for PanthorPhysicalDevice {
    fn create_device(
        self: Arc<PanthorPhysicalDevice>,
        _device_info: &MagmaPhysicalDeviceInfo,
    ) -> Result<Arc<dyn BackendDevice>> {
        Ok(Arc::new(Panthor::new(self)?))
    }

    fn get_memory_types(&self) -> Result<Vec<MagmaMemoryType>> {
        Ok(ioctl::query_memory_types(&self.gpu_info))
    }

    fn get_memory_heaps(&self) -> Result<Vec<MagmaHeap>> {
        Ok(ioctl::query_memory_heaps())
    }

    fn get_queue_family_properties(&self) -> Result<Vec<MagmaQueueFamilyProperties>> {
        Ok(vec![
            MagmaQueueFamilyProperties::new(
                MagmaQueueFlags::Graphics
                    | MagmaQueueFlags::Compute
                    | MagmaQueueFlags::Transfer
                    | MagmaQueueFlags::SparseBinding,
                2,
            ),
            MagmaQueueFamilyProperties::new(MagmaQueueFlags::SparseBinding, 1),
        ])
    }
}

impl Panthor {
    pub fn new(physical_device: Arc<PanthorPhysicalDevice>) -> Result<Panthor> {
        let mem_types = physical_device.get_memory_types()?;
        let mem_heaps = physical_device.get_memory_heaps()?;
        Ok(Panthor {
            physical_device,
            mem_types,
            mem_heaps,
        })
    }
}

impl PlatformDevice for Panthor {}
impl BackendDevice for Panthor {}

impl GenericDevice for Panthor {
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

    fn create_address_space(self: Arc<Panthor>) -> Result<Arc<dyn BackendAddressSpace>> {
        Ok(Arc::new(PanthorAddressSpace::new(
            self.physical_device.clone(),
        )?))
    }

    fn create_queue(
        self: Arc<Panthor>,
        address_space: &Arc<dyn BackendAddressSpace>,
        info: &MagmaCreateQueueInfo,
    ) -> Result<Arc<dyn BackendQueue>> {
        let vm_id = address_space.as_vm_id().ok_or(Error::Unimplemented)?;
        Ok(Arc::new(PanthorQueue::new(
            self.physical_device.clone(),
            vm_id,
            info,
        )?))
    }

    fn create_buffer(
        self: Arc<Panthor>,
        create_info: &MagmaCreateBufferInfo,
    ) -> Result<Arc<dyn BackendBuffer>> {
        let buf = PanthorBuffer::new(self.physical_device.clone(), create_info, &self.mem_types)?;
        Ok(Arc::new(buf))
    }

    fn import(self: Arc<Panthor>, info: MagmaImportHandleInfo) -> Result<Arc<dyn BackendBuffer>> {
        let gem_handle = self.physical_device.import(info.handle)?;
        let mem_type = self
            .mem_types
            .get(info.memory_type_idx as usize)
            .ok_or(Error::InvalidArgs)?;
        let buf = PanthorBuffer::from_existing(
            self.physical_device.clone(),
            gem_handle,
            info.size.try_into()?,
            mem_type.get_map_info(),
        )?;
        Ok(Arc::new(buf))
    }

    fn create_sync_obj(
        self: Arc<Panthor>,
        info: &MagmaCreateSyncObjInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        let sync_obj = DrmSyncObject::new_from_info(self.physical_device.clone(), info)?;
        Ok(Arc::new(sync_obj))
    }

    fn import_sync_obj(
        self: Arc<Panthor>,
        info: MagmaImportHandleInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        let sync_obj = DrmSyncObject::new(self.physical_device.clone(), MagmaSyncType::Binary)?;
        sync_obj.import(info.handle)?;
        Ok(Arc::new(sync_obj))
    }
}
