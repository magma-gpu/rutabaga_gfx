// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

#![allow(clippy::all)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]

#[cfg(not(use_meson))]
include!(concat!(env!("OUT_DIR"), "/i915_bindings.rs"));

#[cfg(use_meson)]
pub use i915_bindings::*;
