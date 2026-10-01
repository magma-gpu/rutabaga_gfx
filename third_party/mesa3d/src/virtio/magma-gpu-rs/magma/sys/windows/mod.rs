// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

mod amd;
mod macros;
mod wddm;

pub use amd::Amd;
pub use wddm::enumerate_devices;
pub use wddm::VendorPrivateData;
pub use wddm::WindowsDevice as PlatformDevice;
pub use wddm::WindowsPhysicalDevice as PlatformPhysicalDevice;
