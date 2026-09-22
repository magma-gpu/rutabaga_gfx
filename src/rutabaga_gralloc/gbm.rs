// Copyright 2018 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Implements swapchain allocation using Mesa's GBM (Generic Buffer Management) library.

#![cfg(feature = "gbm")]

use std::fs::File;
use std::io::Error;
use std::io::Seek;
use std::io::SeekFrom;
use std::sync::Arc;

use magma_gpu::util::AsRawDescriptor;
use magma_gpu::util::Error as MagmaGpuError;
use magma_gpu::util::FromRawDescriptor;
use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MAGMA_GPU_HANDLE_TYPE_MEM_DMABUF;

use crate::rutabaga_gralloc::formats::DrmFormat;
use crate::rutabaga_gralloc::gbm_bindings::*;
use crate::rutabaga_gralloc::gralloc::Gralloc;
use crate::rutabaga_gralloc::gralloc::ImageAllocationInfo;
use crate::rutabaga_gralloc::gralloc::ImageMemoryRequirements;
use crate::rutabaga_gralloc::gralloc::RutabagaGrallocFlags;
use crate::rutabaga_gralloc::gralloc::RUTABAGA_GRALLOC_USE_LINEAR;
use crate::rutabaga_gralloc::gralloc::RUTABAGA_GRALLOC_USE_PROTECTED;
use crate::rutabaga_gralloc::gralloc::RUTABAGA_GRALLOC_USE_RENDERING;
use crate::rutabaga_gralloc::gralloc::RUTABAGA_GRALLOC_USE_SCANOUT;
use crate::rutabaga_gralloc::rendernode;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaResult;
use crate::rutabaga_utils::RUTABAGA_MAP_CACHE_CACHED;
use crate::rutabaga_utils::RUTABAGA_MAP_CACHE_WC;

struct GbmDeviceInner {
    _fd: File,
    gbm: *mut gbm_device,
}

// SAFETY:
// Safe because GBM handles synchronization internally.
unsafe impl Send for GbmDeviceInner {}
// SAFETY:
// Safe because GBM handles synchronization internally.
unsafe impl Sync for GbmDeviceInner {}

impl Drop for GbmDeviceInner {
    fn drop(&mut self) {
        // SAFETY:
        // Safe because GbmDeviceInner is only constructed with a valid gbm_device.
        unsafe {
            gbm_device_destroy(self.gbm);
        }
    }
}

/// A device capable of allocating `GbmBuffer`.
#[derive(Clone)]
pub struct GbmDevice {
    gbm_device: Arc<GbmDeviceInner>,
    last_buffer: Option<Arc<GbmBuffer>>,
    device_name: String,
}

impl GbmDevice {
    /// Returns a new `GbmDevice` if there is a rendernode in `/dev/dri/` that is accepted by
    /// the gbm library.
    pub fn init() -> RutabagaResult<Box<dyn Gralloc>> {
        // Filter out virtual DRM devices like "vgem" which do not support hardware-accelerated
        // rendering or scanout allocation.
        let undesired: &[&str] = &["vgem"];
        let (descriptor, device_name) = rendernode::open_device(undesired)?;

        // SAFETY:
        // gbm_create_device is safe to call with a valid fd, and we check that a valid one is
        // returned.  If the fd does not refer to a DRM device, gbm_create_device will reject it.
        let gbm = unsafe { gbm_create_device(descriptor.as_raw_descriptor()) };
        if gbm.is_null() {
            return Err(MagmaGpuError::IoError(Error::last_os_error()).into());
        }

        Ok(Box::new(GbmDevice {
            gbm_device: Arc::new(GbmDeviceInner {
                _fd: descriptor,
                gbm,
            }),
            last_buffer: None,
            device_name,
        }))
    }
}

/// Translates guest RutabagaGrallocFlags (minigbm usage bits) to upstream Mesa GBM bo flags.
pub fn rutabaga_gralloc_flags_to_gbm_flags(flags: RutabagaGrallocFlags) -> u32 {
    let mut gbm_flags = 0;

    if flags.0 & RUTABAGA_GRALLOC_USE_SCANOUT != 0 {
        gbm_flags |= GBM_BO_USE_SCANOUT;
    }
    if flags.0 & RUTABAGA_GRALLOC_USE_RENDERING != 0 {
        gbm_flags |= GBM_BO_USE_RENDERING;
    }
    if flags.0 & RUTABAGA_GRALLOC_USE_LINEAR != 0 {
        gbm_flags |= GBM_BO_USE_LINEAR;
    }
    if flags.0 & RUTABAGA_GRALLOC_USE_PROTECTED != 0 {
        gbm_flags |= GBM_BO_USE_PROTECTED;
    }

    gbm_flags
}

impl Gralloc for GbmDevice {
    fn supports_external_gpu_memory(&self) -> bool {
        true
    }

    fn supports_dmabuf(&self) -> bool {
        true
    }

