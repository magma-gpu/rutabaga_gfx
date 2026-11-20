// Copyright 2021 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! vulkano_gralloc: Implements swapchain allocation and memory mapping
//! using Vulkano.
//!
//! External code found at https://github.com/vulkano-rs/vulkano.

#![cfg(feature = "vulkano")]

mod sys;

use std::collections::HashMap as Map;
use std::convert::TryInto;
use std::sync::Arc;

use log::warn;
use magma_gpu::util::Error as MagmaGpuError;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;
use magma_gpu::util::RawMapping;
use magma_gpu::util::MAGMA_GPU_HANDLE_TYPE_MEM_DMABUF;
use magma_gpu::util::MAGMA_GPU_HANDLE_TYPE_MEM_OPAQUE_FD;
use vulkano::device::physical::PhysicalDeviceType;
use vulkano::device::Device;
use vulkano::device::DeviceCreateInfo;
use vulkano::device::DeviceCreationError;
use vulkano::device::QueueCreateInfo;
use vulkano::device::QueueFlags;
use vulkano::image;
use vulkano::image::ImageDimensions;
use vulkano::image::ImageError;
use vulkano::image::ImageTiling;
use vulkano::image::ImageUsage;
use vulkano::image::SampleCount;
use vulkano::instance::Instance;
use vulkano::instance::InstanceCreateInfo;
use vulkano::instance::InstanceCreationError;
use vulkano::instance::InstanceExtensions;
use vulkano::instance::Version;
use vulkano::memory::DeviceMemory;
use vulkano::memory::DeviceMemoryError;
use vulkano::memory::ExternalMemoryHandleType;
use vulkano::memory::ExternalMemoryHandleTypes;
use vulkano::memory::MappedDeviceMemory;
use vulkano::memory::MemoryAllocateInfo;
use vulkano::memory::MemoryMapError;
use vulkano::memory::MemoryPropertyFlags;
use vulkano::memory::MemoryRequirements;
use vulkano::memory::MemoryType;
use vulkano::sync::Sharing;
use vulkano::LoadingError;
use vulkano::VulkanError;
use vulkano::VulkanLibrary;

use crate::rutabaga_gralloc::formats::canonical_image_requirements;
use crate::rutabaga_gralloc::formats::DrmFormat;
use crate::rutabaga_gralloc::gralloc::Gralloc;
use crate::rutabaga_gralloc::gralloc::ImageAllocationInfo;
use crate::rutabaga_gralloc::gralloc::ImageMemoryRequirements;
use crate::rutabaga_utils::DeviceId;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaResult;
use crate::rutabaga_utils::VulkanInfo;
use crate::rutabaga_utils::RUTABAGA_MAP_CACHE_CACHED;
use crate::rutabaga_utils::RUTABAGA_MAP_CACHE_WC;

/// A convenience enum for allocation
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AllocFromRequirementsFilter {
    Preferred,
    Allowed,
    Forbidden,
}

/// A gralloc implementation capable of allocation `VkDeviceMemory`.
pub struct VulkanoGralloc {
    devices: Map<PhysicalDeviceType, Arc<Device>>,
    device_by_id: Map<DeviceId, Arc<Device>>,
    has_integrated_gpu: bool,
}

struct VulkanoMapping {
    mapped_memory: MappedDeviceMemory,
    size: usize,
}

impl VulkanoMapping {
    pub fn new(mapped_memory: MappedDeviceMemory, size: usize) -> VulkanoMapping {
        VulkanoMapping {
            mapped_memory,
            size,
        }
    }
}

unsafe impl MappedRegion for VulkanoMapping {
    /// Used for passing this region for hypervisor memory mappings.  We trust crosvm to use this
    /// safely.
    fn as_ptr(&self) -> *mut u8 {
        unsafe {
            // Will not panic since the requested range of the device memory was verified on
            // creation
            let x = self.mapped_memory.write(0..self.size as u64).unwrap();
            x.as_mut_ptr()
        }
    }

    /// Returns the size of the memory region in bytes.
    fn size(&self) -> usize {
        self.size
    }

