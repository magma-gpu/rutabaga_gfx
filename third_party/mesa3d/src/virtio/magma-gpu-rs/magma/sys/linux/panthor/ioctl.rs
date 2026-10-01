// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::os::fd::BorrowedFd;

use crate::defines::MagmaCreateQueueInfo;
use crate::defines::MagmaHeap;
use crate::defines::MagmaMappedMemoryRange;
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
use crate::protocol::MagmaGpuMapFlags;
use crate::sys::linux::bindings::drm_bindings::DRM_COMMAND_BASE;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;
use crate::sys::linux::bindings::panthor_bindings::*;

ioctl_readwrite!(
    drm_ioctl_panthor_dev_query,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_DEV_QUERY,
    drm_panthor_dev_query
);

ioctl_readwrite!(
    drm_ioctl_panthor_vm_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_VM_CREATE,
    drm_panthor_vm_create
);

ioctl_write_ptr!(
    drm_ioctl_panthor_vm_destroy,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_VM_DESTROY,
    drm_panthor_vm_destroy
);

ioctl_readwrite!(
    drm_ioctl_panthor_vm_bind,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_VM_BIND,
    drm_panthor_vm_bind
);

ioctl_readwrite!(
    drm_ioctl_panthor_vm_get_state,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_VM_GET_STATE,
    drm_panthor_vm_get_state
);

ioctl_readwrite!(
    drm_ioctl_panthor_bo_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_BO_CREATE,
    drm_panthor_bo_create
);

ioctl_readwrite!(
    drm_ioctl_panthor_bo_mmap_offset,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_BO_MMAP_OFFSET,
    drm_panthor_bo_mmap_offset
);

ioctl_readwrite!(
    drm_ioctl_panthor_group_create,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_GROUP_CREATE,
    drm_panthor_group_create
);

ioctl_write_ptr!(
    drm_ioctl_panthor_group_destroy,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_GROUP_DESTROY,
    drm_panthor_group_destroy
);

ioctl_readwrite!(
    drm_ioctl_panthor_group_submit,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_GROUP_SUBMIT,
    drm_panthor_group_submit
);

ioctl_readwrite!(
    drm_ioctl_panthor_group_get_state,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_GROUP_GET_STATE,
    drm_panthor_group_get_state
);

ioctl_readwrite!(
    drm_ioctl_panthor_bo_sync,
    DRM_IOCTL_BASE,
    DRM_COMMAND_BASE + DRM_PANTHOR_BO_SYNC,
    drm_panthor_bo_sync
);

pub fn query_gpu_info(fd: BorrowedFd<'_>) -> Result<drm_panthor_gpu_info> {
    let mut gpu_info: drm_panthor_gpu_info = Default::default();
    let mut query = drm_panthor_dev_query {
        type_: DRM_PANTHOR_DEV_QUERY_GPU_INFO,
        size: size_of::<drm_panthor_gpu_info>() as u32,
        pointer: &mut gpu_info as *mut _ as u64,
    };
    // SAFETY: Valid fd and drm_panthor_dev_query struct.
    unsafe {
        drm_ioctl_panthor_dev_query(fd, &mut query)?;
    }
    Ok(gpu_info)
}

pub fn query_csif_info(fd: BorrowedFd<'_>) -> Result<drm_panthor_csif_info> {
    let mut csif_info: drm_panthor_csif_info = Default::default();
    let mut query = drm_panthor_dev_query {
        type_: DRM_PANTHOR_DEV_QUERY_CSIF_INFO,
        size: size_of::<drm_panthor_csif_info>() as u32,
        pointer: &mut csif_info as *mut _ as u64,
    };
    // SAFETY: Valid fd and drm_panthor_dev_query struct.
    unsafe {
        drm_ioctl_panthor_dev_query(fd, &mut query)?;
    }
    Ok(csif_info)
}

