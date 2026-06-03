// Copyright 2021 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! The cross-domain component type, specialized for allocating and sharing resources across domain
//! boundaries.

mod atomic_memory_sentinel_manager;
mod common;
mod component;
mod context;
mod cross_domain_protocol;
mod worker;

pub use component::CrossDomain;
