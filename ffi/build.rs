// Copyright 2023 The ChromiumOS Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();

    println!("cargo::rustc-check-cfg=cfg(goldfish)"); // Silences warnings
                                                      // Override prefix from environment variable (with a default)
    println!("cargo:rerun-if-changed=build.rs"); // Rebuild if build.rs changes

    if target_os.contains("linux") || target_os.contains("nto") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,librutabaga_gfx_ffi.so.0");
    }
}
