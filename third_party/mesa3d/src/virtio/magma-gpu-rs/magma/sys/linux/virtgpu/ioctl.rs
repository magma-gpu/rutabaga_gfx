// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::os::fd::BorrowedFd;

use magma_gpu::util::AsRawDescriptor;
use magma_gpu::util::FromRawDescriptor;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::MAGMA_GPU_HANDLE_TYPE_SIGNAL_SYNC_FD;

use crate::error::Error;
use crate::error::Result;
use crate::ioctl_readwrite;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_array;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_create;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_destroy;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_handle;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_wait;
use crate::sys::linux::bindings::drm_bindings::DRM_COMMAND_BASE;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;
use crate::sys::linux::bindings::virtgpu_bindings::drm_virtgpu_context_init;
use crate::sys::linux::bindings::virtgpu_bindings::drm_virtgpu_context_set_param;
use crate::sys::linux::bindings::virtgpu_bindings::drm_virtgpu_execbuffer;
use crate::sys::linux::bindings::virtgpu_bindings::drm_virtgpu_execbuffer_syncobj;
use crate::sys::linux::bindings::virtgpu_bindings::drm_virtgpu_get_caps;
use crate::sys::linux::bindings::virtgpu_bindings::drm_virtgpu_getparam;
use crate::sys::linux::bindings::virtgpu_bindings::drm_virtgpu_map;
use crate::sys::linux::bindings::virtgpu_bindings::drm_virtgpu_resource_create_blob;
use crate::sys::linux::bindings::virtgpu_bindings::DRM_VIRTGPU_CONTEXT_INIT;
use crate::sys::linux::bindings::virtgpu_bindings::DRM_VIRTGPU_EXECBUFFER;
use crate::sys::linux::bindings::virtgpu_bindings::DRM_VIRTGPU_GETPARAM;
use crate::sys::linux::bindings::virtgpu_bindings::DRM_VIRTGPU_GET_CAPS;
use crate::sys::linux::bindings::virtgpu_bindings::DRM_VIRTGPU_MAP;
use crate::sys::linux::bindings::virtgpu_bindings::DRM_VIRTGPU_RESOURCE_CREATE_BLOB;
use crate::sys::linux::bindings::virtgpu_bindings::VIRTGPU_BLOB_FLAG_USE_MAPPABLE;
use crate::sys::linux::bindings::virtgpu_bindings::VIRTGPU_BLOB_FLAG_USE_SHAREABLE;
use crate::sys::linux::bindings::virtgpu_bindings::VIRTGPU_BLOB_MEM_HOST3D;
use crate::sys::linux::bindings::virtgpu_bindings::VIRTGPU_CONTEXT_PARAM_CAPSET_ID;
use crate::sys::linux::bindings::virtgpu_bindings::VIRTGPU_CONTEXT_PARAM_NUM_RINGS;
use crate::sys::linux::bindings::virtgpu_bindings::VIRTGPU_EXECBUF_RING_IDX;
use crate::sys::linux::drm::drm_ioctl_syncobj_create;
use crate::sys::linux::drm::drm_ioctl_syncobj_destroy;
use crate::sys::linux::drm::drm_ioctl_syncobj_fd_to_handle;
use crate::sys::linux::drm::drm_ioctl_syncobj_handle_to_fd;
use crate::sys::linux::drm::drm_ioctl_syncobj_signal;
use crate::sys::linux::drm::drm_ioctl_syncobj_wait;
use crate::sys::linux::drm::DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE;
use crate::sys::linux::drm::DRM_SYNCOBJ_WAIT_FLAGS_WAIT_ALL;
use crate::sys::linux::drm::DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT;

ioctl_readwrite!(
    drm_ioctl_virtgpu_map,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_VIRTGPU_MAP,
    drm_virtgpu_map
);

ioctl_readwrite!(
    drm_ioctl_virtgpu_execbuffer,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_VIRTGPU_EXECBUFFER,
    drm_virtgpu_execbuffer
);

ioctl_readwrite!(
    drm_ioctl_virtgpu_getparam,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_VIRTGPU_GETPARAM,
    drm_virtgpu_getparam
);

ioctl_readwrite!(
    drm_ioctl_virtgpu_get_caps,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_VIRTGPU_GET_CAPS,
    drm_virtgpu_get_caps
);

