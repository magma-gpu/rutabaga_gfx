# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'anstyle-1-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '1.0.11',
  license: 'MIT OR Apache-2.0',
)

lib = static_library(
   'anstyle',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
)

dep_anstyle = declare_dependency(
  link_with: lib,
)
