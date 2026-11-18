# Copyright (C) 2026 The Magma GPU Project
# SPDX-License-Identifier: MIT

dep_clap = dependency(
  'rust-clap',
  version: '>= 4.1.8',
  fallback: ['clap-4-rs', 'dep_clap'],
  required: true,
)
dep_broccolini = dep_rutabaga_gfx + [dep_clap]

broccolini_exe = executable(
  'broccolini',
  'src/main.rs',
  dependencies: dep_broccolini,
  link_with: [librutabaga_gfx, libmagma_gpu_rs],
  rust_args: dep_link_args,
  install: true,
)
