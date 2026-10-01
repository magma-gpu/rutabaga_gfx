// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT
//
// Generated via:
//   https://gitlab.freedesktop.org/mesa/mesa/-/tree/main/src/magma/gorgonzola
//
// Submit patches, do not hand-edit.

#![allow(unused_imports)]
#![allow(dead_code)]

use bitflags::bitflags;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, TryFromBytes};

pub const MAGMA_OPCODE_CREATE_DEVICE: u32 = 2;
pub const MAGMA_OPCODE_GET_MEMORY_BUDGET: u32 = 3;
pub const MAGMA_OPCODE_CREATE_BUFFER: u32 = 4;
pub const MAGMA_OPCODE_CREATE_ADDRESS_SPACE: u32 = 5;
pub const MAGMA_OPCODE_CREATE_QUEUE: u32 = 6;
pub const MAGMA_OPCODE_DEVICE_CLOSE: u32 = 7;
pub const MAGMA_OPCODE_PHYSICAL_DEVICE_CLOSE: u32 = 8;
pub const MAGMA_OPCODE_BUFFER_CLOSE: u32 = 9;
pub const MAGMA_OPCODE_QUEUE_CLOSE: u32 = 10;
pub const MAGMA_OPCODE_ADDRESS_SPACE_CLOSE: u32 = 11;
pub const MAGMA_OPCODE_VIRTIO_CREATE_RING: u32 = 12;
pub const MAGMA_OPCODE_MAP_BUFFER_GPU: u32 = 13;
pub const MAGMA_OPCODE_UNMAP_BUFFER_GPU: u32 = 14;
pub const MAGMA_OPCODE_SUBMIT_COMMAND: u32 = 15;
pub const MAGMA_OPCODE_CREATE_SYNC_OBJ: u32 = 16;
pub const MAGMA_OPCODE_SYNC_OBJ_CLOSE: u32 = 17;
pub const MAGMA_OPCODE_SYNC_OBJ_SIGNAL: u32 = 19;
pub const MAGMA_OPCODE_SYNC_OBJ_TIMELINE_WAIT: u32 = 20;
pub const MAGMA_OPCODE_SYNC_OBJ_TIMELINE_SIGNAL: u32 = 21;
pub const MAGMA_OPCODE_SYNC_OBJ_TIMELINE_QUERY: u32 = 22;
pub const MAGMA_OPCODE_QUEUE_CHECK_STATUS: u32 = 26;
pub const MAGMA_OPCODE_VIRTIO_PING: u32 = 33;
pub const MAGMA_OPCODE_VIRTIO_CREATE_FENCE: u32 = 34;
pub const MAGMA_OPCODE_VIRTIO_SYNC_OBJ_CLOSE: u32 = 35;

pub const MAGMA_OPCODE_RESP_CREATE_DEVICE: u32 = 0x8000_0002;
pub const MAGMA_OPCODE_RESP_GET_MEMORY_BUDGET: u32 = 0x8000_0003;
pub const MAGMA_OPCODE_RESP_CREATE_BUFFER: u32 = 0x8000_0004;
pub const MAGMA_OPCODE_RESP_CREATE_ADDRESS_SPACE: u32 = 0x8000_0005;
pub const MAGMA_OPCODE_RESP_CREATE_QUEUE: u32 = 0x8000_0006;
pub const MAGMA_OPCODE_RESP_MAP_BUFFER_GPU: u32 = 0x8000_000d;
pub const MAGMA_OPCODE_RESP_UNMAP_BUFFER_GPU: u32 = 0x8000_000e;
pub const MAGMA_OPCODE_RESP_SUBMIT_COMMAND: u32 = 0x8000_000f;
pub const MAGMA_OPCODE_RESP_CREATE_SYNC_OBJ: u32 = 0x8000_0010;
pub const MAGMA_OPCODE_RESP_SYNC_OBJ_SIGNAL: u32 = 0x8000_0013;
pub const MAGMA_OPCODE_RESP_SYNC_OBJ_TIMELINE_WAIT: u32 = 0x8000_0014;
pub const MAGMA_OPCODE_RESP_SYNC_OBJ_TIMELINE_SIGNAL: u32 = 0x8000_0015;
pub const MAGMA_OPCODE_RESP_SYNC_OBJ_TIMELINE_QUERY: u32 = 0x8000_0016;
pub const MAGMA_OPCODE_RESP_QUEUE_CHECK_STATUS: u32 = 0x8000_001a;

