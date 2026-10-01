// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

mod device;
pub mod kumquat;
mod memory;
mod queue;
mod sync;

pub use device::VirtioGpu;

use std::sync::Arc;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;
use magma_gpu::util::Reader;
use magma_gpu::virtgpu_kumquat::defines::VirtGpuParam;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_3D_FEATURES;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_CAPSET_QUERY_FIX;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_CONTEXT_INIT;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_CREATE_GUEST_HANDLE;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_CROSS_DEVICE;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_EXPLICIT_DEBUG_NAME;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_FENCE_PASSING;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_HOST_VISIBLE;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_RESOURCE_BLOB;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_PARAM_SUPPORTED_CAPSET_IDS;

use crate::decoder::decode_extensible_struct;
use crate::defines::MagmaHeap;
use crate::defines::MagmaMemoryType;
use crate::defines::MagmaPhysicalDeviceInfo;
use crate::defines::MagmaQueueFamilyProperties;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaExtensibleStruct;
use crate::protocol::MagmaVirtCapabilities;
use crate::traits::BackendDevice;
use crate::traits::GenericPhysicalDevice;

#[repr(usize)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VirtGpuFeature {
    GpuRendering = 0,
    CapsetQueryFix = 1,
    ResourceBlob = 2,
    HostVisible = 3,
    CrossDevice = 4,
    ContextInit = 5,
    SupportedCapsetIds = 6,
    ExplicitDebugName = 7,
    FencePassing = 8,
    CreateGuestHandle = 9,
}

pub struct VirtGpuFeatures {
    params: [VirtGpuParam; 10],
}

#[derive(Default, Clone, Debug)]
pub struct DecodedCapabilities {
    pub caps: MagmaVirtCapabilities,
    pub devices: Vec<DecodedPhysicalDevice>,
}

#[derive(Default, Clone, Debug)]
pub struct DecodedPhysicalDevice {
    pub info: MagmaPhysicalDeviceInfo,
    pub memory_types: Vec<MagmaMemoryType>,
    pub memory_heaps: Vec<MagmaHeap>,
}

pub trait VirtioGpuTransport: Send + Sync + 'static {
    fn caps(&self) -> &MagmaVirtCapabilities;
    fn device(&self) -> &DecodedPhysicalDevice;
    fn pdev_idx(&self) -> u32;
    fn init_context(&self) -> Result<()>;
    fn create_blob(&self, blob_id: u64, size: usize) -> Result<(u32, u32)>;
    fn map_blob(self: &Arc<Self>, bo_handle: u32, size: usize) -> Result<Arc<dyn MappedRegion>>;
    fn export_blob(&self, bo_handle: u32) -> Result<MagmaGpuHandle>;
    fn close_blob(&self, bo_handle: u32);
    fn submit_cmd(&self, cmd: &[u8], ring_idx: u32, syncobjs: &[u32]) -> Result<()>;
    fn syncobj_create(&self, initial_signaled: bool) -> Result<u32>;
    fn syncobj_destroy(&self, sync_handle: u32);
    fn syncobj_wait(&self, sync_handle: u32, timeout_ns: u64) -> Result<()>;
    fn syncobj_signal(&self, sync_handle: u32) -> Result<()>;
    fn syncobj_export(&self, sync_handle: u32, is_cpu_signaled: bool) -> Result<MagmaGpuHandle>;
    fn syncobj_import(&self, sync_handle: u32, handle: MagmaGpuHandle) -> Result<()>;
}

impl VirtGpuFeatures {
    pub fn new<F>(mut query: F) -> VirtGpuFeatures
    where
        F: FnMut(&mut VirtGpuParam),
    {
        let mut params = [
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_3D_FEATURES),
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_CAPSET_QUERY_FIX),
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_RESOURCE_BLOB),
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_HOST_VISIBLE),
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_CROSS_DEVICE),
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_CONTEXT_INIT),
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_SUPPORTED_CAPSET_IDS),
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_EXPLICIT_DEBUG_NAME),
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_FENCE_PASSING),
            VirtGpuParam::new(VIRTGPU_KUMQUAT_PARAM_CREATE_GUEST_HANDLE),
        ];
        for param in params.iter_mut() {
            query(param);
        }
        VirtGpuFeatures { params }
    }

    pub fn support(&self, feature: VirtGpuFeature) -> bool {
        self.params[feature as usize].value != 0
    }

    pub fn support_capset(&self, capset_id: u32) -> bool {
        (self.params[VirtGpuFeature::SupportedCapsetIds as usize].value & (1 << capset_id)) != 0
    }
}

