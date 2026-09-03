// Copyright 2025 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::Arc;
use std::sync::Mutex;

use magma_gpu::util::Error as MagmaGpuError;
use magma_gpu::util::Handle as MagmaGpuHandle;

use crate::context_common::ContextResource;
use crate::context_common::ContextResources;
use crate::handle::RutabagaHandle;
use crate::resource::RutabagaResource;
use crate::rutabaga_core::RutabagaContext;
use crate::rutabaga_utils::ResourceCreateBlob;
use crate::rutabaga_utils::RutabagaComponentType;
use crate::rutabaga_utils::RutabagaFence;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaResult;
use crate::RutabagaIovec;

pub struct MagmaVirtioGpuContext {
    context_resources: ContextResources,
    _fence_handler: RutabagaFenceHandler,
}

impl MagmaVirtioGpuContext {
    pub fn new(fence_handler: RutabagaFenceHandler) -> MagmaVirtioGpuContext {
        MagmaVirtioGpuContext {
            context_resources: Arc::new(Mutex::new(Default::default())),
            _fence_handler: fence_handler,
        }
    }
}

impl RutabagaContext for MagmaVirtioGpuContext {
    fn context_create_blob(
        &mut self,
        _resource_id: u32,
        _resource_create_blob: ResourceCreateBlob,
        _iovec_opt: Option<Vec<RutabagaIovec>>,
        _handle_opt: Option<RutabagaHandle>,
    ) -> RutabagaResult<RutabagaResource> {
        Err(MagmaGpuError::Unsupported.into())
    }

    fn submit_cmd(
        &mut self,
        _commands: &mut [u8],
        _fence_ids: &[u64],
        _shareable_fences: Vec<MagmaGpuHandle>,
    ) -> RutabagaResult<()> {
        Ok(())
    }

    fn attach(&mut self, resource: &mut RutabagaResource) {
        self.context_resources.lock().unwrap().insert(
            resource.resource_id,
            ContextResource {
                handle: resource.handle.clone(),
                backing_iovecs: resource.backing_iovecs.take(),
            },
        );
    }

    fn detach(&mut self, resource: &RutabagaResource) {
        self.context_resources
            .lock()
            .unwrap()
            .remove(&resource.resource_id);
    }

    fn context_create_fence(
        &mut self,
        _fence: RutabagaFence,
    ) -> RutabagaResult<Option<MagmaGpuHandle>> {
        Ok(None)
    }

    fn component_type(&self) -> RutabagaComponentType {
        RutabagaComponentType::Magma
    }
}
