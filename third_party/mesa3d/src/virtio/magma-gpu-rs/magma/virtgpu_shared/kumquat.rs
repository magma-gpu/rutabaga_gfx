// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;
use std::sync::Mutex;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu::util::MappedRegion;
use magma_gpu::util::RawMapping;
use magma_gpu::util::DEFAULT_RAW_DESCRIPTOR;
use magma_gpu::virtgpu_kumquat::defines::VirtGpuResourceCreateBlob;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_BLOB_FLAG_USE_MAPPABLE;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_BLOB_FLAG_USE_SHAREABLE;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_BLOB_MEM_HOST3D;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_EXECBUF_RING_IDX;
use magma_gpu::virtgpu_kumquat::defines::VIRTGPU_KUMQUAT_EXECBUF_SHAREABLE_OUT;
use magma_gpu::virtgpu_kumquat::VirtGpuKumquat;

use crate::device::PhysicalDevice;
use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaHandleType;
use crate::protocol::MagmaVirtCapabilities;
use crate::sys::platform::PlatformPhysicalDevice;
use crate::traits::AsVirtGpu;
use crate::traits::BackendPhysicalDevice;
use crate::virtgpu_shared::decode_capabilities;
use crate::virtgpu_shared::DecodedPhysicalDevice;
use crate::virtgpu_shared::VirtGpuFeature;
use crate::virtgpu_shared::VirtGpuFeatures;
use crate::virtgpu_shared::VirtioGpuTransport;

pub const VIRTGPU_KUMQUAT_CAPSET_MAGMA: u32 = 7;

pub struct KumquatVirtioGpu {
    virtgpu: Mutex<VirtGpuKumquat>,
    pub caps: MagmaVirtCapabilities,
    pub device: DecodedPhysicalDevice,
    pub pdev_idx: u32,
}

struct KumquatMappedRegion {
    raw: RawMapping,
    physical_device: Arc<KumquatVirtioGpu>,
    bo_handle: u32,
}

impl AsVirtGpu for KumquatVirtioGpu {
    fn as_virtgpu(&self) -> Option<&Mutex<VirtGpuKumquat>> {
        Some(&self.virtgpu)
    }
}

impl PlatformPhysicalDevice for KumquatVirtioGpu {}
impl BackendPhysicalDevice for KumquatVirtioGpu {}

impl Drop for KumquatMappedRegion {
    fn drop(&mut self) {
        if let Ok(mut virtgpu) = self.physical_device.virtgpu.lock() {
            let _ = virtgpu.unmap(self.bo_handle);
        }
    }
}

unsafe impl Send for KumquatMappedRegion {}
unsafe impl Sync for KumquatMappedRegion {}

unsafe impl MappedRegion for KumquatMappedRegion {
    fn as_ptr(&self) -> *mut u8 {
        self.raw.ptr as *mut u8
    }

    fn size(&self) -> usize {
        self.raw.size as usize
    }

    fn as_raw_mapping(&self) -> RawMapping {
        self.raw
    }
}

