// Copyright 2026 Magma-GPU project
// SPDX-License-Identifier: MIT

use std::cmp::max;
use std::collections::BTreeMap as Map;
use std::collections::VecDeque;
use std::mem::size_of;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;

use mesa3d_util::Event;
use mesa3d_util::MesaError;
use mesa3d_util::MesaHandle;
use mesa3d_util::OwnedDescriptor;
use mesa3d_util::ReadPipe;
use mesa3d_util::Tube;
use mesa3d_util::WritePipe;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;

use crate::context_common::ContextResources;
use crate::cross_domain::atomic_memory_sentinel_manager::AtomicMemorySentinelManager;
use crate::cross_domain::cross_domain_protocol::CROSS_DOMAIN_PIPE_READ_START;
use crate::cross_domain::cross_domain_protocol::CrossDomainReadWrite;
use crate::rutabaga_utils::RutabagaError;
use crate::rutabaga_utils::RutabagaFence;
use crate::rutabaga_utils::RutabagaResult;
use crate::ImageMemoryRequirements;

pub const CROSS_DOMAIN_CONTEXT_CHANNEL_ID: u64 = 1;
pub const CROSS_DOMAIN_RESAMPLE_ID: u64 = 2;
pub const CROSS_DOMAIN_KILL_ID: u64 = 3;

pub type SentinelManager = Arc<Mutex<AtomicMemorySentinelManager>>;

pub enum CrossDomainItem {
    ImageRequirements(ImageMemoryRequirements),
    Blob(MesaHandle),
    WaylandReadPipe(ReadPipe),
    WaylandWritePipe(WritePipe),
    Event(Event),
    RegularFile(OwnedDescriptor),
    Socket(OwnedDescriptor),
}

pub enum CrossDomainJob {
    HandleFence(RutabagaFence),
    AddReadPipe(u32),
    Finish,
    AddAtomicMemorySentinel(u32, Event),
    AddReadEvent(u32),
}

