// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

use std::mem::size_of;
use std::os::fd::BorrowedFd;
use std::os::raw::c_void;

use magma_gpu::util::AsRawDescriptor;
use magma_gpu::util::FromRawDescriptor;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::MAGMA_GPU_HANDLE_TYPE_SIGNAL_SYNC_FD;

use crate::defines::MagmaSubmitInfo;
use crate::error::Error;
use crate::error::Result;
use crate::ioctl_readwrite;
use crate::ioctl_write_ptr;
use crate::sys::linux::bindings::kgsl_bindings::*;

const KGSL_IOC_TYPE: u32 = 0x09;

pub const VBO_SIZE_LADDER: [u64; 6] = [
    0x100_0000_0000, // 1 TiB
    0x40_0000_0000,  // 256 GiB
    0x20_0000_0000,  // 128 GiB
    0x10_0000_0000,  // 64 GiB
    0x4_0000_0000,   // 16 GiB
    0x1_0000_0000,   // 4 GiB
];

ioctl_readwrite!(
    kgsl_ioctl_device_getproperty,
    KGSL_IOC_TYPE,
    0x02,
    kgsl_device_getproperty
);

ioctl_readwrite!(
    kgsl_ioctl_drawctxt_create,
    KGSL_IOC_TYPE,
    0x13,
    kgsl_drawctxt_create
);

ioctl_write_ptr!(
    kgsl_ioctl_drawctxt_destroy,
    KGSL_IOC_TYPE,
    0x14,
    kgsl_drawctxt_destroy
);

ioctl_readwrite!(
    kgsl_ioctl_gpumem_alloc_id,
    KGSL_IOC_TYPE,
    0x34,
    kgsl_gpumem_alloc_id
);

ioctl_readwrite!(
    kgsl_ioctl_gpumem_free_id,
    KGSL_IOC_TYPE,
    0x35,
    kgsl_gpumem_free_id
);

ioctl_readwrite!(
    kgsl_ioctl_gpuobj_alloc,
    KGSL_IOC_TYPE,
    0x45,
    kgsl_gpuobj_alloc
);

ioctl_write_ptr!(
    kgsl_ioctl_gpuobj_free,
    KGSL_IOC_TYPE,
    0x46,
    kgsl_gpuobj_free
);

ioctl_readwrite!(
    kgsl_ioctl_gpuobj_info,
    KGSL_IOC_TYPE,
    0x47,
    kgsl_gpuobj_info
);

ioctl_readwrite!(
    kgsl_ioctl_gpuobj_import,
    KGSL_IOC_TYPE,
    0x48,
    kgsl_gpuobj_import
);

ioctl_readwrite!(
    kgsl_ioctl_gpu_command,
    KGSL_IOC_TYPE,
    0x4A,
    kgsl_gpu_command
);

ioctl_readwrite!(
    kgsl_ioctl_gpumem_bind_ranges,
    KGSL_IOC_TYPE,
    0x56,
    kgsl_gpumem_bind_ranges
);

ioctl_readwrite!(
    kgsl_ioctl_syncsource_create,
    KGSL_IOC_TYPE,
    0x40,
    kgsl_syncsource_create
);

ioctl_readwrite!(
    kgsl_ioctl_syncsource_destroy,
    KGSL_IOC_TYPE,
    0x41,
    kgsl_syncsource_destroy
);

ioctl_readwrite!(
    kgsl_ioctl_syncsource_create_fence,
    KGSL_IOC_TYPE,
    0x42,
    kgsl_syncsource_create_fence
);

ioctl_readwrite!(
    kgsl_ioctl_syncsource_signal_fence,
    KGSL_IOC_TYPE,
    0x43,
    kgsl_syncsource_signal_fence
);

pub fn get_devinfo(fd: BorrowedFd<'_>) -> Result<kgsl_devinfo> {
    let mut info: kgsl_devinfo = Default::default();
    let mut getprop = kgsl_device_getproperty {
        type_: KGSL_PROP_DEVICE_INFO,
        value: &mut info as *mut _ as *mut c_void,
        sizebytes: size_of::<kgsl_devinfo>() as _,
    };
    // SAFETY: fd is valid and info is properly aligned and sized.
    unsafe {
        kgsl_ioctl_device_getproperty(fd, &mut getprop)?;
    }
    Ok(info)
}

