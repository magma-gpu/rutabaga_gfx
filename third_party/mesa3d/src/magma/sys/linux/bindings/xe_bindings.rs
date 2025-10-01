// Copyright 2025 Google
// SPDX-License-Identifier: MIT

#![allow(clippy::all)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]

#[cfg(avoid_cargo)]
pub use mesa3d_magma_xe_bindgen::*;

#[cfg(not(avoid_cargo))]
include!(concat!(env!("OUT_DIR"), "/mesa3d_magma_xe_bindgen.rs"));
