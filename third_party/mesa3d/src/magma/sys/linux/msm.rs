// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::ioctl_readwrite;
use crate::ioctl_write_ptr;

use mesa3d_util::MappedRegion;
use mesa3d_util::MesaError;
use mesa3d_util::MesaHandle;
use mesa3d_util::MesaResult;

use crate::traits::Buffer;
use crate::traits::Context;
use crate::traits::Device;
use crate::traits::GenericBuffer;
use crate::traits::GenericDevice;
use crate::traits::PhysicalDevice;

use crate::magma_defines::MagmaCreateBufferInfo;
use crate::magma_defines::MagmaHeapBudget;
use crate::magma_defines::MagmaImportHandleInfo;
use crate::magma_defines::MagmaMappedMemoryRange;
use crate::magma_defines::MagmaMemoryProperties;

use crate::sys::linux::bindings::drm_bindings::DRM_COMMAND_BASE;
use crate::sys::linux::bindings::drm_bindings::DRM_IOCTL_BASE;
use crate::sys::linux::bindings::msm_bindings::*;
use crate::sys::linux::PlatformDevice;

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

struct MsmContext {
    physical_device: Arc<dyn PhysicalDevice>,
    submit_queue_id: u32,
}

impl Drop for MsmContext {
    fn drop(&mut self) {
        // SAFETY: This is a valid file descriptor and a valid submitqueue id.
        unsafe {
            let _ =
                msm_submitqueue_close(self.physical_device.as_fd().unwrap(), &self.submit_queue_id);
        }
    }
}

impl Context for MsmContext {}

pub struct Msm {
    physical_device: Arc<dyn PhysicalDevice>,
    mem_props: MagmaMemoryProperties,
}

struct MsmBuffer {
    physical_device: Arc<dyn PhysicalDevice>,
    gem_handle: u32,
    size: usize,
}

impl Msm {
    pub fn new(physical_device: Arc<dyn PhysicalDevice>) -> Msm {
        Msm {
            physical_device,
            mem_props: Default::default(),
        }
    }
}

impl GenericDevice for Msm {
    fn get_memory_properties(&self) -> MesaResult<MagmaMemoryProperties> {
        Err(MesaError::Unsupported)
    }

    fn get_memory_budget(&self, _heap_idx: u32) -> MesaResult<MagmaHeapBudget> {
        Err(MesaError::Unsupported)
    }

    fn create_context(&self, _device: &Arc<dyn Device>) -> MesaResult<Arc<dyn Context>> {
        let mut new_submit_queue = drm_msm_submitqueue {
            flags: 0,
            prio: 0,
            ..Default::default()
        };

        // SAFETY: This is a valid file descriptor.
        unsafe {
            msm_submitqueue_new(self.physical_device.as_fd().unwrap(), &mut new_submit_queue)?;
        }

        Ok(Arc::new(MsmContext {
            physical_device: self.physical_device.clone(),
            submit_queue_id: new_submit_queue.id,
        }))
    }

    fn create_buffer(
        &self,
        _device: &Arc<dyn Device>,
        create_info: &MagmaCreateBufferInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        let buf = MsmBuffer::new(self.physical_device.clone(), create_info, &self.mem_props)?;
        Ok(Arc::new(buf))
    }

    fn import(
        &self,
        _device: &Arc<dyn Device>,
        info: MagmaImportHandleInfo,
    ) -> MesaResult<Arc<dyn Buffer>> {
        let gem_handle = self.physical_device.import(info.handle)?;
        let buf = MsmBuffer::from_existing(
            self.physical_device.clone(),
            gem_handle,
            info.size.try_into()?,
        )?;
        Ok(Arc::new(buf))
    }
}

impl PlatformDevice for Msm {}
impl Device for Msm {}

impl MsmBuffer {
    fn new(
        physical_device: Arc<dyn PhysicalDevice>,
        create_info: &MagmaCreateBufferInfo,
        _mem_props: &MagmaMemoryProperties,
    ) -> MesaResult<MsmBuffer> {
        let mut gem_new = drm_msm_gem_new {
            size: create_info.size,
            flags: 0,
            ..Default::default()
        };

        // SAFETY: This is a well-formed ioctl conforming the driver specificiation.
        unsafe {
            drm_ioctl_msm_gem_new(physical_device.as_fd().unwrap(), &mut gem_new)?;
        }

        Ok(MsmBuffer {
            physical_device,
            gem_handle: gem_new.handle,
            size: create_info.size.try_into()?,
        })
    }

    fn from_existing(
        physical_device: Arc<dyn PhysicalDevice>,
        gem_handle: u32,
        size: usize,
    ) -> MesaResult<MsmBuffer> {
        Ok(MsmBuffer {
            physical_device,
            gem_handle,
            size,
        })
    }
}

impl GenericBuffer for MsmBuffer {
    fn map(&self, _buffer: &Arc<dyn Buffer>) -> MesaResult<Arc<dyn MappedRegion>> {
        let mut gem_info: drm_msm_gem_info = drm_msm_gem_info {
            handle: self.gem_handle,
            info: MSM_INFO_GET_OFFSET,
            ..Default::default()
        };

        // SAFETY:
        // Valid arguments are supplied for the following arguments:
        //   - Underlying descriptor
        //   - drm_msm_gem_info
        let offset = unsafe {
            drm_ioctl_msm_gem_info(self.physical_device.as_fd().unwrap(), &mut gem_info)?;
            gem_info.value
        };

        let mapping = self.physical_device.cpu_map(offset, self.size)?;
        Ok(Arc::new(mapping))
    }

    fn export(&self) -> MesaResult<MesaHandle> {
        self.physical_device.export(self.gem_handle)
    }

    fn invalidate(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> MesaResult<()> {
        let prep = drm_msm_gem_cpu_prep {
            handle: self.gem_handle,
            op: MSM_PREP_READ | MSM_PREP_WRITE,
            ..Default::default()
        };

        // SAFETY: This is a valid file descriptor and a valid gem handle.
        unsafe {
            msm_gem_cpu_prep(self.physical_device.as_fd().unwrap(), &prep)?;
        }
        Ok(())
    }

    fn flush(&self, _sync_flags: u64, _ranges: &[MagmaMappedMemoryRange]) -> MesaResult<()> {
        let fini = drm_msm_gem_cpu_fini {
            handle: self.gem_handle,
        };

        // SAFETY: This is a valid file descriptor and a valid gem handle.
        unsafe {
            msm_gem_cpu_fini(self.physical_device.as_fd().unwrap(), &fini)?;
        }
        Ok(())
    }
}

impl Drop for MsmBuffer {
    fn drop(&mut self) {
        // GEM close
    }
}

impl Buffer for MsmBuffer {}

unsafe impl Send for Msm {}
unsafe impl Sync for Msm {}

unsafe impl Send for MsmContext {}
unsafe impl Sync for MsmContext {}

unsafe impl Send for MsmBuffer {}
unsafe impl Sync for MsmBuffer {}
