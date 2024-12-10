// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use log::error;

use mesa3d_util::log_status;
use mesa3d_util::MappedRegion;
use mesa3d_util::MesaError;
use mesa3d_util::MesaHandle;
use mesa3d_util::MesaResult;

use crate::flexible_array_impl;
use crate::ioctl_readwrite;
use crate::ioctl_write_ptr;
use crate::sys::linux::flexible_array::FlexibleArray;
use crate::sys::linux::flexible_array::FlexibleArrayWrapper;

use crate::magma_defines::MagmaCreateBufferInfo;
use crate::magma_defines::MagmaHeapBudget;
use crate::magma_defines::MagmaImportHandleInfo;
use crate::magma_defines::MagmaMemoryProperties;
use crate::magma_defines::MAGMA_HEAP_CPU_VISIBLE_BIT;
use crate::magma_defines::MAGMA_HEAP_DEVICE_LOCAL_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT;
use crate::magma_defines::MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT;

use crate::sys::linux::bindings::drm_bindings::DRM_COMMAND_BASE;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;
use crate::sys::linux::bindings::i915_bindings::*;
use crate::sys::linux::PlatformDevice;

use crate::traits::Buffer;
use crate::traits::Context;
use crate::traits::Device;
use crate::traits::GenericBuffer;
use crate::traits::GenericDevice;
use crate::traits::PhysicalDevice;

ioctl_readwrite!(
    drm_ioctl_i915_getparam,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_I915_GETPARAM,
    drm_i915_getparam
);

ioctl_readwrite!(
    drm_ioctl_i915_query,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_I915_QUERY,
    drm_i915_query
);

ioctl_readwrite!(
    drm_ioctl_i915_gem_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_I915_GEM_CREATE,
    drm_i915_gem_create
);

ioctl_readwrite!(
    drm_ioctl_i915_gem_mmap_offset,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_I915_GEM_MMAP_GTT,
    drm_i915_gem_mmap_offset
);

ioctl_readwrite!(
    drm_ioctl_i915_gem_context_create_ext,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_I915_GEM_CONTEXT_CREATE,
    drm_i915_gem_context_create_ext
);

ioctl_write_ptr!(
    drm_ioctl_i915_gem_context_destroy,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_I915_GEM_CONTEXT_DESTROY,
    drm_i915_gem_context_destroy
);

flexible_array_impl!(
    drm_i915_query_memory_regions,
    drm_i915_memory_region_info,
    num_regions,
    regions
);

fn i915_query<T, S>(
    physical_device: &Arc<dyn PhysicalDevice>,
    query_id: u64,
) -> MesaResult<FlexibleArrayWrapper<T, S>>
where
    T: FlexibleArray<S> + Default,
{
    let mut item = drm_i915_query_item {
        query_id,
        length: 0,
        flags: 0,
        data_ptr: 0,
    };

    let mut query = drm_i915_query {
        num_items: 1,
        flags: 0,
        items_ptr: &mut item as *mut _ as u64,
    };

    // SAFETY: First call to get the size
    unsafe {
        drm_ioctl_i915_query(physical_device.as_fd().unwrap(), &mut query)?;
    }

    if item.length < 0 {
        return Err(MesaError::from(std::io::Error::from_raw_os_error(
            -item.length,
        )));
    }

    let total_size = item.length as usize;
    if total_size == 0 {
        return Ok(FlexibleArrayWrapper::<T, S>::from_total_size(0));
    }

    let mut wrapper = FlexibleArrayWrapper::<T, S>::from_total_size(total_size);
    item.data_ptr = wrapper.as_mut_ptr() as u64;

    // SAFETY: Second call to get the data
    unsafe {
        drm_ioctl_i915_query(physical_device.as_fd().unwrap(), &mut query)?;
    };

    Ok(wrapper)
}

#[derive(Default)]
struct I915MemoryInfo {
    sysmem_total: u64,
    sysmem_free: u64,
    vram_mappable_total: u64,
    vram_mappable_free: u64,
    vram_unmappable_total: u64,
    vram_unmappable_free: u64,
}