pub fn query_memory_heaps() -> Vec<MagmaHeap> {
    let heap_size = 4 * 1024 * 1024 * 1024; // 4 GiB default unified memory heap
    vec![MagmaHeap {
        heap_size,
        heap_flags: MAGMA_HEAP_DEVICE_LOCAL_BIT | MAGMA_HEAP_CPU_VISIBLE_BIT,
    }]
}

pub fn query_memory_types(gpu_info: &drm_panthor_gpu_info) -> Vec<MagmaMemoryType> {
    let mut types = Vec::new();
    types.push(MagmaMemoryType {
        property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
        heap_idx: 0,
    });
    types.push(MagmaMemoryType {
        property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
            | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
            | MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT,
        heap_idx: 0,
    });
    let is_coherent = gpu_info.selected_coherency != DRM_PANTHOR_GPU_COHERENCY_NONE;
    if is_coherent {
        types.push(MagmaMemoryType {
            property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_COHERENT_BIT
                | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT,
            heap_idx: 0,
        });
    }
    types.push(MagmaMemoryType {
        property_flags: MAGMA_MEMORY_PROPERTY_DEVICE_LOCAL_BIT
            | MAGMA_MEMORY_PROPERTY_HOST_VISIBLE_BIT
            | MAGMA_MEMORY_PROPERTY_HOST_CACHED_BIT,
        heap_idx: 0,
    });
    types
}

pub fn vm_create(fd: BorrowedFd<'_>) -> Result<u32> {
    let mut vm_create = drm_panthor_vm_create {
        flags: 0,
        user_va_range: 0,
        ..Default::default()
    };

    // SAFETY: Valid fd and drm_panthor_vm_create struct.
    unsafe {
        drm_ioctl_panthor_vm_create(fd, &mut vm_create)?;
    }

    Ok(vm_create.id)
}

pub fn vm_destroy(fd: BorrowedFd<'_>, vm_id: u32) -> Result<()> {
    let destroy = drm_panthor_vm_destroy { id: vm_id, pad: 0 };
    // SAFETY: Valid fd and drm_panthor_vm_destroy struct.
    unsafe {
        drm_ioctl_panthor_vm_destroy(fd, &destroy)?;
    }
    Ok(())
}

pub fn vm_map(
    fd: BorrowedFd<'_>,
    vm_id: u32,
    gem_handle: u32,
    buffer_offset: u64,
    gpu_va: u64,
    size: u64,
    flags: MagmaGpuMapFlags,
) -> Result<()> {
    let mut bind_flags = DRM_PANTHOR_VM_BIND_OP_TYPE_MAP as u32;

    if (flags & MagmaGpuMapFlags::Execute).bits() == 0 {
        bind_flags |= DRM_PANTHOR_VM_BIND_OP_MAP_NOEXEC as u32;
    }
    if (flags & MagmaGpuMapFlags::Write).bits() == 0 {
        bind_flags |= DRM_PANTHOR_VM_BIND_OP_MAP_READONLY as u32;
    }

    let op = drm_panthor_vm_bind_op {
        flags: bind_flags,
        bo_handle: gem_handle,
        bo_offset: buffer_offset,
        va: gpu_va,
        size,
        syncs: drm_panthor_obj_array {
            stride: 0,
            count: 0,
            array: 0,
        },
    };

    let mut vm_bind = drm_panthor_vm_bind {
        vm_id,
        flags: 0,
        ops: drm_panthor_obj_array {
            stride: size_of::<drm_panthor_vm_bind_op>() as u32,
            count: 1,
            array: &op as *const _ as u64,
        },
    };

    // SAFETY: Valid fd and drm_panthor_vm_bind struct.
    unsafe {
        drm_ioctl_panthor_vm_bind(fd, &mut vm_bind)?;
    }
    Ok(())
}