pub const MAGMA_MAX_SYNCOBJS: usize = 16;
pub const MAGMA_MAX_PHYSICAL_DEVICES: usize = 8;
pub const MAGMA_MAX_MEMORY_HEAPS: usize = 32;
pub const MAGMA_MAX_MEMORY_TYPES: usize = 16;
pub const MAGMA_MAX_QUEUES: usize = 16;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, TryFromBytes, IntoBytes, Immutable)]
#[repr(u16)]
pub enum MagmaVendorId {
    #[default]
    Undefined = 0,
    Amd = 4098,
    Arm = 5045,
    VirtGpu = 6900,
    Qualcomm = 21523,
    Intel = 32902,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, TryFromBytes, IntoBytes, Immutable)]
#[repr(i32)]
pub enum MagmaStatus {
    #[default]
    Success = 0,
    InternalError = -1,
    InvalidArgs = -2,
    AccessDenied = -3,
    MemoryError = -4,
    ContextKilled = -5,
    TimedOut = -6,
    Unimplemented = -7,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, TryFromBytes, IntoBytes, Immutable)]
#[repr(i32)]
pub enum VirtioGpuBlobPreference {
    #[default]
    Undefined = 0,
    Guest = 1,
    Host = 2,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, TryFromBytes, IntoBytes, Immutable)]
#[repr(u32)]
pub enum MagmaSyncObjType {
    #[default]
    Binary = 0,
    Timeline = 1,
}

#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct MagmaMemoryProperty(u32);
bitflags! {
    impl MagmaMemoryProperty: u32 {
        const DeviceLocalBit = 1;
        const HostVisibleBit = 2;
        const HostCoherentBit = 4;
        const HostCachedBit = 8;
        const LazilyAllocatedBit = 16;
        const ProtectedBit = 32;
    }
}

#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct MagmaQueueFlags(u32);
bitflags! {
    impl MagmaQueueFlags: u32 {
        const Graphics = 1;
        const Compute = 2;
        const Transfer = 4;
        const SparseBinding = 8;
        const Protected = 16;
    }
}

#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct MagmaGpuMapFlags(u64);
bitflags! {
    impl MagmaGpuMapFlags: u64 {
        const Read = 1;
        const Write = 2;
        const Execute = 4;
        const GrowUp = 8;
        const GrowDown = 16;
    }
}

#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct MagmaBufferFlag(u32);
bitflags! {
    impl MagmaBufferFlag: u32 {
        const External = 1;
        const Scanout = 2;
    }
}

#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct MagmaSyncCapability(u32);
bitflags! {
    impl MagmaSyncCapability: u32 {
        const Binary = 1;
        const Timeline = 2;
        const CpuWait = 4;
        const CpuSignal = 8;
        const ExportSyncFile = 16;
        const ImportSyncFile = 32;
    }
}

#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct MagmaSyncObjUseFlags(u32);
bitflags! {
    impl MagmaSyncObjUseFlags: u32 {
        const Exportable = 1;
        const Fence = 2;
    }
}

#[derive(
    Copy, Clone, Default, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct MagmaHandleType(u32);
bitflags! {
    impl MagmaHandleType: u32 {
        const MemOpaqueFd = 1;
        const MemDmabuf = 2;
        const MemOpaqueWin32 = 3;
        const MemShm = 4;
        const MemZircon = 5;
        const SignalOpaqueFd = 16;
        const SignalSyncFd = 32;
        const SignalOpaqueWin32 = 48;
        const SignalZircon = 64;
        const SignalEventFd = 80;
    }
}

#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaHeap {
    pub heap_size: u64,
    pub heap_flags: u64,
}

#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaHeapBudget {
    pub budget: u64,
    pub usage: u64,
}

#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaMemoryType {
    pub property_flags: u32,
    pub heap_idx: u32,
}

#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaPciBusInfo {
    pub domain: u16,
    pub subvendor_id: u16,
    pub subdevice_id: u16,
    pub revision_id: u8,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub _padding: [u8; 6],
}

#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaCreateQueueInfo {
    pub queue_family_idx: u32,
    pub priority: u32,
    pub flags: u32,
    pub _padding: u32,
}

#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaQueueFamilyProperties {
    pub queue_flags: MagmaQueueFlags,
    pub queue_count: u32,
}

