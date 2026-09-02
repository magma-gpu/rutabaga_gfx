# Copyright © 2026 Mesa3D authors
# SPDX-License-Identifier: MIT

project(
  'mach2-0.7-rs',
  'rust',
  version : '0.7.0',
  license : 'BSD-2-Clause OR MIT OR Apache-2.0',
  meson_version : '>= 1.7.0',
)

# The crate is edition 2024, but uses nothing from it that 2021 lacks, so it
# builds with the same edition as the rest of the Rust here and needs no
# dependencies of its own.
lib = static_library(
  'mach2',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021', 'warning_level=0'],
  rust_abi : 'rust',
)

dep_mach2 = declare_dependency(
  link_with : [lib],
)