pub fn vm_unmap(fd: BorrowedFd<'_>, vm_id: u32, gpu_va: u64, size: u64) -> Result<()> {
    let op = drm_panthor_vm_bind_op {
        flags: DRM_PANTHOR_VM_BIND_OP_TYPE_UNMAP as u32,
        bo_handle: 0,
        bo_offset: 0,
        va: gpu_va,
        size,
        syncs: drm_panthor_obj_array {
            stride: 0,
            count: 0,
            array: 0,
        },
    };

    let mut vm_bind = drm_panthor_vm_bind {
        vm_id,
        flags: 0,
        ops: drm_panthor_obj_array {
            stride: size_of::<drm_panthor_vm_bind_op>() as u32,
            count: 1,
            array: &op as *const _ as u64,
        },
    };

    // SAFETY: Valid fd and drm_panthor_vm_bind struct.
    unsafe {
        drm_ioctl_panthor_vm_bind(fd, &mut vm_bind)?;
    }
    Ok(())
}

pub fn bo_create(fd: BorrowedFd<'_>, size: u64, is_cached: bool) -> Result<(u32, usize)> {
    let mut flags = 0u32;
    if is_cached {
        flags |= DRM_PANTHOR_BO_WB_MMAP;
    }

    let mut bo_create = drm_panthor_bo_create {
        size,
        flags,
        exclusive_vm_id: 0,
        ..Default::default()
    };

    // SAFETY: Valid fd and drm_panthor_bo_create struct.
    unsafe {
        drm_ioctl_panthor_bo_create(fd, &mut bo_create)?;
    }

    Ok((bo_create.handle, bo_create.size as usize))
}

pub fn bo_mmap_offset(fd: BorrowedFd<'_>, gem_handle: u32) -> Result<u64> {
    let mut mmap_offset = drm_panthor_bo_mmap_offset {
        handle: gem_handle,
        ..Default::default()
    };

    // SAFETY: Valid fd and drm_panthor_bo_mmap_offset struct.
    unsafe {
        drm_ioctl_panthor_bo_mmap_offset(fd, &mut mmap_offset)?;
    }
    Ok(mmap_offset.offset)
}

fn bo_sync(
    fd: BorrowedFd<'_>,
    gem_handle: u32,
    total_size: u64,
    sync_type: u32,
    ranges: &[MagmaMappedMemoryRange],
) -> Result<()> {
    if ranges.is_empty() {
        let sync_op = drm_panthor_bo_sync_op {
            handle: gem_handle,
            type_: sync_type,
            offset: 0,
            size: total_size,
        };
        let mut sync = drm_panthor_bo_sync {
            ops: drm_panthor_obj_array {
                stride: size_of::<drm_panthor_bo_sync_op>() as u32,
                count: 1,
                array: &sync_op as *const _ as u64,
            },
        };
        // SAFETY: Valid fd and drm_panthor_bo_sync struct.
        unsafe {
            drm_ioctl_panthor_bo_sync(fd, &mut sync)?;
        }
    } else {
        let sync_ops: Vec<drm_panthor_bo_sync_op> = ranges
            .iter()
            .map(|r| drm_panthor_bo_sync_op {
                handle: gem_handle,
                type_: sync_type,
                offset: r.offset,
                size: r.size,
            })
            .collect();
        let mut sync = drm_panthor_bo_sync {
            ops: drm_panthor_obj_array {
                stride: size_of::<drm_panthor_bo_sync_op>() as u32,
                count: sync_ops.len() as u32,
                array: sync_ops.as_ptr() as u64,
            },
        };
        // SAFETY: Valid fd and drm_panthor_bo_sync struct.
        unsafe {
            drm_ioctl_panthor_bo_sync(fd, &mut sync)?;
        }
    }
    Ok(())
}

pub fn bo_invalidate(
    fd: BorrowedFd<'_>,
    gem_handle: u32,
    total_size: u64,
    ranges: &[MagmaMappedMemoryRange],
) -> Result<()> {
    bo_sync(
        fd,
        gem_handle,
        total_size,
        DRM_PANTHOR_BO_SYNC_CPU_CACHE_FLUSH_AND_INVALIDATE,
        ranges,
    )
}

