// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::ffi::CString;
use std::os::fd::AsFd;
use std::os::raw::c_char;
use std::os::raw::c_uint;
use std::ptr::null_mut;
use std::sync::Arc;

use magma_gpu::util::AsRawDescriptor;
use magma_gpu::util::FromRawDescriptor;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::MAGMA_GPU_HANDLE_TYPE_SIGNAL_SYNC_FD;

use crate::defines::MagmaCreateSyncObjInfo;
use crate::defines::MagmaSyncType;
use crate::error::Error;
use crate::error::Result;
use crate::ioctl_readwrite;
use crate::ioctl_write_ptr;
use crate::sys::linux::bindings::drm_bindings::__kernel_size_t;
use crate::sys::linux::bindings::drm_bindings::drm_gem_close;
use crate::sys::linux::bindings::drm_bindings::drm_prime_handle;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_array;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_create;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_destroy;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_handle;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_timeline_array;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_timeline_wait;
use crate::sys::linux::bindings::drm_bindings::drm_syncobj_wait;
use crate::sys::linux::bindings::drm_bindings::drm_version;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;
use crate::traits::AsVirtioSyncObject;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendSyncObject;
use crate::traits::GenericSyncObject;

pub const DRM_DIR_NAME: &str = "/dev/dri";
pub const DRM_RENDER_MINOR_NAME: &str = "renderD";
const DRM_IOCTL_VERSION: c_uint = 0x00;

pub const DRM_IOCTL_SYNCOBJ_CREATE: c_uint = 0xbf;
pub const DRM_IOCTL_SYNCOBJ_DESTROY: c_uint = 0xc0;
pub const DRM_IOCTL_SYNCOBJ_HANDLE_TO_FD: c_uint = 0xc1;
pub const DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE: c_uint = 0xc2;
pub const DRM_IOCTL_SYNCOBJ_WAIT: c_uint = 0xc3;
#[allow(dead_code)]
pub const DRM_IOCTL_SYNCOBJ_RESET: c_uint = 0xc4;
pub const DRM_IOCTL_SYNCOBJ_SIGNAL: c_uint = 0xc5;
pub const DRM_IOCTL_SYNCOBJ_TIMELINE_WAIT: c_uint = 0xca;
pub const DRM_IOCTL_SYNCOBJ_QUERY: c_uint = 0xcb;
pub const DRM_IOCTL_SYNCOBJ_TIMELINE_SIGNAL: c_uint = 0xcd;

pub const DRM_SYNCOBJ_WAIT_FLAGS_WAIT_ALL: u32 = 1 << 0;
pub const DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT: u32 = 1 << 1;
#[allow(dead_code)]
pub const DRM_SYNCOBJ_WAIT_FLAGS_WAIT_AVAILABLE: u32 = 1 << 2;

pub const DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE: u32 = 1 << 0;
pub const DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_IMPORT_SYNC_FILE: u32 = 1 << 0;

ioctl_readwrite!(
    drm_get_version,
    DRM_IOCTL_BASE,
    DRM_IOCTL_VERSION,
    drm_version
);

ioctl_readwrite!(
    drm_ioctl_prime_handle_to_fd,
    DRM_IOCTL_BASE,
    0x2d,
    drm_prime_handle
);

ioctl_readwrite!(
    drm_ioctl_prime_fd_to_handle,
    DRM_IOCTL_BASE,
    0x2e,
    drm_prime_handle
);

ioctl_write_ptr!(drm_ioctl_gem_close, DRM_IOCTL_BASE, 0x09, drm_gem_close);

ioctl_readwrite!(
    drm_ioctl_syncobj_create,
    DRM_IOCTL_BASE,
    DRM_IOCTL_SYNCOBJ_CREATE,
    drm_syncobj_create
);

ioctl_readwrite!(
    drm_ioctl_syncobj_destroy,
    DRM_IOCTL_BASE,
    DRM_IOCTL_SYNCOBJ_DESTROY,
    drm_syncobj_destroy
);

ioctl_readwrite!(
    drm_ioctl_syncobj_handle_to_fd,
    DRM_IOCTL_BASE,
    DRM_IOCTL_SYNCOBJ_HANDLE_TO_FD,
    drm_syncobj_handle
);

ioctl_readwrite!(
    drm_ioctl_syncobj_fd_to_handle,
    DRM_IOCTL_BASE,
    DRM_IOCTL_SYNCOBJ_FD_TO_HANDLE,
    drm_syncobj_handle
);

