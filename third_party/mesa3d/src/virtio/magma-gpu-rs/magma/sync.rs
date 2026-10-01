// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use magma_gpu::util::Handle as MagmaGpuHandle;

use crate::defines::MagmaSyncType;
use crate::error::Result;
use crate::protocol::MagmaSyncObjUseFlags;
use crate::traits::BackendSyncObject;

#[derive(Clone)]
pub struct SyncObj {
    pub sync_obj: Arc<dyn BackendSyncObject>,
}

impl SyncObj {
    pub fn new(sync_obj: Arc<dyn BackendSyncObject>) -> SyncObj {
        SyncObj { sync_obj }
    }

    pub fn wait(&self, timeout_ns: u64) -> Result<()> {
        self.sync_obj.wait(timeout_ns)
    }

    pub fn signal(&self) -> Result<()> {
        self.sync_obj.signal()
    }

    pub fn timeline_wait(&self, point: u64, timeout_ns: u64, flags: u32) -> Result<()> {
        self.sync_obj.timeline_wait(point, timeout_ns, flags)
    }

    pub fn timeline_signal(&self, point: u64) -> Result<()> {
        self.sync_obj.timeline_signal(point)
    }

    pub fn timeline_query(&self) -> Result<u64> {
        self.sync_obj.timeline_query()
    }

    pub fn export_fence(&self) -> Result<MagmaGpuHandle> {
        self.sync_obj.export_fence()
    }

    pub fn import(&self, handle: MagmaGpuHandle) -> Result<()> {
        self.sync_obj.import(handle)
    }

    pub fn as_raw_handle(&self) -> Option<u32> {
        self.sync_obj.as_raw_handle()
    }

    pub fn use_flags(&self) -> MagmaSyncObjUseFlags {
        self.sync_obj.use_flags()
    }

    pub fn get_type(&self) -> MagmaSyncType {
        self.sync_obj.get_type()
    }

    pub fn sync_obj(&self) -> &Arc<dyn BackendSyncObject> {
        &self.sync_obj
    }

    pub fn is_timeline(&self) -> bool {
        self.sync_obj.is_timeline()
    }
}
