// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null_mut;
use std::sync::Arc;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MAGMA_MAP_ACCESS_RW;
use magma_gpu::util::MAGMA_MAP_CACHE_CACHED;
use magma_gpu::util::MAGMA_MAP_CACHE_UNCACHED;
use magma_gpu::util::MAGMA_MAP_CACHE_WC;
use zerocopy::FromBytes;
use zerocopy::IntoBytes;

pub use crate::protocol::*;
use crate::sync::SyncObj;
use crate::traits::BackendSyncObject;

pub const MAGMA_BUS_TYPE_UNKNOWN: u32 = 0;
pub const MAGMA_BUS_TYPE_PCI: u32 = 1;
pub const MAGMA_BUS_TYPE_PLATFORM: u32 = 2;

// Same as PCI id
pub const MAGMA_VENDOR_ID_INTEL: u16 = 0x8086;
pub const MAGMA_VENDOR_ID_AMD: u16 = 0x1002;
pub const MAGMA_VENDOR_ID_MALI: u16 = 0x13B5;
pub const MAGMA_VENDOR_ID_QCOM: u16 = 0x5413;
pub const MAGMA_VENDOR_ID_VIRTGPU: u16 = 0x1AF4;

// Should be set in the case of VRAM only
pub const MAGMA_HEAP_DEVICE_LOCAL_BIT: u64 = 0x00000001;
pub const MAGMA_HEAP_CPU_VISIBLE_BIT: u64 = 0x00000010;

impl MagmaHeap {
    pub fn is_device_local(&self) -> bool {
        self.heap_flags & MAGMA_HEAP_DEVICE_LOCAL_BIT != 0
    }

    pub fn is_cpu_visible(&self) -> bool {
        self.heap_flags & MAGMA_HEAP_CPU_VISIBLE_BIT != 0
    }
}

pub const MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT: u32 = 0x00000001;
pub const MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT: u32 = 0x00000002;
pub const MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT: u32 = 0x00000004;
pub const MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT: u32 = 0x00000008;
pub const MAGMA_MEMORY_PROPERTY_LAZILY_ALLOCATED_BIT: u32 = 0x00000010;
pub const MAGMA_MEMORY_PROPERTY_PROTECTED_BIT: u32 = 0x00000020;

impl MagmaMemoryType {
    pub fn is_device_local(&self) -> bool {
        self.property_flags & MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT != 0
    }

    pub fn is_host_visible(&self) -> bool {
        self.property_flags & MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT != 0
    }

    pub fn is_coherent(&self) -> bool {
        self.property_flags & MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT != 0
    }

    pub fn is_cached(&self) -> bool {
        self.property_flags & MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT != 0
    }

    pub fn is_protected(&self) -> bool {
        self.property_flags & MAGMA_MEMORY_PROPERTY_PROTECTED_BIT != 0
    }

