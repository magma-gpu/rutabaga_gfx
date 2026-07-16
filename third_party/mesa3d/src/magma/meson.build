# Copyright © 2024 Google
# SPDX-License-Identifier: MIT

magma_generated_libs = []

dep_windows_sys = null_dep
dep_libc_rs = null_dep
if host_machine.system() == 'linux' or host_machine.system() == 'android'
  bindgen_configs = [
    ['drm',     drm_h,        ['DRM_.+', 'drm_.+'],               ['drm_.+']],
    ['amdgpu',  amdgpu_drm_h, ['DRM_AMDGPU_.+', 'AMDGPU_.+'],     ['drm_amdgpu_.+']],
    ['msm',     msm_drm_h,    ['DRM_MSM_.+', 'MSM_.+'],           ['drm_msm_.+']],
    ['virtgpu', virtgpu_drm_h,['DRM_VIRTGPU_.+', 'VIRTGPU_.+'],   ['drm_virtgpu_.+']],
    ['i915',    i915_drm_h,   ['DRM_I915_.+', 'I915_.+'],         ['drm_i915_.+']],
    ['xe',      xe_drm_h,     ['DRM_XE_.+', 'XE_.+'],             ['drm_xe_.+']]
  ]

  foreach config : bindgen_configs
    name = config[0]
    header = config[1]

    bindgen_args = [
      '--with-derive-default',
      '--no-prepend-enum-name',
      '--no-doc-comments',
      '--no-layout-tests'
    ]

    foreach var_pattern : config[2]
      bindgen_args += ['--allowlist-var', var_pattern]
    endforeach

    foreach type_pattern : config[3]
      bindgen_args += ['--allowlist-type', type_pattern]
    endforeach

    generated_rs = rust.bindgen(
      input : header,
      output : 'magma_gpu_magma_' + name + '_bindgen.rs',
      include_directories : [inc_include],
      args : bindgen_args,
    )

    magma_generated_libs += static_library(
      'magma_gpu_magma_' + name + '_bindgen',
      generated_rs,
      gnu_symbol_visibility : 'hidden',
      rust_abi : 'rust',
    )

    dep_libc_rs = dependency('rust-libc',
      version: '>= 0.2.185',
      fallback: ['libc-0.2-rs', 'dep_libc'],
      required: true,
    )
  endforeach
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
  'magma_gpu_magma',
  'lib.rs',
  gnu_symbol_visibility : 'hidden',
  rust_abi : 'rust',
  rust_args: magma_args,
  link_with: [magma_generated_libs, libmagma_gpu_rs],
  dependencies: [dep_magma_gpu_rs, dep_windows_sys, dep_libc_rs]
)
