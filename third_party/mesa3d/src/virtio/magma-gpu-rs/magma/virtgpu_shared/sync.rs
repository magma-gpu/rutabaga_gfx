// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use magma_gpu::util::Handle as MagmaGpuHandle;
use zerocopy::IntoBytes;

use crate::error::Error;
use crate::error::Result;
use crate::protocol::MagmaCommandHeader;
use crate::protocol::MagmaSyncObjType;
use crate::protocol::MagmaSyncObjUseFlags;
use crate::protocol::SyncObjSignal;
use crate::protocol::SyncObjTimelineSignal;
use crate::protocol::VirtioCreateFence;
use crate::protocol::VirtioSyncObjClose;
use crate::protocol::MAGMA_OPCODE_SYNC_OBJ_SIGNAL;
use crate::protocol::MAGMA_OPCODE_SYNC_OBJ_TIMELINE_SIGNAL;
use crate::protocol::MAGMA_OPCODE_VIRTIO_CREATE_FENCE;
use crate::protocol::MAGMA_OPCODE_VIRTIO_SYNC_OBJ_CLOSE;
use crate::traits::AsVirtioSyncObject;
use crate::traits::BackendSyncObject;
use crate::traits::GenericSyncObject;
use crate::traits::VirtioSyncObject;
use crate::traits::VirtioSyncOp;
use crate::virtgpu_shared::device::VirtioGpu;
use crate::virtgpu_shared::VirtioGpuTransport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncObjectState {
    Unsubmitted,
    CpuSignaled,
    Imported,
    Submitted { ring_idx: u32, fence_created: bool },
}

pub struct SyncObjectInner {
    pub state: SyncObjectState,
    pub timeline_point: u64,
    pub pending_point: u64,
    pub last_used_ring_idx: u32,
    pub last_used_seqno: u32,
}

pub struct SyncObjectTracker {
    inner: Mutex<SyncObjectInner>,
    condvar: Condvar,
}

pub struct VirtioGpuSyncObject<T: VirtioGpuTransport> {
    pub device: Arc<VirtioGpu<T>>,
    pub object_id: u32,
    pub guest_syncobj_handle: u32,
    pub use_flags: MagmaSyncObjUseFlags,
    pub sync_type: MagmaSyncObjType,
    pub tracker: SyncObjectTracker,
}

impl SyncObjectTracker {
    pub fn new(initial_point: u64) -> SyncObjectTracker {
        let initial_state = if initial_point > 0 {
            SyncObjectState::CpuSignaled
        } else {
            SyncObjectState::Unsubmitted
        };
        SyncObjectTracker {
            inner: Mutex::new(SyncObjectInner {
                state: initial_state,
                timeline_point: initial_point,
                pending_point: initial_point,
                last_used_ring_idx: 0,
                last_used_seqno: 0,
            }),
            condvar: Condvar::new(),
        }
    }

    pub fn last_used_ring_and_seqno(&self) -> (u32, u32, SyncObjectState) {
        self.inner
            .lock()
            .map(|inner| (inner.last_used_ring_idx, inner.last_used_seqno, inner.state))
            .unwrap_or((0, 0, SyncObjectState::Unsubmitted))
    }

