// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::thread;

use crate::magma::device_state::MagmaDeviceState;
use crate::magma::dispatcher::MagmaDispatcher;
use magma_gpu::util::AtomicMemorySentinel;
use magma_gpu::util::Reader;
use magma_gpu_magma::decoder::decode;
use magma_gpu_magma::ring::MagmaRingBuffer;
use magma_gpu_magma::ring::RING_STATUS_ACTIVE;
use magma_gpu_magma::ring::RING_STATUS_ASLEEP;
use magma_gpu_magma::ring::RING_STATUS_SPINNING;

const DEFAULT_SPIN_COUNT: u32 = 2000;

/// Represents a distinct execution channel in shared memory (e.g. CPU channel = 0, queue channels = 1..N).
pub struct MagmaRingChannel {
    pub channel_id: u32,
    pub ring: MagmaRingBuffer,
    pub sentinel: Arc<AtomicMemorySentinel>,
    pub dispatcher: MagmaDispatcher,
    pub state: Arc<MagmaDeviceState>,
    pub is_scheduled: AtomicBool,
    pub is_processing: Mutex<()>,
    progress_mutex: Mutex<()>,
    progress_condvar: Condvar,
}

impl MagmaRingChannel {
    pub fn new(
        channel_id: u32,
        ring: MagmaRingBuffer,
        sentinel: Arc<AtomicMemorySentinel>,
        state: Arc<MagmaDeviceState>,
    ) -> Self {
        Self {
            channel_id,
            ring,
            sentinel,
            dispatcher: MagmaDispatcher::new(state.clone(), channel_id),
            state,
            is_scheduled: AtomicBool::new(false),
            is_processing: Mutex::new(()),
            progress_mutex: Mutex::new(()),
            progress_condvar: Condvar::new(),
        }
    }

    #[inline]
    pub fn ping(&self) {
        let _ = self.sentinel.signal();
    }

    #[inline]
    #[allow(dead_code)]
    pub fn is_idle(&self) -> bool {
        !self.ring.has_data() && self.is_processing.try_lock().is_ok()
    }

    /// Wait for consumption through a queue sequence point.
    pub fn wait_until_completed(&self, target: u32) {
        let mut guard = self.progress_mutex.lock().unwrap();
        while (self.ring.completed_seqno().wrapping_sub(target) as i32) < 0 {
            guard = self.progress_condvar.wait(guard).unwrap();
        }
    }

    pub fn wait_until_idle(self: &Arc<Self>, pool: &MagmaVirtioGpuThreadPool) {
        loop {
            let target = self.ring.submitted_seqno();
            pool.schedule(self.clone());
            self.wait_until_completed(target);
            let _guard = self.is_processing.lock().unwrap();
            if self.ring.submitted_seqno() == target && !self.ring.has_data() {
                break;
            }
        }
    }

    pub fn notify_progress(&self) {
        let _guard = self.progress_mutex.lock().unwrap();
        self.progress_condvar.notify_all();
    }
}

struct PoolState {
    queue: VecDeque<Arc<MagmaRingChannel>>,
    shutdown: bool,
}

/// Component-level thread pool that services work across all active MagmaRingChannels.
pub struct MagmaVirtioGpuThreadPool {
    state: Arc<Mutex<PoolState>>,
    condvar: Arc<Condvar>,
    workers: Mutex<Vec<thread::JoinHandle<()>>>,
}

impl MagmaVirtioGpuThreadPool {
    pub fn new() -> Arc<Self> {
        let num_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .max(2);
        let state = Arc::new(Mutex::new(PoolState {
            queue: VecDeque::new(),
            shutdown: false,
        }));
        let condvar = Arc::new(Condvar::new());
        let mut workers = Vec::with_capacity(num_threads);

        for i in 0..num_threads {
            let thread_state = state.clone();
            let thread_condvar = condvar.clone();
            let builder = thread::Builder::new().name(format!("magma_worker_{i}"));
            let handle = builder
                .spawn(move || loop {
                    let channel = {
                        let mut guard = thread_state.lock().unwrap();
                        loop {
                            if guard.shutdown {
                                return;
                            }
                            if let Some(channel) = guard.queue.pop_front() {
                                break channel;
                            }
                            guard = thread_condvar.wait(guard).unwrap();
                        }
                    };

                    Self::process_ring(&channel);
                })
                .expect("failed to spawn magma worker");
            workers.push(handle);
        }

        Arc::new(Self {
            state,
            condvar,
            workers: Mutex::new(workers),
        })
    }

    pub fn schedule(&self, channel: Arc<MagmaRingChannel>) {
        if !channel.is_scheduled.swap(true, Ordering::SeqCst) {
            let mut guard = self.state.lock().unwrap();
            guard.queue.push_back(channel);
            self.condvar.notify_one();
        }
    }

    fn process_ring(channel: &Arc<MagmaRingChannel>) {
        // Only one thread at a time drains any particular ring channel, preserving intra-channel FIFO order.
        let _proc_guard = channel.is_processing.lock().unwrap();
        channel.is_scheduled.store(false, Ordering::Release);
        channel.ring.set_status(RING_STATUS_ACTIVE);

        loop {
            while channel.ring.has_data() {
                channel.ring.set_status(RING_STATUS_ACTIVE);
                let consumed = channel.ring.read_available(|slice| {
                    let initial_available = slice.len();
                    let mut reader = Reader::new(slice);
                    while let Some(msg) = decode(&mut reader) {
                        if let Err(err) = channel.dispatcher.dispatch(msg, &mut reader) {
                            eprintln!(
                                "Magma ring ch{} command failed after decode: {err:?}",
                                channel.channel_id
                            );
                        }
                        let completed = channel.ring.completed_seqno().wrapping_add(1);
                        channel.ring.set_completed_seqno(completed);
                        channel
                            .state
                            .update_ring_completed_seqno(channel.channel_id, completed as u64);
                        channel.notify_progress();
                    }
                    initial_available - reader.available_bytes()
                });

                if consumed == 0 {
                    break;
                }
            }

            // Spin briefly for back-to-back incoming commands
            channel.ring.set_status(RING_STATUS_SPINNING);
            let mut got_work = false;
            for _ in 0..DEFAULT_SPIN_COUNT {
                std::hint::spin_loop();
                if channel.ring.has_data() {
                    got_work = true;
                    break;
                }
            }

            if got_work && channel.ring.has_data() {
                continue;
            }

            // No pending data: mark ASLEEP
            channel.ring.set_status(RING_STATUS_ASLEEP);
            std::sync::atomic::fence(Ordering::SeqCst);

            // Final check to prevent lost wakeups if data arrived right before setting ASLEEP
            if channel.ring.has_data() {
                channel.ring.set_status(RING_STATUS_ACTIVE);
                continue;
            }

            break;
        }
    }
}

impl Drop for MagmaVirtioGpuThreadPool {
    fn drop(&mut self) {
        {
            let mut guard = self.state.lock().unwrap();
            guard.shutdown = true;
        }
        self.condvar.notify_all();
        let mut workers = self.workers.lock().unwrap();
        for worker in workers.drain(..) {
            let _ = worker.join();
        }
    }
}
