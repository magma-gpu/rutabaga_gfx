// Copyright 2025 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::collections::BTreeMap as Map;
use std::sync::Arc;
use std::sync::Mutex;

use crate::handle::RutabagaHandle;

use crate::rutabaga_utils::RutabagaIovec;

pub struct ContextResource {
    pub handle: Option<Arc<RutabagaHandle>>,
    pub backing_iovecs: Option<Vec<RutabagaIovec>>,
}

pub type ContextResources = Arc<Mutex<Map<u32, ContextResource>>>;