fn i915_query_memory_regions(
    physical_device: &Arc<dyn PhysicalDevice>,
) -> MesaResult<I915MemoryInfo> {
    let query_mem_regions = i915_query::<drm_i915_query_memory_regions, drm_i915_memory_region_info>(
        physical_device,
        DRM_I915_QUERY_MEMORY_REGIONS as u64,
    )?;

    let regions = query_mem_regions.entries_slice();
    let mut info = I915MemoryInfo::default();

    for region in regions {
        // SAFETY: Accessing a C union's fields is unsafe in Rust.
        let (probed_cpu_visible_size, unallocated_cpu_visible_size) = unsafe {
            (
                region
                    .__bindgen_anon_1
                    .__bindgen_anon_1
                    .probed_cpu_visible_size,
                region
                    .__bindgen_anon_1
                    .__bindgen_anon_1
                    .unallocated_cpu_visible_size,
            )
        };

        match region.region.memory_class as u32 {
            I915_MEMORY_CLASS_SYSTEM => {
                info.sysmem_total = region.probed_size;
                info.sysmem_free = region.unallocated_size;
            }
            I915_MEMORY_CLASS_DEVICE => {
                if probed_cpu_visible_size > 0 {
                    info.vram_mappable_total = probed_cpu_visible_size;
                    info.vram_unmappable_total = region.probed_size - probed_cpu_visible_size;
                    if region.unallocated_size != u64::MAX {
                        info.vram_mappable_free = unallocated_cpu_visible_size;
                        info.vram_unmappable_free =
                            region.unallocated_size - unallocated_cpu_visible_size;
                    }
                } else {
                    info.vram_mappable_total = region.probed_size;
                    info.vram_unmappable_total = 0;
                    if region.unallocated_size != u64::MAX {
                        info.vram_mappable_free = region.unallocated_size;
                        info.vram_unmappable_free = 0;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(info)
}

pub struct I915 {
    physical_device: Arc<dyn PhysicalDevice>,
    mem_props: MagmaMemoryProperties,
}

struct I915Context {
    physical_device: Arc<dyn PhysicalDevice>,
    context_id: u32,
}

struct I915Buffer {
    physical_device: Arc<dyn PhysicalDevice>,
    gem_handle: u32,
    size: usize,
}

impl I915 {
    pub fn new(physical_device: Arc<dyn PhysicalDevice>) -> MesaResult<I915> {
        let mut val: i32 = 0;
        let mut getparam = drm_i915_getparam {
            param: I915_PARAM_HAS_ALIASING_PPGTT as i32,
            value: &mut val as *mut _,
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_i915_getparam struct
        unsafe {
            drm_ioctl_i915_getparam(physical_device.as_fd().unwrap(), &mut getparam)?;
        }

        let mem_info = i915_query_memory_regions(&physical_device).unwrap_or_default();
        let mut mem_props: MagmaMemoryProperties = Default::default();

        if mem_info.sysmem_total > 0 {
            mem_props.add_heap(mem_info.sysmem_total, MAGMA_HEAP_CPU_VISIBLE_BIT);
            mem_props.add_memory_type(
                MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT,
            );
            mem_props.increment_heap_count();
        }

        if mem_info.vram_mappable_total > 0 {
            mem_props.add_heap(
                mem_info.vram_mappable_total,
                MAGMA_HEAP_CPU_VISIBLE_BIT | MAGMA_HEAP_DEVICE_LOCAL_BIT,
            );
            mem_props.add_memory_type(
                MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
            );
            mem_props.increment_heap_count();
        }

        if mem_info.vram_unmappable_total > 0 {
            mem_props.add_heap(mem_info.vram_unmappable_total, MAGMA_HEAP_DEVICE_LOCAL_BIT);
            mem_props.add_memory_type(MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
            mem_props.increment_heap_count();
        }

        if mem_props.memory_heap_count == 0 {
            // Fallback for older kernels
            mem_props.add_heap(4 * 1024 * 1024 * 1024, MAGMA_HEAP_CPU_VISIBLE_BIT);
            mem_props.add_memory_type(
                MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                    | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT,
            );
            mem_props.increment_heap_count();
        }

        Ok(I915 {
            physical_device,
            mem_props,
        })
    }
}

impl GenericDevice for I915 {
    fn get_memory_properties(&self) -> MesaResult<MagmaMemoryProperties> {
        Ok(self.mem_props.clone())
    }

    fn get_memory_budget(&self, heap_idx: u32) -> MesaResult<MagmaHeapBudget> {
        if heap_idx >= self.mem_props.memory_heap_count {
            return Err(MesaError::WithContext("Heap Index out of bounds"));
        }

        let mem_info = i915_query_memory_regions(&self.physical_device)?;
        let heap = &self.mem_props.memory_heaps[heap_idx as usize];

        let (budget, free) = if heap.is_cpu_visible() && !heap.is_device_local() {
            (mem_info.sysmem_total, mem_info.sysmem_free)
        } else if heap.is_cpu_visible() && heap.is_device_local() {
            (mem_info.vram_mappable_total, mem_info.vram_mappable_free)
        } else if !heap.is_cpu_visible() && heap.is_device_local() {
            (
                mem_info.vram_unmappable_total,
                mem_info.vram_unmappable_free,
            )
        } else {
            return Err(MesaError::Unsupported);
        };

        Ok(MagmaHeapBudget {
            budget,
            usage: budget - free,
        })
    }

    fn create_context(&self, _device: &Arc<dyn Device>) -> MesaResult<Arc<dyn Context>> {
        let ctx = I915Context::new(self.physical_device.clone())?;
        Ok(Arc::new(ctx))
    }

    fn create_buffer(
        &self,
        _device: &Arc<dyn Device>,
        create_info: &MagmaCreateBufferInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        let buf = I915Buffer::new(self.physical_device.clone(), create_info)?;
        Ok(Arc::new(buf))
    }

    fn import(
        &self,
        _device: &Arc<dyn Device>,
        info: MagmaImportHandleInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        let gem_handle = self.physical_device.import(info.handle)?;
        let buf = I915Buffer::from_existing(
            self.physical_device.clone(),
            gem_handle,
            info.size.try_into()?,
        )?;
        Ok(Arc::new(buf))
    }
}

impl Device for I915 {}
impl PlatformDevice for I915 {}

impl I915Context {
    fn new(physical_device: Arc<dyn PhysicalDevice>) -> MesaResult<I915Context> {
        let mut ctx_create = drm_i915_gem_context_create_ext::default();

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_i915_gem_context_create_ext struct
        unsafe {
            drm_ioctl_i915_gem_context_create_ext(
                physical_device.as_fd().unwrap(),
                &mut ctx_create,
            )?;
        };

        Ok(I915Context {
            physical_device,
            context_id: ctx_create.ctx_id,
        })
    }
}

impl Drop for I915Context {
    fn drop(&mut self) {
        let ctx_destroy = drm_i915_gem_context_destroy {
            ctx_id: self.context_id,
            pad: 0,
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_i915_gem_context_destroy struct
        let result = unsafe {
            drm_ioctl_i915_gem_context_destroy(self.physical_device.as_fd().unwrap(), &ctx_destroy)
        };
        log_status!(result);
    }
}

impl Context for I915Context {}

impl I915Buffer {
    fn new(
        physical_device: Arc<dyn PhysicalDevice>,
        create_info: &MagmaCreateBufferInfo,
    ) -> MesaResult<I915Buffer> {
        let mut gem_create = drm_i915_gem_create {
            size: create_info.size,
            handle: 0,
            pad: 0,
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_i915_gem_create struct
        unsafe {
            drm_ioctl_i915_gem_create(physical_device.as_fd().unwrap(), &mut gem_create)?;
        };

        Ok(I915Buffer {
            physical_device,
            gem_handle: gem_create.handle,
            size: create_info.size.try_into()?,
        })
    }

    fn from_existing(
        physical_device: Arc<dyn PhysicalDevice>,
        gem_handle: u32,
        size: usize,
    ) -> MesaResult<I915Buffer> {
        Ok(I915Buffer {
            physical_device,
            gem_handle,
            size,
        })
    }
}

impl GenericBuffer for I915Buffer {
    fn map(&self, _buffer: &Arc<dyn Buffer>) -> MesaResult<Arc<dyn MappedRegion>> {
        let mut gem_mmap = drm_i915_gem_mmap_offset {
            handle: self.gem_handle,
            pad: 0,
            offset: 0,
            flags: I915_MMAP_OFFSET_WC as u64,
            extensions: 0,
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_i915_gem_mmap_offset struct
        let offset = unsafe {
            drm_ioctl_i915_gem_mmap_offset(self.physical_device.as_fd().unwrap(), &mut gem_mmap)?;
            gem_mmap.offset
        };

        let mapping = self.physical_device.cpu_map(offset, self.size)?;
        Ok(Arc::new(mapping))
    }

    fn export(&self) -> MesaResult<MesaHandle> {
        self.physical_device.export(self.gem_handle)
    }

    fn invalidate(
        &self,
        _sync_flags: u64,
        _ranges: &[crate::magma_defines::MagmaMappedMemoryRange],
    ) -> MesaResult<()> {
        Err(MesaError::Unsupported)
    }

    fn flush(
        &self,
        _sync_flags: u64,
        _ranges: &[crate::magma_defines::MagmaMappedMemoryRange],
    ) -> MesaResult<()> {
        Err(MesaError::Unsupported)
    }
}

impl Drop for I915Buffer {
    fn drop(&mut self) {
        self.physical_device.close(self.gem_handle);
    }
}

impl Buffer for I915Buffer {}

unsafe impl Send for I915 {}
unsafe impl Sync for I915 {}

unsafe impl Send for I915Context {}
unsafe impl Sync for I915Context {}

unsafe impl Send for I915Buffer {}
unsafe impl Sync for I915Buffer {}
