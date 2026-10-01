// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::cmp::min;
use std::io::Error as IoError;
use std::mem::size_of;
use std::os::fd::BorrowedFd;

use zerocopy::FromZeros;
use zerocopy::KnownLayout;

use crate::defines::MagmaHeap;
use crate::defines::MagmaMemoryType;
use crate::defines::MagmaSubmitInfo;
use crate::defines::MAGMA_HEAP_CPU_VISIBLE_BIT;
use crate::defines::MAGMA_HEAP_DEVICE_LOCAL_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT;
use crate::defines::MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT;
use crate::error::Error;
use crate::error::Result;
use crate::ioctl_readwrite;
use crate::ioctl_write_ptr;
use crate::sys::linux::bindings::drm_bindings::DRM_COMMAND_BASE;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;
use crate::sys::linux::bindings::i915_bindings::*;

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

ioctl_readwrite!(
    drm_ioctl_i915_gem_execbuffer2,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_I915_GEM_EXECBUFFER2_WR,
    drm_i915_gem_execbuffer2
);

#[derive(Default)]
pub struct I915MemoryInfo {
    pub sysmem_total: u64,
    pub sysmem_free: u64,
    pub vram_mappable_total: u64,
    pub vram_mappable_free: u64,
    pub vram_unmappable_total: u64,
    pub vram_unmappable_free: u64,
}