    fn get_image_memory_requirements(
        &mut self,
        info: ImageAllocationInfo,
    ) -> RutabagaResult<ImageMemoryRequirements> {
        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        let bo = unsafe {
            gbm_bo_create(
                self.gbm_device.gbm,
                info.width,
                info.height,
                info.drm_format.0,
                rutabaga_gralloc_flags_to_gbm_flags(info.flags),
            )
        };
        if bo.is_null() {
            return Err(MagmaGpuError::IoError(Error::last_os_error()).into());
        }

        let mut reqs: ImageMemoryRequirements = Default::default();
        let gbm_buffer = GbmBuffer {
            bo,
            _device: self.clone(),
        };

        if info.flags.uses_scanout() {
            reqs.map_info = RUTABAGA_MAP_CACHE_WC;
        } else if self.device_name == "i915" || self.device_name == "xe" {
            reqs.map_info = RUTABAGA_MAP_CACHE_CACHED;
        } else {
            reqs.map_info = RUTABAGA_MAP_CACHE_WC;
        }

        reqs.modifier = gbm_buffer.format_modifier();
        for plane in 0..gbm_buffer.num_planes() {
            reqs.strides[plane] = gbm_buffer.plane_stride(plane);
            reqs.offsets[plane] = gbm_buffer.plane_offset(plane);
        }

        let mut fd = gbm_buffer.export()?;
        let size = fd.seek(SeekFrom::End(0)).map_err(MagmaGpuError::IoError)?;

        // Upstream Mesa GBM doesn't have a TEST_ALLOC flag to query requirements without
        // allocating memory. We stash the allocated buffer so allocate_memory can reuse it.
        // If a previous buffer was stashed and never consumed, replacing it drops and frees it cleanly.
        self.last_buffer = Some(Arc::new(gbm_buffer));
        reqs.info = info;
        reqs.size = size;
        Ok(reqs)
    }

    fn allocate_memory(&mut self, reqs: ImageMemoryRequirements) -> RutabagaResult<MagmaGpuHandle> {
        let last_buffer = self.last_buffer.take();
        if let Some(gbm_buffer) = last_buffer {
            if gbm_buffer.width() == reqs.info.width
                && gbm_buffer.height() == reqs.info.height
                && gbm_buffer.format() == reqs.info.drm_format
            {
                let dmabuf = gbm_buffer.export()?.into();
                return Ok(MagmaGpuHandle {
                    os_handle: dmabuf,
                    handle_type: MAGMA_GPU_HANDLE_TYPE_MEM_DMABUF,
                });
            }
            // If dimensions or format don't match, drop the stashed buffer and fall through
            // to allocate a new one matching the requested requirements.
        }

        // TODO(b/315870313): Add safety comment
        #[allow(clippy::undocumented_unsafe_blocks)]
        let bo = unsafe {
            gbm_bo_create(
                self.gbm_device.gbm,
                reqs.info.width,
                reqs.info.height,
                reqs.info.drm_format.0,
                rutabaga_gralloc_flags_to_gbm_flags(reqs.info.flags),
            )
        };

        if bo.is_null() {
            return Err(MagmaGpuError::IoError(Error::last_os_error()).into());
        }

        let gbm_buffer = GbmBuffer {
            bo,
            _device: self.clone(),
        };
        let dmabuf = gbm_buffer.export()?.into();
        Ok(MagmaGpuHandle {
            os_handle: dmabuf,
            handle_type: MAGMA_GPU_HANDLE_TYPE_MEM_DMABUF,
        })
    }
}

/// An allocation from a `GbmDevice`.
pub struct GbmBuffer {
    bo: *mut gbm_bo,
    _device: GbmDevice,
}

// SAFETY:
// Safe because GBM handles synchronization internally.
unsafe impl Send for GbmBuffer {}
// SAFETY:
// Safe because GBM handles synchronization internally.
unsafe impl Sync for GbmBuffer {}

impl GbmBuffer {
    /// Width in pixels.
    pub fn width(&self) -> u32 {
        // SAFETY:
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_width(self.bo) }
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        // SAFETY:
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_height(self.bo) }
    }

    /// `DrmFormat` of the buffer.
    pub fn format(&self) -> DrmFormat {
        // SAFETY:
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { DrmFormat(gbm_bo_get_format(self.bo)) }
    }

    /// DrmFormat modifier flags for the buffer.
    pub fn format_modifier(&self) -> u64 {
        // SAFETY:
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_modifier(self.bo) }
    }

    /// Number of planes present in this buffer.
    pub fn num_planes(&self) -> usize {
        // SAFETY:
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_plane_count(self.bo) as usize }
    }

    /// Offset in bytes for the given plane.
    pub fn plane_offset(&self, plane: usize) -> u32 {
        // SAFETY:
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_offset(self.bo, plane) }
    }

    /// Length in bytes of one row for the given plane.
    pub fn plane_stride(&self, plane: usize) -> u32 {
        // SAFETY:
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_get_stride_for_plane(self.bo, plane) }
    }

    /// Exports a new dmabuf/prime file descriptor.
    pub fn export(&self) -> RutabagaResult<File> {
        // SAFETY:
        // This is always safe to call with a valid gbm_bo pointer.
        match unsafe { gbm_bo_get_fd(self.bo) } {
            fd if fd >= 0 => {
                // SAFETY: fd is expected to be valid.
                let dmabuf = unsafe { File::from_raw_descriptor(fd) };
                Ok(dmabuf)
            }
            ret => Err(RutabagaError::ComponentError(ret)),
        }
    }
}

impl Drop for GbmBuffer {
    fn drop(&mut self) {
        // SAFETY:
        // This is always safe to call with a valid gbm_bo pointer.
        unsafe { gbm_bo_destroy(self.bo) }
    }
}
