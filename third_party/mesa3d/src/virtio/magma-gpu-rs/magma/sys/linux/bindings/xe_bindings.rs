// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

#![allow(clippy::all)]
#![allow(dead_code)]
#![allow(non_camel_case_types)]

#[cfg(avoid_cargo)]
pub use magma_gpu_magma_xe_bindgen::*;

#[cfg(not(avoid_cargo))]
include!(concat!(env!("OUT_DIR"), "/magma_gpu_magma_xe_bindgen.rs"));
