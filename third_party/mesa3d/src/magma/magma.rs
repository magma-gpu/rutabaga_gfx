// Copyright 2025 Google
// SPDX-License-Identifier: MIT

//! Magma: Rust implementation of Fuchsia's driver model.
//!
//! Design found at <https://fuchsia.dev/fuchsia-third_party/mesa3d/src/development/graphics/magma/concepts/design>.

use std::sync::Arc;

use mesa3d_util::MappedRegion;
use mesa3d_util::MesaHandle;
use mesa3d_util::OwnedDescriptor;

use crate::magma_defines::MagmaCreateBufferInfo;
use crate::magma_defines::MagmaError;
use crate::magma_defines::MagmaHeapBudget;
use crate::magma_defines::MagmaImportHandleInfo;
use crate::magma_defines::MagmaMappedMemoryRange;
use crate::magma_defines::MagmaMemoryProperties;
use crate::magma_defines::MagmaPciBusInfo;
use crate::magma_defines::MagmaPciInfo;
use crate::magma_defines::MagmaResult;

use crate::traits::Buffer;
use crate::traits::Context;
use crate::traits::Device;
use crate::traits::PhysicalDevice;

use crate::magma_kumquat::enumerate_devices as magma_kumquat_enumerate_devices;
use crate::sys::platform::enumerate_devices as platform_enumerate_devices;

const VIRTGPU_KUMQUAT_ENABLED: &str = "VIRTGPU_KUMQUAT";

#[repr(C)]
#[derive(Clone)]
pub struct MagmaPhysicalDevice {
    physical_device: Arc<dyn PhysicalDevice>,
    pci_info: MagmaPciInfo,
    pci_bus_info: MagmaPciBusInfo,
}

#[derive(Clone)]
pub struct MagmaDevice {
    device: Arc<dyn Device>,
}

#[derive(Clone)]
pub struct MagmaContext {
    _context: Arc<dyn Context>,
}

#[derive(Clone)]
pub struct MagmaBuffer {
    buffer: Arc<dyn Buffer>,
}

pub fn magma_enumerate_devices() -> MagmaResult<Vec<MagmaPhysicalDevice>> {
    let devices = match std::env::var(VIRTGPU_KUMQUAT_ENABLED) {
        Ok(_) => magma_kumquat_enumerate_devices()?,
        Err(_) => platform_enumerate_devices()?,
    };

    Ok(devices)
}

impl MagmaPhysicalDevice {
    pub(crate) fn new(
        physical_device: Arc<dyn PhysicalDevice>,
        pci_info: MagmaPciInfo,
        pci_bus_info: MagmaPciBusInfo,
    ) -> MagmaPhysicalDevice {
        MagmaPhysicalDevice {
            physical_device,
            pci_info,
            pci_bus_info,
        }
    }

    pub fn create_device(&self) -> MagmaResult<MagmaDevice> {
        let device = self
            .physical_device
            .create_device(&self.physical_device, &self.pci_info)?;
        Ok(MagmaDevice { device })
    }
}

#[allow(dead_code)]
pub struct MagmaSemaphore {
    semaphore: OwnedDescriptor,
}

#[allow(dead_code)]
struct MagmaExecResource {
    buffer: MagmaBuffer,
    offset: u64,
    length: u64,
}

#[allow(dead_code)]
struct MagmaExecCommandBuffer {
    resource_idx: u32,
    unused: u32,
    start_offset: u64,
}

#[allow(dead_code)]
struct MagmaCommandDescriptor {
    flags: u64,
    command_buffers: Vec<MagmaExecCommandBuffer>,
    resources: Vec<MagmaExecResource>,
    wait_semaphores: Vec<MagmaSemaphore>,
    signal_semaphores: Vec<MagmaSemaphore>,
}

#[allow(dead_code)]
struct MagmaInlineCommandBuffer {
    data: Vec<u8>,
    wait_semaphores: Vec<MagmaSemaphore>,
    signal_semaphores: Vec<MagmaSemaphore>,
}

impl MagmaDevice {
    pub fn get_memory_properties(&self) -> MagmaResult<MagmaMemoryProperties> {
        let mem_props = self.device.get_memory_properties()?;
        Ok(mem_props)
    }

    pub fn get_memory_budget(&self, heap_idx: u32) -> MagmaResult<MagmaHeapBudget> {
        let budget = self.device.get_memory_budget(heap_idx)?;
        Ok(budget)
    }

    pub fn create_context(&self) -> MagmaResult<MagmaContext> {
        let context = self.device.create_context(&self.device)?;
        Ok(MagmaContext { _context: context })
    }

    pub fn create_buffer(&self, create_info: &MagmaCreateBufferInfo) -> MagmaResult<MagmaBuffer> {
        let buffer = self.device.create_buffer(&self.device, create_info)?;
        Ok(MagmaBuffer { buffer })
    }

