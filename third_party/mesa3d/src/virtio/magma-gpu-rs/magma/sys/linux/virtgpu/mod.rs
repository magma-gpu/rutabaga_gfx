// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

mod ioctl;

use std::fs::OpenOptions;
use std::os::fd::AsFd;
use std::path::Path;
use std::sync::Arc;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;
use magma_gpu::util::OwnedDescriptor;

use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaVirtCapabilities;
use crate::sys::linux::virtgpu::ioctl::virtgpu_context_init;
use crate::sys::linux::virtgpu::ioctl::virtgpu_execbuffer;
use crate::sys::linux::virtgpu::ioctl::virtgpu_get_caps;
use crate::sys::linux::virtgpu::ioctl::virtgpu_getparam;
use crate::sys::linux::virtgpu::ioctl::virtgpu_map;
use crate::sys::linux::virtgpu::ioctl::virtgpu_resource_create_blob;
use crate::sys::linux::virtgpu::ioctl::virtgpu_syncobj_create;
use crate::sys::linux::virtgpu::ioctl::virtgpu_syncobj_destroy;
use crate::sys::linux::virtgpu::ioctl::virtgpu_syncobj_export;
use crate::sys::linux::virtgpu::ioctl::virtgpu_syncobj_import;
use crate::sys::linux::virtgpu::ioctl::virtgpu_syncobj_signal;
use crate::sys::linux::virtgpu::ioctl::virtgpu_syncobj_wait;
use crate::sys::linux::PlatformPhysicalDevice;
use crate::traits::AsVirtGpu;
use crate::traits::BackendPhysicalDevice;
use crate::virtgpu_shared::decode_capabilities;
use crate::virtgpu_shared::DecodedPhysicalDevice;
use crate::virtgpu_shared::VirtGpuFeature;
use crate::virtgpu_shared::VirtGpuFeatures;
use crate::virtgpu_shared::VirtioGpuTransport;

pub const VIRTGPU_CAPSET_MAGMA: u32 = 7;

#[derive(Debug)]
pub struct DrmVirtioGpu {
    descriptor: OwnedDescriptor,
    pub caps: MagmaVirtCapabilities,
    pub device: DecodedPhysicalDevice,
    pub pdev_idx: u32,
}

pub fn virtgpu_enumerate_devices(
    first_descriptor: OwnedDescriptor,
    render_path: &Path,
) -> Result<Vec<Arc<dyn BackendPhysicalDevice>>> {
    let features = VirtGpuFeatures::new(|param| {
        virtgpu_getparam(first_descriptor.as_fd(), param.param, &mut param.value);
    });

    if !features.support(VirtGpuFeature::ResourceBlob)
        || !features.support(VirtGpuFeature::ContextInit)
        || !features.support_capset(VIRTGPU_CAPSET_MAGMA)
    {
        return Err(Error::Unimplemented);
    }

    let mut caps_bytes = [0u8; 4096];
    virtgpu_get_caps(
        first_descriptor.as_fd(),
        VIRTGPU_CAPSET_MAGMA,
        &mut caps_bytes,
    )?;
    let decoded = decode_capabilities(&caps_bytes)?;

    let mut devices: Vec<Arc<dyn BackendPhysicalDevice>> = Vec::new();
    let mut first_desc_opt = Some(first_descriptor);

    for (idx, dev) in decoded.devices.into_iter().enumerate() {
        let descriptor = match first_desc_opt.take() {
            Some(d) => d,
            None => OpenOptions::new()
                .read(true)
                .write(true)
                .open(render_path)?
                .into(),
        };
        devices.push(Arc::new(DrmVirtioGpu {
            descriptor,
            caps: decoded.caps,
            device: dev,
            pdev_idx: idx as u32,
        }));
    }

    Ok(devices)
}

impl PlatformPhysicalDevice for DrmVirtioGpu {
    fn as_descriptor(&self) -> Option<&OwnedDescriptor> {
        Some(&self.descriptor)
    }
}

impl AsVirtGpu for DrmVirtioGpu {}
impl BackendPhysicalDevice for DrmVirtioGpu {}

impl VirtioGpuTransport for DrmVirtioGpu {
    fn caps(&self) -> &MagmaVirtCapabilities {
        &self.caps
    }

    fn device(&self) -> &DecodedPhysicalDevice {
        &self.device
    }

    fn pdev_idx(&self) -> u32 {
        self.pdev_idx
    }

    fn init_context(&self) -> Result<()> {
        virtgpu_context_init(self.as_fd().unwrap(), VIRTGPU_CAPSET_MAGMA, 64)
    }

    fn create_blob(&self, blob_id: u64, size: usize) -> Result<(u32, u32)> {
        virtgpu_resource_create_blob(self.as_fd().unwrap(), blob_id, size)
    }

    fn map_blob(
        self: &Arc<DrmVirtioGpu>,
        bo_handle: u32,
        size: usize,
    ) -> Result<Arc<dyn MappedRegion>> {
        let offset = virtgpu_map(self.as_fd().unwrap(), bo_handle)?;
        let mapping = self.cpu_map(offset, size)?;
        Ok(Arc::new(mapping))
    }

    fn export_blob(&self, bo_handle: u32) -> Result<MagmaGpuHandle> {
        self.export(bo_handle)
    }

    fn close_blob(&self, bo_handle: u32) {
        self.close(bo_handle);
    }

    fn submit_cmd(&self, cmd: &[u8], ring_idx: u32, syncobjs: &[u32]) -> Result<()> {
        virtgpu_execbuffer(self.as_fd().unwrap(), cmd, ring_idx, syncobjs)
    }

    fn syncobj_create(&self, initial_signaled: bool) -> Result<u32> {
        virtgpu_syncobj_create(self.as_fd().unwrap(), initial_signaled)
    }

    fn syncobj_destroy(&self, sync_handle: u32) {
        virtgpu_syncobj_destroy(self.as_fd().unwrap(), sync_handle);
    }

    fn syncobj_wait(&self, sync_handle: u32, timeout_ns: u64) -> Result<()> {
        virtgpu_syncobj_wait(self.as_fd().unwrap(), sync_handle, timeout_ns)
    }

    fn syncobj_signal(&self, sync_handle: u32) -> Result<()> {
        virtgpu_syncobj_signal(self.as_fd().unwrap(), sync_handle)
    }

    fn syncobj_export(&self, sync_handle: u32, _is_cpu_signaled: bool) -> Result<MagmaGpuHandle> {
        virtgpu_syncobj_export(self.as_fd().unwrap(), sync_handle)
    }

    fn syncobj_import(&self, sync_handle: u32, handle: MagmaGpuHandle) -> Result<()> {
        virtgpu_syncobj_import(self.as_fd().unwrap(), sync_handle, handle)
    }
}
