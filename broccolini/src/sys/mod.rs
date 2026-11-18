// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

cfg_if::cfg_if! {
    if #[cfg(any(target_os = "android", target_os = "linux"))] {
        pub mod linux;
        use linux as platform;
    } else if #[cfg(windows)] {
        pub mod windows;
        use windows as platform;
    } else if #[cfg(target_os = "macos")] {
        pub mod mac;
        use mac as platform;
    } else {
        compile_error!("Unsupported platform");
    }
}

#[allow(unused_imports)]
pub use platform::ProcessId;
pub use platform::WorkerProcess;