pub const MAGMA_STRUCTURE_TYPE_CREATE_BUFFER_INFO: u32 = 1;
pub const MAGMA_STRUCTURE_TYPE_DEVICE_CREATE_INFO: u32 = 2;
pub const MAGMA_STRUCTURE_TYPE_SUBMIT_SYNC_INFO: u32 = 3;
pub const MAGMA_STRUCTURE_TYPE_SUBMIT_INFO: u32 = 4;
pub const MAGMA_STRUCTURE_TYPE_SUBMIT_ADDRESS_SPACE_INFO: u32 = 5;
pub const MAGMA_STRUCTURE_TYPE_SUBMIT_BUFFER_INFO: u32 = 6;
pub const MAGMA_STRUCTURE_TYPE_CREATE_SYNC_OBJ_INFO: u32 = 7;
pub const MAGMA_STRUCTURE_TYPE_SYNC_PROPERTIES: u32 = 8;
pub const MAGMA_STRUCTURE_TYPE_VIRT_CAPABILITIES: u32 = 65537;
pub const MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_INFO: u32 = 65538;
pub const MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_TYPE: u32 = 65539;
pub const MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_HEAP: u32 = 65540;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromBytes, IntoBytes, Immutable)]
#[repr(u32)]
pub enum MagmaStructureType {
    CreateBufferInfo = 1,
    DeviceCreateInfo = 2,
    SubmitSyncInfo = 3,
    SubmitInfo = 4,
    SubmitAddressSpaceInfo = 5,
    SubmitBufferInfo = 6,
    CreateSyncObjInfo = 7,
    SyncProperties = 8,
    VirtCapabilities = 65537,
    PhysicalDeviceInfo = 65538,
    PhysicalDeviceMemoryType = 65539,
    PhysicalDeviceMemoryHeap = 65540,
}

#[derive(Debug, Default, Clone, Copy, FromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaStructureTypeHeader {
    pub stype: u32,
    pub size: u32,
    pub p_next: u64,
}

/// Marker trait indicating that structure `Self` is allowed to extend structure `Root`.
///
/// # Safety
/// Implementors must only be declared when the specification allows `Self` to extend `Root`.
pub unsafe trait MagmaExtends<Root> {}

/// Trait implemented by extensible structures indicating their structure type.
///
/// # Safety
/// Implementors must guarantee that `STRUCTURE_TYPE` matches the actual header's stype
/// and that the structure begins with a valid `MagmaStructureTypeHeader`.
pub unsafe trait MagmaTaggedStructure {
    const STRUCTURE_TYPE: MagmaStructureType;
    fn header(&self) -> &MagmaStructureTypeHeader;
    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader;

    /// Find an extension in the `p_next` chain matching type `T`.
    fn get_extension<T: MagmaTaggedStructure + MagmaExtends<Self>>(&self) -> Option<&T>
    where
        Self: Sized,
    {
        let mut curr = self.header().p_next;
        while curr != 0 {
            let header = unsafe { &*(curr as *const MagmaStructureTypeHeader) };
            if header.stype == T::STRUCTURE_TYPE as u32 {
                return Some(unsafe { &*(curr as *const T) });
            }
            curr = header.p_next;
        }
        None
    }

    /// Find a mutable extension in the `p_next` chain matching type `T`.
    ///
    /// # Safety
    /// Caller must ensure exclusive access to the structures in the chain.
    unsafe fn get_extension_mut<T: MagmaTaggedStructure + MagmaExtends<Self>>(
        &mut self,
    ) -> Option<&mut T>
    where
        Self: Sized,
    {
        let mut curr = self.header().p_next;
        while curr != 0 {
            let header = &mut *(curr as *mut MagmaStructureTypeHeader);
            if header.stype == T::STRUCTURE_TYPE as u32 {
                return Some(&mut *(curr as *mut T));
            }
            curr = header.p_next;
        }
        None
    }

    /// Append an extension structure to the end of the `p_next` chain.
    ///
    /// # Safety
    /// `next` must point to a valid extensible structure whose memory remains valid
    /// while the chain is referenced.
    unsafe fn push_next<T: MagmaTaggedStructure + MagmaExtends<Self>>(&mut self, next: &mut T)
    where
        Self: Sized,
    {
        let mut curr: *mut MagmaStructureTypeHeader = self.header_mut();
        while (*curr).p_next != 0 {
            curr = (*curr).p_next as *mut MagmaStructureTypeHeader;
        }
        (*curr).p_next = (next.header_mut() as *mut MagmaStructureTypeHeader) as usize as u64;
    }
}

