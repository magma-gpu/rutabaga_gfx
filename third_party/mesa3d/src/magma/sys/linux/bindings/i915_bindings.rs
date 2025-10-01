// Copyright 2025 Google
// SPDX-License-Identifier: MIT

#![allow(clippy::all)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

#[cfg(avoid_cargo)]
pub use mesa3d_magma_i915_bindgen::*;

#[cfg(not(avoid_cargo))]
include!(concat!(env!("OUT_DIR"), "/mesa3d_magma_i915_bindgen.rs"));