impl std::ops::Index<VirtGpuFeature> for VirtGpuFeatures {
    type Output = u64;

    fn index(&self, feature: VirtGpuFeature) -> &u64 {
        &self.params[feature as usize].value
    }
}

pub fn validate_ring_buffer_len(len: u64) -> Result<usize> {
    const MIN_RING_BUFFER_LEN: u64 = 4096;
    const MAX_RING_BUFFER_LEN: u64 = 256 * 1024 * 1024; // 256 MB

    if !(MIN_RING_BUFFER_LEN..=MAX_RING_BUFFER_LEN).contains(&len) || len % 4096 != 0 {
        return Err(Error::InvalidArgs);
    }

    Ok(len as usize)
}

pub fn decode_capabilities(buf: &[u8]) -> Result<DecodedCapabilities> {
    let mut reader = Reader::new(buf);
    let mut decoded = DecodedCapabilities::default();
    let mut current_device: Option<DecodedPhysicalDevice> = None;

    while let Some(ext) = decode_extensible_struct(&mut reader) {
        match ext {
            MagmaExtensibleStruct::VirtCapabilities(caps) => {
                validate_ring_buffer_len(caps.ring_buffer_len)?;
                decoded.caps = caps;
            }
            MagmaExtensibleStruct::PhysicalDeviceInfo(info) => {
                if let Some(dev) = current_device.take() {
                    decoded.devices.push(dev);
                }
                current_device = Some(DecodedPhysicalDevice {
                    info,
                    ..Default::default()
                });
            }
            MagmaExtensibleStruct::PhysicalDeviceMemoryType(mt) => {
                if let Some(ref mut dev) = current_device {
                    dev.memory_types.push(MagmaMemoryType {
                        property_flags: mt.property_flags,
                        heap_idx: mt.heap_idx,
                    });
                }
            }
            MagmaExtensibleStruct::PhysicalDeviceMemoryHeap(mh) => {
                if let Some(ref mut dev) = current_device {
                    dev.memory_heaps.push(MagmaHeap {
                        heap_size: mh.heap_size,
                        heap_flags: mh.heap_flags,
                    });
                }
            }
            _ => {}
        }
    }

    if let Some(dev) = current_device {
        decoded.devices.push(dev);
    }

    validate_ring_buffer_len(decoded.caps.ring_buffer_len)?;
    if decoded.devices.len() != decoded.caps.num_physical_devices as usize {
        return Err(Error::InvalidArgs);
    }

    Ok(decoded)
}

impl<T: VirtioGpuTransport> GenericPhysicalDevice for T {
    fn create_device(
        self: Arc<T>,
        _info: &MagmaPhysicalDeviceInfo,
    ) -> Result<Arc<dyn BackendDevice>> {
        Ok(Arc::new(VirtioGpu::new(self)?))
    }

    fn get_device_info(&self, info: &mut MagmaPhysicalDeviceInfo) -> Result<()> {
        *info = self.device().info;
        Ok(())
    }

    fn get_memory_types(&self) -> Result<Vec<MagmaMemoryType>> {
        Ok(self.device().memory_types.clone())
    }

    fn get_memory_heaps(&self) -> Result<Vec<MagmaHeap>> {
        Ok(self.device().memory_heaps.clone())
    }

    fn get_queue_family_properties(&self) -> Result<Vec<MagmaQueueFamilyProperties>> {
        let info = &self.device().info;
        let count = info.queue_family_count as usize;
        Ok(info.queue_families[..count.min(info.queue_families.len())].to_vec())
    }
}
