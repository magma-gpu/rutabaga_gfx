// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::os::fd::BorrowedFd;

use crate::error::Result;
use crate::ioctl_readwrite;
use crate::ioctl_write_ptr;
use crate::sys::linux::bindings::amdgpu_bindings::*;
use crate::sys::linux::bindings::drm_bindings::DRM_COMMAND_BASE;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;

ioctl_readwrite!(
    drm_ioctl_amdgpu_cs,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_AMDGPU_CS,
    drm_amdgpu_cs
);

ioctl_readwrite!(
    drm_ioctl_amdgpu_ctx,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_AMDGPU_CTX,
    drm_amdgpu_ctx
);

ioctl_readwrite!(
    drm_ioctl_amdgpu_gem_va,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_AMDGPU_GEM_VA,
    drm_amdgpu_gem_va
);

ioctl_write_ptr!(
    drm_ioctl_amdgpu_info,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_AMDGPU_INFO,
    drm_amdgpu_info
);

ioctl_readwrite!(
    drm_ioctl_amdgpu_gem_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_AMDGPU_GEM_CREATE,
    drm_amdgpu_gem_create
);

ioctl_readwrite!(
    drm_ioctl_amdgpu_gem_mmap,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_AMDGPU_GEM_MMAP,
    drm_amdgpu_gem_mmap
);

macro_rules! amdgpu_info_ioctl {
    ($(#[$attr:meta])* $name:ident, $nr:expr, $ty:ty) => (
        $(#[$attr])*
        pub fn $name(fd: BorrowedFd<'_>) -> Result<$ty> {
            let mut data: $ty = Default::default();
            let info = drm_amdgpu_info {
                return_pointer: &mut data as *mut $ty as __u64,
                return_size: size_of::<$ty>() as u32,
                query: $nr,
                ..Default::default()
            };
            // SAFETY: `fd` is valid and `data` is stack-allocated with matching `return_size`.
            unsafe {
                drm_ioctl_amdgpu_info(fd, &info)?;
            }
            Ok(data)
        }
    )
}

amdgpu_info_ioctl!(
    amdgpu_info_memory,
    AMDGPU_INFO_MEMORY,
    drm_amdgpu_memory_info
);

amdgpu_info_ioctl!(
    amdgpu_info_vram_gtt,
    AMDGPU_INFO_VRAM_GTT,
    drm_amdgpu_info_vram_gtt
);

amdgpu_info_ioctl!(amdgpu_info_gtt_usage, AMDGPU_INFO_GTT_USAGE, u64);

amdgpu_info_ioctl!(amdgpu_info_vram_usage, AMDGPU_INFO_VRAM_USAGE, u64);

amdgpu_info_ioctl!(amdgpu_info_vis_vram_usage, AMDGPU_INFO_VIS_VRAM_USAGE, u64);

pub fn amdgpu_info_hw_ip(
    fd: BorrowedFd<'_>,
    type_: u32,
    ip_instance: u32,
) -> Result<drm_amdgpu_info_hw_ip> {
    let mut data: drm_amdgpu_info_hw_ip = Default::default();
    let mut info = drm_amdgpu_info {
        return_pointer: &mut data as *mut drm_amdgpu_info_hw_ip as __u64,
        return_size: size_of::<drm_amdgpu_info_hw_ip>() as u32,
        query: AMDGPU_INFO_HW_IP_INFO,
        ..Default::default()
    };
    info.__bindgen_anon_1.query_hw_ip.type_ = type_;
    info.__bindgen_anon_1.query_hw_ip.ip_instance = ip_instance;
    // SAFETY: `fd` is valid and `data` is stack-allocated with matching `return_size`.
    unsafe {
        drm_ioctl_amdgpu_info(fd, &info)?;
    }
    Ok(data)
}

pub fn amdgpu_gem_create(
    fd: BorrowedFd<'_>,
    bo_size: u64,
    alignment: u64,
    domains: u64,
    domain_flags: u64,
) -> Result<u32> {
    let mut gem_create = drm_amdgpu_gem_create::default();
    gem_create.in_ = drm_amdgpu_gem_create_in {
        bo_size,
        alignment,
        domains,
        domain_flags,
    };
    // SAFETY: `fd` is valid and `gem_create` is stack-allocated.
    unsafe {
        drm_ioctl_amdgpu_gem_create(fd, &mut gem_create)?;
        Ok(gem_create.out.handle)
    }
}

pub fn amdgpu_gem_mmap(fd: BorrowedFd<'_>, gem_handle: u32) -> Result<u64> {
    let mut gem_mmap = drm_amdgpu_gem_mmap::default();
    gem_mmap.in_.handle = gem_handle;
    // SAFETY: `fd` is valid and `gem_mmap` is stack-allocated.
    unsafe {
        drm_ioctl_amdgpu_gem_mmap(fd, &mut gem_mmap)?;
        Ok(gem_mmap.out.addr_ptr)
    }
}

pub fn amdgpu_gem_va(
    fd: BorrowedFd<'_>,
    handle: u32,
    operation: u32,
    flags: u32,
    va_address: u64,
    offset_in_bo: u64,
    map_size: u64,
) -> Result<()> {
    let mut va_arg = drm_amdgpu_gem_va {
        handle,
        operation,
        flags,
        va_address,
        offset_in_bo,
        map_size,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `va_arg` is stack-allocated.
    unsafe {
        drm_ioctl_amdgpu_gem_va(fd, &mut va_arg)?;
    }
    Ok(())
}

pub fn amdgpu_ctx_alloc(fd: BorrowedFd<'_>) -> Result<u32> {
    let mut ctx_arg = drm_amdgpu_ctx::default();
    ctx_arg.in_.op = AMDGPU_CTX_OP_ALLOC_CTX;
    // SAFETY: `fd` is valid and `ctx_arg` is stack-allocated.
    unsafe {
        drm_ioctl_amdgpu_ctx(fd, &mut ctx_arg)?;
        Ok(ctx_arg.out.alloc.ctx_id)
    }
}

pub fn amdgpu_ctx_free(fd: BorrowedFd<'_>, ctx_id: u32) -> Result<()> {
    let mut ctx_arg = drm_amdgpu_ctx::default();
    ctx_arg.in_.op = AMDGPU_CTX_OP_FREE_CTX;
    ctx_arg.in_.ctx_id = ctx_id;
    // SAFETY: `fd` is valid and `ctx_arg` is stack-allocated.
    unsafe {
        drm_ioctl_amdgpu_ctx(fd, &mut ctx_arg)?;
    }
    Ok(())
}

pub fn amdgpu_cs_submit(
    fd: BorrowedFd<'_>,
    ctx_id: u32,
    chunks: &[drm_amdgpu_cs_chunk],
) -> Result<()> {
    let chunk_ptrs: Vec<u64> = chunks.iter().map(|c| c as *const _ as u64).collect();
    let mut cs = drm_amdgpu_cs::default();
    cs.in_.ctx_id = ctx_id;
    cs.in_.num_chunks = chunks.len() as u32;
    cs.in_.chunks = chunk_ptrs.as_ptr() as u64;
    // SAFETY: `fd` is valid and `chunks` / `chunk_ptrs` outlive the ioctl call.
    unsafe {
        drm_ioctl_amdgpu_cs(fd, &mut cs)?;
    }
    Ok(())
}