pub fn gpuobj_import_dmabuf(fd: BorrowedFd<'_>, handle: &MagmaGpuHandle) -> Result<u32> {
    let mut import_dmabuf = kgsl_gpuobj_import_dma_buf {
        fd: handle.os_handle.as_raw_descriptor(),
    };
    let mut req = kgsl_gpuobj_import {
        priv_: &mut import_dmabuf as *mut _ as u64,
        priv_len: size_of::<kgsl_gpuobj_import_dma_buf>() as u64,
        flags: 0,
        type_: KGSL_USER_MEM_TYPE_DMABUF,
        id: 0,
    };
    // SAFETY: fd is valid and req references valid import payload.
    unsafe {
        kgsl_ioctl_gpuobj_import(fd, &mut req)?;
    }
    Ok(req.id)
}

pub fn gpumem_free_id(fd: BorrowedFd<'_>, gem_handle: u32) -> Result<()> {
    let mut req = kgsl_gpumem_free_id {
        id: gem_handle,
        __pad: 0,
    };
    // SAFETY: fd is valid and req is properly initialized.
    unsafe {
        kgsl_ioctl_gpumem_free_id(fd, &mut req)?;
    }
    Ok(())
}

pub fn gpumem_alloc_id(fd: BorrowedFd<'_>, size: u64, is_cached: bool) -> Result<(u32, u64)> {
    let flags = if is_cached {
        (KGSL_CACHEMODE_WRITEBACK << KGSL_CACHEMODE_SHIFT) | KGSL_MEMFLAGS_IOCOHERENT
    } else {
        KGSL_CACHEMODE_WRITECOMBINE << KGSL_CACHEMODE_SHIFT
    };

    let mut req = kgsl_gpumem_alloc_id {
        size: size as _,
        flags,
        ..Default::default()
    };

    // SAFETY: Underlying descriptor is valid.
    unsafe {
        kgsl_ioctl_gpumem_alloc_id(fd, &mut req)?;
    }

    Ok((req.id, req.gpuaddr as _))
}

pub fn gpuobj_info_gpuaddr(fd: BorrowedFd<'_>, id: u32) -> u64 {
    let mut info_req = kgsl_gpuobj_info {
        id,
        ..Default::default()
    };
    // SAFETY: Underlying descriptor is valid.
    unsafe {
        if kgsl_ioctl_gpuobj_info(fd, &mut info_req).is_ok() {
            info_req.gpuaddr
        } else {
            0
        }
    }
}

pub fn vbo_alloc(fd: BorrowedFd<'_>) -> (u32, u64, u64) {
    let mut vbo_id = 0;
    let mut vbo_base = 0;
    let mut vbo_size = 0;

    for &size in &VBO_SIZE_LADDER {
        let mut req = kgsl_gpuobj_alloc {
            size,
            flags: KGSL_MEMFLAGS_VBO | KGSL_MEMFLAGS_VBO_NO_MAP_ZERO,
            ..Default::default()
        };

        // SAFETY: fd is valid and req is properly initialized.
        if unsafe { kgsl_ioctl_gpuobj_alloc(fd, &mut req).is_ok() } {
            let mut info = kgsl_gpuobj_info {
                id: req.id,
                ..Default::default()
            };
            // SAFETY: fd is valid and info is properly initialized.
            if unsafe { kgsl_ioctl_gpuobj_info(fd, &mut info).is_ok() } {
                vbo_id = req.id;
                vbo_base = info.gpuaddr;
                vbo_size = size;
                break;
            } else {
                let _ = gpumem_free_id(fd, req.id);
            }
        }
    }

    (vbo_id, vbo_base, vbo_size)
}

