// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;
use zerocopy::IntoBytes;

use crate::defines::MagmaMappedMemoryRange;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::BufferClose;
use crate::protocol::MagmaCommandHeader;
use crate::protocol::MagmaGpuMapFlags;
use crate::protocol::MapBufferGpu;
use crate::protocol::UnmapBufferGpu;
use crate::protocol::MAGMA_OPCODE_BUFFER_CLOSE;
use crate::protocol::MAGMA_OPCODE_MAP_BUFFER_GPU;
use crate::protocol::MAGMA_OPCODE_UNMAP_BUFFER_GPU;
use crate::traits::BackendAddressSpace;
use crate::traits::BackendBuffer;
use crate::traits::GenericAddressSpace;
use crate::traits::GenericBuffer;
use crate::virtgpu_shared::device::VirtioGpu;
use crate::virtgpu_shared::VirtioGpuTransport;

pub struct VirtioGpuAddressSpace<T: VirtioGpuTransport> {
    pub device: Arc<VirtioGpu<T>>,
    pub object_id: u32,
}

pub struct VirtioGpuBuffer<T: VirtioGpuTransport> {
    pub device: Arc<VirtioGpu<T>>,
    pub object_id: u32,
    pub bo_handle: AtomicU32,
    pub _res_handle: AtomicU32,
    pub blob_lock: Mutex<()>,
    pub size: usize,
    pub map_info: Option<u32>,
}

impl<T: VirtioGpuTransport> GenericAddressSpace for VirtioGpuAddressSpace<T> {
    fn as_vm_id(&self) -> Option<u32> {
        Some(self.object_id)
    }

    fn map_buffer_gpu(
        &self,
        buffer: &Arc<dyn BackendBuffer>,
        buffer_offset: u64,
        gpu_va: u64,
        size: u64,
        flags: MagmaGpuMapFlags,
    ) -> Result<()> {
        let buf_id = buffer.as_gem_handle().unwrap_or(1);
        let req = MapBufferGpu {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_MAP_BUFFER_GPU,
                size: std::mem::size_of::<MapBufferGpu>() as u32,
            },
            address_space: self.object_id,
            buffer: buf_id,
            buffer_offset,
            gpu_va,
            size,
            flags,
        };
        self.device.submit_encoded_cpu_cmd(req.as_bytes())
    }

    fn unmap_buffer_gpu(&self, gpu_va: u64, size: u64) -> Result<()> {
        let req = UnmapBufferGpu {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_UNMAP_BUFFER_GPU,
                size: std::mem::size_of::<UnmapBufferGpu>() as u32,
            },
            address_space: self.object_id,
            _pad0: 0,
            gpu_va,
            size,
        };
        self.device.submit_encoded_cpu_cmd(req.as_bytes())
    }
}

impl<T: VirtioGpuTransport> BackendAddressSpace for VirtioGpuAddressSpace<T> {}

impl<T: VirtioGpuTransport> VirtioGpuBuffer<T> {
    fn ensure_blob(&self) -> Result<u32> {
        let handle = self.bo_handle.load(Ordering::Acquire);
        if handle != 0 {
            return Ok(handle);
        }
        let _guard = self.blob_lock.lock().map_err(|_| Error::InternalError)?;
        let handle = self.bo_handle.load(Ordering::Acquire);
        if handle != 0 {
            return Ok(handle);
        }
        let (bo_handle, res_handle) = self
            .device
            .transport
            .create_blob(self.object_id as u64, self.size)?;
        self.bo_handle.store(bo_handle, Ordering::Release);
        self._res_handle.store(res_handle, Ordering::Release);
        Ok(bo_handle)
    }
}

impl<T: VirtioGpuTransport> GenericBuffer for VirtioGpuBuffer<T> {
    fn map(self: Arc<VirtioGpuBuffer<T>>) -> Result<Arc<dyn MappedRegion>> {
        let bo_handle = self.ensure_blob()?;
        self.device.transport.map_blob(bo_handle, self.size)
    }

    fn export(&self) -> Result<MagmaGpuHandle> {
        let bo_handle = self.ensure_blob()?;
        self.device.transport.export_blob(bo_handle)
    }

    fn invalidate(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        Ok(())
    }

    fn flush(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> Result<()> {
        Ok(())
    }

    fn as_gem_handle(&self) -> Option<u32> {
        Some(self.object_id)
    }

    fn get_map_info(&self) -> Option<u32> {
        self.map_info
    }
}

impl<T: VirtioGpuTransport> Drop for VirtioGpuBuffer<T> {
    fn drop(&mut self) {
        let bo_handle = self.bo_handle.load(Ordering::Acquire);
        if bo_handle != 0 {
            self.device.transport.close_blob(bo_handle);
        }
        let req = BufferClose {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_BUFFER_CLOSE,
                size: std::mem::size_of::<BufferClose>() as u32,
            },
            buffer: self.object_id,
            _padding: 0,
        };
        let _ = self.device.submit_encoded_cpu_cmd(req.as_bytes());
    }
}

impl<T: VirtioGpuTransport> BackendBuffer for VirtioGpuBuffer<T> {}
