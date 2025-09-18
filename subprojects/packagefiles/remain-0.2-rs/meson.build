# Copyright © 2024 Google
# SPDX-License-Identifier: MIT

project(
  'remain-0.2-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '0.2.12',
  license: 'MIT',
)

syn = subproject('syn-2-rs').get_variable('lib')
quote = subproject('quote-1-rs').get_variable('lib')
proc_macro2 = subproject('proc-macro2-1-rs').get_variable('lib')

rust = import('rust')
lib = rust.proc_macro(
  'remain',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  link_with : [syn, quote, proc_macro2]
)

dep_remain = declare_dependency(
  link_with : [lib, syn, quote, proc_macro2]
)
