// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use std::env;
use std::path::PathBuf;

fn generate_linux_bindings(source_dir: PathBuf, out_dir: PathBuf) {
    println!("cargo::rustc-check-cfg=cfg(use_meson)");

    let generated_path = std::path::Path::new(&out_dir).join("drm_bindings.rs");
    if generated_path.exists() {
        return;
    }

    let drm_header = format!("{}/headers/drm.h", source_dir.display());
    let i915_drm_header = format!("{}/headers/i915_drm.h", source_dir.display());
    let xe_drm_header = format!("{}/headers/xe_drm.h", source_dir.display());
    let amdgpu_drm_header = format!("{}/headers/amdgpu_drm.h", source_dir.display());
    let virtgpu_drm_header = format!("{}/headers/virtgpu_drm.h", source_dir.display());
    let msm_drm_header = format!("{}/headers/msm_drm.h", source_dir.display());

    bindgen::Builder::default()
        .header(drm_header)
        .derive_default(true)
        .derive_debug(true)
        .allowlist_var("DRM_.+")
        .allowlist_type("drm_.+")
        .prepend_enum_name(false)
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("Unable to generate drm bindings")
        .write_to_file(out_dir.join("drm_bindings.rs"))
        .expect("Unable to generate bindings");

    bindgen::Builder::default()
        .header(i915_drm_header)
        .derive_default(true)
        .derive_debug(true)
        .allowlist_var("DRM_I915_.+")
        .allowlist_var("I915_.+")
        .allowlist_type("drm_i915_.+")
        .prepend_enum_name(false)
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("Unable to generate i915 bindings")
        .write_to_file(out_dir.join("i915_bindings.rs"))
        .expect("Unable to generate bindings");

    bindgen::Builder::default()
        .header(xe_drm_header)
        .derive_default(true)
        .derive_debug(true)
        .allowlist_var("DRM_XE_.+")
        .allowlist_var("XE_.+")
        .allowlist_type("drm_xe_.+")
        .prepend_enum_name(false)
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("Unable to generate xe bindings")
        .write_to_file(out_dir.join("xe_bindings.rs"))
        .expect("Unable to generate bindings");

    bindgen::Builder::default()
        .header(amdgpu_drm_header)
        .derive_default(true)
        .derive_debug(true)
        .allowlist_var("DRM_AMDGPU_.+")
        .allowlist_var("AMDGPU_.+")
        .allowlist_type("drm_amdgpu_.+")
        .prepend_enum_name(false)
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("Unable to generate amdgpu bindings")
        .write_to_file(out_dir.join("amdgpu_bindings.rs"))
        .expect("Unable to generate bindings");

    bindgen::Builder::default()
        .header(msm_drm_header)
        .derive_default(true)
        .derive_debug(true)
        .allowlist_var("DRM_MSM_.+")
        .allowlist_var("MSM_.+")
        .allowlist_type("drm_msm_.+")
        .prepend_enum_name(false)
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("Unable to generate msm bindings")
        .write_to_file(out_dir.join("msm_bindings.rs"))
        .expect("Unable to generate bindings");

    bindgen::Builder::default()
        .header(virtgpu_drm_header)
        .derive_default(true)
        .derive_debug(true)
        .allowlist_var("DRM_VIRTGPU_.+")
        .allowlist_var("VIRTGPU_.+")
        .allowlist_type("drm_virtgpu_.+")
        .prepend_enum_name(false)
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("Unable to generate virtgpu bindings")
        .write_to_file(out_dir.join("virtgpu_bindings.rs"))
        .expect("Unable to generate virtgpu bindings");
}

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let source_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should always be set"),
    );
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR should always be set"));

    if target_os.as_str() == "linux" {
        generate_linux_bindings(source_dir, out_dir)
    }
}
