// Copyright 2025 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::magma::context::MagmaVirtioGpuContext;
use crate::rutabaga_core::RutabagaComponent;
use crate::rutabaga_core::RutabagaContext;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaResult;

pub struct MagmaVirtioGpu {
    _fence_handler: RutabagaFenceHandler,
}

impl MagmaVirtioGpu {
    /// Initializes the magma component.
    pub fn init(
        _fence_handler: RutabagaFenceHandler,
    ) -> RutabagaResult<Box<dyn RutabagaComponent>> {
        Ok(Box::new(MagmaVirtioGpu { _fence_handler }))
    }
}

impl RutabagaComponent for MagmaVirtioGpu {
    fn get_capset_info(&self, _capset_id: u32) -> (u32, u32) {
        (0u32, 0u32)
    }

    fn get_capset(&self, _capset_id: u32, _version: u32) -> Vec<u8> {
        Vec::new()
    }

    fn create_context(
        &self,
        _ctx_id: u32,
        _context_init: u32,
        _context_name: Option<&str>,
        _fence_handler: RutabagaFenceHandler,
    ) -> RutabagaResult<Box<dyn RutabagaContext>> {
        Ok(Box::new(MagmaVirtioGpuContext::new(_fence_handler)))
    }
}
