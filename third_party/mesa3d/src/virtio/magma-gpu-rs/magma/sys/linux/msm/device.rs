// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

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
use crate::sys::linux::drm::DrmSyncObject;
use crate::sys::linux::msm::memory::MsmAddressSpace;
use crate::sys::linux::msm::memory::MsmBuffer;
use crate::sys::linux::msm::queue::MsmQueue;
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
pub struct MsmPhysicalDevice {
    descriptor: OwnedDescriptor,
}

pub struct Msm {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    mem_types: Vec<MagmaMemoryType>,
}

impl MsmPhysicalDevice {
    pub fn new(descriptor: OwnedDescriptor) -> MsmPhysicalDevice {
        MsmPhysicalDevice { descriptor }
    }
}

impl PlatformPhysicalDevice for MsmPhysicalDevice {
    fn as_descriptor(&self) -> Option<&OwnedDescriptor> {
        Some(&self.descriptor)
    }
}

impl AsVirtGpu for MsmPhysicalDevice {}
impl BackendPhysicalDevice for MsmPhysicalDevice {}

impl GenericPhysicalDevice for MsmPhysicalDevice {
    fn create_device(
        self: Arc<MsmPhysicalDevice>,
        _info: &MagmaPhysicalDeviceInfo,
    ) -> Result<Arc<dyn BackendDevice>> {
        Ok(Arc::new(Msm::new(self)))
    }

    fn get_memory_types(&self) -> Result<Vec<MagmaMemoryType>> {
        Ok(Vec::new())
    }

    fn get_memory_heaps(&self) -> Result<Vec<MagmaHeap>> {
        Ok(Vec::new())
    }

    fn get_queue_family_properties(&self) -> Result<Vec<MagmaQueueFamilyProperties>> {
        Ok(vec![MagmaQueueFamilyProperties::new(
            MagmaQueueFlags::Graphics | MagmaQueueFlags::Compute,
            1,
        )])
    }
}

impl Msm {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> Msm {
        Msm {
            physical_device,
            mem_types: Vec::new(),
        }
    }
}

impl GenericDevice for Msm {
    fn get_memory_budget(&self, _heap_idx: u32) -> Result<MagmaHeapBudget> {
        Err(Error::Unimplemented)
    }

    fn create_address_space(self: Arc<Msm>) -> Result<Arc<dyn BackendAddressSpace>> {
        Ok(Arc::new(MsmAddressSpace::new(self.physical_device.clone())))
    }

    fn create_queue(
        self: Arc<Msm>,
        _address_space: &Arc<dyn BackendAddressSpace>,
        info: &MagmaCreateQueueInfo,
    ) -> Result<Arc<dyn BackendQueue>> {
        let queue = MsmQueue::new(self.physical_device.clone(), info.priority)?;
        Ok(Arc::new(queue))
    }

    fn create_buffer(
        self: Arc<Msm>,
        create_info: &MagmaCreateBufferInfo,
    ) -> Result<Arc<dyn BackendBuffer>> {
        let buf = MsmBuffer::new(self.physical_device.clone(), create_info, &self.mem_types)?;
        Ok(Arc::new(buf))
    }

    fn import(self: Arc<Msm>, info: MagmaImportHandleInfo) -> Result<Arc<dyn BackendBuffer>> {
        let gem_handle = self.physical_device.import(info.handle)?;
        let map_info = self
            .mem_types
            .get(info.memory_type_idx as usize)
            .and_then(|mt| mt.get_map_info());
        let buf = MsmBuffer::from_existing(
            self.physical_device.clone(),
            gem_handle,
            info.size.try_into()?,
            map_info,
        )?;
        Ok(Arc::new(buf))
    }

    fn create_sync_obj(
        self: Arc<Msm>,
        info: &MagmaCreateSyncObjInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        let sync_obj = DrmSyncObject::new_from_info(self.physical_device.clone(), info)?;
        Ok(Arc::new(sync_obj))
    }

    fn import_sync_obj(
        self: Arc<Msm>,
        info: MagmaImportHandleInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        let sync_obj = DrmSyncObject::new(self.physical_device.clone(), MagmaSyncType::Binary)?;
        sync_obj.import(info.handle)?;
        Ok(Arc::new(sync_obj))
    }
}

impl PlatformDevice for Msm {}
impl BackendDevice for Msm {}
