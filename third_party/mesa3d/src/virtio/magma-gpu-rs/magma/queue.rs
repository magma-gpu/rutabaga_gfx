// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use crate::defines::MagmaSubmitInfo;
use crate::error::Result;
use crate::traits::BackendQueue;

#[derive(Clone)]
pub struct Queue {
    queue: Arc<dyn BackendQueue>,
}

impl Queue {
    pub fn new(queue: Arc<dyn BackendQueue>) -> Queue {
        Queue { queue }
    }

    pub fn submit_command(&self, submit_info: &MagmaSubmitInfo) -> Result<()> {
        self.queue.submit_command(submit_info)
    }

    pub fn check_status(&self) -> Result<()> {
        self.queue.check_status()
    }
}
