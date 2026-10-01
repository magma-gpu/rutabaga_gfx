// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::os::raw::c_void;
use std::sync::Arc;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;
use magma_gpu::util::RawMapping;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaMappedMemoryRange;
use crate::defines::MagmaMemoryType;
use crate::error::Error;
use crate::error::Result;
use crate::sys::windows::wddm::d3dkmt;
use crate::sys::windows::wddm::d3dkmt::D3dkmtHandle;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::BackendDevice;
use crate::traits::GenericAddressSpace;
use crate::traits::GenericBuffer;

pub struct WddmAddressSpace {
    _device: Arc<dyn BackendDevice>,
}

pub struct WddmBuffer {
    handle: D3dkmtHandle,
    device: Arc<dyn BackendDevice>,
    size: u64,
    map_info: Option<u32>,
}

struct WddmMapping {
    _buffer: Arc<dyn BackendBuffer>,
    pdata: *mut c_void,
    size: usize,
}

impl WddmAddressSpace {
    pub fn new(device: Arc<dyn BackendDevice>) -> WddmAddressSpace {
        WddmAddressSpace { _device: device }
    }
}

impl GenericAddressSpace for WddmAddressSpace {}
impl BackendAddressSpace for WddmAddressSpace {}

impl WddmBuffer {
    pub fn new(
        device: Arc<dyn BackendDevice>,
        create_info: &MagmaCreateBufferInfo,
        mem_types: &[MagmaMemoryType],
    ) -> Result<WddmBuffer> {
        let vendor_private_data = device.vendor_private_data().unwrap();
        let handle = d3dkmt::create_allocation(
            device.as_wddm_handle(),
            vendor_private_data,
            create_info,
            mem_types,
        )?;
        let map_info = mem_types
            .get(create_info.memory_type_idx as usize)
            .and_then(|mt| mt.get_map_info());

        Ok(WddmBuffer {
            handle,
            device,
            size: create_info.size,
            map_info,
        })
    }

    pub fn from_existing(
        device: Arc<dyn BackendDevice>,
        handle: D3dkmtHandle,
        size: u64,
        map_info: Option<u32>,
    ) -> Result<WddmBuffer> {
        Ok(WddmBuffer {
            handle,
            device,
            size,
            map_info,
        })
    }
}

impl GenericBuffer for WddmBuffer {
    fn map(self: Arc<WddmBuffer>) -> Result<Arc<dyn MappedRegion>> {
        let pdata = d3dkmt::lock_allocation(self.device.as_wddm_handle(), self.handle)?;

        Ok(Arc::new(WddmMapping {
            _buffer: self.clone(),
            pdata,
            size: self.size.try_into()?,
        }))
    }

    fn export(&self) -> Result<MagmaGpuHandle> {
        Err(Error::Unimplemented)
    }

    fn invalidate(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        d3dkmt::invalidate_cache(
            self.device.as_wddm_handle(),
            self.handle,
            self.size,
            sync_flags,
            ranges,
        )
    }

    fn flush(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        Ok(())
    }

    fn get_map_info(&self) -> Option<u32> {
        self.map_info
    }
}

impl Drop for WddmBuffer {
    fn drop(&mut self) {
        d3dkmt::destroy_allocation(self.device.as_wddm_handle(), self.handle);
    }
}

impl BackendBuffer for WddmBuffer {}

unsafe impl Send for WddmMapping {}
unsafe impl Sync for WddmMapping {}

unsafe impl MappedRegion for WddmMapping {
    fn as_ptr(&self) -> *mut u8 {
        self.pdata as *mut u8
    }

    fn size(&self) -> usize {
        self.size
    }

    fn as_raw_mapping(&self) -> RawMapping {
        RawMapping {
            ptr: self.pdata as u64,
            size: self.size as u64,
        }
    }
}
