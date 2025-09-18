# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'memchr-2-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '2.7.5',
  license: 'Unlicense OR MIT',
)

rust_args = ['--cfg=feature="default"', '--cfg=feature="std"']

lib = static_library(
  'memchr',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
  rust_args: rust_args,
)

dep_memchr = declare_dependency(
  link_with: [lib]
)
