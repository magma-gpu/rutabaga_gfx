# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'clap-4-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '4.5.48',
  license: 'MIT OR Apache-2.0',
)

clap_builder = subproject('clap_builder-4-rs').get_variable('lib')
clap_lex = subproject('clap_lex-0.7-rs').get_variable('lib')
clap_derive = subproject('clap_derive-4-rs').get_variable('lib')

rust_args = ['--cfg', 'feature="derive"']

lib = static_library(
  'clap',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
  rust_args: rust_args,
  link_with: [clap_builder, clap_lex, clap_derive],
)

dep_clap = declare_dependency(
  link_with: [lib, clap_builder, clap_lex, clap_derive],
)
