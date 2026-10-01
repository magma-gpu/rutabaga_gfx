// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::os::fd::BorrowedFd;

use crate::defines::MagmaSubmitInfo;
use crate::error::Error;
use crate::error::Result;
use crate::ioctl_readwrite;
use crate::ioctl_write_ptr;
use crate::sys::linux::bindings::drm_bindings::DRM_COMMAND_BASE;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;
use crate::sys::linux::bindings::msm_bindings::*;

ioctl_readwrite!(
    drm_ioctl_msm_gem_new,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_MSM_GEM_NEW,
    drm_msm_gem_new
);

ioctl_readwrite!(
    drm_ioctl_msm_gem_info,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_MSM_GEM_INFO,
    drm_msm_gem_info
);

ioctl_write_ptr!(
    msm_gem_cpu_prep,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_MSM_GEM_CPU_PREP,
    drm_msm_gem_cpu_prep
);

ioctl_write_ptr!(
    msm_gem_cpu_fini,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_MSM_GEM_CPU_FINI,
    drm_msm_gem_cpu_fini
);

ioctl_readwrite!(
    msm_submitqueue_new,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_MSM_SUBMITQUEUE_NEW,
    drm_msm_submitqueue
);

ioctl_write_ptr!(
    msm_submitqueue_close,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_MSM_SUBMITQUEUE_CLOSE,
    __u32
);

ioctl_readwrite!(
    msm_gem_submit,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_MSM_GEM_SUBMIT,
    drm_msm_gem_submit
);

pub fn gem_new(fd: BorrowedFd<'_>, size: u64) -> Result<u32> {
    let mut gem_new = drm_msm_gem_new {
        size,
        flags: 0,
        ..Default::default()
    };

    // SAFETY: Valid fd and drm_msm_gem_new struct.
    unsafe {
        drm_ioctl_msm_gem_new(fd, &mut gem_new)?;
    }

    Ok(gem_new.handle)
}

pub fn gem_info_offset(fd: BorrowedFd<'_>, gem_handle: u32) -> Result<u64> {
    let mut gem_info = drm_msm_gem_info {
        handle: gem_handle,
        info: MSM_INFO_GET_OFFSET,
        ..Default::default()
    };

    // SAFETY: Valid fd and drm_msm_gem_info struct.
    unsafe {
        drm_ioctl_msm_gem_info(fd, &mut gem_info)?;
    }
    Ok(gem_info.value)
}

pub fn gem_cpu_prep(fd: BorrowedFd<'_>, gem_handle: u32) -> Result<()> {
    let prep = drm_msm_gem_cpu_prep {
        handle: gem_handle,
        op: MSM_PREP_READ | MSM_PREP_WRITE,
        ..Default::default()
    };

    // SAFETY: Valid fd and gem handle.
    unsafe {
        msm_gem_cpu_prep(fd, &prep)?;
    }
    Ok(())
}

pub fn gem_cpu_fini(fd: BorrowedFd<'_>, gem_handle: u32) -> Result<()> {
    let fini = drm_msm_gem_cpu_fini { handle: gem_handle };

    // SAFETY: Valid fd and gem handle.
    unsafe {
        msm_gem_cpu_fini(fd, &fini)?;
    }
    Ok(())
}

pub fn submitqueue_create(fd: BorrowedFd<'_>, priority: u32) -> Result<u32> {
    let mut new_submit_queue = drm_msm_submitqueue {
        flags: 0,
        prio: priority,
        ..Default::default()
    };

    // SAFETY: Valid fd and drm_msm_submitqueue struct.
    unsafe {
        msm_submitqueue_new(fd, &mut new_submit_queue)?;
    }
    Ok(new_submit_queue.id)
}

pub fn submitqueue_destroy(fd: BorrowedFd<'_>, submit_queue_id: u32) {
    // SAFETY: Valid fd and submit_queue_id.
    unsafe {
        let _ = msm_submitqueue_close(fd, &submit_queue_id);
    }
}

pub fn gem_submit(
    fd: BorrowedFd<'_>,
    submit_queue_id: u32,
    submit_info: &MagmaSubmitInfo,
) -> Result<()> {
    let buf_info = submit_info.buffer_info().ok_or(Error::Unimplemented)?;

    let bo = drm_msm_gem_submit_bo {
        flags: MSM_SUBMIT_BO_READ,
        handle: buf_info.command_buffer,
        presumed: 0,
    };

    let mut cmd = drm_msm_gem_submit_cmd {
        type_: MSM_SUBMIT_CMD_BUF,
        submit_offset: buf_info.start_offset as u32,
        size: buf_info.length as u32,
        ..Default::default()
    };
    cmd.__bindgen_anon_1.relocs = 0;

    let mut in_syncs: Vec<drm_msm_syncobj> = Vec::new();
    let mut out_syncs: Vec<drm_msm_syncobj> = Vec::new();
    let mut submit_flags = 0u32;

    for sync_obj in submit_info.sync_info.wait_sync_objs() {
        if let Some(handle) = sync_obj.as_raw_handle() {
            in_syncs.push(drm_msm_syncobj {
                handle,
                flags: 0,
                point: 0,
            });
        }
    }
    for sync_obj in submit_info.sync_info.signal_sync_objs() {
        if let Some(handle) = sync_obj.as_raw_handle() {
            out_syncs.push(drm_msm_syncobj {
                handle,
                flags: 0,
                point: 0,
            });
        }
    }

    if !in_syncs.is_empty() {
        submit_flags |= MSM_SUBMIT_SYNCOBJ_IN;
    }
    if !out_syncs.is_empty() {
        submit_flags |= MSM_SUBMIT_SYNCOBJ_OUT;
    }

    let mut submit = drm_msm_gem_submit {
        flags: submit_flags,
        fence: 0,
        nr_bos: 1,
        nr_cmds: 1,
        bos: &bo as *const _ as u64,
        cmds: &cmd as *const _ as u64,
        fence_fd: -1,
        queueid: submit_queue_id,
        in_syncobjs: in_syncs.as_ptr() as u64,
        out_syncobjs: out_syncs.as_ptr() as u64,
        nr_in_syncobjs: in_syncs.len() as u32,
        nr_out_syncobjs: out_syncs.len() as u32,
        syncobj_stride: size_of::<drm_msm_syncobj>() as u32,
        pad: 0,
    };

    // SAFETY: Valid fd and drm_msm_gem_submit struct.
    unsafe {
        msm_gem_submit(fd, &mut submit)?;
    }
    Ok(())
}
