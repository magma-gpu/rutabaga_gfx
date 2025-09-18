# Copyright © 2025 Google
# SPDX-License-Identifier: MIT

project(
  'errno-0.3-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '0.3.12',
  license: 'MIT OR Apache-2.0',
)

libc = subproject('libc-0.2-rs').get_variable('lib')

lib = static_library(
  'libc_errno',
  'src/lib.rs',
  override_options : ['rust_std=2018', 'build.rust_std=2018'],
  link_with: [libc],
  rust_abi : 'rust',
)

dep_errno = declare_dependency(
  link_with : [lib]
)