ioctl_readwrite!(
    drm_ioctl_virtgpu_resource_create_blob,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_VIRTGPU_RESOURCE_CREATE_BLOB,
    drm_virtgpu_resource_create_blob
);

ioctl_readwrite!(
    drm_ioctl_virtgpu_context_init,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_VIRTGPU_CONTEXT_INIT,
    drm_virtgpu_context_init
);

pub fn virtgpu_getparam(fd: BorrowedFd<'_>, param: u64, value: &mut u64) {
    let mut getparam = drm_virtgpu_getparam {
        param,
        value: value as *mut u64 as u64,
    };
    // SAFETY: `fd` is valid and `value` points to a stack-allocated `u64`.
    unsafe {
        let _ = drm_ioctl_virtgpu_getparam(fd, &mut getparam);
    }
}

pub fn virtgpu_get_caps(fd: BorrowedFd<'_>, cap_set_id: u32, caps_bytes: &mut [u8]) -> Result<()> {
    let mut get_caps = drm_virtgpu_get_caps {
        cap_set_id,
        cap_set_ver: 0,
        addr: caps_bytes.as_mut_ptr() as u64,
        size: caps_bytes.len() as u32,
        pad: 0,
    };
    // SAFETY: `fd` is valid and `caps_bytes` has length `size`.
    unsafe {
        drm_ioctl_virtgpu_get_caps(fd, &mut get_caps)?;
    }
    Ok(())
}

pub fn virtgpu_context_init(fd: BorrowedFd<'_>, capset_id: u32, num_rings: u64) -> Result<()> {
    let ctx_params = [
        drm_virtgpu_context_set_param {
            param: VIRTGPU_CONTEXT_PARAM_CAPSET_ID as u64,
            value: capset_id as u64,
        },
        drm_virtgpu_context_set_param {
            param: VIRTGPU_CONTEXT_PARAM_NUM_RINGS as u64,
            value: num_rings,
        },
    ];
    let mut init = drm_virtgpu_context_init {
        num_params: ctx_params.len() as u32,
        pad: 0,
        ctx_set_params: ctx_params.as_ptr() as u64,
    };
    // SAFETY: `fd` is valid and `ctx_params` outlives the ioctl call.
    unsafe {
        drm_ioctl_virtgpu_context_init(fd, &mut init)?;
    }
    Ok(())
}

