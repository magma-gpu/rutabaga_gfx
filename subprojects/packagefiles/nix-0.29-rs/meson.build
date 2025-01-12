# Copyright © 2024 Google
# SPDX-License-Identifier: MIT

project(
  'nix-0.29-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '0.29.0',
  license: 'MIT',
)

# nix depends on libc, bitflags, and cfg-if
dep_libc_internal = dependency('libc-0.2-rs',
  version: '>= 0.2.153',
  fallback: ['libc-0.2-rs', 'dep_libc'],
)

dep_bitflags_internal = dependency('bitflags-2-rs',
  version: '>= 2.3',
  fallback: ['bitflags-2-rs', 'dep_bitflags'],
)

dep_cfg_if_internal = dependency('cfg-if-1-rs',
  version: '>= 1.0',
  fallback: ['cfg-if-1-rs', 'dep_cfg_if'],
)

nix_args = [
  '--cfg', 'feature="default"',
]

lib = static_library(
  'nix',
  'src/lib.rs',
  dependencies: [dep_libc_internal, dep_bitflags_internal, dep_cfg_if_internal],
  rust_abi : 'rust',
  override_options : ['rust_std=2018', 'build.rust_std=2018'],
  rust_args: nix_args,
)

dep_nix = declare_dependency(
  link_with : [lib],
  dependencies: [dep_libc_internal, dep_bitflags_internal, dep_cfg_if_internal],
)