pub fn bo_flush(
    fd: BorrowedFd<'_>,
    gem_handle: u32,
    total_size: u64,
    ranges: &[MagmaMappedMemoryRange],
) -> Result<()> {
    bo_sync(
        fd,
        gem_handle,
        total_size,
        DRM_PANTHOR_BO_SYNC_CPU_CACHE_FLUSH,
        ranges,
    )
}

pub fn group_create(
    fd: BorrowedFd<'_>,
    gpu_info: &drm_panthor_gpu_info,
    vm_id: u32,
    info: &MagmaCreateQueueInfo,
) -> Result<u32> {
    let compute_core_mask = if gpu_info.shader_present != 0 {
        gpu_info.shader_present
    } else {
        1
    };
    let fragment_core_mask = if gpu_info.shader_present != 0 {
        gpu_info.shader_present
    } else {
        1
    };
    let tiler_core_mask = if gpu_info.tiler_present != 0 {
        gpu_info.tiler_present
    } else {
        1
    };

    let max_compute_cores = compute_core_mask.count_ones().max(1) as u8;
    let max_fragment_cores = fragment_core_mask.count_ones().max(1) as u8;
    let max_tiler_cores = tiler_core_mask.count_ones().max(1) as u8;

    let priority = match info.priority {
        0 => PANTHOR_GROUP_PRIORITY_LOW as u8,
        1 => PANTHOR_GROUP_PRIORITY_MEDIUM as u8,
        2 => PANTHOR_GROUP_PRIORITY_HIGH as u8,
        _ => PANTHOR_GROUP_PRIORITY_MEDIUM as u8,
    };

    let qc = [drm_panthor_queue_create {
        priority: 1,
        pad: [0; 3],
        ringbuf_size: 64 * 1024,
    }];

    let mut gc = drm_panthor_group_create {
        queues: drm_panthor_obj_array {
            stride: size_of::<drm_panthor_queue_create>() as u32,
            count: qc.len() as u32,
            array: qc.as_ptr() as u64,
        },
        max_compute_cores,
        max_fragment_cores,
        max_tiler_cores,
        priority,
        pad: 0,
        compute_core_mask,
        fragment_core_mask,
        tiler_core_mask,
        vm_id,
        group_handle: 0,
    };

    // SAFETY: Valid fd and drm_panthor_group_create struct.
    unsafe {
        drm_ioctl_panthor_group_create(fd, &mut gc)?;
    }

    Ok(gc.group_handle)
}

pub fn group_destroy(fd: BorrowedFd<'_>, group_handle: u32) -> Result<()> {
    let destroy = drm_panthor_group_destroy {
        group_handle,
        pad: 0,
    };
    // SAFETY: Valid fd and drm_panthor_group_destroy struct.
    unsafe {
        drm_ioctl_panthor_group_destroy(fd, &destroy)?;
    }
    Ok(())
}