#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaCreateBufferInfo {
    pub header: MagmaStructureTypeHeader,
    pub memory_type_idx: u32,
    pub alignment: u32,
    pub common_flags: u32,
    pub vendor_flags: u32,
    pub size: u64,
}

unsafe impl MagmaTaggedStructure for MagmaCreateBufferInfo {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::CreateBufferInfo;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaDeviceCreateInfo {
    pub header: MagmaStructureTypeHeader,
    pub flags: u32,
    pub _padding: u32,
}

unsafe impl MagmaTaggedStructure for MagmaDeviceCreateInfo {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::DeviceCreateInfo;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaWireSubmitSyncInfo {
    pub header: MagmaStructureTypeHeader,
    pub num_wait_sync_objs: u32,
    pub num_signal_sync_objs: u32,
    pub wait_sync_objs: [u32; MAGMA_MAX_SYNCOBJS],
    pub signal_sync_objs: [u32; MAGMA_MAX_SYNCOBJS],
    pub wait_points: [u64; MAGMA_MAX_SYNCOBJS],
    pub signal_points: [u64; MAGMA_MAX_SYNCOBJS],
}

unsafe impl MagmaTaggedStructure for MagmaWireSubmitSyncInfo {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::SubmitSyncInfo;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaWireSubmitInfo {
    pub header: MagmaStructureTypeHeader,
    pub flags: u32,
    pub _pad0: u32,
    pub sync_info: MagmaWireSubmitSyncInfo,
}

unsafe impl MagmaTaggedStructure for MagmaWireSubmitInfo {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::SubmitInfo;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaSubmitAddressSpaceInfo {
    pub header: MagmaStructureTypeHeader,
    pub command_va: u64,
    pub length: u64,
}

unsafe impl MagmaTaggedStructure for MagmaSubmitAddressSpaceInfo {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::SubmitAddressSpaceInfo;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
unsafe impl MagmaExtends<MagmaWireSubmitInfo> for MagmaSubmitAddressSpaceInfo {}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaWireSubmitBufferInfo {
    pub header: MagmaStructureTypeHeader,
    pub command_buffer: u32,
    pub _pad0: u32,
    pub start_offset: u64,
    pub length: u64,
}

unsafe impl MagmaTaggedStructure for MagmaWireSubmitBufferInfo {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::SubmitBufferInfo;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
unsafe impl MagmaExtends<MagmaWireSubmitInfo> for MagmaWireSubmitBufferInfo {}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaCreateSyncObjInfo {
    pub header: MagmaStructureTypeHeader,
    pub sync_type: u32,
    pub use_flags: MagmaSyncObjUseFlags,
    pub initial_point: u64,
}

unsafe impl MagmaTaggedStructure for MagmaCreateSyncObjInfo {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::CreateSyncObjInfo;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaSyncProperties {
    pub header: MagmaStructureTypeHeader,
    pub capabilities: u32,
    pub _pad0: u32,
    pub max_timeline_value_difference: u64,
}

unsafe impl MagmaTaggedStructure for MagmaSyncProperties {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::SyncProperties;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaVirtCapabilities {
    pub header: MagmaStructureTypeHeader,
    pub capset_version: u32,
    pub num_physical_devices: u32,
    pub blob_preference: VirtioGpuBlobPreference,
    pub _pad0: u32,
    pub ring_buffer_len: u64,
}

unsafe impl MagmaTaggedStructure for MagmaVirtCapabilities {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::VirtCapabilities;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaPhysicalDeviceInfo {
    pub header: MagmaStructureTypeHeader,
    pub bus_type: u32,
    pub vendor_id: MagmaVendorId,
    pub device_id: u16,
    pub flags: u32,
    pub pci_bus_info: MagmaPciBusInfo,
    pub queue_family_count: u32,
    pub queue_families: [MagmaQueueFamilyProperties; MAGMA_MAX_QUEUES],
}

unsafe impl MagmaTaggedStructure for MagmaPhysicalDeviceInfo {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::PhysicalDeviceInfo;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
unsafe impl MagmaExtends<MagmaVirtCapabilities> for MagmaPhysicalDeviceInfo {}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaPhysicalDeviceMemoryType {
    pub header: MagmaStructureTypeHeader,
    pub property_flags: u32,
    pub heap_idx: u32,
}

unsafe impl MagmaTaggedStructure for MagmaPhysicalDeviceMemoryType {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::PhysicalDeviceMemoryType;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
unsafe impl MagmaExtends<MagmaPhysicalDeviceInfo> for MagmaPhysicalDeviceMemoryType {}
#[derive(Debug, Default, Clone, Copy, TryFromBytes, IntoBytes, Immutable)]
#[repr(C)]
pub struct MagmaPhysicalDeviceMemoryHeap {
    pub header: MagmaStructureTypeHeader,
    pub heap_size: u64,
    pub heap_flags: u64,
}

unsafe impl MagmaTaggedStructure for MagmaPhysicalDeviceMemoryHeap {
    const STRUCTURE_TYPE: MagmaStructureType = MagmaStructureType::PhysicalDeviceMemoryHeap;

