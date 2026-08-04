// Copyright 2025 Google
// SPDX-License-Identifier: MIT

use std::env;
use std::path::PathBuf;

#[derive(Debug)]
struct ParseDerives<'a> {
    types: &'a [&'a str],
    derives: &'a [&'a str],
}

impl bindgen::callbacks::ParseCallbacks for ParseDerives<'_> {
    fn add_derives(&self, info: &bindgen::callbacks::DeriveInfo<'_>) -> Vec<String> {
        if self.types.contains(&info.name) {
            self.derives.iter().map(|s| s.to_string()).collect()
        } else {
            vec![]
        }
    }
}

fn add_zerocopy_derives(
    builder: bindgen::Builder,
    types: &'static [&'static str],
) -> bindgen::Builder {
    let zerocopy_derives = &[
        "zerocopy::FromBytes",
        "zerocopy::Immutable",
        "zerocopy::KnownLayout",
    ];
    builder.parse_callbacks(Box::new(ParseDerives {
        types,
        derives: zerocopy_derives,
    }))
}

fn generate_linux_bindgen(source_dir: PathBuf, out_dir: PathBuf) {
    println!("cargo::rustc-check-cfg=cfg(avoid_cargo)");
    println!("cargo::rerun-if-changed=build.rs");

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
        .write_to_file(out_dir.join("magma_gpu_magma_drm_bindgen.rs"))
        .expect("Unable to generate bindings");

    let i915_zerocopy_types = &[
        "drm_i915_gem_memory_class_instance",
        "drm_i915_memory_region_info",
        "drm_i915_memory_region_info__bindgen_ty_1",
        "drm_i915_memory_region_info__bindgen_ty_1__bindgen_ty_1",
        "drm_i915_query_memory_regions",
    ];

    let i915_builder = add_zerocopy_derives(
        bindgen::Builder::default()
            .header(i915_drm_header)
            .flexarray_dst(true)
            .derive_default(true)
            .derive_debug(true)
            .allowlist_var("DRM_I915_.+")
            .allowlist_var("I915_.+")
            .allowlist_type("drm_i915_.+")
            .prepend_enum_name(false)
            .generate_comments(false)
            .layout_tests(false),
        i915_zerocopy_types,
    );

    i915_builder
        .generate()
        .expect("Unable to generate i915 bindings")
        .write_to_file(out_dir.join("magma_gpu_magma_i915_bindgen.rs"))
        .expect("Unable to generate bindings");

    let xe_zerocopy_types = &[
        "drm_xe_mem_region",
        "drm_xe_query_mem_regions",
        "drm_xe_query_config",
        "drm_xe_query_engines",
        "drm_xe_query_gt_list",
        "drm_xe_query_topology_mask",
        "drm_xe_oa_unit",
        "drm_xe_query_oa_units",
        "drm_xe_query_eu_stall",
    ];

    let xe_builder = add_zerocopy_derives(
        bindgen::Builder::default()
            .header(xe_drm_header)
            .flexarray_dst(true)
            .derive_default(true)
            .derive_debug(true)
            .allowlist_var("DRM_XE_.+")
            .allowlist_var("XE_.+")
            .allowlist_type("drm_xe_.+")
            .prepend_enum_name(false)
            .generate_comments(false)
            .layout_tests(false),
        xe_zerocopy_types,
    );

    xe_builder
        .generate()
        .expect("Unable to generate xe bindings")
        .write_to_file(out_dir.join("magma_gpu_magma_xe_bindgen.rs"))
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
        .write_to_file(out_dir.join("magma_gpu_magma_amdgpu_bindgen.rs"))
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
        .write_to_file(out_dir.join("magma_gpu_magma_msm_bindgen.rs"))
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
        .write_to_file(out_dir.join("magma_gpu_magma_virtgpu_bindgen.rs"))
        .expect("Unable to generate virtgpu bindings");
}

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let source_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should always be set"),
    );
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR should always be set"));

    if target_os.as_str() == "linux" {
        generate_linux_bindgen(source_dir, out_dir)
    }
}
