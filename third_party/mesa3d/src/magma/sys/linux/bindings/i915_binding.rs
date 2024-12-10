// Copyright 2025 Google
// SPDX-License-Identifier: MIT

#![allow(clippy::all)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]

#[cfg(not(use_meson))]
include!(concat!(env!("OUT_DIR"), "/i915_bindings.rs"));

#[cfg(use_meson)]
pub use i915_bindings::*;
