// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use windows_sys::Wdk::Graphics::Direct3D::D3DKMT_SEGMENTGROUPSIZEINFO;
use windows_sys::Win32::Foundation::LUID;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaCreateQueueInfo;
use crate::defines::MagmaHeap;
use crate::defines::MagmaHeapBudget;
use crate::defines::MagmaImportHandleInfo;
use crate::defines::MagmaMemoryType;
use crate::defines::MagmaPhysicalDeviceInfo;
use crate::defines::MagmaQueueFamilyProperties;
use crate::defines::MagmaQueueFlags;
use crate::defines::MAGMA_HEAP_DEVICE_LOCAL_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaVendorId;
use crate::sys::windows::wddm::d3dkmt;
use crate::sys::windows::wddm::d3dkmt::D3dkmtHandle;
use crate::sys::windows::wddm::memory::WddmAddressSpace;
use crate::sys::windows::wddm::memory::WddmBuffer;
use crate::sys::windows::wddm::queue::WddmQueue;
use crate::sys::windows::wddm::WindowsDevice;
use crate::sys::windows::wddm::WindowsPhysicalDevice;
use crate::sys::windows::Amd;
use crate::sys::windows::VendorPrivateData;
use crate::traits::AsVirtGpu;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendDevice;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendQueue;
use crate::traits::GenericDevice;
use crate::traits::GenericPhysicalDevice;

pub struct WddmAdapter {
    handle: D3dkmtHandle,
    _luid: LUID,
    segment_group_size: D3DKMT_SEGMENTGROUPSIZEINFO,
    _hw_sch_enabled: bool,
    _hw_sch_supported: bool,
    adapter_name: String,
    chip_type: String,
}

pub struct WddmDevice {
    handle: D3dkmtHandle,
    adapter: Arc<dyn BackendPhysicalDevice>,
    vendor_private_data: Box<dyn VendorPrivateData>,
    mem_types: Vec<MagmaMemoryType>,
    mem_heaps: Vec<MagmaHeap>,
}

impl WddmAdapter {
    pub fn new(handle: D3dkmtHandle, luid: LUID) -> WddmAdapter {
        WddmAdapter {
            handle,
            _luid: luid,
            segment_group_size: Default::default(),
            _hw_sch_enabled: Default::default(),
            _hw_sch_supported: Default::default(),
            adapter_name: Default::default(),
            chip_type: Default::default(),
        }
    }

    pub fn initialize(&mut self) -> Result<MagmaPhysicalDeviceInfo> {
        let query = d3dkmt::query_adapter(self.handle)?;
        self.segment_group_size = query.segment_group_size;
        self.adapter_name = query.adapter_name;
        self.chip_type = query.chip_type;
        Ok(query.info)
    }
}

impl GenericPhysicalDevice for WddmAdapter {
    fn create_device(
        self: Arc<WddmAdapter>,
        info: &MagmaPhysicalDeviceInfo,
    ) -> Result<Arc<dyn BackendDevice>> {
        let vendor_private_data = match info.vendor_id {
            MagmaVendorId::Amd => Box::new(Amd(())),
            _ => todo!(),
        };

        let device = WddmDevice::new(self, vendor_private_data)?;
        Ok(Arc::new(device))
    }

    fn get_memory_types(&self) -> Result<Vec<MagmaMemoryType>> {
        let mut types = Vec::new();
        let mut heap_idx = 0u32;
        if self.segment_group_size.NonLocalMemory > 0 {
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
        if self.segment_group_size.LocalMemory > 0 {
            types.push(MagmaMemoryType {
                property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
                heap_idx,
            });
        }
        Ok(types)
    }

    fn get_memory_heaps(&self) -> Result<Vec<MagmaHeap>> {
        let mut heaps = Vec::new();
        if self.segment_group_size.NonLocalMemory > 0 {
            heaps.push(MagmaHeap {
                heap_size: self.segment_group_size.NonLocalMemory,
                heap_flags: 0,
            });
        }
        if self.segment_group_size.LocalMemory > 0 {
            heaps.push(MagmaHeap {
                heap_size: self.segment_group_size.LocalMemory,
                heap_flags: MAGMA_HEAP_DEVICE_LOCAL_BIT,
            });
        }
        Ok(heaps)
    }

    fn get_queue_family_properties(&self) -> Result<Vec<MagmaQueueFamilyProperties>> {
        Ok(vec![MagmaQueueFamilyProperties::new(
            MagmaQueueFlags::Graphics | MagmaQueueFlags::Compute,
            1,
        )])
    }
}

impl WindowsPhysicalDevice for WddmAdapter {
    fn as_wddm_handle(&self) -> D3dkmtHandle {
        self.handle
    }

