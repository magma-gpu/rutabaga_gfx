// Copyright 2026 Magma-GPU project
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap as Map;
use std::mem::size_of;
use std::sync::Arc;
use std::sync::Mutex;

use magma_gpu::util::Error as MagmaGpuError;
use magma_gpu::util::Tube;
use zerocopy::IntoBytes;

use crate::cross_domain::atomic_memory_sentinel_manager::AtomicMemorySentinelManager;
use crate::cross_domain::context::CrossDomainContext;
use crate::cross_domain::cross_domain_protocol::CrossDomainCapabilities;
use crate::handle::RutabagaHandle;
use crate::rutabaga_core::RutabagaComponent;
use crate::rutabaga_core::RutabagaContext;
use crate::rutabaga_core::RutabagaResource;
use crate::rutabaga_core::VirtioFsLookup;
use crate::rutabaga_utils::ResourceCreateBlob;
use crate::rutabaga_utils::RutabagaComponentType;
use crate::rutabaga_utils::RutabagaFence;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaIovec;
use crate::rutabaga_utils::RutabagaPath;
use crate::rutabaga_utils::RutabagaResult;
use crate::rutabaga_utils::RUTABAGA_BLOB_FLAG_USE_MAPPABLE;
use crate::rutabaga_utils::RUTABAGA_BLOB_MEM_GUEST;
use crate::RutabagaGralloc;
use crate::RutabagaGrallocBackendFlags;

/// The CrossDomain component contains a list of paths that the guest may connect to and the
/// ability to allocate memory.
pub struct CrossDomain {
    paths: Option<Vec<RutabagaPath>>,
    gralloc: Arc<Mutex<RutabagaGralloc>>,
    fence_handler: RutabagaFenceHandler,
    internal_sockets: Arc<Mutex<Map<u128, Tube>>>,
    virtiofs_lookup: Option<Arc<dyn VirtioFsLookup>>,
}

impl CrossDomain {
    /// Initializes the cross-domain component by taking the the rutabaga paths (if any) and
    /// initializing rutabaga gralloc.
    pub fn init(
        paths: Option<Vec<RutabagaPath>>,
        fence_handler: RutabagaFenceHandler,
        virtiofs_lookup: Option<Arc<dyn VirtioFsLookup>>,
    ) -> RutabagaResult<Box<dyn RutabagaComponent>> {
        let gralloc = RutabagaGralloc::new(RutabagaGrallocBackendFlags::new())?;
        Ok(Box::new(CrossDomain {
            paths,
            gralloc: Arc::new(Mutex::new(gralloc)),
            fence_handler,
            internal_sockets: Arc::new(Mutex::new(Map::new())),
            virtiofs_lookup,
        }))
    }
}

impl RutabagaComponent for CrossDomain {
    fn get_capset_info(&self, _capset_id: u32) -> (u32, u32) {
        (0u32, size_of::<CrossDomainCapabilities>() as u32)
    }

    fn get_capset(&self, _capset_id: u32, _version: u32) -> Vec<u8> {
        let mut caps: CrossDomainCapabilities = Default::default();
        if let Some(ref paths) = self.paths {
            for path in paths {
                caps.supported_channels |= 1 << path.path_type;
            }
        }

        if self.gralloc.lock().unwrap().supports_dmabuf() {
            caps.supports_dmabuf = 1;
        }

        if self.gralloc.lock().unwrap().supports_external_gpu_memory() {
            caps.supports_external_gpu_memory = 1;
        }

        // Version 1 supports all commands up to and including CROSS_DOMAIN_CMD_WRITE.
        caps.version = 1;
        caps.as_bytes().to_vec()
    }

    fn create_blob(
        &mut self,
        _ctx_id: u32,
        resource_id: u32,
        resource_create_blob: ResourceCreateBlob,
        iovec_opt: Option<Vec<RutabagaIovec>>,
        _handle_opt: Option<RutabagaHandle>,
    ) -> RutabagaResult<RutabagaResource> {
        if resource_create_blob.blob_mem != RUTABAGA_BLOB_MEM_GUEST
            && resource_create_blob.blob_flags != RUTABAGA_BLOB_FLAG_USE_MAPPABLE
        {
            return Err(MagmaGpuError::WithContext("expected only guest memory blobs").into());
        }

        Ok(RutabagaResource {
            resource_id,
            handle: None,
            blob: true,
            blob_mem: resource_create_blob.blob_mem,
            blob_flags: resource_create_blob.blob_flags,
            map_info: None,
            info_2d: None,
            info_3d: None,
            vulkan_info: None,
            backing_iovecs: iovec_opt,
            component_mask: 1 << (RutabagaComponentType::CrossDomain as u8),
            size: resource_create_blob.size,
            mapping: None,
        })
    }

    fn create_context(
        &self,
        _ctx_id: u32,
        _context_init: u32,
        _context_name: Option<&str>,
        fence_handler: RutabagaFenceHandler,
    ) -> RutabagaResult<Box<dyn RutabagaContext>> {
        Ok(Box::new(CrossDomainContext {
            paths: self.paths.clone(),
            gralloc: self.gralloc.clone(),
            state: None,
            context_resources: Arc::new(Mutex::new(Default::default())),
            item_state: Arc::new(Mutex::new(Default::default())),
            sentinel_manager: Arc::new(Mutex::new(AtomicMemorySentinelManager::new(
                self.virtiofs_lookup.clone(),
            ))),
            fence_handler,
            virtiofs_lookup: self.virtiofs_lookup.clone(),
            internal_sockets: self.internal_sockets.clone(),
            worker_thread: None,
            resample_evt: None,
            kill_evt: None,
        }))
    }

    // With "drm/virtio: Conditionally allocate virtio_gpu_fence" in the kernel, global fences for
    // cross-domain aren't created.  However, that change is projected to land in the v6.6 kernel.
    // For older kernels, signal the fence immediately on creation.
    fn create_fence(&mut self, fence: RutabagaFence) -> RutabagaResult<()> {
        self.fence_handler.call(fence);
        Ok(())
    }
}
