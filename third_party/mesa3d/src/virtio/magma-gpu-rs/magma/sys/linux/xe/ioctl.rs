// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::cmp::min;
use std::mem::size_of;
use std::os::fd::BorrowedFd;

use zerocopy::FromZeros;
use zerocopy::KnownLayout;

use crate::defines::MagmaHeap;
use crate::defines::MagmaMemoryType;
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
use crate::sys::linux::bindings::xe_bindings::*;

const GEN12_IDS: [u16; 50] = [
    0x4c8a, 0x4c8b, 0x4c8c, 0x4c90, 0x4c9a, 0x4680, 0x4681, 0x4682, 0x4683, 0x4688, 0x4689, 0x4690,
    0x4691, 0x4692, 0x4693, 0x4698, 0x4699, 0x4626, 0x4628, 0x462a, 0x46a0, 0x46a1, 0x46a2, 0x46a3,
    0x46a6, 0x46a8, 0x46aa, 0x46b0, 0x46b1, 0x46b2, 0x46b3, 0x46c0, 0x46c1, 0x46c2, 0x46c3, 0x9A40,
    0x9A49, 0x9A59, 0x9A60, 0x9A68, 0x9A70, 0x9A78, 0x9AC0, 0x9AC9, 0x9AD9, 0x9AF8, 0x4905, 0x4906,
    0x4907, 0x4908,
];

const ADLP_IDS: [u16; 23] = [
    0x46A0, 0x46A1, 0x46A2, 0x46A3, 0x46A6, 0x46A8, 0x46AA, 0x462A, 0x4626, 0x4628, 0x46B0, 0x46B1,
    0x46B2, 0x46B3, 0x46C0, 0x46C1, 0x46C2, 0x46C3, 0x46D0, 0x46D1, 0x46D2, 0x46D3, 0x46D4,
];

const RPLP_IDS: [u16; 10] = [
    0xA720, 0xA721, 0xA7A0, 0xA7A1, 0xA7A8, 0xA7A9, 0xA7AA, 0xA7AB, 0xA7AC, 0xA7AD,
];

const MTL_IDS: [u16; 5] = [0x7D40, 0x7D60, 0x7D45, 0x7D55, 0x7DD5];

const LNL_IDS: [u16; 3] = [0x6420, 0x64A0, 0x64B0];

const PTL_IDS: [u16; 8] = [
    0xB080, 0xB081, 0xB082, 0xB083, 0xB08F, 0xB090, 0xB0A0, 0xB0B0,
];

const DG2_IDS: [u16; 26] = [
    0x5690, 0x5691, 0x5692, 0x5693, 0x5694, 0x5695, 0x5696, 0x5697, 0x56a0, 0x56a1, 0x56a2, 0x56a3,
    0x56a4, 0x56a5, 0x56a6, 0x56b0, 0x56b1, 0x56b2, 0x56b3, 0x56ba, 0x56bb, 0x56bc, 0x56bd, 0x56be,
    0x56bf, 0x56c0,
];

#[derive(Default)]
pub struct XeMemoryInfo {
    pub sysmem_size: u64,
    pub vram_size: u64,
    pub sysmem_used: u64,
    pub vram_used: u64,
    pub vram_cpu_visible_size: u64,
    pub vram_cpu_visible_used: u64,
    pub sysmem_instance: u16,
    pub vram_instance: u16,
}

ioctl_readwrite!(
    drm_ioctl_xe_device_query,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_DEVICE_QUERY,
    drm_xe_device_query
);

ioctl_readwrite!(
    drm_ioctl_xe_gem_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_GEM_CREATE,
    drm_xe_gem_create
);

ioctl_readwrite!(
    drm_ioctl_xe_gem_mmap_offset,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_GEM_MMAP_OFFSET,
    drm_xe_gem_mmap_offset
);

ioctl_readwrite!(
    drm_ioctl_xe_vm_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_VM_CREATE,
    drm_xe_vm_create
);

