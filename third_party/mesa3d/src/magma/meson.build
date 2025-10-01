# Copyright © 2024 Google
# SPDX-License-Identifier: MIT

magma_generated_libs = []

dep_log = dependency('log',
  version: '>= 0.4.22',
  fallback: ['log-0.4-rs', 'dep_log'],
  required: true,
)

dep_windows_sys = null_dep
if host_machine.system() == 'linux' or host_machine.system() == 'android'
  drm_bindgen = rust.bindgen(
    input : drm_h,
    output : 'mesa3d_magma_drm_bindgen.rs',
    include_directories : [inc_include],
    args : [
      '--with-derive-default',
      '--allowlist-var', 'DRM_.+',
      '--allowlist-var', 'drm_.+',
      '--allowlist-type', 'drm_.+',
      '--no-prepend-enum-name',
      '--no-doc-comments',
      '--no-layout-tests'
    ],
  )

  amdgpu_bindgen = rust.bindgen(
    input : amdgpu_drm_h,
    output : 'mesa3d_magma_amdgpu_bindgen.rs',
    include_directories : [inc_include],
    args : [
      '--with-derive-default',
      '--allowlist-var', 'DRM_AMDGPU_.+',
      '--allowlist-var', 'AMDGPU_.+',
      '--allowlist-type', 'drm_amdgpu_.+',
      '--no-prepend-enum-name',
      '--no-doc-comments',
      '--no-layout-tests'
    ],
  )

  msm_bindgen = rust.bindgen(
    input : msm_drm_h,
    output : 'mesa3d_magma_msm_bindgen.rs',
    include_directories : [inc_include],
    args : [
      '--with-derive-default',
      '--allowlist-var', 'DRM_MSM_.+',
      '--allowlist-var', 'MSM_.+',
      '--allowlist-type', 'drm_msm_.+',
      '--no-prepend-enum-name',
      '--no-doc-comments',
      '--no-layout-tests'
    ],
  )

  virtgpu_bindgen = rust.bindgen(
    input : virtgpu_drm_h,
    output : 'mesa3d_magma_virtgpu_bindgen.rs',
    include_directories : [inc_include],
    args : [
      '--with-derive-default',
      '--allowlist-var', 'DRM_VIRTGPU_.+',
      '--allowlist-var', 'VIRTGPU_.+',
      '--allowlist-type', 'drm_virtgpu_.+',
      '--no-prepend-enum-name',
      '--no-doc-comments',
      '--no-layout-tests'
    ]
  )

  xe_bindgen = rust.bindgen(
    input : xe_drm_h,
    output : 'mesa3d_magma_xe_bindgen.rs',
    include_directories : [inc_include],
    args : [
      '--with-derive-default',
      '--allowlist-var', 'DRM_XE.+',
      '--allowlist-var', 'XE_.+',
      '--allowlist-type', 'drm_xe_.+',
      '--no-prepend-enum-name',
      '--no-doc-comments',
      '--no-layout-tests'
    ],
  )

  i915_bindgen = rust.bindgen(
    input : i915_drm_h,
    output : 'mesa3d_magma_i915_bindgen.rs',
    include_directories : [inc_include],
    args : [
      '--with-derive-default',
      '--allowlist-var', 'DRM_I915.+',
      '--allowlist-var', 'I915_.+',
      '--allowlist-type', 'drm_i915_.+',
      '--no-prepend-enum-name',
      '--no-doc-comments',
      '--no-layout-tests'
    ],
  )

  drm_bindgen_gen = static_library(
    'mesa3d_magma_drm_bindgen',
    drm_bindgen,
    gnu_symbol_visibility : 'hidden',
    rust_abi : 'rust',
  )

  amdgpu_bindgen_gen = static_library(
    'mesa3d_magma_amdgpu_bindgen',
    amdgpu_bindgen,
    gnu_symbol_visibility : 'hidden',
    rust_abi : 'rust',
  )

  msm_bindgen_gen = static_library(
    'mesa3d_magma_msm_bindgen',
    msm_bindgen,
    gnu_symbol_visibility : 'hidden',
    rust_abi : 'rust',
  )

  virtgpu_bindgen_gen = static_library(
    'mesa3d_magma_virtgpu_bindgen',
    virtgpu_bindgen,
    gnu_symbol_visibility : 'hidden',
    rust_abi : 'rust',
  )

  i915_bindgen_gen = static_library(
    'mesa3d_magma_i915_bindgen',
    i915_bindgen,
    gnu_symbol_visibility : 'hidden',
    rust_abi : 'rust',
  )

  xe_bindgen_gen = static_library(
    'mesa3d_magma_xe_bindgen',
    xe_bindgen,
    gnu_symbol_visibility : 'hidden',
    rust_abi : 'rust',
  )

  magma_generated_libs += [drm_bindgen_gen, amdgpu_bindgen_gen,
                           msm_bindgen_gen, virtgpu_bindgen_gen,
                           i915_bindgen_gen, xe_bindgen_gen]
elif host_machine.system() == 'windows'
  dep_windows_sys = dependency('windows-sys-rs',
    version: '>= 0.61.1',
    fallback: ['windows-sys-0.6-rs', 'dep_windows_sys'],
    required: true,
  )
endif

magma_args = []
magma_args += ['--cfg', 'avoid_cargo']

libmesa_magma = static_library(
  'mesa3d_magma',
  'lib.rs',
  gnu_symbol_visibility : 'hidden',
  rust_abi : 'rust',
  rust_args: magma_args,
  link_with: [magma_generated_libs, libmesa_rust_util, libmesa_protocols,
              libvirtgpu_kumquat],
  dependencies: [dep_mesa3d_util, dep_log, dep_windows_sys]
)
