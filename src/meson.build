# Copyright 2025 Magma-GPU project
# SPDX-License-Identifier: MIT

dep_libc = dependency('libc-0.2-rs',
  version: '>= 0.2.153',
  fallback: ['libc-0.2-rs', 'dep_libc'],
  required: true,
)

dep_log = dependency('log-0.4-rs',
  version: '>= 0.4',
  fallback: ['log-0.4-rs', 'dep_log'],
  required: true,
)

dep_serde = dependency('serde-1-rs',
  version: '>= 1',
  fallback: ['serde-1-rs', 'dep_serde'],
  required: true,
)

dep_serde_json = dependency('serde_json-1-rs',
  version: '>= 1',
  fallback: ['serde_json-1-rs', 'dep_serde_json'],
  required: true,
)

dep_rutabaga_gfx += dep_mesa3d_util
dep_rutabaga_gfx += [
  dep_libc,
  dep_log,
  dep_serde_json,
  dep_serde,
]

librutabaga_gfx = static_library(
  'rutabaga_gfx',
  'lib.rs',
  dependencies: dep_rutabaga_gfx,
  link_with: [libmesa_rust_util],
  rust_abi: 'rust',
  gnu_symbol_visibility: 'hidden',
  rust_args: rutabaga_args,
)