pub fn gpumem_bind_range(
    fd: BorrowedFd<'_>,
    vbo_id: u32,
    child_id: u32,
    child_offset: u64,
    target_offset: u64,
    length: u64,
) -> Result<()> {
    let range = kgsl_gpumem_bind_range {
        child_offset,
        target_offset,
        length,
        child_id,
        op: KGSL_GPUMEM_RANGE_OP_BIND,
    };

    let mut req = kgsl_gpumem_bind_ranges {
        ranges: &range as *const _ as u64,
        ranges_nents: 1,
        ranges_size: size_of::<kgsl_gpumem_bind_range>() as u32,
        id: vbo_id,
        flags: 0,
        fence_id: 0,
        padding: 0,
    };

    // SAFETY: Underlying descriptor is valid and req references valid range struct.
    unsafe {
        kgsl_ioctl_gpumem_bind_ranges(fd, &mut req)?;
    }
    Ok(())
}

pub fn gpumem_unbind_range(
    fd: BorrowedFd<'_>,
    vbo_id: u32,
    target_offset: u64,
    length: u64,
) -> Result<()> {
    let range = kgsl_gpumem_bind_range {
        child_offset: 0,
        target_offset,
        length,
        child_id: 0,
        op: KGSL_GPUMEM_RANGE_OP_UNBIND,
    };

    let mut req = kgsl_gpumem_bind_ranges {
        ranges: &range as *const _ as u64,
        ranges_nents: 1,
        ranges_size: size_of::<kgsl_gpumem_bind_range>() as u32,
        id: vbo_id,
        flags: 0,
        fence_id: 0,
        padding: 0,
    };

    // SAFETY: Underlying descriptor is valid and req references valid range struct.
    unsafe {
        kgsl_ioctl_gpumem_bind_ranges(fd, &mut req)?;
    }
    Ok(())
}

pub fn drawctxt_create(fd: BorrowedFd<'_>) -> Result<u32> {
    let mut create_req = kgsl_drawctxt_create {
        flags: KGSL_CONTEXT_SAVE_GMEM | KGSL_CONTEXT_NO_GMEM_ALLOC | KGSL_CONTEXT_PREAMBLE,
        drawctxt_id: 0,
    };
    // SAFETY: Underlying descriptor is valid.
    unsafe {
        kgsl_ioctl_drawctxt_create(fd, &mut create_req)?;
    }
    Ok(create_req.drawctxt_id)
}

pub fn drawctxt_destroy(fd: BorrowedFd<'_>, context_id: u32) {
    let destroy_req = kgsl_drawctxt_destroy {
        drawctxt_id: context_id,
    };
    // SAFETY: Descriptor is valid and context_id corresponds to a created drawctxt.
    unsafe {
        let _ = kgsl_ioctl_drawctxt_destroy(fd, &destroy_req);
    }
}

pub fn gpu_command_submit(
    fd: BorrowedFd<'_>,
    context_id: u32,
    submit_info: &MagmaSubmitInfo,
) -> Result<()> {
    let (cmd_id, cmd_gpuaddr, cmd_offset, cmd_size) =
        if let Some(as_info) = submit_info.address_space_info() {
            (0, as_info.command_va, 0, as_info.length)
        } else if let Some(buf_info) = submit_info.buffer_info() {
            (
                buf_info.command_buffer,
                0,
                buf_info.start_offset,
                buf_info.length,
            )
        } else {
            return Err(Error::Unimplemented);
        };

    let cmd_obj = kgsl_command_object {
        offset: cmd_offset,
        gpuaddr: cmd_gpuaddr,
        size: cmd_size,
        flags: KGSL_CMDLIST_IB,
        id: cmd_id,
    };

    let mut sync_fences: Vec<kgsl_cmd_syncpoint_fence> = Vec::new();
    let mut syncpoints: Vec<kgsl_command_syncpoint> = Vec::new();

    for sync_obj in submit_info.sync_info.wait_sync_objs() {
        if let Some(handle) = sync_obj.as_raw_handle() {
            sync_fences.push(kgsl_cmd_syncpoint_fence { fd: handle as i32 });
        }
    }

    for fence in &sync_fences {
        syncpoints.push(kgsl_command_syncpoint {
            priv_: fence as *const _ as u64,
            size: size_of::<kgsl_cmd_syncpoint_fence>() as u64,
            type_: KGSL_CMD_SYNCPOINT_TYPE_FENCE,
        });
    }

    let mut req = kgsl_gpu_command {
        flags: KGSL_CMDBATCH_SUBMIT_IB_LIST as u64,
        cmdlist: &cmd_obj as *const _ as u64,
        cmdsize: size_of::<kgsl_command_object>() as u32,
        numcmds: 1,
        synclist: if syncpoints.is_empty() {
            0
        } else {
            syncpoints.as_ptr() as u64
        },
        syncsize: size_of::<kgsl_command_syncpoint>() as u32,
        numsyncs: syncpoints.len() as u32,
        context_id,
        ..Default::default()
    };

    // SAFETY: Descriptor is valid and req references valid command object.
    unsafe {
        kgsl_ioctl_gpu_command(fd, &mut req)?;
    }
    Ok(())
}

