# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'serde_core-1-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '1.0.226',
  license: 'MIT OR Apache-2.0',
)

serde_derive = subproject('serde_derive-1-rs').get_variable('lib')

rust_args = ['--cfg=feature="default"', '--cfg=feature="std"']

lib = static_library(
  'serde_core',
  'src/lib.rs',
  override_options : ['rust_std=2021'],
  rust_abi : 'rust',
  rust_args: rust_args,
  link_with: [serde_derive],
)

dep_serde_core = declare_dependency(
  link_with: [lib, serde_derive]
)
