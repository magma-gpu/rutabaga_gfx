# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'ryu-1-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '1.0.15',
  license: 'Apache-2.0 OR BSL-1.0',
)

lib = static_library(
  'ryu',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
)

dep_ryu = declare_dependency(
  link_with: [lib],
)