pub fn queue_submit(
    fd: BorrowedFd<'_>,
    is_bind_queue: bool,
    vm_id: u32,
    group_handle: u32,
    submit_info: &MagmaSubmitInfo,
) -> Result<()> {
    let mut sync_ops: Vec<drm_panthor_sync_op> = Vec::new();

    for sync_obj in submit_info.sync_info.wait_sync_objs() {
        if let Some(handle) = sync_obj.as_raw_handle() {
            sync_ops.push(drm_panthor_sync_op {
                flags: (DRM_PANTHOR_SYNC_OP_HANDLE_TYPE_SYNCOBJ as u32)
                    | (DRM_PANTHOR_SYNC_OP_WAIT as u32),
                handle,
                timeline_value: 0,
            });
        }
    }

    for sync_obj in submit_info.sync_info.signal_sync_objs() {
        if let Some(handle) = sync_obj.as_raw_handle() {
            sync_ops.push(drm_panthor_sync_op {
                flags: (DRM_PANTHOR_SYNC_OP_HANDLE_TYPE_SYNCOBJ as u32)
                    | (DRM_PANTHOR_SYNC_OP_SIGNAL as u32),
                handle,
                timeline_value: 0,
            });
        }
    }

    if is_bind_queue {
        let op = drm_panthor_vm_bind_op {
            flags: DRM_PANTHOR_VM_BIND_OP_TYPE_SYNC_ONLY as u32,
            bo_handle: 0,
            bo_offset: 0,
            va: 0,
            size: 0,
            syncs: drm_panthor_obj_array {
                stride: size_of::<drm_panthor_sync_op>() as u32,
                count: sync_ops.len() as u32,
                array: sync_ops.as_ptr() as u64,
            },
        };

        let mut vm_bind = drm_panthor_vm_bind {
            vm_id,
            flags: DRM_PANTHOR_VM_BIND_ASYNC,
            ops: drm_panthor_obj_array {
                stride: size_of::<drm_panthor_vm_bind_op>() as u32,
                count: 1,
                array: &op as *const _ as u64,
            },
        };

        // SAFETY: Valid fd and drm_panthor_vm_bind struct.
        unsafe {
            drm_ioctl_panthor_vm_bind(fd, &mut vm_bind)?;
        }
    } else {
        let (stream_addr, stream_size) = if let Some(as_info) = submit_info.address_space_info() {
            (as_info.command_va, as_info.length as u32)
        } else if let Some(buf_info) = submit_info.buffer_info() {
            (buf_info.start_offset, buf_info.length as u32)
        } else {
            (0, 0)
        };

        let qsubmit = drm_panthor_queue_submit {
            queue_index: 0,
            stream_size,
            stream_addr,
            latest_flush: 0,
            pad: 0,
            syncs: drm_panthor_obj_array {
                stride: size_of::<drm_panthor_sync_op>() as u32,
                count: sync_ops.len() as u32,
                array: sync_ops.as_ptr() as u64,
            },
        };

        let mut gsubmit = drm_panthor_group_submit {
            group_handle,
            pad: 0,
            queue_submits: drm_panthor_obj_array {
                stride: size_of::<drm_panthor_queue_submit>() as u32,
                count: 1,
                array: &qsubmit as *const _ as u64,
            },
        };

        // SAFETY: Valid fd and drm_panthor_group_submit struct.
        unsafe {
            drm_ioctl_panthor_group_submit(fd, &mut gsubmit)?;
        }
    }
    Ok(())
}

pub fn check_queue_status(
    fd: BorrowedFd<'_>,
    is_bind_queue: bool,
    vm_id: u32,
    group_handle: u32,
) -> Result<()> {
    if is_bind_queue {
        let mut state = drm_panthor_vm_get_state { vm_id, state: 0 };
        // SAFETY: Valid fd and drm_panthor_vm_get_state struct.
        unsafe {
            drm_ioctl_panthor_vm_get_state(fd, &mut state).map_err(|_| Error::InternalError)?;
        }
        if state.state == DRM_PANTHOR_VM_STATE_UNUSABLE {
            return Err(Error::ContextKilled);
        }
        return Ok(());
    }

    let mut state = drm_panthor_group_get_state {
        group_handle,
        state: 0,
        fatal_queues: 0,
        pad: 0,
    };
    // SAFETY: Valid fd and drm_panthor_group_get_state struct.
    unsafe {
        drm_ioctl_panthor_group_get_state(fd, &mut state).map_err(|_| Error::InternalError)?;
    }
    if (state.state & DRM_PANTHOR_GROUP_STATE_TIMEDOUT) != 0 {
        return Err(Error::TimedOut);
    }
    if (state.state & DRM_PANTHOR_GROUP_STATE_FATAL_FAULT) != 0 {
        return Err(Error::ContextKilled);
    }
    Ok(())
}