    fn segment_group_size(&self) -> D3DKMT_SEGMENTGROUPSIZEINFO {
        self.segment_group_size
    }
}

impl AsVirtGpu for WddmAdapter {}
impl BackendPhysicalDevice for WddmAdapter {}

impl Drop for WddmAdapter {
    fn drop(&mut self) {
        d3dkmt::close_adapter(self.handle);
    }
}

impl WddmDevice {
    pub fn new(
        adapter: Arc<dyn BackendPhysicalDevice>,
        vendor_private_data: Box<dyn VendorPrivateData>,
    ) -> Result<WddmDevice> {
        let handle = d3dkmt::create_device(adapter.as_wddm_handle())?;
        let mem_types = adapter.get_memory_types()?;
        let mem_heaps = adapter.get_memory_heaps()?;

        Ok(WddmDevice {
            handle,
            adapter,
            vendor_private_data,
            mem_types,
            mem_heaps,
        })
    }
}

impl GenericDevice for WddmDevice {
    fn get_memory_budget(&self, heap_idx: u32) -> Result<MagmaHeapBudget> {
        let heap = self
            .mem_heaps
            .get(heap_idx as usize)
            .ok_or(Error::InvalidArgs)?;
        d3dkmt::query_video_memory_info(self.adapter.as_wddm_handle(), heap.is_device_local())
    }

    fn create_address_space(self: Arc<WddmDevice>) -> Result<Arc<dyn BackendAddressSpace>> {
        Ok(Arc::new(WddmAddressSpace::new(self)))
    }

    fn create_queue(
        self: Arc<WddmDevice>,
        _address_space: &Arc<dyn BackendAddressSpace>,
        _info: &MagmaCreateQueueInfo,
    ) -> Result<Arc<dyn BackendQueue>> {
        let queue = WddmQueue::new(self)?;
        Ok(Arc::new(queue))
    }

    fn create_buffer(
        self: Arc<WddmDevice>,
        create_info: &MagmaCreateBufferInfo,
    ) -> Result<Arc<dyn BackendBuffer>> {
        let buf = WddmBuffer::new(self.clone(), create_info, &self.mem_types)?;
        Ok(Arc::new(buf))
    }

    fn import(
        self: Arc<WddmDevice>,
        info: MagmaImportHandleInfo,
    ) -> Result<Arc<dyn BackendBuffer>> {
        let alloc_handle = d3dkmt::open_resource_from_nt_handle(self.handle, info.handle)?;
        let map_info = self
            .mem_types
            .get(info.memory_type_idx as usize)
            .and_then(|mt| mt.get_map_info());
        let buf = WddmBuffer::from_existing(self, alloc_handle, info.size, map_info)?;
        Ok(Arc::new(buf))
    }
}

impl Drop for WddmDevice {
    fn drop(&mut self) {
        d3dkmt::destroy_device(self.handle);
    }
}

impl WindowsDevice for WddmDevice {
    fn as_wddm_handle(&self) -> D3dkmtHandle {
        self.handle
    }

    fn vendor_private_data(&self) -> Option<&dyn VendorPrivateData> {
        Some(&*self.vendor_private_data)
    }
}

impl BackendDevice for WddmDevice {}

pub fn enumerate_adapters() -> Result<Vec<(WddmAdapter, MagmaPhysicalDeviceInfo)>> {
    let adapter_slice = d3dkmt::enum_adapters()?;
    let mut adapters = Vec::with_capacity(adapter_slice.len());

    for adapter in adapter_slice {
        let mut adapter = WddmAdapter::new(adapter.hAdapter, adapter.AdapterLuid);
        let info = adapter.initialize()?;
        adapters.push((adapter, info));
    }

    Ok(adapters)
}
