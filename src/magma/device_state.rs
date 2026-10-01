// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap as Map;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::RwLock;

use magma_gpu::util::Handle as MagmaGpuHandle;
use magma_gpu_magma::AddressSpace;
use magma_gpu_magma::Buffer;
use magma_gpu_magma::Device;
use magma_gpu_magma::PhysicalDevice;
use magma_gpu_magma::Queue;
use magma_gpu_magma::SyncObj;

/// Shared state container for a Magma device context, accessible across all per-queue worker threads.
pub struct MagmaDeviceState {
    pub physical_devices: Vec<PhysicalDevice>,
    pub device: Mutex<Option<Arc<Device>>>,
    pub address_spaces: RwLock<Map<u32, Arc<AddressSpace>>>,
    pub buffers: RwLock<Map<u32, Arc<Buffer>>>,
    pub queues: RwLock<Map<u32, Arc<Queue>>>,
    pub sync_objs: RwLock<Map<u32, Arc<SyncObj>>>,
    pub pending_queue_fences: Mutex<Map<u32, VecDeque<MagmaGpuHandle>>>,
    pub ring_completed_seqno: [AtomicU64; 64],
    pub seqno_condvar: Condvar,
    pub seqno_mutex: Mutex<()>,
}

// SAFETY: Underlying kernel driver objects are thread-safe and internal access is
// synchronized via Mutex and RwLock.
unsafe impl Send for MagmaDeviceState {}
unsafe impl Sync for MagmaDeviceState {}

impl MagmaDeviceState {
    pub fn new(physical_devices: Vec<PhysicalDevice>) -> Self {
        Self {
            physical_devices,
            device: Mutex::new(None),
            address_spaces: RwLock::new(Map::new()),
            buffers: RwLock::new(Map::new()),
            queues: RwLock::new(Map::new()),
            sync_objs: RwLock::new(Map::new()),
            pending_queue_fences: Mutex::new(Map::new()),
            ring_completed_seqno: std::array::from_fn(|_| AtomicU64::new(0)),
            seqno_condvar: Condvar::new(),
            seqno_mutex: Mutex::new(()),
        }
    }

    pub fn set_device(&self, device: Device) {
        let mut dev_lock = self.device.lock().unwrap();
        *dev_lock = Some(Arc::new(device));
    }

    pub fn get_device(&self) -> Option<Arc<Device>> {
        self.device.lock().unwrap().clone()
    }

    pub fn update_ring_completed_seqno(&self, ring_idx: u32, seqno: u64) {
        let idx = (ring_idx as usize) & 63;
        let slot = &self.ring_completed_seqno[idx];
        let mut prev = slot.load(Ordering::Relaxed);
        while seqno > prev {
            match slot.compare_exchange_weak(prev, seqno, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => {
                    let _guard = self.seqno_mutex.lock().unwrap();
                    self.seqno_condvar.notify_all();
                    break;
                }
                Err(actual) => prev = actual,
            }
        }
    }

    pub fn wait_for_ring_seqno(&self, ring_idx: u32, target: u64) {
        let idx = (ring_idx as usize) & 63;
        let slot = &self.ring_completed_seqno[idx];
        if slot.load(Ordering::Acquire) >= target {
            return;
        }
        let mut guard = self.seqno_mutex.lock().unwrap();
        while slot.load(Ordering::Acquire) < target {
            guard = self.seqno_condvar.wait(guard).unwrap();
        }
    }

    pub fn wait_for_cpu_seqno(&self, target: u64) {
        self.wait_for_ring_seqno(0, target);
    }
}
