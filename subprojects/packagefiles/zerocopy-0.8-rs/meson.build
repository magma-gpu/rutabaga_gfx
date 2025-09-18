# Copyright © 2024 Google
# SPDX-License-Identifier: MIT

project(
  'zerocopy-0.8-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '0.8.13',
  license: 'BSD-2-Clause OR Apache-2.0 OR MIT',
)

zerocopy_args = ['--cfg', 'feature="derive"']
zerocopy_derive = subproject('zerocopy-derive-0.8-rs').get_variable('lib')

lib = static_library(
  'zerocopy',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
  rust_args: zerocopy_args,
  link_with : [zerocopy_derive]
)

dep_zerocopy = declare_dependency(
  link_with : [lib, zerocopy_derive]
)
