# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

dep_clap = dependency('clap-4-rs',
  version: '>= 4.1.8',
  fallback: ['clap-4-rs', 'dep_clap'],
  required: true,
)

dep_kumquat = dep_rutabaga_gfx + [dep_clap]

kumquat_exe = executable(
  'kumquat_virtio',
  'server/src/main.rs',
  dependencies: dep_kumquat,
  link_with: [
    librutabaga_gfx,
    libmesa_rust_util,
    libmesa_protocols
  ],
  install: true,
)