fn i915_query<T, S, H>(fd: BorrowedFd<'_>, query_id: u32) -> Result<Box<T>>
where
    T: ?Sized + FromZeros + KnownLayout<PointerMetadata = usize>,
{
    let mut item = drm_i915_query_item {
        query_id: query_id as u64,
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
        drm_ioctl_i915_query(fd, &mut query)?;
    }

    if item.length < 0 {
        return Err(Error::from(IoError::from_raw_os_error(-item.length)));
    }

    let total_size = item.length as usize;
    let header_size = size_of::<H>();
    let element_size = size_of::<S>();
    let count = total_size
        .saturating_sub(header_size)
        .div_ceil(element_size);

    let mut query_data = T::new_box_zeroed_with_elems(count).map_err(|_| Error::MemoryError)?;

    item.data_ptr = &mut *query_data as *mut T as *mut () as u64;
    query.items_ptr = &mut item as *mut _ as u64;

    // SAFETY: Second call to get the data
    unsafe {
        drm_ioctl_i915_query(fd, &mut query)?;
    }

    Ok(query_data)
}

pub fn query_memory_regions(fd: BorrowedFd<'_>) -> Result<I915MemoryInfo> {
    let query_mem_regions = i915_query::<
        drm_i915_query_memory_regions<[drm_i915_memory_region_info]>,
        drm_i915_memory_region_info,
        drm_i915_query_memory_regions<[drm_i915_memory_region_info; 0]>,
    >(fd, DRM_I915_QUERY_MEMORY_REGIONS)?;

    let num_regions = min(
        query_mem_regions.num_regions as usize,
        query_mem_regions.regions.len(),
    );
    let regions = &query_mem_regions.regions[..num_regions];
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

pub fn query_memory(fd: BorrowedFd<'_>) -> Result<(Vec<MagmaMemoryType>, Vec<MagmaHeap>)> {
    let mem_info = query_memory_regions(fd).unwrap_or_default();
    let mut heaps = Vec::new();
    let mut types = Vec::new();
    let mut heap_idx = 0u32;

    if mem_info.sysmem_total > 0 {
        heaps.push(MagmaHeap {
            heap_size: mem_info.sysmem_total,
            heap_flags: MAGMA_HEAP_CPU_VISIBLE_BIT,
        });
        types.push(MagmaMemoryType {
            property_flags: MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT,
            heap_idx,
        });
        heap_idx += 1;
    }

    if mem_info.vram_mappable_total > 0 {
        heaps.push(MagmaHeap {
            heap_size: mem_info.vram_mappable_total,
            heap_flags: MAGMA_HEAP_CPU_VISIBLE_BIT | MAGMA_HEAP_DEVICE_LOCAL_BIT,
        });
        types.push(MagmaMemoryType {
            property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
            heap_idx,
        });
        heap_idx += 1;
    }

    if mem_info.vram_unmappable_total > 0 {
        heaps.push(MagmaHeap {
            heap_size: mem_info.vram_unmappable_total,
            heap_flags: MAGMA_HEAP_DEVICE_LOCAL_BIT,
        });
        types.push(MagmaMemoryType {
            property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
            heap_idx,
        });
    }

    if heaps.is_empty() {
        // Fallback for older kernels
        heaps.push(MagmaHeap {
            heap_size: 4 * 1024 * 1024 * 1024,
            heap_flags: MAGMA_HEAP_CPU_VISIBLE_BIT,
        });
        types.push(MagmaMemoryType {
            property_flags: MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT,
            heap_idx: 0,
        });
    }

    Ok((types, heaps))
}

pub fn check_aliasing_ppgtt(fd: BorrowedFd<'_>) -> Result<()> {
    let mut val: i32 = 0;
    let mut getparam = drm_i915_getparam {
        param: I915_PARAM_HAS_ALIASING_PPGTT as i32,
        value: &mut val as *mut _,
    };

    // SAFETY: This is a well-formed ioctl conforming the driver specification.
    unsafe {
        drm_ioctl_i915_getparam(fd, &mut getparam)?;
    }
    Ok(())
}

pub fn gem_create(fd: BorrowedFd<'_>, size: u64) -> Result<u32> {
    let mut gem_create = drm_i915_gem_create {
        size,
        handle: 0,
        pad: 0,
    };

    // SAFETY: Valid fd and drm_i915_gem_create struct.
    unsafe {
        drm_ioctl_i915_gem_create(fd, &mut gem_create)?;
    }

    Ok(gem_create.handle)
}

pub fn gem_mmap_offset(fd: BorrowedFd<'_>, gem_handle: u32) -> Result<u64> {
    let mut gem_mmap = drm_i915_gem_mmap_offset {
        handle: gem_handle,
        pad: 0,
        offset: 0,
        flags: I915_MMAP_OFFSET_WC as u64,
        extensions: 0,
    };

    // SAFETY: Valid fd and drm_i915_gem_mmap_offset struct.
    unsafe {
        drm_ioctl_i915_gem_mmap_offset(fd, &mut gem_mmap)?;
    }
    Ok(gem_mmap.offset)
}

pub fn gem_context_create(fd: BorrowedFd<'_>) -> Result<u32> {
    let mut ctx_create = drm_i915_gem_context_create_ext::default();

    // SAFETY: Valid fd and drm_i915_gem_context_create_ext struct.
    unsafe {
        drm_ioctl_i915_gem_context_create_ext(fd, &mut ctx_create)?;
    }

    Ok(ctx_create.ctx_id)
}

pub fn gem_context_destroy(fd: BorrowedFd<'_>, context_id: u32) -> Result<()> {
    let ctx_destroy = drm_i915_gem_context_destroy {
        ctx_id: context_id,
        pad: 0,
    };

    // SAFETY: Valid fd and drm_i915_gem_context_destroy struct.
    unsafe {
        drm_ioctl_i915_gem_context_destroy(fd, &ctx_destroy)?;
    }
    Ok(())
}

pub fn gem_execbuffer2(
    fd: BorrowedFd<'_>,
    context_id: u32,
    submit_info: &MagmaSubmitInfo,
) -> Result<()> {
    let buf_info = submit_info.buffer_info().ok_or(Error::Unimplemented)?;

    let mut obj = drm_i915_gem_exec_object2 {
        handle: buf_info.command_buffer,
        ..Default::default()
    };

    let mut exec = drm_i915_gem_execbuffer2 {
        buffers_ptr: &mut obj as *mut _ as u64,
        buffer_count: 1,
        batch_start_offset: buf_info.start_offset as u32,
        batch_len: buf_info.length as u32,
        flags: I915_EXEC_RENDER as u64,
        rsvd1: context_id as u64,
        ..Default::default()
    };

    // SAFETY: Valid fd and drm_i915_gem_execbuffer2 struct.
    unsafe {
        drm_ioctl_i915_gem_execbuffer2(fd, &mut exec)?;
    }
    Ok(())
}