pub fn virtgpu_resource_create_blob(
    fd: BorrowedFd<'_>,
    blob_id: u64,
    size: usize,
) -> Result<(u32, u32)> {
    let mut create_blob = drm_virtgpu_resource_create_blob {
        blob_mem: VIRTGPU_BLOB_MEM_HOST3D,
        blob_flags: VIRTGPU_BLOB_FLAG_USE_MAPPABLE | VIRTGPU_BLOB_FLAG_USE_SHAREABLE,
        size: size as u64,
        blob_id,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `create_blob` is stack-allocated.
    unsafe {
        drm_ioctl_virtgpu_resource_create_blob(fd, &mut create_blob)?;
    }
    Ok((create_blob.bo_handle, create_blob.res_handle))
}

pub fn virtgpu_map(fd: BorrowedFd<'_>, bo_handle: u32) -> Result<u64> {
    let mut map_arg = drm_virtgpu_map {
        handle: bo_handle,
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `map_arg` is stack-allocated.
    unsafe {
        drm_ioctl_virtgpu_map(fd, &mut map_arg)?;
    }
    Ok(map_arg.offset)
}

pub fn virtgpu_execbuffer(
    fd: BorrowedFd<'_>,
    cmd: &[u8],
    ring_idx: u32,
    syncobjs: &[u32],
) -> Result<()> {
    let mut out_syncobjs: Vec<drm_virtgpu_execbuffer_syncobj> = syncobjs
        .iter()
        .filter(|&&h| h != 0)
        .map(|&handle| drm_virtgpu_execbuffer_syncobj {
            handle,
            flags: 0,
            point: 0,
        })
        .collect();

    let mut flags = 0;
    if ring_idx != 0 {
        flags |= VIRTGPU_EXECBUF_RING_IDX;
    }

    let mut execbuf = drm_virtgpu_execbuffer {
        flags,
        size: cmd.len() as u32,
        command: cmd.as_ptr() as u64,
        ring_idx,
        fence_fd: -1,
        syncobj_stride: size_of::<drm_virtgpu_execbuffer_syncobj>() as u32,
        num_out_syncobjs: out_syncobjs.len() as u32,
        out_syncobjs: if out_syncobjs.is_empty() {
            0
        } else {
            out_syncobjs.as_mut_ptr() as u64
        },
        ..Default::default()
    };
    // SAFETY: `fd` is valid and `cmd` / `out_syncobjs` outlive the ioctl call.
    unsafe {
        drm_ioctl_virtgpu_execbuffer(fd, &mut execbuf)?;
    }
    Ok(())
}

pub fn virtgpu_syncobj_create(fd: BorrowedFd<'_>, initial_signaled: bool) -> Result<u32> {
    let mut create = drm_syncobj_create {
        handle: 0,
        flags: 0,
    };
    // SAFETY: `fd` is valid and `create` is stack-allocated.
    unsafe {
        drm_ioctl_syncobj_create(fd, &mut create)?;
    }
    let handle = create.handle;
    if initial_signaled {
        virtgpu_syncobj_signal(fd, handle)?;
    }
    Ok(handle)
}

pub fn virtgpu_syncobj_destroy(fd: BorrowedFd<'_>, sync_handle: u32) {
    let mut destroy = drm_syncobj_destroy {
        handle: sync_handle,
        pad: 0,
    };
    // SAFETY: `fd` is valid and `destroy` is stack-allocated.
    let _ = unsafe { drm_ioctl_syncobj_destroy(fd, &mut destroy) };
}

pub fn virtgpu_syncobj_wait(fd: BorrowedFd<'_>, sync_handle: u32, timeout_ns: u64) -> Result<()> {
    let mut handles = [sync_handle];
    let timeout_nsec = timeout_ns.min(i64::MAX as u64) as i64;
    let mut wait_arg = drm_syncobj_wait {
        handles: handles.as_mut_ptr() as u64,
        timeout_nsec,
        count_handles: 1,
        flags: DRM_SYNCOBJ_WAIT_FLAGS_WAIT_ALL | DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT,
        first_signaled: 0,
        pad: 0,
        deadline_nsec: 0,
    };
    // SAFETY: `fd` is valid and `handles` outlives the ioctl call.
    let ret = unsafe { drm_ioctl_syncobj_wait(fd, &mut wait_arg) };
    match ret {
        Ok(_) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::ETIME) => Err(Error::TimedOut),
        Err(e) => Err(Error::PlatformError(e.into())),
    }
}

pub fn virtgpu_syncobj_signal(fd: BorrowedFd<'_>, sync_handle: u32) -> Result<()> {
    let mut signal_arg = drm_syncobj_array {
        handles: &sync_handle as *const _ as u64,
        count_handles: 1,
        pad: 0,
    };
    // SAFETY: `fd` is valid and `sync_handle` outlives the ioctl call.
    unsafe {
        drm_ioctl_syncobj_signal(fd, &mut signal_arg)?;
    }
    Ok(())
}

pub fn virtgpu_syncobj_export(fd: BorrowedFd<'_>, sync_handle: u32) -> Result<MagmaGpuHandle> {
    let mut handle_arg = drm_syncobj_handle {
        handle: sync_handle,
        flags: DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE,
        fd: -1,
        pad: 0,
    };
    // SAFETY: `fd` is valid and `handle_arg` is stack-allocated.
    unsafe {
        drm_ioctl_syncobj_handle_to_fd(fd, &mut handle_arg)?;
    }
    // SAFETY: `handle_arg.fd` is a newly created file descriptor returned by the kernel.
    let descriptor = unsafe { OwnedDescriptor::from_raw_descriptor(handle_arg.fd) };
    Ok(MagmaGpuHandle {
        os_handle: descriptor,
        handle_type: MAGMA_GPU_HANDLE_TYPE_SIGNAL_SYNC_FD,
    })
}

pub fn virtgpu_syncobj_import(
    fd: BorrowedFd<'_>,
    sync_handle: u32,
    handle: MagmaGpuHandle,
) -> Result<()> {
    let mut sync_arg = drm_syncobj_handle {
        handle: sync_handle,
        flags: DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE,
        fd: handle.os_handle.as_raw_descriptor(),
        pad: 0,
    };
    // SAFETY: `fd` and `handle.os_handle` are valid descriptors.
    unsafe {
        drm_ioctl_syncobj_fd_to_handle(fd, &mut sync_arg)?;
    }
    Ok(())
}