    pub fn has_imported_fence(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.state == SyncObjectState::Imported)
            .unwrap_or(false)
    }

    pub fn last_ring_idx(&self) -> u32 {
        self.inner
            .lock()
            .map(|inner| match inner.state {
                SyncObjectState::Submitted { ring_idx, .. } => ring_idx,
                _ => 0,
            })
            .unwrap_or(0)
    }

    pub fn record_submission(&self, op: VirtioSyncOp) {
        if let Ok(mut inner) = self.inner.lock() {
            match op {
                VirtioSyncOp::Wait { ring_idx, seqno } => {
                    inner.last_used_ring_idx = ring_idx;
                    inner.last_used_seqno = seqno;
                }
                VirtioSyncOp::Signal {
                    ring_idx,
                    seqno,
                    point,
                } => {
                    inner.last_used_ring_idx = ring_idx;
                    inner.last_used_seqno = seqno;
                    inner.state = SyncObjectState::Submitted {
                        ring_idx,
                        fence_created: false,
                    };
                    if point > inner.pending_point {
                        inner.pending_point = point;
                    }
                    self.condvar.notify_all();
                }
            }
        }
    }

    pub fn wait_for_submission<F>(
        &self,
        timeout_ns: u64,
        create_fence_for_ring: F,
    ) -> Result<SyncObjectState>
    where
        F: FnOnce(u32) -> Result<()>,
    {
        let mut inner = self.inner.lock().map_err(|_| Error::InternalError)?;

        if inner.state == SyncObjectState::Unsubmitted {
            if timeout_ns == 0 {
                return Err(Error::TimedOut);
            }
            let timeout = Duration::from_nanos(timeout_ns.min(i64::MAX as u64));
            let deadline = Instant::now()
                .checked_add(timeout)
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600));
            while inner.state == SyncObjectState::Unsubmitted {
                let now = Instant::now();
                if now >= deadline {
                    return Err(Error::TimedOut);
                }
                let (guard, wait_res) = self
                    .condvar
                    .wait_timeout(inner, deadline - now)
                    .map_err(|_| Error::InternalError)?;
                inner = guard;
                if wait_res.timed_out() && inner.state == SyncObjectState::Unsubmitted {
                    return Err(Error::TimedOut);
                }
            }
        }

        if let SyncObjectState::Submitted {
            ring_idx,
            fence_created: false,
        } = inner.state
        {
            create_fence_for_ring(ring_idx)?;
            inner.state = SyncObjectState::Submitted {
                ring_idx,
                fence_created: true,
            };
        }

        Ok(inner.state)
    }

    /// Waits until `timeline_point >= point` or `pending_point >= point`.
    /// Returns `Ok(true)` if `timeline_point >= point` is already satisfied in userspace,
    /// or `Ok(false)` if the caller should now wait on the underlying GPU fence.
    pub fn wait_for_timeline_point(&self, point: u64, timeout_ns: u64) -> Result<bool> {
        if point == 0 {
            return Ok(true);
        }
        let mut inner = self.inner.lock().map_err(|_| Error::InternalError)?;
        if inner.timeline_point >= point {
            return Ok(true);
        }
        if timeout_ns == 0 && inner.pending_point < point {
            return Err(Error::TimedOut);
        }
        let timeout = Duration::from_nanos(timeout_ns.min(i64::MAX as u64));
        let deadline = Instant::now()
            .checked_add(timeout)
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600));
        while inner.timeline_point < point && inner.pending_point < point {
            let now = Instant::now();
            if now >= deadline {
                return Err(Error::TimedOut);
            }
            let (guard, wait_res) = self
                .condvar
                .wait_timeout(inner, deadline - now)
                .map_err(|_| Error::InternalError)?;
            inner = guard;
            if wait_res.timed_out() && inner.timeline_point < point && inner.pending_point < point {
                return Err(Error::TimedOut);
            }
        }
        Ok(inner.timeline_point >= point)
    }

    pub fn complete_gpu_wait(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if inner.pending_point > inner.timeline_point {
                inner.timeline_point = inner.pending_point;
            }
        }
    }

    pub fn mark_cpu_signaled(&self, point: Option<u64>) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| Error::InternalError)?;
        if let Some(pt) = point {
            if pt > inner.timeline_point {
                inner.timeline_point = pt;
            }
            if pt > inner.pending_point {
                inner.pending_point = pt;
            }
        }
        inner.state = SyncObjectState::CpuSignaled;
        self.condvar.notify_all();
        Ok(())
    }

    pub fn mark_imported(&self) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| Error::InternalError)?;
        inner.state = SyncObjectState::Imported;
        self.condvar.notify_all();
        Ok(())
    }

    pub fn timeline_query(&self) -> Result<u64> {
        let inner = self.inner.lock().map_err(|_| Error::InternalError)?;
        Ok(inner.timeline_point.max(inner.pending_point))
    }
}

impl<T: VirtioGpuTransport> VirtioGpuSyncObject<T> {
    fn create_fence_for_ring(&self, ring_idx: u32) -> Result<()> {
        if ring_idx == 0 {
            return Err(Error::Unimplemented);
        }
        let req = VirtioCreateFence {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_VIRTIO_CREATE_FENCE,
                size: std::mem::size_of::<VirtioCreateFence>() as u32,
            },
            ring_idx,
            guest_sync_id: self.object_id,
        };
        self.device
            .submit_encoded_cmd(req.as_bytes(), ring_idx, &[self.guest_syncobj_handle])
    }

    fn wait_for_submission(&self, timeout_ns: u64) -> Result<SyncObjectState> {
        self.tracker
            .wait_for_submission(timeout_ns, |ring_idx| self.create_fence_for_ring(ring_idx))
    }
}

impl<T: VirtioGpuTransport> GenericSyncObject for VirtioGpuSyncObject<T> {
    fn get_type(&self) -> MagmaSyncObjType {
        self.sync_type
    }