impl VirtioGpuTransport for KumquatVirtioGpu {
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
        let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
        let _ctx_id = virtgpu.context_create(VIRTGPU_KUMQUAT_CAPSET_MAGMA as u64, "magma")?;
        Ok(())
    }

    fn create_blob(&self, blob_id: u64, size: usize) -> Result<(u32, u32)> {
        let mut create_blob = VirtGpuResourceCreateBlob {
            blob_mem: VIRTGPU_BLOB_MEM_HOST3D,
            blob_flags: VIRTGPU_BLOB_FLAG_USE_MAPPABLE | VIRTGPU_BLOB_FLAG_USE_SHAREABLE,
            bo_handle: 0,
            res_handle: 0,
            size: size as u64,
            pad: 0,
            cmd_size: 0,
            cmd: 0,
            blob_id,
        };
        let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
        virtgpu.resource_create_blob(&mut create_blob, &[])?;
        Ok((create_blob.bo_handle, create_blob.res_handle))
    }

    fn map_blob(
        self: &Arc<KumquatVirtioGpu>,
        bo_handle: u32,
        _size: usize,
    ) -> Result<Arc<dyn MappedRegion>> {
        let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
        let raw_mapping = virtgpu.map(bo_handle)?;
        Ok(Arc::new(KumquatMappedRegion {
            raw: raw_mapping,
            physical_device: self.clone(),
            bo_handle,
        }))
    }

    fn export_blob(&self, bo_handle: u32) -> Result<MagmaGpuHandle> {
        let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
        Ok(virtgpu.resource_export(bo_handle, 0)?)
    }

    fn close_blob(&self, bo_handle: u32) {
        if let Ok(mut virtgpu) = self.virtgpu.lock() {
            let _ = virtgpu.resource_unref(bo_handle);
        }
    }

    fn submit_cmd(&self, cmd: &[u8], ring_idx: u32, syncobjs: &[u32]) -> Result<()> {
        let mut flags = VIRTGPU_KUMQUAT_EXECBUF_RING_IDX;
        if !syncobjs.is_empty() {
            flags |= VIRTGPU_KUMQUAT_EXECBUF_SHAREABLE_OUT;
        }
        let mut raw_descriptor = DEFAULT_RAW_DESCRIPTOR;
        let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
        virtgpu.submit_command(
            flags,
            &[],
            cmd,
            ring_idx,
            &[],
            &mut raw_descriptor,
            syncobjs,
        )?;
        Ok(())
    }

    fn syncobj_create(&self, _initial_signaled: bool) -> Result<u32> {
        let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
        Ok(virtgpu.syncobj_create()?)
    }

    fn syncobj_destroy(&self, sync_handle: u32) {
        if let Ok(mut virtgpu) = self.virtgpu.lock() {
            let _ = virtgpu.syncobj_destroy(sync_handle);
        }
    }

    fn syncobj_wait(&self, sync_handle: u32, timeout_ns: u64) -> Result<()> {
        let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
        virtgpu
            .syncobj_wait(sync_handle, timeout_ns)
            .map_err(|_| Error::TimedOut)
    }

    fn syncobj_signal(&self, _sync_handle: u32) -> Result<()> {
        Ok(())
    }

    fn syncobj_export(&self, sync_handle: u32, is_cpu_signaled: bool) -> Result<MagmaGpuHandle> {
        if is_cpu_signaled {
            let (signaler, waiter) = magma_gpu::util::create_event_pair()?;
            signaler.signal()?;
            let handle: MagmaGpuHandle = waiter.into();
            let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
            virtgpu.syncobj_import(sync_handle, handle.try_clone()?)?;
            return Ok(handle);
        }
        let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
        Ok(virtgpu.syncobj_export(sync_handle)?)
    }

    fn syncobj_import(&self, sync_handle: u32, handle: MagmaGpuHandle) -> Result<()> {
        if handle.handle_type != MagmaHandleType::SignalSyncFd.bits()
            && handle.handle_type != MagmaHandleType::SignalEventFd.bits()
        {
            return Err(Error::InvalidArgs);
        }
        let mut virtgpu = self.virtgpu.lock().map_err(|_| Error::InternalError)?;
        Ok(virtgpu.syncobj_import(sync_handle, handle)?)
    }
}

pub fn enumerate_devices() -> Result<Vec<PhysicalDevice>> {
    let first_virtgpu = VirtGpuKumquat::new("/tmp/kumquat-gpu-0")?;

    let features = VirtGpuFeatures::new(|param| {
        let _ = first_virtgpu.get_param(param);
    });

    if !features.support(VirtGpuFeature::ResourceBlob)
        || !features.support(VirtGpuFeature::ContextInit)
        || !features.support_capset(VIRTGPU_KUMQUAT_CAPSET_MAGMA)
    {
        return Err(Error::Unimplemented);
    }

    let mut caps_bytes = [0u8; 4096];
    first_virtgpu.get_caps(VIRTGPU_KUMQUAT_CAPSET_MAGMA, &mut caps_bytes)?;
    let decoded = decode_capabilities(&caps_bytes)?;

    let mut devices: Vec<PhysicalDevice> = Vec::new();
    let mut first_virtgpu_opt = Some(first_virtgpu);

    for (idx, dev) in decoded.devices.into_iter().enumerate() {
        let virtgpu = match first_virtgpu_opt.take() {
            Some(v) => v,
            None => VirtGpuKumquat::new("/tmp/kumquat-gpu-0")?,
        };
        let info = dev.info;
        let kumquat = Arc::new(KumquatVirtioGpu {
            virtgpu: Mutex::new(virtgpu),
            caps: decoded.caps,
            device: dev,
            pdev_idx: idx as u32,
        });
        devices.push(PhysicalDevice::new(kumquat, info));
    }

    Ok(devices)
}