    pub fn get_map_info(&self) -> Option<u32> {
        if !self.is_host_visible() {
            return None;
        }

        let cache = if self.is_cached() {
            MAGMA_MAP_CACHE_CACHED
        } else if self.is_coherent() {
            MAGMA_MAP_CACHE_WC
        } else {
            MAGMA_MAP_CACHE_UNCACHED
        };

        Some(cache | MAGMA_MAP_ACCESS_RW)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagmaMemoryProperties {
    pub memory_type_count: u32,
    pub memory_heap_count: u32,
    pub memory_types: *mut MagmaMemoryType,
    pub memory_heaps: *mut MagmaHeap,
}

impl Default for MagmaMemoryProperties {
    fn default() -> MagmaMemoryProperties {
        MagmaMemoryProperties {
            memory_type_count: 0,
            memory_heap_count: 0,
            memory_types: null_mut(),
            memory_heaps: null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagmaMapping {
    pub ptr: *mut c_void,
    pub size: u64,
}

unsafe impl Send for MagmaMapping {}
unsafe impl Sync for MagmaMapping {}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MagmaHandle {
    pub os_handle: i64,
    pub handle_type: u32,
}

// Common allocation flags
//  - MAGMA_BUFFER_FLAG_EXTERNAL: The buffer *may* be exported as an OS-specific handle
//  - MAGMA_BUFFER_FLAG_SCANOUT: The buffer *may* be used by the scanout engine directly
pub const MAGMA_BUFFER_FLAG_EXTERNAL: u32 = 0x000000001;
pub const MAGMA_BUFFER_FLAG_SCANOUT: u32 = 0x000000002;

// Acceptable buffer vendor flags if the vendor is AMD:
//  - MAGMA_BUFFER_FLAG_AMD_FLAG_OA: Ordered append, used by 3D/Compute engines
//  - MAGMA_BUFFER_FLAG_AMD_FLAG_GDS: Global on-chip data storage. Used to share
//                                    data across shader threads
pub const MAGMA_BUFFER_FLAG_AMD_OA: u32 = 0x000000001;
pub const MAGMA_BUFFER_FLAG_AMD_GDS: u32 = 0x000000002;

pub const MAGMA_SYNC_WHOLE_RANGE: u64 = 1 << 0;
pub const MAGMA_SYNC_RANGES: u64 = 1 << 1;
pub const MAGMA_SYNC_INVALIDATE_READ: u64 = 1 << 2;
pub const MAGMA_SYNC_INVALIDATE_WRITE: u64 = 1 << 3;

pub type MagmaSyncType = MagmaSyncObjType;
pub const MAGMA_SYNC_OBJ_TYPE_BINARY: u32 = MagmaSyncObjType::Binary as u32;
pub const MAGMA_SYNC_OBJ_TYPE_TIMELINE: u32 = MagmaSyncObjType::Timeline as u32;

impl From<u32> for MagmaSyncType {
    fn from(val: u32) -> MagmaSyncType {
        match val {
            1 => MagmaSyncType::Timeline,
            _ => MagmaSyncType::Binary,
        }
    }
}

#[repr(C)]
#[derive(Clone, Default, Debug, IntoBytes, FromBytes)]
pub struct MagmaMappedMemoryRange {
    pub offset: u64,
    pub size: u64,
}

pub struct MagmaImportHandleInfo {
    pub handle: MagmaGpuHandle,
    pub size: u64,
    pub memory_type_idx: u32,
}

#[derive(Clone, Default)]
pub struct MagmaSubmitSyncInfo {
    pub header: MagmaStructureTypeHeader,
    pub num_wait_sync_objs: u32,
    pub num_signal_sync_objs: u32,
    pub wait_sync_objs: [Option<SyncObj>; MAGMA_MAX_SYNCOBJS],
    pub signal_sync_objs: [Option<SyncObj>; MAGMA_MAX_SYNCOBJS],
    pub wait_points: [u64; MAGMA_MAX_SYNCOBJS],
    pub signal_points: [u64; MAGMA_MAX_SYNCOBJS],
}

impl MagmaSubmitSyncInfo {
    pub fn from_sync_objs(
        wait_sync_objs: &[SyncObj],
        signal_sync_objs: &[SyncObj],
    ) -> MagmaSubmitSyncInfo {
        let mut wait: [Option<SyncObj>; MAGMA_MAX_SYNCOBJS] = Default::default();
        let mut signal: [Option<SyncObj>; MAGMA_MAX_SYNCOBJS] = Default::default();
        let num_wait = wait_sync_objs.len().min(MAGMA_MAX_SYNCOBJS);
        let num_signal = signal_sync_objs.len().min(MAGMA_MAX_SYNCOBJS);
        for (i, obj) in wait_sync_objs.iter().take(num_wait).enumerate() {
            wait[i] = Some(obj.clone());
        }
        for (i, obj) in signal_sync_objs.iter().take(num_signal).enumerate() {
            signal[i] = Some(obj.clone());
        }
        MagmaSubmitSyncInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SubmitSyncInfo as u32,
                size: size_of::<MagmaSubmitSyncInfo>() as u32,
                p_next: 0,
            },
            num_wait_sync_objs: num_wait as u32,
            num_signal_sync_objs: num_signal as u32,
            wait_sync_objs: wait,
            signal_sync_objs: signal,
            wait_points: [0; MAGMA_MAX_SYNCOBJS],
            signal_points: [0; MAGMA_MAX_SYNCOBJS],
        }
    }

    pub fn wait_sync_objs(&self) -> impl Iterator<Item = &Arc<dyn BackendSyncObject>> {
        self.wait_sync_objs
            .iter()
            .take(self.num_wait_sync_objs as usize)
            .filter_map(|opt| opt.as_ref().map(|obj| obj.sync_obj()))
    }

    pub fn wait_sync_objs_with_points(
        &self,
    ) -> impl Iterator<Item = (&Arc<dyn BackendSyncObject>, u64)> {
        self.wait_sync_objs
            .iter()
            .zip(self.wait_points.iter())
            .take(self.num_wait_sync_objs as usize)
            .filter_map(|(opt, &point)| opt.as_ref().map(|obj| (obj.sync_obj(), point)))
    }

    pub fn signal_sync_objs(&self) -> impl Iterator<Item = &Arc<dyn BackendSyncObject>> {
        self.signal_sync_objs
            .iter()
            .take(self.num_signal_sync_objs as usize)
            .filter_map(|opt| opt.as_ref().map(|obj| obj.sync_obj()))
    }

    pub fn signal_sync_objs_with_points(
        &self,
    ) -> impl Iterator<Item = (&Arc<dyn BackendSyncObject>, u64)> {
        self.signal_sync_objs
            .iter()
            .zip(self.signal_points.iter())
            .take(self.num_signal_sync_objs as usize)
            .filter_map(|(opt, &point)| opt.as_ref().map(|obj| (obj.sync_obj(), point)))
    }
}

#[derive(Clone, Default)]
pub struct MagmaSubmitInfo {
    pub header: MagmaStructureTypeHeader,
    pub flags: u32,
    pub sync_info: MagmaSubmitSyncInfo,
}

impl MagmaSubmitInfo {
    pub fn new_with_address_space(
        flags: u32,
        sync_info: MagmaSubmitSyncInfo,
        as_info: &MagmaSubmitAddressSpaceInfo,
    ) -> MagmaSubmitInfo {
        MagmaSubmitInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SubmitInfo as u32,
                size: size_of::<MagmaSubmitInfo>() as u32,
                p_next: as_info as *const _ as u64,
            },
            flags,
            sync_info,
        }
    }

    pub fn new_with_buffer(
        flags: u32,
        sync_info: MagmaSubmitSyncInfo,
        buf_info: &MagmaSubmitBufferInfo,
    ) -> MagmaSubmitInfo {
        MagmaSubmitInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SubmitInfo as u32,
                size: size_of::<MagmaSubmitInfo>() as u32,
                p_next: buf_info as *const _ as u64,
            },
            flags,
            sync_info,
        }
    }

    pub fn sync_info(&self) -> Option<&MagmaSubmitSyncInfo> {
        if self.sync_info.header.stype == MagmaStructureType::SubmitSyncInfo as u32 {
            Some(&self.sync_info)
        } else {
            None
        }
    }

    pub fn set_sync_info(&mut self, sync_info: MagmaSubmitSyncInfo) {
        self.sync_info = sync_info;
    }

    pub fn address_space_info(&self) -> Option<&MagmaSubmitAddressSpaceInfo> {
        if self.header.p_next == 0 {
            return None;
        }
        let header = unsafe { &*(self.header.p_next as *const MagmaStructureTypeHeader) };
        if header.stype == MagmaStructureType::SubmitAddressSpaceInfo as u32 {
            Some(unsafe { &*(self.header.p_next as *const MagmaSubmitAddressSpaceInfo) })
        } else {
            None
        }
    }

    pub fn buffer_info(&self) -> Option<&MagmaSubmitBufferInfo> {
        if self.header.p_next == 0 {
            return None;
        }
        let header = unsafe { &*(self.header.p_next as *const MagmaStructureTypeHeader) };
        if header.stype == MagmaStructureType::SubmitBufferInfo as u32 {
            Some(unsafe { &*(self.header.p_next as *const MagmaSubmitBufferInfo) })
        } else {
            None
        }
    }
}

#[derive(Clone, Default)]
pub struct MagmaSubmitBufferInfo {
    pub header: MagmaStructureTypeHeader,
    pub command_buffer: u32,
    pub start_offset: u64,
    pub length: u64,
}