    // FIXME: we probably want to import with a memory type
    pub fn import(&self, info: MagmaImportHandleInfo) -> MagmaResult<MagmaBuffer> {
        let buffer = self.device.import(&self.device, info)?;
        Ok(MagmaBuffer { buffer })
    }
}

impl MagmaBuffer {
    pub fn map(&self) -> MagmaResult<Arc<dyn MappedRegion>> {
        let region = self.buffer.map(&self.buffer)?;
        Ok(region)
    }

    pub fn export(&self) -> MagmaResult<MesaHandle> {
        let handle = self.buffer.export()?;
        Ok(handle)
    }

    pub fn invalidate(
        &self,
        sync_flags: u64,
        ranges: &[MagmaMappedMemoryRange],
    ) -> MagmaResult<()> {
        self.buffer.invalidate(sync_flags, ranges)?;
        Ok(())
    }

    pub fn flush(&self, sync_flags: u64, ranges: &[MagmaMappedMemoryRange]) -> MagmaResult<()> {
        self.buffer.flush(sync_flags, ranges)?;
        Ok(())
    }
}

impl MagmaContext {
    pub fn execute_command(
        _connection: &MagmaPhysicalDevice,
        _command_descriptor: u64,
    ) -> MagmaResult<u64> {
        Err(MagmaError::Unimplemented)
    }

    pub fn execute_immediate_commands(
        _connection: &MagmaPhysicalDevice,
        _wait_semaphores: Vec<MagmaSemaphore>,
        _signal_semaphore: Vec<MagmaSemaphore>,
    ) -> MagmaResult<u64> {
        Err(MagmaError::Unimplemented)
    }

    pub fn raw_handle() -> MagmaResult<u64> {
        Err(MagmaError::Unimplemented)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    fn get_physical_device() -> Option<MagmaPhysicalDevice> {
        let valid_vendor_ids: [u16; 4] = [
            MAGMA_VENDOR_ID_INTEL,
            MAGMA_VENDOR_ID_AMD,
            MAGMA_VENDOR_ID_MALI,
            MAGMA_VENDOR_ID_QCOM,
        ];

        let physical_devices = magma_enumerate_devices().unwrap();
        physical_devices
            .into_iter()
            .find(|device| valid_vendor_ids.contains(&device.pci_info.vendor_id))
    }

    #[test]
    fn test_memory_properties() {
        let physical_device = get_physical_device().unwrap();
        let device = physical_device.create_device().unwrap();
        let mem_props = device.get_memory_properties().unwrap();

        assert_ne!(mem_props.memory_type_count, 0);
        assert_ne!(mem_props.memory_heap_count, 0);

        assert_ne!(mem_props.memory_type_count, 0,);
        assert_ne!(mem_props.memory_heap_count, 0,);

        println!("--- Retrieved Magma Memory Properties ---");
        println!("  Total Memory Type Count: {}", mem_props.memory_type_count);
        println!("  Total Memory Heap Count: {}", mem_props.memory_heap_count);

        println!("--- Validating Memory Heaps ---");
        for i in 0..mem_props.memory_heap_count as usize {
            let heap = &mem_props.memory_heaps[i];
            println!("  Heap {}:", i);
            println!("    Size: {} bytes", heap.heap_size);
            println!("    Flags: {:#x}", heap.heap_flags); // Print flags in hex for clarity

            // Assertions for heaps
            assert!(heap.heap_size > 0);
        }

        // Loop through and validate Memory Types
        println!("--- Validating Memory Types ---");
        for i in 0..mem_props.memory_type_count as usize {
            let mem_type = &mem_props.memory_types[i];
            println!("  Memory Type {}:", i);
            println!("    Heap Index: {}", mem_type.heap_idx);
            println!("    Property Flags: {:#x}", mem_type.property_flags); // Print flags in hex

            // Assertions for memory types
            assert!(mem_type.heap_idx < mem_props.memory_heap_count,);

            // Assert that each memory type has at least one property flag set (not NONE)
            assert_ne!(mem_type.property_flags, 0,);
        }

        println!("--- Magma Memory Properties Test Passed Successfully! ---");
    }

    #[test]
    fn test_memory_allocation() {
        let physical_device = get_physical_device().unwrap();
        let device = physical_device.create_device().unwrap();

        let mem_props = device.get_memory_properties().unwrap();

        let mut chosen_memory_type_idx: Option<u32> = None;
        for i in 0..mem_props.memory_type_count as usize {
            let mem_type = &mem_props.memory_types[i];
            if (mem_type.property_flags & MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT != 0)
                && (mem_type.property_flags & MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT != 0)
            {
                chosen_memory_type_idx = Some(i as u32);
                break;
            }
        }

        let memory_type_idx = chosen_memory_type_idx.unwrap();
        let buffer_size: u64 = 4096;

        let create_info = MagmaCreateBufferInfo {
            memory_type_idx,
            alignment: 4096,
            common_flags: 0,
            vendor_flags: 0,
            size: buffer_size,
        };

        let buffer = device.create_buffer(&create_info).unwrap();
    }
}
