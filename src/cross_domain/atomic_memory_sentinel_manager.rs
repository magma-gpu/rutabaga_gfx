// Copyright 2026 Red Hat, Inc.
// Copyright 2021 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Manager for atomic memory sentinel instances.
//!
//! This module encapsulates the management of AtomicMemorySentinelThread instances,
//! providing a clean interface for creating, signaling, and destroying memory watchers.

use std::collections::BTreeMap as Map;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use log::info;
use mesa3d_util::AtomicMemorySentinel;
use mesa3d_util::Event;
use mesa3d_util::MemoryMapping;
use mesa3d_util::MesaError;

use crate::rutabaga_core::VirtioFsLookup;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaResult;
use crate::rutabaga_utils::RUTABAGA_MAP_ACCESS_RW;
use crate::rutabaga_utils::RUTABAGA_MAP_CACHE_CACHED;

use super::cross_domain_protocol::CrossDomainSignalAtomicMemorySentinel;

/// Thread worker for monitoring atomic memory changes.
struct AtomicMemorySentinelThread {
    sentinel: Arc<AtomicMemorySentinel>,
    shutdown: Arc<AtomicBool>,
    sender: Event,
    initial_value: u32,
}

impl AtomicMemorySentinelThread {
    fn new(
        sentinel: Arc<AtomicMemorySentinel>,
        shutdown: Arc<AtomicBool>,
        sender: Event,
        initial_value: u32,
    ) -> Self {
        Self {
            sentinel,
            shutdown,
            sender,
            initial_value,
        }
    }

    fn run(mut self) {
        // The goal of this code is to ensure that the other side observes at least
        // the latest wake event along with the value that the futex had when that
        // wake event was signaled.

        // The initial value is passed in from the futex creation, and therefore
        // was loaded synchronously with the cross domain operation that created
        // the futex, so it cannot have an associated wake event yet.
        let mut val = self.initial_value;
        let _ = self.sender.signal();

        loop {
            // This returns when the futex is woken up OR if the value has changed.
            self.sentinel.wait(val);
            // Load the new value, which the other side is guaranteed to observe.
            val = self.sentinel.load();

            // If this wake was triggered by the shutdown code below, just bail.
            // If the shutdown command is issued after this point, then it will
            // change the futex value, which will disagree with the one we read
            // above, so we will still not block in futex wait.
            if self.shutdown.load(Ordering::SeqCst) {
                // Signal the futex to process the shutdown and remove it from
                // the waiter table
                let _ = self.sender.signal();
                break;
            }

            // Signal the other side after the load. If another change occurs and
            // another wake is signaled here, we will miss the wake, but the
            // disagreeing value will cause futex wait to return early.
            if self.sender.signal().is_err() {
                break;
            }
        }
    }
}

/// Private struct containing a single memory watcher instance
struct SentinelInstance {
    sentinel: Arc<AtomicMemorySentinel>,
    watcher_thread: Option<thread::JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
    evt: Event,
}

impl SentinelInstance {
    fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.sentinel.wake_all();
        if let Some(thread) = self.watcher_thread.take() {
            let _ = thread.join();
        }
    }

    fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }
}

impl Drop for SentinelInstance {
    fn drop(&mut self) {
        if !self.is_shutdown() {
            info!("Atomic memory sentinel worker dropped without shutdown");
            self.shutdown();
        }
    }
}

/// Manager for all atomic memory sentinel watchers
pub struct AtomicMemorySentinelManager {
    watchers: Map<u32, SentinelInstance>,
    virtiofs_lookup: Option<Arc<dyn VirtioFsLookup>>,
}

impl AtomicMemorySentinelManager {
    /// Creates a new AtomicMemorySentinelManager
    pub fn new(virtiofs_lookup: Option<Arc<dyn VirtioFsLookup>>) -> Self {
        Self {
            watchers: Map::new(),
            virtiofs_lookup,
        }
    }

