# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'itoa-1-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '1.0.9',
  license: 'MIT OR Apache-2.0',
)

lib = static_library(
  'itoa',
  'src/lib.rs',
  rust_args: ['--cfg=feature="default"', '--cfg=feature="std"'],
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
)

dep_itoa = declare_dependency(
  link_with: lib,
)
