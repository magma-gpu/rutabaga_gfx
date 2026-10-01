// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT
//
// Generated via:
//   https://gitlab.freedesktop.org/mesa/mesa/-/tree/main/src/magma/gorgonzola
//
// Submit patches, do not hand-edit.

use crate::protocol::*;

impl MagmaHeap {
    pub fn new(heap_size: u64, heap_flags: u64) -> MagmaHeap {
        MagmaHeap {
            heap_size,
            heap_flags,
        }
    }
}

impl MagmaHeapBudget {
    pub fn new(budget: u64, usage: u64) -> MagmaHeapBudget {
        MagmaHeapBudget { budget, usage }
    }
}

impl MagmaMemoryType {
    pub fn new(property_flags: u32, heap_idx: u32) -> MagmaMemoryType {
        MagmaMemoryType {
            property_flags,
            heap_idx,
        }
    }
}

impl MagmaPciBusInfo {
    pub fn new(
        domain: u16,
        subvendor_id: u16,
        subdevice_id: u16,
        revision_id: u8,
        bus: u8,
        device: u8,
        function: u8,
    ) -> MagmaPciBusInfo {
        MagmaPciBusInfo {
            domain,
            subvendor_id,
            subdevice_id,
            revision_id,
            bus,
            device,
            function,
            ..Default::default()
        }
    }
}

impl MagmaCreateQueueInfo {
    pub fn new(queue_family_idx: u32, priority: u32, flags: u32) -> MagmaCreateQueueInfo {
        MagmaCreateQueueInfo {
            queue_family_idx,
            priority,
            flags,
            ..Default::default()
        }
    }
}

impl MagmaQueueFamilyProperties {
    pub fn new(queue_flags: MagmaQueueFlags, queue_count: u32) -> MagmaQueueFamilyProperties {
        MagmaQueueFamilyProperties {
            queue_flags,
            queue_count,
        }
    }
}

impl MagmaCreateBufferInfo {
    pub fn new(
        memory_type_idx: u32,
        alignment: u32,
        common_flags: u32,
        vendor_flags: u32,
        size: u64,
    ) -> MagmaCreateBufferInfo {
        MagmaCreateBufferInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::CreateBufferInfo as u32,
                size: std::mem::size_of::<MagmaCreateBufferInfo>() as u32,
                p_next: 0,
            },
            memory_type_idx,
            alignment,
            common_flags,
            vendor_flags,
            size,
        }
    }
}

impl MagmaDeviceCreateInfo {
    pub fn new(flags: u32) -> MagmaDeviceCreateInfo {
        MagmaDeviceCreateInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::DeviceCreateInfo as u32,
                size: std::mem::size_of::<MagmaDeviceCreateInfo>() as u32,
                p_next: 0,
            },
            flags,
            ..Default::default()
        }
    }
}

impl MagmaWireSubmitSyncInfo {
    pub fn new(
        num_wait_sync_objs: u32,
        num_signal_sync_objs: u32,
        wait_sync_objs: [u32; MAGMA_MAX_SYNCOBJS],
        signal_sync_objs: [u32; MAGMA_MAX_SYNCOBJS],
        wait_points: [u64; MAGMA_MAX_SYNCOBJS],
        signal_points: [u64; MAGMA_MAX_SYNCOBJS],
    ) -> MagmaWireSubmitSyncInfo {
        MagmaWireSubmitSyncInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SubmitSyncInfo as u32,
                size: std::mem::size_of::<MagmaWireSubmitSyncInfo>() as u32,
                p_next: 0,
            },
            num_wait_sync_objs,
            num_signal_sync_objs,
            wait_sync_objs,
            signal_sync_objs,
            wait_points,
            signal_points,
        }
    }
}

impl MagmaWireSubmitInfo {
    pub fn new(flags: u32, sync_info: MagmaWireSubmitSyncInfo) -> MagmaWireSubmitInfo {
        MagmaWireSubmitInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SubmitInfo as u32,
                size: std::mem::size_of::<MagmaWireSubmitInfo>() as u32,
                p_next: 0,
            },
            flags,
            sync_info,
            ..Default::default()
        }
    }

    pub fn address_space_info(&self) -> Option<&MagmaSubmitAddressSpaceInfo> {
        self.get_extension::<MagmaSubmitAddressSpaceInfo>()
    }

    pub fn address_space_info_mut(&mut self) -> Option<&mut MagmaSubmitAddressSpaceInfo> {
        unsafe { self.get_extension_mut::<MagmaSubmitAddressSpaceInfo>() }
    }

    pub fn buffer_info(&self) -> Option<&MagmaWireSubmitBufferInfo> {
        self.get_extension::<MagmaWireSubmitBufferInfo>()
    }

    pub fn buffer_info_mut(&mut self) -> Option<&mut MagmaWireSubmitBufferInfo> {
        unsafe { self.get_extension_mut::<MagmaWireSubmitBufferInfo>() }
    }
}

impl MagmaSubmitAddressSpaceInfo {
    pub fn new(command_va: u64, length: u64) -> MagmaSubmitAddressSpaceInfo {
        MagmaSubmitAddressSpaceInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SubmitAddressSpaceInfo as u32,
                size: std::mem::size_of::<MagmaSubmitAddressSpaceInfo>() as u32,
                p_next: 0,
            },
            command_va,
            length,
        }
    }
}