ioctl_readwrite!(
    drm_ioctl_syncobj_wait,
    DRM_IOCTL_BASE,
    DRM_IOCTL_SYNCOBJ_WAIT,
    drm_syncobj_wait
);

ioctl_readwrite!(
    drm_ioctl_syncobj_signal,
    DRM_IOCTL_BASE,
    DRM_IOCTL_SYNCOBJ_SIGNAL,
    drm_syncobj_array
);

ioctl_readwrite!(
    drm_ioctl_syncobj_timeline_wait,
    DRM_IOCTL_BASE,
    DRM_IOCTL_SYNCOBJ_TIMELINE_WAIT,
    drm_syncobj_timeline_wait
);

ioctl_readwrite!(
    drm_ioctl_syncobj_query,
    DRM_IOCTL_BASE,
    DRM_IOCTL_SYNCOBJ_QUERY,
    drm_syncobj_timeline_array
);

ioctl_readwrite!(
    drm_ioctl_syncobj_timeline_signal,
    DRM_IOCTL_BASE,
    DRM_IOCTL_SYNCOBJ_TIMELINE_SIGNAL,
    drm_syncobj_timeline_array
);

pub struct DrmSyncObject {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    handle: u32,
    sync_type: MagmaSyncType,
}

impl DrmSyncObject {
    pub fn new_from_info(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        info: &MagmaCreateSyncObjInfo,
    ) -> Result<DrmSyncObject> {
        let sync_type = MagmaSyncType::from(info.sync_type);
        let obj = DrmSyncObject::new(physical_device, sync_type)?;
        if info.initial_point > 0 {
            obj.timeline_signal(info.initial_point)?;
        }
        Ok(obj)
    }

    pub fn new(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        sync_type: MagmaSyncType,
    ) -> Result<DrmSyncObject> {
        let fd = physical_device.as_fd().unwrap();
        let mut create = drm_syncobj_create {
            handle: 0,
            flags: 0,
        };
        unsafe {
            drm_ioctl_syncobj_create(fd, &mut create)?;
        }
        Ok(DrmSyncObject {
            physical_device,
            handle: create.handle,
            sync_type,
        })
    }
}

impl Drop for DrmSyncObject {
    fn drop(&mut self) {
        let fd = self.physical_device.as_fd().unwrap();
        let mut destroy = drm_syncobj_destroy {
            handle: self.handle,
            pad: 0,
        };
        unsafe {
            let _ = drm_ioctl_syncobj_destroy(fd, &mut destroy);
        }
    }
}

impl GenericSyncObject for DrmSyncObject {
    fn get_type(&self) -> MagmaSyncType {
        self.sync_type
    }

