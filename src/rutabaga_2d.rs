// Copyright 2020 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! rutabaga_2d: Handles 2D virtio-gpu hypercalls.

use std::io::IoSlice;
use std::io::IoSliceMut;

use magma_gpu::util::Error as MagmaGpuError;

use crate::handle::RutabagaHandle;
use crate::resource::Rutabaga2DInfo;
use crate::resource::RutabagaResource;
use crate::rutabaga_core::RutabagaComponent;
use crate::rutabaga_utils::ResourceCreate3D;
use crate::rutabaga_utils::ResourceCreateBlob;
use crate::rutabaga_utils::RutabagaComponentType;
use crate::rutabaga_utils::RutabagaFence;
use crate::rutabaga_utils::RutabagaFenceHandler;
use crate::rutabaga_utils::RutabagaIovec;
use crate::rutabaga_utils::RutabagaResult;
use crate::rutabaga_utils::Transfer3D;
use crate::snapshot::RutabagaSnapshotReader;
use crate::snapshot::RutabagaSnapshotWriter;
use crate::RUTABAGA_BLOB_MEM_GUEST;

pub struct Rutabaga2D {
    fence_handler: RutabagaFenceHandler,
}

impl Rutabaga2D {
    pub fn init(fence_handler: RutabagaFenceHandler) -> RutabagaResult<Box<dyn RutabagaComponent>> {
        Ok(Box::new(Rutabaga2D { fence_handler }))
    }
}

impl RutabagaComponent for Rutabaga2D {
    fn create_fence(&mut self, fence: RutabagaFence) -> RutabagaResult<()> {
        self.fence_handler.call(fence);
        Ok(())
    }

    fn create_3d(
        &self,
        resource_id: u32,
        resource_create_3d: ResourceCreate3D,
    ) -> RutabagaResult<RutabagaResource> {
        RutabagaResource::new_2d(
            resource_id,
            resource_create_3d,
            RutabagaComponentType::Rutabaga2D,
        )
    }

    // Blob resources may be used for scanout of images with non-packed stride.
    fn create_blob(
        &mut self,
        _ctx_id: u32,
        resource_id: u32,
        resource_create_blob: ResourceCreateBlob,
        iovec_opt: Option<Vec<RutabagaIovec>>,
        _handle_opt: Option<RutabagaHandle>,
    ) -> RutabagaResult<RutabagaResource> {
        if resource_create_blob.blob_mem != RUTABAGA_BLOB_MEM_GUEST {
            return Err(MagmaGpuError::Unsupported.into());
        }

        let info_2d = Rutabaga2DInfo {
            width: 0,
            height: 0,
            host_mem: None,
            scanout_stride: None,
        };

        Ok(RutabagaResource {
            resource_id,
            handle: None,
            blob: true,
            blob_mem: resource_create_blob.blob_mem,
            blob_flags: resource_create_blob.blob_flags,
            map_info: None,
            info_2d: Some(info_2d),
            info_3d: None,
            vulkan_info: None,
            backing_iovecs: iovec_opt,
            component_mask: 1 << (RutabagaComponentType::Rutabaga2D as u8),
            size: resource_create_blob.size,
            mapping: None,
        })
    }

    fn transfer_write(
        &self,
        _ctx_id: u32,
        resource: &mut RutabagaResource,
        transfer: Transfer3D,
        buf: Option<IoSlice>,
    ) -> RutabagaResult<()> {
        resource.transfer_write_2d(transfer, buf)
    }

    fn transfer_read(
        &self,
        _ctx_id: u32,
        resource: &mut RutabagaResource,
        transfer: Transfer3D,
        buf: Option<IoSliceMut>,
    ) -> RutabagaResult<()> {
        resource.transfer_read_2d(transfer, buf)
    }

    fn snapshot(&self, writer: RutabagaSnapshotWriter) -> RutabagaResult<()> {
        let v = serde_json::Value::String("rutabaga2d".to_string());
        writer.add_fragment("rutabaga2d_snapshot", &v)?;
        Ok(())
    }

    fn restore(&self, reader: RutabagaSnapshotReader) -> RutabagaResult<()> {
        let _: serde_json::Value = reader.get_fragment("rutabaga2d_snapshot")?;
        Ok(())
    }
}