impl MagmaWireSubmitBufferInfo {
    pub fn new(command_buffer: u32, start_offset: u64, length: u64) -> MagmaWireSubmitBufferInfo {
        MagmaWireSubmitBufferInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SubmitBufferInfo as u32,
                size: std::mem::size_of::<MagmaWireSubmitBufferInfo>() as u32,
                p_next: 0,
            },
            command_buffer,
            start_offset,
            length,
            ..Default::default()
        }
    }
}

impl MagmaCreateSyncObjInfo {
    pub fn new(
        sync_type: u32,
        use_flags: MagmaSyncObjUseFlags,
        initial_point: u64,
    ) -> MagmaCreateSyncObjInfo {
        MagmaCreateSyncObjInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::CreateSyncObjInfo as u32,
                size: std::mem::size_of::<MagmaCreateSyncObjInfo>() as u32,
                p_next: 0,
            },
            sync_type,
            use_flags,
            initial_point,
        }
    }
}

impl MagmaSyncProperties {
    pub fn new(capabilities: u32, max_timeline_value_difference: u64) -> MagmaSyncProperties {
        MagmaSyncProperties {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::SyncProperties as u32,
                size: std::mem::size_of::<MagmaSyncProperties>() as u32,
                p_next: 0,
            },
            capabilities,
            max_timeline_value_difference,
            ..Default::default()
        }
    }
}

impl MagmaVirtCapabilities {
    pub fn new(
        capset_version: u32,
        num_physical_devices: u32,
        blob_preference: VirtioGpuBlobPreference,
        ring_buffer_len: u64,
    ) -> MagmaVirtCapabilities {
        MagmaVirtCapabilities {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::VirtCapabilities as u32,
                size: std::mem::size_of::<MagmaVirtCapabilities>() as u32,
                p_next: 0,
            },
            capset_version,
            num_physical_devices,
            blob_preference,
            ring_buffer_len,
            ..Default::default()
        }
    }

    pub fn physical_device_info(&self) -> Option<&MagmaPhysicalDeviceInfo> {
        self.get_extension::<MagmaPhysicalDeviceInfo>()
    }

    pub fn physical_device_info_mut(&mut self) -> Option<&mut MagmaPhysicalDeviceInfo> {
        unsafe { self.get_extension_mut::<MagmaPhysicalDeviceInfo>() }
    }
}

impl MagmaPhysicalDeviceInfo {
    pub fn new(
        bus_type: u32,
        vendor_id: MagmaVendorId,
        device_id: u16,
        flags: u32,
        pci_bus_info: MagmaPciBusInfo,
        queue_family_count: u32,
        queue_families: [MagmaQueueFamilyProperties; MAGMA_MAX_QUEUES],
    ) -> MagmaPhysicalDeviceInfo {
        MagmaPhysicalDeviceInfo {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::PhysicalDeviceInfo as u32,
                size: std::mem::size_of::<MagmaPhysicalDeviceInfo>() as u32,
                p_next: 0,
            },
            bus_type,
            vendor_id,
            device_id,
            flags,
            pci_bus_info,
            queue_family_count,
            queue_families,
        }
    }

    pub fn memory_type(&self) -> Option<&MagmaPhysicalDeviceMemoryType> {
        self.get_extension::<MagmaPhysicalDeviceMemoryType>()
    }

    pub fn memory_type_mut(&mut self) -> Option<&mut MagmaPhysicalDeviceMemoryType> {
        unsafe { self.get_extension_mut::<MagmaPhysicalDeviceMemoryType>() }
    }

    pub fn memory_heap(&self) -> Option<&MagmaPhysicalDeviceMemoryHeap> {
        self.get_extension::<MagmaPhysicalDeviceMemoryHeap>()
    }

    pub fn memory_heap_mut(&mut self) -> Option<&mut MagmaPhysicalDeviceMemoryHeap> {
        unsafe { self.get_extension_mut::<MagmaPhysicalDeviceMemoryHeap>() }
    }
}

impl MagmaPhysicalDeviceMemoryType {
    pub fn new(property_flags: u32, heap_idx: u32) -> MagmaPhysicalDeviceMemoryType {
        MagmaPhysicalDeviceMemoryType {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::PhysicalDeviceMemoryType as u32,
                size: std::mem::size_of::<MagmaPhysicalDeviceMemoryType>() as u32,
                p_next: 0,
            },
            property_flags,
            heap_idx,
        }
    }
}

impl MagmaPhysicalDeviceMemoryHeap {
    pub fn new(heap_size: u64, heap_flags: u64) -> MagmaPhysicalDeviceMemoryHeap {
        MagmaPhysicalDeviceMemoryHeap {
            header: MagmaStructureTypeHeader {
                stype: MagmaStructureType::PhysicalDeviceMemoryHeap as u32,
                size: std::mem::size_of::<MagmaPhysicalDeviceMemoryHeap>() as u32,
                p_next: 0,
            },
            heap_size,
            heap_flags,
        }
    }
}
