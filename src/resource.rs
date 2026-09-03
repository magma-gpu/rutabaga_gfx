// Copyright 2026 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::sync::Arc;

use magma_gpu::util::MemoryMapping;
use serde::Deserialize;
use serde::Serialize;

use crate::handle::RutabagaHandle;
use crate::rutabaga_utils::Resource3DInfo;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaIovec;
use crate::rutabaga_utils::VulkanInfo;

/// Information required for 2D functionality.
#[derive(Clone, Deserialize, Serialize)]
pub struct Rutabaga2DInfo {
    pub width: u32,
    pub height: u32,
    pub host_mem: Option<Vec<u8>>,
    pub scanout_stride: Option<u32>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct Rutabaga2DSnapshot {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// A Rutabaga resource, supporting 2D and 3D rutabaga features.  Assumes a single-threaded library.
pub struct RutabagaResource {
    pub resource_id: u32,
    pub handle: Option<Arc<RutabagaHandle>>,
    pub blob: bool,
    pub blob_mem: u32,
    pub blob_flags: u32,
    pub map_info: Option<u32>,
    pub info_2d: Option<Rutabaga2DInfo>,
    pub info_3d: Option<Resource3DInfo>,
    pub vulkan_info: Option<VulkanInfo>,
    pub backing_iovecs: Option<Vec<RutabagaIovec>>,
    /// Bitmask of components that have already imported this resource
    pub component_mask: u8,
    pub size: u64,
    pub mapping: Option<MemoryMapping>,
}

/// The preserved fields of `RutabagaResource` that are saved and loaded across snapshot and
/// restore.
#[derive(Deserialize, Serialize)]
pub(crate) struct RutabagaResourceSnapshot {
    pub(crate) resource_id: u32,
    pub(crate) blob: bool,
    pub(crate) blob_mem: u32,
    pub(crate) blob_flags: u32,
    pub(crate) map_info: Option<u32>,
    pub(crate) info_2d: Option<Rutabaga2DSnapshot>,
    pub(crate) info_3d: Option<Resource3DInfo>,
    pub(crate) vulkan_info: Option<VulkanInfo>,
    pub(crate) component_mask: u8,
    pub(crate) size: u64,
}

impl TryFrom<&RutabagaResource> for RutabagaResourceSnapshot {
    type Error = RutabagaError;
    fn try_from(resource: &RutabagaResource) -> Result<Self, Self::Error> {
        Ok(RutabagaResourceSnapshot {
            resource_id: resource.resource_id,
            blob: resource.blob,
            blob_mem: resource.blob_mem,
            blob_flags: resource.blob_flags,
            map_info: resource.map_info,
            info_2d: resource.info_2d.as_ref().map(|info| Rutabaga2DSnapshot {
                width: info.width,
                height: info.height,
            }),
            info_3d: resource.info_3d,
            vulkan_info: resource.vulkan_info,
            size: resource.size,
            component_mask: resource.component_mask,
        })
    }
}

impl TryFrom<RutabagaResourceSnapshot> for RutabagaResource {
    type Error = RutabagaError;
    fn try_from(snapshot: RutabagaResourceSnapshot) -> Result<Self, Self::Error> {
        Ok(RutabagaResource {
            resource_id: snapshot.resource_id,
            handle: None,
            blob: snapshot.blob,
            blob_mem: snapshot.blob_mem,
            blob_flags: snapshot.blob_flags,
            map_info: snapshot.map_info,
            info_2d: snapshot.info_2d.map(|info| {
                let size = u64::from(info.width * info.height * 4);
                Rutabaga2DInfo {
                    width: info.width,
                    height: info.height,
                    host_mem: Some(vec![0; usize::try_from(size).unwrap()]),
                    scanout_stride: None,
                }
            }),
            info_3d: snapshot.info_3d,
            vulkan_info: snapshot.vulkan_info,
            backing_iovecs: None,
            size: snapshot.size,
            component_mask: snapshot.component_mask,
            mapping: None,
        })
    }
}