pub fn syncsource_create_with_fence(fd: BorrowedFd<'_>) -> Result<(u32, i32)> {
    let mut create = kgsl_syncsource_create {
        id: 0,
        ..Default::default()
    };
    // SAFETY: Valid fd and struct.
    unsafe {
        kgsl_ioctl_syncsource_create(fd, &mut create)?;
    }
    let mut create_fence = kgsl_syncsource_create_fence {
        id: create.id,
        fence_fd: -1,
        ..Default::default()
    };
    // SAFETY: Valid fd and struct.
    unsafe {
        kgsl_ioctl_syncsource_create_fence(fd, &mut create_fence)?;
    }
    Ok((create.id, create_fence.fence_fd))
}

pub fn syncsource_destroy(fd: BorrowedFd<'_>, syncsource_id: u32) {
    let mut destroy = kgsl_syncsource_destroy {
        id: syncsource_id,
        ..Default::default()
    };
    // SAFETY: Valid fd and struct.
    unsafe {
        let _ = kgsl_ioctl_syncsource_destroy(fd, &mut destroy);
    }
}

pub fn syncsource_signal_fence(
    fd: BorrowedFd<'_>,
    syncsource_id: u32,
    fence_fd: i32,
) -> Result<()> {
    let mut sig = kgsl_syncsource_signal_fence {
        id: syncsource_id,
        fence_fd,
        ..Default::default()
    };
    // SAFETY: Valid fd and struct.
    unsafe {
        kgsl_ioctl_syncsource_signal_fence(fd, &mut sig)?;
    }
    Ok(())
}

pub fn close_fence_fd(fence_fd: i32) {
    if fence_fd >= 0 {
        // SAFETY: Closing an owned raw fd.
        unsafe {
            libc::close(fence_fd);
        }
    }
}

pub fn dup_fence_fd(raw_fd: i32) -> Result<i32> {
    // SAFETY: Duplicating a valid file descriptor.
    let dup_fd = unsafe { libc::dup(raw_fd) };
    if dup_fd < 0 {
        Err(Error::InternalError)
    } else {
        Ok(dup_fd)
    }
}

pub fn export_fence_fd(fence_fd: i32) -> Result<MagmaGpuHandle> {
    if fence_fd < 0 {
        return Err(Error::Unimplemented);
    }
    let dup_fd = dup_fence_fd(fence_fd)?;
    // SAFETY: dup_fd is a newly duplicated owned file descriptor.
    let descriptor = unsafe { OwnedDescriptor::from_raw_descriptor(dup_fd) };
    Ok(MagmaGpuHandle {
        os_handle: descriptor,
        handle_type: MAGMA_GPU_HANDLE_TYPE_SIGNAL_SYNC_FD,
    })
}

pub fn poll_fence_fd(fence_fd: i32, timeout_ns: u64) -> Result<()> {
    if fence_fd < 0 {
        return Ok(());
    }
    let timeout_ms = if timeout_ns == u64::MAX {
        -1
    } else {
        (timeout_ns / 1_000_000).min(i32::MAX as u64) as i32
    };
    let mut pfd = libc::pollfd {
        fd: fence_fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: Valid pollfd struct and fd.
    let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if ret < 0 {
        Err(Error::InternalError)
    } else if ret == 0 {
        Err(Error::TimedOut)
    } else {
        Ok(())
    }
}
