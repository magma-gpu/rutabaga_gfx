// Copyright 2025 Google
// SPDX-License-Identifier: MIT
#![allow(clippy::all)]
#![allow(non_upper_case_globals)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]

#[cfg(not(use_meson))]
include!(concat!(env!("OUT_DIR"), "/drm_bindings.rs"));

#[cfg(use_meson)]
pub use drm_bindings::*;
