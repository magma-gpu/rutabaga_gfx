# Copyright 2025 Magma-GPU project
# SPDX-License-Identifier: MIT

dep_libc = dependency('rust-libc',
  version: '>= 0.2.153',
  fallback: ['libc-0.2-rs', 'dep_libc'],
  required: true,
)

dep_log = dependency('rust-log',
  version: '>= 0.4',
  fallback: ['log-0.4-rs', 'dep_log'],
  required: true,
)

dep_serde = dependency('rust-serde',
  version: '>= 1',
  fallback: ['serde-1-rs', 'dep_serde'],
  required: true,
)

dep_serde_json = dependency('rust-serde-json',
  version: '>= 1',
  fallback: ['serde_json-1-rs', 'dep_serde_json'],
  required: true,
)

dep_rutabaga_gfx += dep_magma_gpu_rs
dep_rutabaga_gfx += [
  dep_libc,
  dep_log,
  dep_serde_json,
  dep_serde,
]

if host_machine.system() == 'android'
  dep_rutabaga_gfx += dependency('rust-nativewindow')
endif

librutabaga_gfx = static_library(
  'rutabaga_gfx',
  'lib.rs',
  dependencies: dep_rutabaga_gfx,
  link_with: [libmagma_gpu_rs],
  rust_abi: 'rust',
  gnu_symbol_visibility: 'hidden',
  rust_args: rutabaga_args,
)
