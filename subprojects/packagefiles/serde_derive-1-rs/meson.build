# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'serde_derive-1-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '1.0.226',
  license: 'MIT OR Apache-2.0',
)

syn = subproject('syn-2-rs').get_variable('lib')
quote = subproject('quote-1-rs').get_variable('lib')
proc_macro2 = subproject('proc-macro2-1-rs').get_variable('lib')

rust = import('rust')
lib = rust.proc_macro(
  'serde_derive',
  'src/lib.rs',
  link_with : [syn, quote, proc_macro2]
)

dep_serde_derive = declare_dependency(
  link_with : [lib, syn, quote, proc_macro2]
)
