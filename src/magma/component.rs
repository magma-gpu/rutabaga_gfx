// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use magma_gpu::util::Error as MagmaGpuError;
use magma_gpu_magma::encoder::Encoder;
use magma_gpu_magma::enumerate_devices;
use magma_gpu_magma::protocol::*;
use magma_gpu_magma::PhysicalDevice;

use crate::magma::context::MagmaVirtioGpuContext;
use crate::magma::sync_thread::MagmaVirtioGpuSyncThread;
use crate::magma::thread::MagmaVirtioGpuThreadPool;
use crate::rutabaga_core::RutabagaComponent;
use crate::rutabaga_core::RutabagaContext;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaResult;

pub struct MagmaVirtioGpu {
    caps_bytes: Vec<u8>,
    physical_devices: Vec<PhysicalDevice>,
    _fence_handler: RutabagaFenceHandler,
    pool: Arc<MagmaVirtioGpuThreadPool>,
    sync_thread: Arc<MagmaVirtioGpuSyncThread>,
}

fn encode_capabilities(
    caps: &MagmaVirtCapabilities,
    devices: &[PhysicalDevice],
) -> Result<Vec<u8>, MagmaGpuError> {
    let mut encoder = Encoder::new(Vec::new());
    let mut caps = *caps;
    caps.header.stype = MAGMA_STRUCTURE_TYPE_VIRT_CAPABILITIES;
    caps.header.size = std::mem::size_of::<MagmaVirtCapabilities>() as u32;
    encoder.encode_virt_capabilities(&caps)?;
    for dev in devices {
        let mut info = *dev.info();
        info.header.stype = MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_INFO;
        info.header.size = std::mem::size_of::<MagmaPhysicalDeviceInfo>() as u32;
        encoder.encode_physical_device_info(&info)?;
        if let Ok(mem_types) = dev.get_memory_types() {
            for mt in &mem_types {
                let ext_mt = MagmaPhysicalDeviceMemoryType {
                    header: MagmaStructureTypeHeader {
                        stype: MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_TYPE,
                        size: std::mem::size_of::<MagmaPhysicalDeviceMemoryType>() as u32,
                        p_next: 0,
                    },
                    property_flags: mt.property_flags,
                    heap_idx: mt.heap_idx,
                };
                encoder.encode_physical_device_memory_type(&ext_mt)?;
            }
        }
        if let Ok(mem_heaps) = dev.get_memory_heaps() {
            for mh in &mem_heaps {
                let ext_mh = MagmaPhysicalDeviceMemoryHeap {
                    header: MagmaStructureTypeHeader {
                        stype: MAGMA_STRUCTURE_TYPE_PHYSICAL_DEVICE_MEMORY_HEAP,
                        size: std::mem::size_of::<MagmaPhysicalDeviceMemoryHeap>() as u32,
                        p_next: 0,
                    },
                    heap_size: mh.heap_size,
                    heap_flags: mh.heap_flags,
                };
                encoder.encode_physical_device_memory_heap(&ext_mh)?;
            }
        }
    }
    Ok(encoder.into_inner())
}

pub const RING_BUFFER_SIZE: u64 = 65536;

impl MagmaVirtioGpu {
    /// Initializes the magma component.
    pub fn init(fence_handler: RutabagaFenceHandler) -> RutabagaResult<Box<dyn RutabagaComponent>> {
        let physical_devices = enumerate_devices().map_err(|_| MagmaGpuError::Unsupported)?;
        let caps = MagmaVirtCapabilities {
            capset_version: 1,
            num_physical_devices: physical_devices.len() as u32,
            ring_buffer_len: RING_BUFFER_SIZE,
            ..Default::default()
        };

        let caps_bytes = encode_capabilities(&caps, &physical_devices)?;

        let pool = MagmaVirtioGpuThreadPool::new();
        let sync_thread = Arc::new(MagmaVirtioGpuSyncThread::new(fence_handler.clone())?);

        Ok(Box::new(MagmaVirtioGpu {
            caps_bytes,
            physical_devices,
            _fence_handler: fence_handler,
            pool,
            sync_thread,
        }))
    }
}

impl RutabagaComponent for MagmaVirtioGpu {
    fn get_capset_info(&self, _capset_id: u32) -> (u32, u32) {
        (1u32, self.caps_bytes.len() as u32)
    }

    fn get_capset(&self, _capset_id: u32, _version: u32) -> Vec<u8> {
        self.caps_bytes.clone()
    }

    fn create_context(
        &self,
        _ctx_id: u32,
        _context_init: u32,
        context_name: Option<&str>,
        fence_handler: RutabagaFenceHandler,
    ) -> RutabagaResult<Box<dyn RutabagaContext>> {
        Ok(Box::new(MagmaVirtioGpuContext::new(
            context_name,
            fence_handler,
            self.physical_devices.clone(),
            self.pool.clone(),
            self.sync_thread.clone(),
        )))
    }
}
