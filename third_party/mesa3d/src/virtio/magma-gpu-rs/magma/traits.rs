// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;
use std::sync::Mutex;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;
use magma_gpu::virtgpu_kumquat::VirtGpuKumquat;

use crate::defines::MagmaCreateBufferInfo;
use crate::defines::MagmaCreateQueueInfo;
use crate::defines::MagmaCreateSyncObjInfo;
use crate::defines::MagmaHeap;
use crate::defines::MagmaHeapBudget;
use crate::defines::MagmaImportHandleInfo;
use crate::defines::MagmaMappedMemoryRange;
use crate::defines::MagmaMemoryType;
use crate::defines::MagmaPhysicalDeviceInfo;
use crate::defines::MagmaQueueFamilyProperties;
use crate::defines::MagmaSubmitInfo;
use crate::defines::MagmaSyncType;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaGpuMapFlags;
use crate::protocol::MagmaSyncObjUseFlags;
use crate::sys::platform::PlatformDevice;
use crate::sys::platform::PlatformPhysicalDevice;

pub trait AsVirtGpu {
    #[allow(dead_code)]
    fn as_virtgpu(&self) -> Option<&Mutex<VirtGpuKumquat>> {
        None
    }
}

pub trait GenericPhysicalDevice {
    fn create_device(
        self: Arc<Self>,
        device_info: &MagmaPhysicalDeviceInfo,
    ) -> Result<Arc<dyn BackendDevice>>;

    fn get_device_info(&self, _info: &mut MagmaPhysicalDeviceInfo) -> Result<()> {
        Ok(())
    }

    fn get_memory_types(&self) -> Result<Vec<MagmaMemoryType>>;

    fn get_memory_heaps(&self) -> Result<Vec<MagmaHeap>>;

    fn get_queue_family_properties(&self) -> Result<Vec<MagmaQueueFamilyProperties>>;
}

pub trait GenericDevice {
    fn get_memory_budget(&self, heap_idx: u32) -> Result<MagmaHeapBudget>;

    fn create_address_space(self: Arc<Self>) -> Result<Arc<dyn BackendAddressSpace>>;

    fn create_queue(
        self: Arc<Self>,
        address_space: &Arc<dyn BackendAddressSpace>,
        info: &MagmaCreateQueueInfo,
    ) -> Result<Arc<dyn BackendQueue>>;

    fn create_buffer(
        self: Arc<Self>,
        create_info: &MagmaCreateBufferInfo,
    ) -> Result<Arc<dyn BackendBuffer>>;

    fn import(self: Arc<Self>, _info: MagmaImportHandleInfo) -> Result<Arc<dyn BackendBuffer>>;

    fn create_sync_obj(
        self: Arc<Self>,
        _info: &MagmaCreateSyncObjInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        Err(Error::Unimplemented)
    }

    fn import_sync_obj(
        self: Arc<Self>,
        _info: MagmaImportHandleInfo,
    ) -> Result<Arc<dyn BackendSyncObject>> {
        Err(Error::Unimplemented)
    }
}

pub trait GenericBuffer {
    fn map(self: Arc<Self>) -> Result<Arc<dyn MappedRegion>>;

    fn export(&self) -> Result<MagmaGpuHandle>;

    fn invalidate(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> Result<()>;

    fn flush(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> Result<()>;

    fn get_map_info(&self) -> Option<u32>;

    fn as_gem_handle(&self) -> Option<u32> {
        None
    }
}

pub trait GenericSyncObject {
    fn get_type(&self) -> MagmaSyncType {
        MagmaSyncType::Binary
    }

    fn is_timeline(&self) -> bool {
        self.get_type() == MagmaSyncType::Timeline
    }

    fn wait(&self, _timeout_ns: u64) -> Result<()> {
        Err(Error::Unimplemented)
    }

    fn signal(&self) -> Result<()> {
        Err(Error::Unimplemented)
    }

    fn timeline_wait(&self, _point: u64, _timeout_ns: u64, _flags: u32) -> Result<()> {
        Err(Error::Unimplemented)
    }

    fn timeline_signal(&self, _point: u64) -> Result<()> {
        Err(Error::Unimplemented)
    }

    fn timeline_query(&self) -> Result<u64> {
        Err(Error::Unimplemented)
    }

    fn export_fence(&self) -> Result<MagmaGpuHandle> {
        Err(Error::Unimplemented)
    }

    fn import(&self, _handle: MagmaGpuHandle) -> Result<()> {
        Err(Error::Unimplemented)
    }

    fn as_raw_handle(&self) -> Option<u32> {
        None
    }

    fn use_flags(&self) -> MagmaSyncObjUseFlags {
        MagmaSyncObjUseFlags::empty()
    }
}

pub trait GenericAddressSpace {
    fn as_vm_id(&self) -> Option<u32> {
        None
    }

    fn map_buffer_gpu(
        &self,
        _buffer: &Arc<dyn BackendBuffer>,
        _buffer_offset: u64,
        _gpu_va: u64,
        _size: u64,
        _flags: MagmaGpuMapFlags,
    ) -> Result<()> {
        Err(Error::Unimplemented)
    }

    fn unmap_buffer_gpu(&self, _gpu_va: u64, _size: u64) -> Result<()> {
        Err(Error::Unimplemented)
    }
}

pub trait GenericQueue {
    fn submit_command(&self, _submit_info: &MagmaSubmitInfo) -> Result<()> {
        Err(Error::Unimplemented)
    }

    fn check_status(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtioSyncOp {
    Wait {
        ring_idx: u32,
        seqno: u32,
    },
    Signal {
        ring_idx: u32,
        seqno: u32,
        point: u64,
    },
}

pub trait VirtioSyncObject {
    fn host_object_id(&self) -> u32;
    fn guest_syncobj_handle(&self) -> u32;
    fn has_imported_fence(&self) -> bool;
    fn last_ring_idx(&self) -> u32 {
        0
    }
    fn record_submission(&self, _op: VirtioSyncOp) {}
}

pub trait AsVirtioSyncObject {
    fn as_virtio_syncobj(&self) -> Option<&dyn VirtioSyncObject> {
        None
    }
}

pub trait BackendPhysicalDevice:
    PlatformPhysicalDevice + AsVirtGpu + GenericPhysicalDevice + Send + Sync
{
}
pub trait BackendDevice: GenericDevice + PlatformDevice + Send + Sync {}
pub trait BackendAddressSpace: GenericAddressSpace + Send + Sync {}
pub trait BackendQueue: GenericQueue + Send + Sync {}
pub trait BackendBuffer: GenericBuffer + Send + Sync {}
pub trait BackendSyncObject: GenericSyncObject + AsVirtioSyncObject + Send + Sync {}
