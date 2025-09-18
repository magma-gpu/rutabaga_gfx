# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'serde-1-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '1.0.226',
  license: 'MIT OR Apache-2.0',
)

serde_core = subproject('serde_core-1-rs').get_variable('lib')
serde_derive = subproject('serde_derive-1-rs').get_variable('lib')

rust_args = ['--cfg', 'feature="serde_derive"']

lib = static_library(
  'serde',
  'src/lib.rs',
  rust_abi : 'rust',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_args: rust_args,
  link_with : [serde_core, serde_derive]
)

dep_serde = declare_dependency(
  link_with: [lib, serde_core, serde_derive],
)
