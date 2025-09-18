# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'clap_builder-4-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '4.5.48',
  license: 'MIT OR Apache-2.0',
)

anstyle = subproject('anstyle-1-rs').get_variable('lib')
clap_lex = subproject('clap_lex-0.7-rs').get_variable('lib')

rust_args = [
  '--cfg', 'feature="std"',
  '--cfg', 'feature="help"',
  '--cfg', 'feature="usage"',
  '--cfg', 'feature="error-context"',
]

lib = static_library(
  'clap_builder',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
  rust_args: rust_args,
  link_with: [anstyle, clap_lex],
)

dep_clap_builder = declare_dependency(
  link_with: [lib, anstyle, clap_lex],
)
