// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use std::ffi::CString;
use std::os::fd::AsFd;
use std::os::raw::c_char;
use std::os::raw::c_uint;
use std::ptr::null_mut;

use mesa3d_util::MesaError;
use mesa3d_util::MesaResult;
use mesa3d_util::OwnedDescriptor;

use crate::ioctl_readwrite;
use crate::ioctl_write_ptr;

use crate::sys::linux::bindings::drm_bindings::__kernel_size_t;
use crate::sys::linux::bindings::drm_bindings::drm_gem_close;
use crate::sys::linux::bindings::drm_bindings::drm_prime_handle;
use crate::sys::linux::bindings::drm_bindings::drm_version;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;

pub const DRM_DIR_NAME: &str = "/dev/dri";
pub const DRM_RENDER_MINOR_NAME: &str = "renderD";
const DRM_IOCTL_VERSION: c_uint = 0x00;

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

pub fn get_drm_device_name(descriptor: &OwnedDescriptor) -> MesaResult<String> {
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
        .map_err(|_| MesaError::WithContext("couldn't convert string"))
}