ioctl_write_ptr!(
    drm_ioctl_xe_vm_destroy,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_VM_DESTROY,
    drm_xe_vm_destroy
);

ioctl_readwrite!(
    drm_ioctl_xe_vm_bind,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_VM_BIND,
    drm_xe_vm_bind
);

ioctl_readwrite!(
    drm_ioctl_xe_exec_queue_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_EXEC_QUEUE_CREATE,
    drm_xe_exec_queue_create
);

ioctl_write_ptr!(
    drm_ioctl_xe_exec_queue_destroy,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_EXEC_QUEUE_DESTROY,
    drm_xe_exec_queue_destroy
);

ioctl_write_ptr!(
    drm_ioctl_xe_exec,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_EXEC,
    drm_xe_exec
);

ioctl_readwrite!(
    drm_ioctl_xe_exec_queue_get_property,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_XE_EXEC_QUEUE_GET_PROPERTY,
    drm_xe_exec_queue_get_property
);

/// Determines the graphics version of the Intel device based on its PCI ID.
pub fn determine_graphics_version(pci_device_id: u16) -> Result<u32> {
    let mut graphics_version = 0;
    if ADLP_IDS.contains(&pci_device_id)
        || RPLP_IDS.contains(&pci_device_id)
        || MTL_IDS.contains(&pci_device_id)
        || GEN12_IDS.contains(&pci_device_id)
        || DG2_IDS.contains(&pci_device_id)
    {
        graphics_version = 12;
    }

    if LNL_IDS.contains(&pci_device_id) || PTL_IDS.contains(&pci_device_id) {
        graphics_version = 20;
    }

    if graphics_version != 0 {
        Ok(graphics_version)
    } else {
        Ok(12)
    }
}

