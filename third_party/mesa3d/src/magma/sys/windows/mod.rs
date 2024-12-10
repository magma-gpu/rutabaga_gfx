// Copyright 2025 Google
// SPDX-License-Identifier: MIT

mod amd;
mod d3dkmt_common;
mod macros;
mod wddm;

pub use amd::Amd;
pub use d3dkmt_common::WindowsDevice as PlatformDevice;
pub use d3dkmt_common::WindowsPhysicalDevice as PlatformPhysicalDevice;
pub use wddm::enumerate_devices;
pub use wddm::VendorPrivateData;
