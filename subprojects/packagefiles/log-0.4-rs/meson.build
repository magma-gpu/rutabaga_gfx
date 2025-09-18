# Copyright © 2024 Google
# SPDX-License-Identifier: MIT

project(
  'log-0.4-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '0.4.27',
  license: 'MIT OR Apache-2.0',
)

lib = static_library(
  'log',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
)

dep_log = declare_dependency(
  link_with : [lib]
)