    fn wait(&self, timeout_ns: u64) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        // DRM syncobj timeouts are signed 64-bit integers (__s64) representing an
        // absolute deadline in CLOCK_MONOTONIC nanoseconds. Casting UINT64_MAX
        // (e.g. VK_TIMEOUT_INFINITE) directly to i64 wraps to -1, so we need to clamp.
        let timeout_nsec = timeout_ns.min(i64::MAX as u64) as i64;
        let mut wait_arg = drm_syncobj_wait {
            handles: &self.handle as *const _ as u64,
            timeout_nsec,
            count_handles: 1,
            flags: DRM_SYNCOBJ_WAIT_FLAGS_WAIT_ALL | DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT,
            first_signaled: 0,
            pad: 0,
            deadline_nsec: 0,
        };
        match unsafe { drm_ioctl_syncobj_wait(fd, &mut wait_arg) } {
            Ok(_) => Ok(()),
            Err(e) if e.raw_os_error() == Some(libc::ETIME) => Err(Error::TimedOut),
            Err(e) => Err(Error::PlatformError(e.into())),
        }
    }

    fn signal(&self) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        let mut signal_arg = drm_syncobj_array {
            handles: &self.handle as *const _ as u64,
            count_handles: 1,
            pad: 0,
        };
        unsafe {
            drm_ioctl_syncobj_signal(fd, &mut signal_arg)?;
        }
        Ok(())
    }

    fn timeline_wait(&self, point: u64, timeout_ns: u64, flags: u32) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        // DRM timeline syncobj timeouts are signed 64-bit integers (__s64).
        // Clamp to i64::MAX to prevent UINT64_MAX from wrapping to -1.
        let timeout_nsec = timeout_ns.min(i64::MAX as u64) as i64;
        let mut wait_arg = drm_syncobj_timeline_wait {
            handles: &self.handle as *const _ as u64,
            points: &point as *const _ as u64,
            timeout_nsec,
            count_handles: 1,
            flags: flags | DRM_SYNCOBJ_WAIT_FLAGS_WAIT_ALL | DRM_SYNCOBJ_WAIT_FLAGS_WAIT_FOR_SUBMIT,
            first_signaled: 0,
            pad: 0,
            deadline_nsec: 0,
        };
        match unsafe { drm_ioctl_syncobj_timeline_wait(fd, &mut wait_arg) } {
            Ok(_) => Ok(()),
            Err(e) if e.raw_os_error() == Some(libc::ETIME) => Err(Error::TimedOut),
            Err(e) => Err(Error::PlatformError(e.into())),
        }
    }

    fn timeline_signal(&self, point: u64) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        let mut signal_arg = drm_syncobj_timeline_array {
            handles: &self.handle as *const _ as u64,
            points: &point as *const _ as u64,
            count_handles: 1,
            flags: 0,
        };
        unsafe {
            drm_ioctl_syncobj_timeline_signal(fd, &mut signal_arg)?;
        }
        Ok(())
    }

    fn timeline_query(&self) -> Result<u64> {
        let fd = self.physical_device.as_fd().unwrap();
        let mut point: u64 = 0;
        let mut query_arg = drm_syncobj_timeline_array {
            handles: &self.handle as *const _ as u64,
            points: &mut point as *mut _ as u64,
            count_handles: 1,
            flags: 0,
        };
        unsafe {
            drm_ioctl_syncobj_query(fd, &mut query_arg)?;
        }
        Ok(point)
    }

    fn export_fence(&self) -> Result<MagmaGpuHandle> {
        let fd = self.physical_device.as_fd().unwrap();
        let mut handle_arg = drm_syncobj_handle {
            handle: self.handle,
            flags: DRM_SYNCOBJ_HANDLE_TO_FD_FLAGS_EXPORT_SYNC_FILE,
            fd: -1,
            pad: 0,
        };
        unsafe {
            drm_ioctl_syncobj_handle_to_fd(fd, &mut handle_arg)?;
        }
        let descriptor = unsafe { OwnedDescriptor::from_raw_descriptor(handle_arg.fd) };
        Ok(MagmaGpuHandle {
            os_handle: descriptor,
            handle_type: MAGMA_GPU_HANDLE_TYPE_SIGNAL_SYNC_FD,
        })
    }

    fn import(&self, handle: MagmaGpuHandle) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        let mut sync_handle = drm_syncobj_handle {
            handle: self.handle,
            flags: DRM_SYNCOBJ_FD_TO_HANDLE_FLAGS_IMPORT_SYNC_FILE,
            fd: handle.os_handle.as_raw_descriptor(),
            pad: 0,
        };
        unsafe {
            drm_ioctl_syncobj_fd_to_handle(fd, &mut sync_handle)?;
        }
        Ok(())
    }

    fn as_raw_handle(&self) -> Option<u32> {
        Some(self.handle)
    }
}

impl AsVirtioSyncObject for DrmSyncObject {}
impl BackendSyncObject for DrmSyncObject {}

pub fn get_drm_device_name(descriptor: &OwnedDescriptor) -> Result<String> {
    let mut version = drm_version {
        version_major: 0,
        version_minor: 0,
        version_patchlevel: 0,
        name_len: 0,
        name: null_mut(),
        date_len: 0,
        date: null_mut(),
        desc_len: 0,
        desc: null_mut(),
    };

    // SAFETY:
    // Descriptor is valid and borrowed properly..
    unsafe {
        drm_get_version(descriptor.as_fd(), &mut version)?;
    }

    // Enough bytes to hold the device name and terminating null character.
    let mut name_bytes: Vec<u8> = vec![0; (version.name_len + 1) as usize];
    let mut version = drm_version {
        version_major: 0,
        version_minor: 0,
        version_patchlevel: 0,
        name_len: name_bytes.len() as __kernel_size_t,
        name: name_bytes.as_mut_ptr() as *mut c_char,
        date_len: 0,
        date: null_mut(),
        desc_len: 0,
        desc: null_mut(),
    };

    // SAFETY:
    // No more than name_len + 1 bytes will be written to name.
    unsafe {
        drm_get_version(descriptor.as_fd(), &mut version)?;
    }

    CString::new(&name_bytes[..(version.name_len as usize)])?
        .into_string()
        .map_err(|_| Error::InvalidArgs)
}