    fn header(&self) -> &MagmaStructureTypeHeader {
        &self.header
    }

    fn header_mut(&mut self) -> &mut MagmaStructureTypeHeader {
        &mut self.header
    }
}
unsafe impl MagmaExtends<MagmaPhysicalDeviceInfo> for MagmaPhysicalDeviceMemoryHeap {}
#[derive(Debug)]
pub enum MagmaExtensibleStruct {
    CreateBufferInfo(MagmaCreateBufferInfo),
    DeviceCreateInfo(MagmaDeviceCreateInfo),
    WireSubmitSyncInfo(MagmaWireSubmitSyncInfo),
    WireSubmitInfo(MagmaWireSubmitInfo),
    SubmitAddressSpaceInfo(MagmaSubmitAddressSpaceInfo),
    WireSubmitBufferInfo(MagmaWireSubmitBufferInfo),
    CreateSyncObjInfo(MagmaCreateSyncObjInfo),
    SyncProperties(MagmaSyncProperties),
    VirtCapabilities(MagmaVirtCapabilities),
    PhysicalDeviceInfo(MagmaPhysicalDeviceInfo),
    PhysicalDeviceMemoryType(MagmaPhysicalDeviceMemoryType),
    PhysicalDeviceMemoryHeap(MagmaPhysicalDeviceMemoryHeap),
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, FromBytes, IntoBytes, Immutable)]
pub struct MagmaCommandHeader {
    pub opcode: u32,
    pub size: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateDevice {
    pub header: MagmaCommandHeader,
    pub physical_device: u32,
    pub device: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateDeviceResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct GetMemoryBudget {
    pub header: MagmaCommandHeader,
    pub device: u32,
    pub heap_idx: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct GetMemoryBudgetResp {
    pub header: MagmaCommandHeader,
    pub budget: MagmaHeapBudget,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateBuffer {
    pub header: MagmaCommandHeader,
    pub device: u32,
    pub _pad0: u32,
    pub info: MagmaCreateBufferInfo,
    pub buffer_out: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateBufferResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateAddressSpace {
    pub header: MagmaCommandHeader,
    pub device: u32,
    pub address_space: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateAddressSpaceResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateQueue {
    pub header: MagmaCommandHeader,
    pub device: u32,
    pub address_space: u32,
    pub info: MagmaCreateQueueInfo,
    pub queue: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateQueueResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct DeviceClose {
    pub header: MagmaCommandHeader,
    pub device: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct PhysicalDeviceClose {
    pub header: MagmaCommandHeader,
    pub physical_device: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct BufferClose {
    pub header: MagmaCommandHeader,
    pub buffer: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct QueueClose {
    pub header: MagmaCommandHeader,
    pub queue: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct AddressSpaceClose {
    pub header: MagmaCommandHeader,
    pub address_space: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct VirtioCreateRing {
    pub header: MagmaCommandHeader,
    pub channel_id: u32,
    pub ring_blob_id: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct MapBufferGpu {
    pub header: MagmaCommandHeader,
    pub address_space: u32,
    pub buffer: u32,
    pub buffer_offset: u64,
    pub gpu_va: u64,
    pub size: u64,
    pub flags: MagmaGpuMapFlags,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct MapBufferGpuResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct UnmapBufferGpu {
    pub header: MagmaCommandHeader,
    pub address_space: u32,
    pub _pad0: u32,
    pub gpu_va: u64,
    pub size: u64,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct UnmapBufferGpuResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SubmitCommand {
    pub header: MagmaCommandHeader,
    pub queue: u32,
    pub _pad0: u32,
    pub submit_info: MagmaWireSubmitInfo,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SubmitCommandResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateSyncObj {
    pub header: MagmaCommandHeader,
    pub device: u32,
    pub _pad0: u32,
    pub info: MagmaCreateSyncObjInfo,
    pub sync_obj: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct CreateSyncObjResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SyncObjClose {
    pub header: MagmaCommandHeader,
    pub sync_obj: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SyncObjSignal {
    pub header: MagmaCommandHeader,
    pub sync_obj: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SyncObjSignalResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SyncObjTimelineWait {
    pub header: MagmaCommandHeader,
    pub sync_obj: u32,
    pub _pad0: u32,
    pub point: u64,
    pub timeout_ns: u64,
    pub flags: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SyncObjTimelineWaitResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SyncObjTimelineSignal {
    pub header: MagmaCommandHeader,
    pub sync_obj: u32,
    pub _pad0: u32,
    pub point: u64,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SyncObjTimelineSignalResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SyncObjTimelineQuery {
    pub header: MagmaCommandHeader,
    pub sync_obj: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct SyncObjTimelineQueryResp {
    pub header: MagmaCommandHeader,
    pub point_out: u64,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct QueueCheckStatus {
    pub header: MagmaCommandHeader,
    pub queue: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct QueueCheckStatusResp {
    pub header: MagmaCommandHeader,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct VirtioPing {
    pub header: MagmaCommandHeader,
    pub channel_id: u32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct VirtioCreateFence {
    pub header: MagmaCommandHeader,
    pub ring_idx: u32,
    pub guest_sync_id: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, TryFromBytes, IntoBytes, Immutable)]
pub struct VirtioSyncObjClose {
    pub header: MagmaCommandHeader,
    pub sync_obj: u32,
    pub ring_idx: u32,
    pub wait_seqno: u32,
    pub _padding: u32,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum MagmaProtocol {
    CreateDevice(CreateDevice),
    GetMemoryBudget(GetMemoryBudget),
    CreateBuffer(CreateBuffer),
    CreateAddressSpace(CreateAddressSpace),
    CreateQueue(CreateQueue),
    DeviceClose(DeviceClose),
    PhysicalDeviceClose(PhysicalDeviceClose),
    BufferClose(BufferClose),
    QueueClose(QueueClose),
    AddressSpaceClose(AddressSpaceClose),
    VirtioCreateRing(VirtioCreateRing),
    MapBufferGpu(MapBufferGpu),
    UnmapBufferGpu(UnmapBufferGpu),
    SubmitCommand(SubmitCommand),
    CreateSyncObj(CreateSyncObj),
    SyncObjClose(SyncObjClose),
    SyncObjSignal(SyncObjSignal),
    SyncObjTimelineWait(SyncObjTimelineWait),
    SyncObjTimelineSignal(SyncObjTimelineSignal),
    SyncObjTimelineQuery(SyncObjTimelineQuery),
    QueueCheckStatus(QueueCheckStatus),
    VirtioPing(VirtioPing),
    VirtioCreateFence(VirtioCreateFence),
    VirtioSyncObjClose(VirtioSyncObjClose),

    CreateDeviceResp(CreateDeviceResp),
    GetMemoryBudgetResp(GetMemoryBudgetResp),
    CreateBufferResp(CreateBufferResp),
    CreateAddressSpaceResp(CreateAddressSpaceResp),
    CreateQueueResp(CreateQueueResp),
    MapBufferGpuResp(MapBufferGpuResp),
    UnmapBufferGpuResp(UnmapBufferGpuResp),
    SubmitCommandResp(SubmitCommandResp),
    CreateSyncObjResp(CreateSyncObjResp),
    SyncObjSignalResp(SyncObjSignalResp),
    SyncObjTimelineWaitResp(SyncObjTimelineWaitResp),
    SyncObjTimelineSignalResp(SyncObjTimelineSignalResp),
    SyncObjTimelineQueryResp(SyncObjTimelineQueryResp),
    QueueCheckStatusResp(QueueCheckStatusResp),
}
