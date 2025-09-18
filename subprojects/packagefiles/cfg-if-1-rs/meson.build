# Copyright © 2024 Google
# SPDX-License-Identifier: MIT

project(
  'cfg-if-1-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '1.0.0',
  license: 'MIT OR Apache-2.0',
)

lib = static_library(
  'cfg_if',
  'src/lib.rs',
  override_options : ['rust_std=2018', 'build.rust_std=2018'],
  rust_abi : 'rust',
)

dep_cfg_if = declare_dependency(
  link_with : [lib]
)
