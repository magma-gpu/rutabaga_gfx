// Copyright 2025 Google
// SPDX-License-Identifier: MIT

mod magma;
mod magma_defines;
mod magma_kumquat;
mod sys;
mod traits;

pub use magma_defines::*;

pub use magma::magma_enumerate_devices;
pub use magma::MagmaBuffer;
pub use magma::MagmaContext;
pub use magma::MagmaDevice;
pub use magma::MagmaPhysicalDevice;
