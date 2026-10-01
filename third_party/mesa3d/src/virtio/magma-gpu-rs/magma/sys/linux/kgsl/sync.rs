// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use magma_gpu::util::AsRawDescriptor;
use magma_gpu::util::Handle as MagmaGpuHandle;

use crate::defines::MagmaImportHandleInfo;
use crate::error::Result;
use crate::sys::linux::kgsl::ioctl;
use crate::traits::AsVirtioSyncObject;
use crate::traits::BackendPhysicalDevice;
use crate::traits::BackendSyncObject;
use crate::traits::GenericSyncObject;

pub struct KgslSyncObject {
    physical_device: Arc<dyn BackendPhysicalDevice>,
    syncsource_id: u32,
    fence_fd: i32,
}

impl KgslSyncObject {
    pub fn new(physical_device: Arc<dyn BackendPhysicalDevice>) -> Result<KgslSyncObject> {
        let fd = physical_device.as_fd().unwrap();
        let (syncsource_id, fence_fd) = ioctl::syncsource_create_with_fence(fd)?;
        Ok(KgslSyncObject {
            physical_device,
            syncsource_id,
            fence_fd,
        })
    }

    pub fn from_import(
        physical_device: Arc<dyn BackendPhysicalDevice>,
        info: &MagmaImportHandleInfo,
    ) -> Result<KgslSyncObject> {
        let dup_fd = ioctl::dup_fence_fd(info.handle.os_handle.as_raw_descriptor())?;
        Ok(KgslSyncObject {
            physical_device,
            syncsource_id: 0,
            fence_fd: dup_fd,
        })
    }
}

impl Drop for KgslSyncObject {
    fn drop(&mut self) {
        ioctl::close_fence_fd(self.fence_fd);
        if self.syncsource_id != 0 {
            ioctl::syncsource_destroy(self.physical_device.as_fd().unwrap(), self.syncsource_id);
        }
    }
}

impl GenericSyncObject for KgslSyncObject {
    fn wait(&self, timeout_ns: u64) -> Result<()> {
        ioctl::poll_fence_fd(self.fence_fd, timeout_ns)
    }

    fn signal(&self) -> Result<()> {
        let fd = self.physical_device.as_fd().unwrap();
        ioctl::syncsource_signal_fence(fd, self.syncsource_id, self.fence_fd)
    }

    fn export_fence(&self) -> Result<MagmaGpuHandle> {
        ioctl::export_fence_fd(self.fence_fd)
    }

    fn as_raw_handle(&self) -> Option<u32> {
        if self.fence_fd >= 0 {
            Some(self.fence_fd as u32)
        } else {
            None
        }
    }
}

impl AsVirtioSyncObject for KgslSyncObject {}
impl BackendSyncObject for KgslSyncObject {}