    /// Returns rutabaga mapping representation of the region
    fn as_raw_mapping(&self) -> RawMapping {
        let ptr: u64 = unsafe {
            // Will not panic since the requested range of the device memory was verified on
            // creation
            let x = self.mapped_memory.write(0..self.size as u64).unwrap();
            x.as_mut_ptr() as u64
        };

        RawMapping {
            ptr,
            size: self.size as u64,
        }
    }
}

trait DeviceExt {
    /// Get a unique identifier for the device.
    fn get_id(&self) -> DeviceId;
}

impl DeviceExt for Device {
    fn get_id(&self) -> DeviceId {
        let properties = self.physical_device().properties();
        DeviceId {
            device_uuid: properties.device_uuid.expect("Vulkan should support uuid"),
            driver_uuid: properties.driver_uuid.expect("Vulkan should support uuid"),
        }
    }
}

impl VulkanoGralloc {
    /// Returns a new `VulkanGralloc' instance upon success.
    pub fn init() -> RutabagaResult<Box<dyn Gralloc>> {
        // Initialization copied from triangle.rs in Vulkano.  Look there for a more detailed
        // explanation of VK initialization.
        let library = VulkanLibrary::new()?;

        let instance_extensions = InstanceExtensions {
            khr_external_memory_capabilities: true,
            khr_get_physical_device_properties2: true,
            ..InstanceExtensions::empty()
        };
        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                enabled_extensions: instance_extensions,
                max_api_version: Some(Version::V1_1),
                ..Default::default()
            },
        )?;

        let mut devices: Map<PhysicalDeviceType, Arc<Device>> = Default::default();
        let mut device_by_id: Map<DeviceId, Arc<Device>> = Default::default();
        let mut has_integrated_gpu = false;

        let physical_devices: Vec<_> = instance.enumerate_physical_devices()?.collect();
        let has_non_cpu_gpu = physical_devices.iter().any(|p| {
            matches!(
                p.properties().device_type,
                PhysicalDeviceType::DiscreteGpu
                    | PhysicalDeviceType::IntegratedGpu
                    | PhysicalDeviceType::VirtualGpu
            )
        });

        for physical in physical_devices {
            let device_type = physical.properties().device_type;
            if has_non_cpu_gpu && device_type == PhysicalDeviceType::Cpu {
                continue;
            }

            let queue_family_index = match physical.queue_family_properties().iter().position(|q| {
                // We take the first queue family that supports graphics.
                q.queue_flags.intersects(QueueFlags::GRAPHICS)
            }) {
                Some(family) => family,
                None => {
                    warn!(
                        "Skipping Vulkano for {} due to no graphics queue",
                        physical.properties().device_name
                    );
                    continue;
                }
            };

            let supported_extensions = physical.supported_extensions();

            let desired_extensions = Self::get_desired_device_extensions();

            let intersection = supported_extensions.intersection(&desired_extensions);

            if let Ok((device, mut _queues)) = Device::new(
                physical.clone(),
                DeviceCreateInfo {
                    enabled_extensions: intersection,
                    queue_create_infos: vec![QueueCreateInfo {
                        queue_family_index: queue_family_index as u32,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ) {
                if device_type == PhysicalDeviceType::IntegratedGpu {
                    has_integrated_gpu = true
                }

                // If we have two devices of the same type (two integrated GPUs), the old value is
                // dropped.  Vulkano is verbose enough such that a keener selection algorithm may
                // be used, but the need for such complexity does not seem to exist now.
                devices.insert(device_type, device.clone());
                device_by_id.insert(device.get_id(), device);
            };
        }

        if devices.is_empty() {
            return Err(MagmaGpuError::WithContext("no matching VK devices available").into());
        }

        Ok(Box::new(VulkanoGralloc {
            devices,
            device_by_id,
            has_integrated_gpu,
        }))
    }

    fn get_default_device(&self) -> RutabagaResult<&Arc<Device>> {
        if self.has_integrated_gpu {
            if let Some(dev) = self.devices.get(&PhysicalDeviceType::IntegratedGpu) {
                return Ok(dev);
            }
        }
        for dev_type in [
            PhysicalDeviceType::DiscreteGpu,
            PhysicalDeviceType::VirtualGpu,
            PhysicalDeviceType::Cpu,
            PhysicalDeviceType::Other,
        ] {
            if let Some(dev) = self.devices.get(&dev_type) {
                return Ok(dev);
            }
        }
        Err(RutabagaError::InvalidGrallocGpuType)
    }

    // This function is used safely in this module because gralloc does not:
    //
    //  (1) bind images to any memory.
    //  (2) transition the layout of images.
    //  (3) transfer ownership of images between queues.
    //
    // In addition, we trust Vulkano to validate image parameters are within the Vulkan spec.
    unsafe fn create_image(
        &mut self,
        info: ImageAllocationInfo,
        tiling: ImageTiling,
    ) -> RutabagaResult<(Arc<image::sys::RawImage>, MemoryRequirements)> {
        let device = self.get_default_device()?.clone();

        // Reasonable bounds on image width.
        if info.width == 0 || info.width > 16384 {
            return Err(RutabagaError::InvalidGrallocDimensions);
        }

        // Reasonable bounds on image height.
        if info.height == 0 || info.height > 16384 {
            return Err(RutabagaError::InvalidGrallocDimensions);
        }

        let vulkan_format = info.drm_format.vulkan_format()?;

        let usage_candidates: &[ImageUsage] = match (tiling, info.flags.uses_rendering()) {
            (ImageTiling::Linear, _) => &[
                ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST,
                ImageUsage::TRANSFER_DST,
                ImageUsage::SAMPLED,
            ],
            (_, true) => &[
                ImageUsage::COLOR_ATTACHMENT
                    | ImageUsage::SAMPLED
                    | ImageUsage::TRANSFER_SRC
                    | ImageUsage::TRANSFER_DST,
                ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST,
                ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST,
            ],
            (_, false) => &[
                ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST,
                ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST,
            ],
        };

        let mut last_err = None;
        for &usage in usage_candidates {
            match image::sys::RawImage::new(
                device.clone(),
                image::sys::ImageCreateInfo {
                    dimensions: ImageDimensions::Dim2d {
                        width: info.width,
                        height: info.height,
                        array_layers: 1,
                    },
                    format: Some(vulkan_format),
                    tiling,
                    samples: SampleCount::Sample1,
                    usage,
                    mip_levels: 1,
                    sharing: Sharing::Exclusive,
                    ..Default::default()
                },
            ) {
                Ok(raw) => {
                    let raw_image = Arc::new(raw);
                    let memory_requirements = raw_image.memory_requirements()[0];
                    return Ok((raw_image, memory_requirements));
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap().into())
    }
}

impl Gralloc for VulkanoGralloc {
    fn supports_external_gpu_memory(&self) -> bool {
        for device in self.devices.values() {
            if !device.enabled_extensions().khr_external_memory {
                return false;
            }
        }

        true
    }

    fn supports_dmabuf(&self) -> bool {
        for device in self.devices.values() {
            if !device.enabled_extensions().ext_external_memory_dma_buf {
                return false;
            }
        }

        true
    }

    fn get_image_memory_requirements(
        &mut self,
        info: ImageAllocationInfo,
    ) -> RutabagaResult<ImageMemoryRequirements> {
        let mut reqs: ImageMemoryRequirements = Default::default();
        let is_1d_buffer =
            info.height == 1 && info.drm_format == DrmFormat::new(b'R', b'8', b' ', b' ');

        let (raw_size, memory_type_bits) = if is_1d_buffer {
            reqs.strides[0] = info.width;
            reqs.offsets[0] = 0;
            let aligned_size = ((info.width as u64) + 4095) & !4095;
            (aligned_size.max(4096), 0xffff_ffffu32)
        } else {
            let (linear_size, linear_bits) =
                match unsafe { self.create_image(info, ImageTiling::Linear) } {
                    Ok((linear_image, linear_reqs)) => {
                        let planar_layout = info.drm_format.planar_layout()?;
                        for plane in 0..planar_layout.num_planes {
                            let aspect = info.drm_format.vulkan_image_aspect(plane)?;
                            let layout = linear_image.subresource_layout(aspect, 0, 0)?;
                            reqs.strides[plane] = layout.row_pitch as u32;
                            reqs.offsets[plane] = layout.offset as u32;
                        }
                        (
                            linear_reqs.layout.size() as u64,
                            linear_reqs.memory_type_bits,
                        )
                    }
                    Err(_) => {
                        let canonical = canonical_image_requirements(info)?;
                        reqs.strides = canonical.strides;
                        reqs.offsets = canonical.offsets;
                        (canonical.size, 0xffff_ffffu32)
                    }
                };

            if let Ok((_optimal_image, optimal_reqs)) =
                unsafe { self.create_image(info, ImageTiling::Optimal) }
            {
                let size = std::cmp::max(linear_size, optimal_reqs.layout.size() as u64);
                let combined_bits = optimal_reqs.memory_type_bits & linear_bits;
                let bits = if info.flags.host_visible() {
                    linear_bits
                } else if combined_bits != 0 {
                    combined_bits
                } else {
                    optimal_reqs.memory_type_bits
                };
                (size, bits)
            } else {
                (linear_size, linear_bits)
            }
        };
        let size = ((raw_size + 4095) & !4095).max(4096);

        let device = self.get_default_device()?.clone();

        let need_visible = info.flags.host_visible();
        let want_cached = info.flags.host_cached();

        let (memory_type_index, memory_type) = {
            let filter = |current_type: &MemoryType| {
                if need_visible
                    && !current_type
                        .property_flags
                        .intersects(MemoryPropertyFlags::HOST_VISIBLE)
                {
                    return AllocFromRequirementsFilter::Forbidden;
                }

                if !need_visible
                    && current_type
                        .property_flags
                        .intersects(MemoryPropertyFlags::DEVICE_LOCAL)
                {
                    return AllocFromRequirementsFilter::Preferred;
                }

                if need_visible
                    && want_cached
                    && current_type
                        .property_flags
                        .intersects(MemoryPropertyFlags::HOST_CACHED)
                {
                    return AllocFromRequirementsFilter::Preferred;
                }

                if need_visible
                    && !want_cached
                    && current_type
                        .property_flags
                        .intersects(MemoryPropertyFlags::HOST_COHERENT)
                    && !current_type
                        .property_flags
                        .intersects(MemoryPropertyFlags::HOST_CACHED)
                {
                    return AllocFromRequirementsFilter::Preferred;
                }

                AllocFromRequirementsFilter::Allowed
            };

            let first_loop = device
                .physical_device()
                .memory_properties()
                .memory_types
                .iter()
                .enumerate()
                .map(|(i, t)| (i, t, AllocFromRequirementsFilter::Preferred));
            let second_loop = device
                .physical_device()
                .memory_properties()
                .memory_types
                .iter()
                .enumerate()
                .map(|(i, t)| (i, t, AllocFromRequirementsFilter::Allowed));
            let found_type = first_loop
                .clone()
                .chain(second_loop.clone())
                .filter(|&(i, _, _)| (memory_type_bits & (1 << i)) != 0)
                .find(|&(_, t, rq)| filter(t) == rq)
                .or_else(|| {
                    first_loop
                        .chain(second_loop)
                        .find(|&(_, t, rq)| filter(t) == rq)
                })
                .ok_or(MagmaGpuError::WithContext(
                    "unable to find required memory type",
                ))?;
            (found_type.0, found_type.1)
        };

        reqs.info = info;
        reqs.size = size;

        if memory_type
            .property_flags
            .intersects(MemoryPropertyFlags::HOST_VISIBLE)
        {
            if memory_type
                .property_flags
                .intersects(MemoryPropertyFlags::HOST_CACHED)
            {
                reqs.map_info = RUTABAGA_MAP_CACHE_CACHED;
            } else if memory_type
                .property_flags
                .intersects(MemoryPropertyFlags::HOST_COHERENT)
            {
                reqs.map_info = RUTABAGA_MAP_CACHE_WC;
            }
        }

        reqs.vulkan_info = Some(VulkanInfo {
            memory_idx: memory_type_index as u32,
            device_id: device.get_id(),
        });

        Ok(reqs)
    }

    fn allocate_memory(&mut self, reqs: ImageMemoryRequirements) -> RutabagaResult<MagmaGpuHandle> {
        let vulkan_info = reqs.vulkan_info.ok_or(RutabagaError::InvalidVulkanInfo)?;

        let device = self.get_default_device()?.clone();

        if vulkan_info.memory_idx as usize
            >= device
                .physical_device()
                .memory_properties()
                .memory_types
                .len()
        {
            return Err(RutabagaError::InvalidVulkanInfo);
        }

        let (export_handle_type, export_handle_types, rutabaga_type) =
            match device.enabled_extensions().ext_external_memory_dma_buf {
                true => (
                    ExternalMemoryHandleType::DmaBuf,
                    ExternalMemoryHandleTypes::DMA_BUF,
                    MAGMA_GPU_HANDLE_TYPE_MEM_DMABUF,
                ),
                false => (
                    ExternalMemoryHandleType::OpaqueFd,
                    ExternalMemoryHandleTypes::OPAQUE_FD,
                    MAGMA_GPU_HANDLE_TYPE_MEM_OPAQUE_FD,
                ),
            };

        let device_memory = DeviceMemory::allocate(
            device,
            MemoryAllocateInfo {
                allocation_size: reqs.size,
                memory_type_index: vulkan_info.memory_idx,
                dedicated_allocation: None,
                export_handle_types,
                ..Default::default()
            },
        )?;

        let descriptor = device_memory.export_fd(export_handle_type)?.into();

        Ok(MagmaGpuHandle {
            os_handle: descriptor,
            handle_type: rutabaga_type,
        })
    }

    /// Implementations must map the memory associated with the `resource_id` upon success.
    fn import_and_map(
        &mut self,
        handle: MagmaGpuHandle,
        vulkan_info: VulkanInfo,
        size: u64,
    ) -> RutabagaResult<Box<dyn MappedRegion>> {
        let device = self
            .device_by_id
            .get(&vulkan_info.device_id)
            .ok_or(RutabagaError::InvalidVulkanInfo)?;

        let device_memory = unsafe {
            VulkanoGralloc::import_memory(
                device.clone(),
                MemoryAllocateInfo {
                    allocation_size: size,
                    memory_type_index: vulkan_info.memory_idx,
                    ..Default::default()
                },
                handle,
            )?
        };

        let mapped_memory = MappedDeviceMemory::new(device_memory, 0..size)?;

        Ok(Box::new(VulkanoMapping::new(
            mapped_memory,
            size.try_into().map_err(MagmaGpuError::TryFromIntError)?,
        )))
    }
}

// We should really use a universal error kind for all these errors, say
// "VulkanoError".
impl From<InstanceCreationError> for RutabagaError {
    fn from(e: InstanceCreationError) -> RutabagaError {
        RutabagaError::VkInstanceCreationError(e)
    }
}

impl From<ImageError> for RutabagaError {
    fn from(e: ImageError) -> RutabagaError {
        RutabagaError::VkImageCreationError(e)
    }
}

impl From<DeviceCreationError> for RutabagaError {
    fn from(e: DeviceCreationError) -> RutabagaError {
        RutabagaError::VkDeviceCreationError(e)
    }
}

impl From<DeviceMemoryError> for RutabagaError {
    fn from(e: DeviceMemoryError) -> RutabagaError {
        RutabagaError::VkDeviceMemoryError(e)
    }
}

impl From<MemoryMapError> for RutabagaError {
    fn from(e: MemoryMapError) -> RutabagaError {
        RutabagaError::VkMemoryMapError(e)
    }
}

impl From<LoadingError> for RutabagaError {
    fn from(e: LoadingError) -> RutabagaError {
        RutabagaError::VkLoadingError(e)
    }
}

impl From<VulkanError> for RutabagaError {
    fn from(e: VulkanError) -> RutabagaError {
        RutabagaError::VkError(e)
    }
}