    fn wait(&self, timeout_ns: u64) -> Result<()> {
        let state = self.wait_for_submission(timeout_ns)?;
        if state == SyncObjectState::CpuSignaled {
            return Ok(());
        }
        if self.guest_syncobj_handle == 0 {
            return Err(Error::InvalidArgs);
        }
        self.device
            .transport
            .syncobj_wait(self.guest_syncobj_handle, timeout_ns)?;
        self.tracker.complete_gpu_wait();
        Ok(())
    }

    fn signal(&self) -> Result<()> {
        let req = SyncObjSignal {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_SYNC_OBJ_SIGNAL,
                size: std::mem::size_of::<SyncObjSignal>() as u32,
            },
            sync_obj: self.object_id,
            _padding: 0,
        };
        self.device.submit_encoded_cpu_cmd(req.as_bytes())?;
        if self.guest_syncobj_handle != 0 {
            self.device
                .transport
                .syncobj_signal(self.guest_syncobj_handle)?;
        }
        self.tracker.mark_cpu_signaled(None)
    }

    fn timeline_wait(&self, point: u64, timeout_ns: u64, _flags: u32) -> Result<()> {
        if self.tracker.wait_for_timeline_point(point, timeout_ns)? {
            return Ok(());
        }
        self.wait(timeout_ns)
    }

    fn timeline_signal(&self, point: u64) -> Result<()> {
        let req = SyncObjTimelineSignal {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_SYNC_OBJ_TIMELINE_SIGNAL,
                size: std::mem::size_of::<SyncObjTimelineSignal>() as u32,
            },
            sync_obj: self.object_id,
            _pad0: 0,
            point,
        };
        self.device.submit_encoded_cpu_cmd(req.as_bytes())?;
        if self.guest_syncobj_handle != 0 {
            self.device
                .transport
                .syncobj_signal(self.guest_syncobj_handle)?;
        }
        self.tracker.mark_cpu_signaled(Some(point))
    }

    fn timeline_query(&self) -> Result<u64> {
        self.tracker.timeline_query()
    }

    fn export_fence(&self) -> Result<MagmaGpuHandle> {
        let state = self.wait_for_submission(i64::MAX as u64)?;
        if self.guest_syncobj_handle == 0 {
            return Err(Error::Unimplemented);
        }
        self.device.transport.syncobj_export(
            self.guest_syncobj_handle,
            state == SyncObjectState::CpuSignaled,
        )
    }

    fn import(&self, handle: MagmaGpuHandle) -> Result<()> {
        if self.guest_syncobj_handle == 0 {
            return Err(Error::InvalidArgs);
        }
        self.device
            .transport
            .syncobj_import(self.guest_syncobj_handle, handle)?;
        self.tracker.mark_imported()
    }

    fn as_raw_handle(&self) -> Option<u32> {
        Some(self.object_id)
    }

    fn use_flags(&self) -> MagmaSyncObjUseFlags {
        self.use_flags
    }
}

impl<T: VirtioGpuTransport> VirtioSyncObject for VirtioGpuSyncObject<T> {
    fn host_object_id(&self) -> u32 {
        self.object_id
    }

    fn guest_syncobj_handle(&self) -> u32 {
        self.guest_syncobj_handle
    }

    fn has_imported_fence(&self) -> bool {
        self.tracker.has_imported_fence()
    }

    fn last_ring_idx(&self) -> u32 {
        self.tracker.last_ring_idx()
    }

    fn record_submission(&self, op: VirtioSyncOp) {
        self.tracker.record_submission(op);
    }
}

impl<T: VirtioGpuTransport> AsVirtioSyncObject for VirtioGpuSyncObject<T> {
    fn as_virtio_syncobj(&self) -> Option<&dyn VirtioSyncObject> {
        Some(self)
    }
}

impl<T: VirtioGpuTransport> Drop for VirtioGpuSyncObject<T> {
    fn drop(&mut self) {
        let (ring_idx, wait_seqno, _state) = self.tracker.last_used_ring_and_seqno();
        if self.guest_syncobj_handle != 0 {
            self.device
                .transport
                .syncobj_destroy(self.guest_syncobj_handle);
        }
        let req = VirtioSyncObjClose {
            header: MagmaCommandHeader {
                opcode: MAGMA_OPCODE_VIRTIO_SYNC_OBJ_CLOSE,
                size: std::mem::size_of::<VirtioSyncObjClose>() as u32,
            },
            sync_obj: self.object_id,
            ring_idx,
            wait_seqno,
            _padding: 0,
        };
        let _ = self.device.submit_encoded_cpu_cmd(req.as_bytes());
    }
}

impl<T: VirtioGpuTransport> BackendSyncObject for VirtioGpuSyncObject<T> {}
