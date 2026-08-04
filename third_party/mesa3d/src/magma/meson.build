# Copyright © 2024 Google
# SPDX-License-Identifier: MIT

magma_generated_libs = []

dep_windows_sys = null_dep
dep_libc_rs = null_dep
if host_machine.system() == 'linux' or host_machine.system() == 'android'
  i915_zerocopy_types = [
    'drm_i915_gem_memory_class_instance',
    'drm_i915_memory_region_info',
    'drm_i915_memory_region_info__bindgen_ty_1',
    'drm_i915_memory_region_info__bindgen_ty_1__bindgen_ty_1',
    'drm_i915_query_memory_regions',
  ]
  i915_extra_args = ['--flexarray-dst']
  foreach t : i915_zerocopy_types
    i915_extra_args += ['--with-derive-custom', t + '=zerocopy::FromBytes,zerocopy::Immutable,zerocopy::KnownLayout']
  endforeach

  xe_zerocopy_types = [
    'drm_xe_mem_region',
    'drm_xe_query_mem_regions',
    'drm_xe_query_config',
    'drm_xe_query_engines',
    'drm_xe_query_gt_list',
    'drm_xe_query_topology_mask',
    'drm_xe_oa_unit',
    'drm_xe_query_oa_units',
    'drm_xe_query_eu_stall',
  ]
  xe_extra_args = ['--flexarray-dst']
  foreach t : xe_zerocopy_types
    xe_extra_args += ['--with-derive-custom', t + '=zerocopy::FromBytes,zerocopy::Immutable,zerocopy::KnownLayout']
  endforeach

  bindgen_configs = [
    ['drm',     drm_h,        ['DRM_.+', 'drm_.+'],               ['drm_.+'],         []],
    ['amdgpu',  amdgpu_drm_h, ['DRM_AMDGPU_.+', 'AMDGPU_.+'],     ['drm_amdgpu_.+'],  []],
    ['msm',     msm_drm_h,    ['DRM_MSM_.+', 'MSM_.+'],           ['drm_msm_.+'],     []],
    ['virtgpu', virtgpu_drm_h,['DRM_VIRTGPU_.+', 'VIRTGPU_.+'],   ['drm_virtgpu_.+'], []],
    ['i915',    i915_drm_h,   ['DRM_I915_.+', 'I915_.+'],         ['drm_i915_.+'],    i915_extra_args],
    ['xe',      xe_drm_h,     ['DRM_XE_.+', 'XE_.+'],             ['drm_xe_.+'],      xe_extra_args]
  ]

  dep_zerocopy = dependency('zerocopy',
    version: '>= 0.8.13',
    fallback: ['zerocopy-0.8-rs', 'dep_zerocopy'],
    required: true,
  )

  foreach config : bindgen_configs
    name = config[0]
    header = config[1]

    bindgen_args = [
      '--with-derive-default',
      '--no-prepend-enum-name',
      '--no-doc-comments',
      '--no-layout-tests'
    ]

    if config.length() > 4
      bindgen_args += config[4]
    endif

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
      dependencies: [dep_zerocopy],
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
