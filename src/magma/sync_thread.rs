// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use magma_gpu::util::{
    create_event_pair, AsBorrowedDescriptor, EventSignaler, Handle as MagmaGpuHandle, WaitContext,
    WaitTimeout,
};

use crate::rutabaga_utils::{RutabagaError, RutabagaFence, RutabagaFenceHandler, RutabagaResult};

const KILL_TOKEN: u64 = 1;
const RESAMPLE_TOKEN: u64 = 2;

struct SyncThreadState {
    pending_fences: Vec<(RutabagaFence, MagmaGpuHandle)>,
}

pub struct MagmaVirtioGpuSyncThread {
    state: Arc<Mutex<SyncThreadState>>,
    resample_evt: EventSignaler,
    kill_evt: EventSignaler,
    worker_thread: Option<thread::JoinHandle<RutabagaResult<()>>>,
}

impl MagmaVirtioGpuSyncThread {
    pub fn new(fence_handler: RutabagaFenceHandler) -> RutabagaResult<Self> {
        let (kill_evt, thread_kill_evt) = create_event_pair().map_err(RutabagaError::from)?;
        let (resample_evt, thread_resample_evt) =
            create_event_pair().map_err(RutabagaError::from)?;

        let state = Arc::new(Mutex::new(SyncThreadState {
            pending_fences: Vec::new(),
        }));
        let thread_state = state.clone();

        let mut wait_ctx = WaitContext::new().map_err(RutabagaError::from)?;
        wait_ctx
            .add(KILL_TOKEN, thread_kill_evt.as_borrowed_descriptor())
            .map_err(RutabagaError::from)?;
        wait_ctx
            .add(RESAMPLE_TOKEN, thread_resample_evt.as_borrowed_descriptor())
            .map_err(RutabagaError::from)?;

        let worker_thread = thread::Builder::new()
            .name("magma_sync_thread".to_string())
            .spawn(move || -> RutabagaResult<()> {
                let mut active_fences: HashMap<u64, (RutabagaFence, MagmaGpuHandle)> =
                    HashMap::new();
                loop {
                    let events = wait_ctx
                        .wait(WaitTimeout::NoTimeout)
                        .map_err(RutabagaError::from)?;
                    for event in events {
                        if event.connection_id == KILL_TOKEN {
                            return Ok(());
                        } else if event.connection_id == RESAMPLE_TOKEN {
                            let _ = thread_resample_evt.wait();
                            let mut st = thread_state.lock().unwrap();
                            for (fence, handle) in st.pending_fences.drain(..) {
                                let fence_id = fence.fence_id;
                                if let Err(e) = wait_ctx.add(fence_id, &handle.os_handle) {
                                    eprintln!("Failed to add fence to wait_ctx: {e:?}");
                                    fence_handler.call(fence);
                                } else {
                                    active_fences.insert(fence_id, (fence, handle));
                                }
                            }
                        } else if let Some((fence, handle)) =
                            active_fences.remove(&event.connection_id)
                        {
                            let _ = wait_ctx.delete(&handle.os_handle);
                            fence_handler.call(fence);
                        }
                    }
                }
            })
            .map_err(|e| RutabagaError::ComponentError(e.raw_os_error().unwrap_or(-1)))?;

        Ok(MagmaVirtioGpuSyncThread {
            state,
            resample_evt,
            kill_evt,
            worker_thread: Some(worker_thread),
        })
    }

    pub fn add_fence(&self, fence: RutabagaFence, handle: MagmaGpuHandle) -> RutabagaResult<()> {
        let mut st = self.state.lock().unwrap();
        st.pending_fences.push((fence, handle));
        self.resample_evt.signal().map_err(RutabagaError::from)?;
        Ok(())
    }
}

impl Drop for MagmaVirtioGpuSyncThread {
    fn drop(&mut self) {
        let _ = self.kill_evt.signal();
        if let Some(worker) = self.worker_thread.take() {
            let _ = worker.join();
        }
    }
}