pub fn xe_device_query<T, S, H>(fd: BorrowedFd<'_>, query_id: u32) -> Result<Box<T>>
where
    T: ?Sized + FromZeros + KnownLayout<PointerMetadata = usize>,
{
    let mut device_query = drm_xe_device_query {
        query: query_id,
        ..Default::default()
    };

    // SAFETY: `fd` is a valid DRM descriptor and `device_query` is stack-allocated.
    unsafe {
        drm_ioctl_xe_device_query(fd, &mut device_query)?;
    }

    let total_size = device_query.size as usize;
    let header_size = size_of::<H>();
    let element_size = size_of::<S>();
    let count = total_size
        .saturating_sub(header_size)
        .div_ceil(element_size);

    let mut query_data = T::new_box_zeroed_with_elems(count).map_err(|_| Error::MemoryError)?;

    device_query.size = total_size as u32;
    device_query.data = &mut *query_data as *mut T as *mut () as u64;

    // SAFETY: `query_data` is sized to hold `total_size` bytes returned by the initial query.
    unsafe {
        drm_ioctl_xe_device_query(fd, &mut device_query)?;
    }

    Ok(query_data)
}

pub fn xe_query_memory_regions(fd: BorrowedFd<'_>) -> Result<XeMemoryInfo> {
    let mut memory_info: XeMemoryInfo = Default::default();

    let query_mem_regions = xe_device_query::<
        drm_xe_query_mem_regions<[drm_xe_mem_region]>,
        drm_xe_mem_region,
        drm_xe_query_mem_regions<[drm_xe_mem_region; 0]>,
    >(fd, DRM_XE_DEVICE_QUERY_MEM_REGIONS)?;

    let num_regions = min(
        query_mem_regions.num_mem_regions as usize,
        query_mem_regions.mem_regions.len(),
    );
    let mem_regions = &query_mem_regions.mem_regions[..num_regions];
    for region in mem_regions {
        match region.mem_class as u32 {
            DRM_XE_MEM_REGION_CLASS_SYSMEM => {
                if memory_info.sysmem_size != 0 {
                    return Err(Error::InternalError);
                }
                memory_info.sysmem_size = region.total_size;
                memory_info.sysmem_used = region.used;
                memory_info.sysmem_instance = region.instance;
            }
            DRM_XE_MEM_REGION_CLASS_VRAM => {
                if memory_info.vram_size != 0 || memory_info.vram_cpu_visible_size != 0 {
                    return Err(Error::InternalError);
                }
                memory_info.vram_cpu_visible_size = region.cpu_visible_size;
                memory_info.vram_size = region.total_size - region.cpu_visible_size;
                memory_info.vram_cpu_visible_used = region.cpu_visible_used;
                memory_info.vram_used = region.used - region.cpu_visible_used;
                memory_info.vram_instance = region.instance;
            }
            _ => return Err(Error::Unimplemented),
        }
    }

    Ok(memory_info)
}

pub fn xe_query_memory(fd: BorrowedFd<'_>) -> Result<(Vec<MagmaMemoryType>, Vec<MagmaHeap>)> {
    let mut heaps = Vec::new();
    let mut types = Vec::new();
    let memory_info = xe_query_memory_regions(fd)?;
    let mut heap_idx = 0u32;
    if memory_info.sysmem_size != 0 {
        heaps.push(MagmaHeap {
            heap_size: memory_info.sysmem_size,
            heap_flags: MAGMA_HEAP_CPU_VISIBLE_BIT,
        });
        types.push(MagmaMemoryType {
            property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
            heap_idx,
        });
        heap_idx += 1;
    }

    if memory_info.vram_cpu_visible_size != 0 {
        heaps.push(MagmaHeap {
            heap_size: memory_info.vram_cpu_visible_size,
            heap_flags: MAGMA_HEAP_CPU_VISIBLE_BIT | MAGMA_HEAP_DEVICE_LOCAL_BIT,
        });
        types.push(MagmaMemoryType {
            property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT,
            heap_idx,
        });
        heap_idx += 1;
    }

    if memory_info.vram_size != 0 {
        heaps.push(MagmaHeap {
            heap_size: memory_info.vram_size,
            heap_flags: MAGMA_HEAP_DEVICE_LOCAL_BIT,
        });
        types.push(MagmaMemoryType {
            property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
            heap_idx,
        });
    }

    Ok((types, heaps))
}

pub fn xe_gem_create(
    fd: BorrowedFd<'_>,
    size: u64,
    flags: u32,
    cpu_caching: u16,
    placement: u32,
    is_protected: bool,
) -> Result<u32> {
    let mut gem_create = drm_xe_gem_create {
        size,
        flags,
        cpu_caching,
        placement,
        ..Default::default()
    };
    let mut pxp_ext: drm_xe_ext_set_property = Default::default();
    if is_protected {
        pxp_ext.base.name = DRM_XE_GEM_CREATE_EXTENSION_SET_PROPERTY;
        pxp_ext.property = DRM_XE_GEM_CREATE_SET_PROPERTY_PXP_TYPE;
        pxp_ext.__bindgen_anon_1.value = DRM_XE_PXP_TYPE_HWDRM as u64;
        gem_create.extensions = &pxp_ext as *const drm_xe_ext_set_property as u64;
    }
    // SAFETY: `fd` is valid and `gem_create` / `pxp_ext` outlive the ioctl call.
    unsafe {
        drm_ioctl_xe_gem_create(fd, &mut gem_create)?;
    }
    Ok(gem_create.handle)
}

pub fn xe_gem_mmap_offset(fd: BorrowedFd<'_>, gem_handle: u32) -> Result<u64> {
    let mut xe_offset = drm_xe_gem_mmap_offset {
        handle: gem_handle,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `xe_offset` is stack-allocated.
    unsafe {
        drm_ioctl_xe_gem_mmap_offset(fd, &mut xe_offset)?;
    }
    Ok(xe_offset.offset)
}

pub fn xe_vm_create(fd: BorrowedFd<'_>, flags: u32) -> Result<u32> {
    let mut vm_create = drm_xe_vm_create {
        flags,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `vm_create` is stack-allocated.
    unsafe {
        drm_ioctl_xe_vm_create(fd, &mut vm_create)?;
    }
    Ok(vm_create.vm_id)
}

pub fn xe_vm_destroy(fd: BorrowedFd<'_>, vm_id: u32) -> Result<()> {
    let destroy = drm_xe_vm_destroy {
        vm_id,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `destroy` is stack-allocated.
    unsafe {
        drm_ioctl_xe_vm_destroy(fd, &destroy)?;
    }
    Ok(())
}

pub fn xe_vm_bind_single(
    fd: BorrowedFd<'_>,
    vm_id: u32,
    op: u32,
    obj: u32,
    obj_offset: u64,
    addr: u64,
    range: u64,
) -> Result<()> {
    let mut bind_op = drm_xe_vm_bind_op {
        op,
        flags: 0,
        addr,
        range,
        obj,
        ..Default::default()
    };
    bind_op.__bindgen_anon_1.obj_offset = obj_offset;

    let mut vm_bind = drm_xe_vm_bind {
        vm_id,
        num_binds: 1,
        ..Default::default()
    };
    vm_bind.__bindgen_anon_1.bind = bind_op;

    // SAFETY: `fd` is valid and `vm_bind` is stack-allocated.
    unsafe {
        drm_ioctl_xe_vm_bind(fd, &mut vm_bind)?;
    }
    Ok(())
}

pub fn xe_vm_bind_syncs(
    fd: BorrowedFd<'_>,
    vm_id: u32,
    exec_queue_id: u32,
    syncs: &[drm_xe_sync],
) -> Result<()> {
    let mut vm_bind = drm_xe_vm_bind {
        vm_id,
        exec_queue_id,
        num_binds: 0,
        num_syncs: syncs.len() as u32,
        syncs: syncs.as_ptr() as u64,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `syncs` slice remains valid for the duration of the ioctl.
    unsafe {
        drm_ioctl_xe_vm_bind(fd, &mut vm_bind)?;
    }
    Ok(())
}

pub fn xe_exec_queue_create(fd: BorrowedFd<'_>, vm_id: u32, engine_class: u16) -> Result<u32> {
    let instance = drm_xe_engine_class_instance {
        engine_class,
        engine_instance: 0,
        gt_id: 0,
        pad: 0,
    };
    let mut create = drm_xe_exec_queue_create {
        instances: &instance as *const _ as u64,
        width: 1,
        num_placements: 1,
        vm_id,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `instance` outlives the ioctl call.
    unsafe {
        drm_ioctl_xe_exec_queue_create(fd, &mut create)?;
    }
    Ok(create.exec_queue_id)
}

pub fn xe_exec_queue_destroy(fd: BorrowedFd<'_>, exec_queue_id: u32) -> Result<()> {
    let destroy = drm_xe_exec_queue_destroy {
        exec_queue_id,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `destroy` is stack-allocated.
    unsafe {
        drm_ioctl_xe_exec_queue_destroy(fd, &destroy)?;
    }
    Ok(())
}

pub fn xe_exec(
    fd: BorrowedFd<'_>,
    exec_queue_id: u32,
    num_batch_buffer: u16,
    address: u64,
    syncs: &[drm_xe_sync],
) -> Result<()> {
    let exec = drm_xe_exec {
        exec_queue_id,
        num_batch_buffer,
        address,
        num_syncs: syncs.len() as u32,
        syncs: syncs.as_ptr() as u64,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `syncs` slice remains valid for the duration of the ioctl.
    unsafe {
        drm_ioctl_xe_exec(fd, &exec)?;
    }
    Ok(())
}

pub fn xe_exec_queue_get_property(
    fd: BorrowedFd<'_>,
    exec_queue_id: u32,
    property: u32,
) -> Result<u64> {
    let mut prop = drm_xe_exec_queue_get_property {
        extensions: 0,
        exec_queue_id,
        property,
        value: 0,
        reserved: [0; 2],
    };
    // SAFETY: `fd` is valid and `prop` is stack-allocated.
    unsafe {
        drm_ioctl_xe_exec_queue_get_property(fd, &mut prop)?;
    }
    Ok(prop.value)
}
