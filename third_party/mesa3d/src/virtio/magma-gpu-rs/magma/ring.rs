// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use magma_gpu::util::MappedRegion;
use magma_gpu::util::MemoryMapping;

use crate::error::Error;
use crate::error::Result;

pub const MAGMA_RING_MAGIC: u32 = 0x4d41474d; // "MAGM"
pub const RING_STATUS_ACTIVE: u32 = 0;
pub const RING_STATUS_SPINNING: u32 = 1;
pub const RING_STATUS_ASLEEP: u32 = 2;

/// Shared control header placed at the beginning of the ring buffer memory mapping.
/// Padded to 64 bytes so the command ring data starts on a clean cacheline boundary.
#[repr(C)]
pub struct MagmaRingControl {
    /// Sentinel word at offset 0, watched by AtomicMemorySentinel (futex).
    pub sentinel: AtomicU32,
    /// Magic identifier for verification ("MAGM").
    pub magic: u32,
    /// Size of the circular data region in bytes.
    pub ring_size: u32,
    /// Read offset updated by host consumer (Release store, Acquire load by guest).
    pub read_offset: AtomicU32,
    /// Write offset updated by guest producer (Release store, Acquire load by host).
    pub write_offset: AtomicU32,
    /// Current thread status: 0 = ACTIVE, 1 = SPINNING, 2 = ASLEEP.
    pub status: AtomicU32,
    /// Latest seqno submitted by guest producer.
    pub submitted_seqno: AtomicU32,
    /// Latest seqno completed by host consumer.
    pub completed_seqno: AtomicU32,
    /// Padding to align header to 64 bytes.
    pub _pad: [u32; 8],
}

const _: () = assert!(size_of::<MagmaRingControl>() == 64);

pub const CONTROL_HEADER_SIZE: usize = size_of::<MagmaRingControl>();

pub struct MagmaRingBuffer {
    _mapping: Option<MemoryMapping>,
    control: *mut MagmaRingControl,
    data_ptr: *mut u8,
    data_size: usize,
    write_lock: Mutex<()>,
}

// SAFETY: MagmaRingBuffer operates on thread-safe atomic control pointers
// and raw ring memory mapped from a shared descriptor, with producer writes
// serialized by `write_lock`.
unsafe impl Send for MagmaRingBuffer {}
unsafe impl Sync for MagmaRingBuffer {}

impl MagmaRingBuffer {
    pub fn new(mapping: MemoryMapping) -> Result<MagmaRingBuffer> {
        let base_ptr = mapping.as_ptr();
        let size = mapping.size();
        let mut ring = unsafe { MagmaRingBuffer::from_raw_parts(base_ptr, size)? };
        ring._mapping = Some(mapping);
        Ok(ring)
    }

    /// # Safety
    ///
    /// The caller must ensure that `base_ptr` points to a valid, properly aligned,
    /// and accessible region of memory of at least `total_size` bytes for the lifetime
    /// of the returned `MagmaRingBuffer`.
    pub unsafe fn from_raw_parts(base_ptr: *mut u8, total_size: usize) -> Result<MagmaRingBuffer> {
        if total_size <= CONTROL_HEADER_SIZE {
            return Err(Error::InvalidArgs);
        }

        let control = base_ptr as *mut MagmaRingControl;
        let data_ptr = unsafe { base_ptr.add(CONTROL_HEADER_SIZE) };
        let data_size = total_size - CONTROL_HEADER_SIZE;

        // Initialize header if uninitialized
        let ctrl_ref = unsafe { &*control };
        if ctrl_ref.magic != MAGMA_RING_MAGIC {
            ctrl_ref.sentinel.store(0, Ordering::Relaxed);
            unsafe {
                (*control).magic = MAGMA_RING_MAGIC;
                (*control).ring_size = data_size as u32;
            }
            ctrl_ref.read_offset.store(0, Ordering::Relaxed);
            ctrl_ref.write_offset.store(0, Ordering::Relaxed);
            ctrl_ref.status.store(RING_STATUS_ASLEEP, Ordering::Relaxed);
            ctrl_ref.submitted_seqno.store(0, Ordering::Relaxed);
            ctrl_ref.completed_seqno.store(0, Ordering::Relaxed);
        }

        Ok(MagmaRingBuffer {
            _mapping: None,
            control,
            data_ptr,
            data_size,
            write_lock: Mutex::new(()),
        })
    }

    #[inline]
    pub fn control(&self) -> &MagmaRingControl {
        unsafe { &*self.control }
    }

    #[inline]
    pub fn has_data(&self) -> bool {
        let ctrl = self.control();
        ctrl.read_offset.load(Ordering::Relaxed) != ctrl.write_offset.load(Ordering::Acquire)
    }

    #[inline]
    pub fn status(&self) -> u32 {
        self.control().status.load(Ordering::Acquire)
    }

    #[inline]
    pub fn set_status(&self, status: u32) {
        self.control().status.store(status, Ordering::Release);
    }

    #[inline]
    pub fn submitted_seqno(&self) -> u32 {
        self.control().submitted_seqno.load(Ordering::Acquire)
    }

    #[inline]
    pub fn set_submitted_seqno(&self, seqno: u32) {
        self.control()
            .submitted_seqno
            .store(seqno, Ordering::Release);
    }

