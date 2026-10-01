// Copyright 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

mod amdgpu;
mod bindings;
mod common;
mod drm;
mod i915;
mod kgsl;
mod macros;
mod msm;
mod panthor;
mod virtgpu;
mod xe;

pub use amdgpu::AmdGpuPhysicalDevice;
pub use common::enumerate_devices;
pub use common::PlatformDevice;
pub use common::PlatformPhysicalDevice;
pub use drm::*;
pub use i915::I915PhysicalDevice;
pub use kgsl::KgslPhysicalDevice;
pub use msm::MsmPhysicalDevice;
pub use panthor::PanthorPhysicalDevice;
pub use virtgpu::virtgpu_enumerate_devices;
pub use xe::XePhysicalDevice;
