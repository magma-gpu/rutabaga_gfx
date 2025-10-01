// Copyright 2025 Google
// SPDX-License-Identifier: MIT

#![allow(clippy::all)]
#![allow(non_upper_case_globals)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]

#[cfg(avoid_cargo)]
pub use mesa3d_magma_amdgpu_bindgen::*;

#[cfg(not(avoid_cargo))]
include!(concat!(env!("OUT_DIR"), "/mesa3d_magma_amdgpu_bindgen.rs"));