    #[inline]
    pub fn completed_seqno(&self) -> u32 {
        self.control().completed_seqno.load(Ordering::Acquire)
    }

    #[inline]
    pub fn set_completed_seqno(&self, seqno: u32) {
        self.control()
            .completed_seqno
            .store(seqno, Ordering::Release);
    }

    #[inline]
    #[allow(dead_code)]
    pub fn load_sentinel(&self) -> u32 {
        self.control().sentinel.load(Ordering::Acquire)
    }

    /// Write a command into the circular buffer.
    pub fn write(&self, bytes: &[u8]) -> Result<usize> {
        let ctrl = self.control();
        let ring_size = self.data_size;
        let write_len = bytes.len();

        if write_len == 0 {
            return Ok(0);
        }
        if write_len >= ring_size {
            return Err(Error::InvalidArgs);
        }

        let mut spin_count = 0;
        loop {
            let read = ctrl.read_offset.load(Ordering::Acquire) as usize;
            let write = ctrl.write_offset.load(Ordering::Relaxed) as usize;

            let available = if write >= read {
                ring_size - 1 - (write - read)
            } else {
                read - write - 1
            };

            if write_len <= available {
                if write + write_len <= ring_size {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            self.data_ptr.add(write),
                            write_len,
                        );
                    }
                    let new_write = (write + write_len) % ring_size;
                    ctrl.write_offset.store(new_write as u32, Ordering::Release);
                } else {
                    let first_chunk = ring_size - write;
                    let second_chunk = write_len - first_chunk;
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            self.data_ptr.add(write),
                            first_chunk,
                        );
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr().add(first_chunk),
                            self.data_ptr,
                            second_chunk,
                        );
                    }
                    ctrl.write_offset
                        .store(second_chunk as u32, Ordering::Release);
                }
                return Ok(write_len);
            }

            spin_count += 1;
            if spin_count > 100_000 {
                return Err(Error::TimedOut);
            }
            std::hint::spin_loop();
        }
    }

    /// Writes command bytes to the ring, increments submitted seqno, applies a memory fence,
    /// and invokes `ping` if the consumer thread is not active.
    /// Lockless SPSC fast-path for single-producer queue rings.
    pub fn write_and_ping<F>(&self, bytes: &[u8], ping: F) -> Result<()>
    where
        F: FnOnce() -> Result<()>,
    {
        self.write(bytes)?;
        let next_seq = self.submitted_seqno().wrapping_add(1);
        self.set_submitted_seqno(next_seq);
        std::sync::atomic::fence(Ordering::SeqCst);
        if self.status() != RING_STATUS_ACTIVE {
            ping()?;
        }
        Ok(())
    }

    /// Locked MPSC variant of `write_and_ping` for rings shared across multiple producer threads
    /// (such as the device-wide `cpu_ring`).
    pub fn write_and_ping_locked<F>(&self, bytes: &[u8], ping: F) -> Result<()>
    where
        F: FnOnce() -> Result<()>,
    {
        let _guard = self.write_lock.lock().map_err(|_| Error::InternalError)?;
        self.write_and_ping(bytes, ping)
    }

    /// Reads commands into a host-private bounce buffer.
    ///
    /// Exposing shared memory directly as `&[u8]` causes aliasing UB if the guest mutates
    /// it concurrently, and invites TOCTOU bugs during packet decoding.
    ///
    /// Note: `copy_nonoverlapping` is technically a data race under LLVM's abstract machine
    /// if the guest writes concurrently. Production hypervisors can use inline assembly
    /// (`rep movsb` on x86_64), `read_volatile`, or a `VolatileSlice` abstraction to avoid
    /// compiler assumptions and allow zero-copy parsing.
    pub fn read_available<F>(&self, mut consumer: F) -> usize
    where
        F: FnMut(&[u8]) -> usize,
    {
        let ctrl = self.control();
        let read = ctrl.read_offset.load(Ordering::Relaxed) as usize;
        let write = ctrl.write_offset.load(Ordering::Acquire) as usize;

        if read == write || self.data_size == 0 {
            return 0;
        }

        let ring_size = self.data_size;
        let available = if write >= read {
            write - read
        } else {
            (ring_size - read) + write
        };

        const SCRATCH_SIZE: usize = 8192;
        let mut scratch = [0u8; SCRATCH_SIZE];
        let copy_len = available.min(SCRATCH_SIZE);

        let copy_chunk = |dst: *mut u8, offset: usize, len: usize| unsafe {
            std::ptr::copy_nonoverlapping(self.data_ptr.add(offset), dst, len);
        };

        if write >= read {
            copy_chunk(scratch.as_mut_ptr(), read, copy_len);
        } else {
            let first = (ring_size - read).min(copy_len);
            let second = copy_len - first;
            copy_chunk(scratch.as_mut_ptr(), read, first);
            if second > 0 {
                copy_chunk(scratch.as_mut_ptr().wrapping_add(first), 0, second);
            }
        }

        let consumed = consumer(&scratch[..copy_len]);
        if consumed > 0 {
            let new_read = (read + consumed) % ring_size;
            ctrl.read_offset.store(new_read as u32, Ordering::Release);
        }

        consumed
    }
}
