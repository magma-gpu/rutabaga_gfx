// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::Mutex;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;

use crate::defines::MagmaMappedMemoryRange;
use crate::defines::MagmaMapping;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaGpuMapFlags;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;

#[derive(Clone)]
pub struct AddressSpace {
    address_space: Arc<dyn BackendAddressSpace>,
}

#[derive(Clone)]
pub struct Buffer {
    buffer: Arc<dyn BackendBuffer>,
    mapping: Arc<Mutex<Option<Arc<dyn MappedRegion>>>>,
}

impl AddressSpace {
    pub fn new(address_space: Arc<dyn BackendAddressSpace>) -> AddressSpace {
        AddressSpace { address_space }
    }

    pub fn inner(&self) -> &Arc<dyn BackendAddressSpace> {
        &self.address_space
    }

    pub fn map_buffer_gpu(
        &self,
        buffer: &Buffer,
        buffer_offset: u64,
        gpu_va: u64,
        size: u64,
        flags: MagmaGpuMapFlags,
    ) -> Result<()> {
        self.address_space
            .map_buffer_gpu(buffer.inner(), buffer_offset, gpu_va, size, flags)
    }

    pub fn unmap_buffer_gpu(&self, gpu_va: u64, size: u64) -> Result<()> {
        self.address_space.unmap_buffer_gpu(gpu_va, size)
    }
}

impl Buffer {
    pub fn new(buffer: Arc<dyn BackendBuffer>) -> Buffer {
        Buffer {
            buffer,
            mapping: Arc::new(Mutex::new(None)),
        }
    }

    pub fn inner(&self) -> &Arc<dyn BackendBuffer> {
        &self.buffer
    }

    pub fn get_map_info(&self) -> Option<u32> {
        self.buffer.get_map_info()
    }

    pub fn map_cpu(&self) -> Result<MagmaMapping> {
        let mut mapping = self.mapping.lock().map_err(|_| Error::InternalError)?;
        if mapping.is_none() {
            let region = self.buffer.clone().map()?;
            *mapping = Some(region);
        }
        let region = mapping.as_ref().ok_or(Error::InternalError)?;
        Ok(MagmaMapping {
            ptr: region.as_ptr() as *mut c_void,
            size: region.size() as u64,
        })
    }

    pub fn unmap_cpu(&self) -> Result<()> {
        let mut mapping = self.mapping.lock().map_err(|_| Error::InternalError)?;
        *mapping = None;
        Ok(())
    }

    pub fn map(&self) -> Result<Arc<dyn MappedRegion>> {
        self.buffer.clone().map()
    }

    pub fn export(&self) -> Result<MagmaGpuHandle> {
        self.buffer.export()
    }

    pub fn invalidate(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        self.buffer.invalidate(sync_flags, ranges)
    }

    pub fn flush(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        self.buffer.flush(sync_flags, ranges)
    }
}
