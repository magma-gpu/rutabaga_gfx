# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'clap_lex-0.7-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '0.7.5',
  license: 'MIT OR Apache-2.0',
)

lib = static_library(
   'clap_lex',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
)

dep_clap_lex = declare_dependency(
  link_with: lib,
)