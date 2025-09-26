# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'heck-0.5-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '0.5.0',
  license: 'MIT OR Apache-2.0',
)

lib = static_library(
  'heck',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
  native : true
)

dep_heck = declare_dependency(
  link_with: lib,
)