pub enum RingWrite<'a, T> {
    Write(T, Option<&'a [u8]>),
    WriteFromPipe(CrossDomainReadWrite, &'a mut ReadPipe, bool),
    WriteFromEvent(CrossDomainReadWrite, &'a mut Event, bool),
}

pub type CrossDomainJobs = Mutex<Option<VecDeque<CrossDomainJob>>>;
pub type CrossDomainItemState = Arc<Mutex<CrossDomainItems>>;

pub struct CrossDomainState {
    pub context_resources: ContextResources,
    pub sentinel_manager: SentinelManager,
    pub query_ring_id: u32,
    pub channel_ring_id: u32,
    pub connection: Option<Tube>,
    pub jobs: CrossDomainJobs,
    pub jobs_cvar: Condvar,
}

impl CrossDomainState {
    pub fn new(
        query_ring_id: u32,
        channel_ring_id: u32,
        context_resources: ContextResources,
        sentinel_manager: SentinelManager,
        connection: Option<Tube>,
    ) -> CrossDomainState {
        CrossDomainState {
            query_ring_id,
            channel_ring_id,
            context_resources,
            sentinel_manager,
            connection,
            jobs: Mutex::new(Some(VecDeque::new())),
            jobs_cvar: Condvar::new(),
        }
    }

    pub fn send_msg(
        &self,
        opaque_data: &[u8],
        descriptors: &[OwnedDescriptor],
    ) -> RutabagaResult<usize> {
        match self.connection {
            Some(ref connection) => connection
                .send(opaque_data, descriptors)
                .map_err(|e| e.into()),
            None => Err(RutabagaError::InvalidCrossDomainChannel),
        }
    }

    pub fn receive_msg(
        &self,
        opaque_data: &mut [u8],
    ) -> RutabagaResult<(usize, Vec<OwnedDescriptor>)> {
        match self.connection {
            Some(ref connection) => connection.receive(opaque_data).map_err(|e| e.into()),
            None => Err(RutabagaError::InvalidCrossDomainChannel),
        }
    }

    pub fn add_job(&self, job: CrossDomainJob) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(queue) = jobs.as_mut() {
            queue.push_back(job);
            self.jobs_cvar.notify_one();
        }
    }

    pub fn wait_for_job(&self) -> Option<CrossDomainJob> {
        let mut jobs = self.jobs.lock().unwrap();
        loop {
            match jobs.as_mut()?.pop_front() {
                Some(job) => return Some(job),
                None => jobs = self.jobs_cvar.wait(jobs).unwrap(),
            }
        }
    }

    pub fn write_to_ring<T>(
        &self,
        mut ring_write: RingWrite<T>,
        ring_id: u32,
    ) -> RutabagaResult<usize>
    where
        T: FromBytes + IntoBytes + Immutable,
    {
        let mut context_resources = self.context_resources.lock().unwrap();
        let mut bytes_read: usize = 0;

        let resource = context_resources
            .get_mut(&ring_id)
            .ok_or(RutabagaError::InvalidResourceId)?;

        let iovecs = resource
            .backing_iovecs
            .as_mut()
            .ok_or(RutabagaError::InvalidIovec)?;
        let slice =
            // SAFETY:
            // Safe because we've verified the iovecs are attached and owned only by this context.
            unsafe { std::slice::from_raw_parts_mut(iovecs[0].base as *mut u8, iovecs[0].len) };

        match ring_write {
            RingWrite::Write(cmd, opaque_data_opt) => {
                if slice.len() < size_of::<T>() {
                    return Err(RutabagaError::InvalidIovec);
                }
                let (cmd_slice, opaque_data_slice) = slice.split_at_mut(size_of::<T>());
                cmd_slice.copy_from_slice(cmd.as_bytes());
                if let Some(opaque_data) = opaque_data_opt {
                    if opaque_data_slice.len() < opaque_data.len() {
                        return Err(RutabagaError::InvalidIovec);
                    }
                    opaque_data_slice[..opaque_data.len()].copy_from_slice(opaque_data);
                }
            }
            RingWrite::WriteFromPipe(mut cmd_read, ref mut read_pipe, readable) => {
                if slice.len() < size_of::<CrossDomainReadWrite>() {
                    return Err(RutabagaError::InvalidIovec);
                }

                let (cmd_slice, opaque_data_slice) =
                    slice.split_at_mut(size_of::<CrossDomainReadWrite>());

                if readable {
                    bytes_read = read_pipe.read(opaque_data_slice)?;
                }

                if bytes_read == 0 {
                    cmd_read.hang_up = 1;
                }

                cmd_read.opaque_data_size =
                    bytes_read.try_into().map_err(MesaError::TryFromIntError)?;
                cmd_slice.copy_from_slice(cmd_read.as_bytes());
            }
            RingWrite::WriteFromEvent(mut cmd_read, ref mut event, readable) => {
                if slice.len() < size_of::<CrossDomainReadWrite>() {
                    return Err(RutabagaError::InvalidIovec);
                }

                let (cmd_slice, opaque_data_slice) =
                    slice.split_at_mut(size_of::<CrossDomainReadWrite>());

                if readable {
                    let value = event.wait()?;
                    bytes_read = 8;
                    opaque_data_slice[0..8].copy_from_slice(&value.to_le_bytes());
                }

                cmd_read.opaque_data_size =
                    bytes_read.try_into().map_err(MesaError::TryFromIntError)?;
                cmd_slice.copy_from_slice(cmd_read.as_bytes());
            }
        }

        Ok(bytes_read)
    }
}

pub struct CrossDomainItems {
    pub descriptor_id: u32,
    pub read_pipe_id: u32,
    pub table: Map<u32, CrossDomainItem>,
}

impl Default for CrossDomainItems {
    fn default() -> Self {
        // Odd for descriptors, and even for requirement blobs.
        CrossDomainItems {
            descriptor_id: 1,
            read_pipe_id: CROSS_DOMAIN_PIPE_READ_START,
            table: Default::default(),
        }
    }
}

// TODO(gurchetansingh): optimize the item tracker.  Each requirements blob is long-lived and can
// be stored in a Slab or vector.  OwnedDescriptors received from the Wayland socket *seem* to come
// one at a time, and can be stored as options.  Need to confirm.
pub fn add_item(item_state: &CrossDomainItemState, item: CrossDomainItem) -> u32 {
    let mut items = item_state.lock().unwrap();

    let item_id = match item {
        CrossDomainItem::WaylandReadPipe(_) => {
            items.read_pipe_id += 1;
            max(items.read_pipe_id, CROSS_DOMAIN_PIPE_READ_START)
        }
        _ => {
            items.descriptor_id += 1;
            items.descriptor_id
        }
    };

    items.table.insert(item_id, item);

    item_id
}
