// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::fmt;

use magma_gpu::protocols::broccolini_protocol::BroccoliniProtocol;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::OwnedDescriptor;

use crate::utils::BroccoliniError;
use crate::utils::BroccoliniResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessId;

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unimplemented")
    }
}

#[allow(dead_code)]
pub struct WorkerProcess {
    pid: ProcessId,
    client_desc: OwnedDescriptor,
}

#[allow(dead_code)]
impl WorkerProcess {
    pub fn new<F>(_f: F) -> BroccoliniResult<Self>
    where
        F: FnOnce(Stream<BroccoliniProtocol>) -> BroccoliniResult<()>,
    {
        Err(BroccoliniError::Unimplemented)
    }

    pub fn pid(&self) -> ProcessId {
        self.pid
    }

    pub fn into_client_descriptor(self) -> OwnedDescriptor {
        self.client_desc
    }

    pub fn current_id() -> ProcessId {
        ProcessId
    }

    pub fn set_current_name(_name: &str) {}
}