    /// Creates a new memory watcher and returns its ID and event descriptor
    pub fn create_watcher(&mut self, id: u32, fs_id: u64, handle: u64) -> RutabagaResult<Event> {
        if self.watchers.contains_key(&id) {
            return Err(RutabagaError::AlreadyInUse);
        }

        let virtiofs_lookup = self
            .virtiofs_lookup
            .as_ref()
            .ok_or(RutabagaError::InvalidCrossDomainItemId)?;

        let handle = virtiofs_lookup.get_exported_descriptor(fs_id, handle)?;
        let mapping = MemoryMapping::from_safe_descriptor(
            handle,
            size_of::<u32>(),
            RUTABAGA_MAP_ACCESS_RW | RUTABAGA_MAP_CACHE_CACHED,
        )?;

        let sentinel = Arc::new(AtomicMemorySentinel::new(mapping)?);
        let initial_value = sentinel.load();

        let memory_watcher_evt = Event::new()?;
        let evt_for_waitctx = memory_watcher_evt.try_clone()?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = shutdown.clone();
        let thread_sentinel = sentinel.clone();

        // Spawn the watcher thread
        let watcher_thread = thread::Builder::new()
            .name(format!("atomic_memory_sentinel_{id}"))
            .spawn(move || {
                AtomicMemorySentinelThread::new(
                    thread_sentinel,
                    thread_shutdown,
                    memory_watcher_evt,
                    initial_value,
                )
                .run();
            })
            .map_err(MesaError::IoError)?;

        self.watchers.insert(
            id,
            SentinelInstance {
                sentinel,
                watcher_thread: Some(watcher_thread),
                shutdown,
                evt: evt_for_waitctx.try_clone()?,
            },
        );

        Ok(evt_for_waitctx)
    }

    /// Handles an event for a memory watcher, returns the command to write to the ring
    pub fn handle_event(
        &mut self,
        id: u32,
    ) -> RutabagaResult<Option<CrossDomainSignalAtomicMemorySentinel>> {
        if let Some(watcher) = self.watchers.get_mut(&id) {
            if watcher.is_shutdown() {
                Ok(None)
            } else {
                watcher.evt.wait()?;

                let mut cmd_memory_watcher: CrossDomainSignalAtomicMemorySentinel =
                    Default::default();
                cmd_memory_watcher.hdr.cmd =
                    super::cross_domain_protocol::CROSS_DOMAIN_CMD_SIGNAL_ATOMIC_MEMORY_SENTINEL;
                cmd_memory_watcher.id = id;
                Ok(Some(cmd_memory_watcher))
            }
        } else {
            Err(RutabagaError::InvalidCrossDomainItemId)
        }
    }

    /// Signals a memory watcher
    pub fn signal_watcher(&self, id: u32) -> RutabagaResult<()> {
        if let Some(worker) = self.watchers.get(&id) {
            worker.sentinel.signal()?;
            Ok(())
        } else {
            Err(RutabagaError::InvalidCrossDomainItemId)
        }
    }

    /// Destroys a memory watcher
    pub fn destroy_watcher(&mut self, id: u32) -> RutabagaResult<()> {
        self.watchers
            .get_mut(&id)
            .ok_or(RutabagaError::InvalidCrossDomainItemId)?
            .shutdown();
        Ok(())
    }

    /// Checks if a watcher is shutdown (for cleanup in handle_fence)
    pub fn is_shutdown(&self, id: u32) -> bool {
        self.watchers
            .get(&id)
            .map(|w| w.is_shutdown())
            .unwrap_or(true)
    }

    /// Removes a watcher from the map (after shutdown)
    pub fn remove_watcher(&mut self, id: u32) {
        self.watchers.remove(&id);
    }

    /// Gets the event descriptor for a watcher
    pub fn get_event(&self, id: u32) -> Option<&Event> {
        self.watchers.get(&id).map(|w| &w.evt)
    }
}

impl Drop for AtomicMemorySentinelManager {
    fn drop(&mut self) {
        for (_, mut watcher) in std::mem::take(&mut self.watchers) {
            watcher.shutdown();
        }
    }
}
