# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'clap_derive-4-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '4.5.47',
  license: 'MIT OR Apache-2.0',
)

heck = subproject('heck-0.5-rs').get_variable('lib')
proc_macro2 = subproject('proc-macro2-1-rs').get_variable('lib')
quote = subproject('quote-1-rs').get_variable('lib')
syn = subproject('syn-2-rs').get_variable('lib')

rust = import('rust')
lib = rust.proc_macro(
  'clap_derive',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  link_with : [heck, syn, quote, proc_macro2]
)

dep_clap_derive = declare_dependency(
  link_with : [lib, heck, syn, quote, proc_macro2]
)
