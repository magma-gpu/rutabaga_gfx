# Copyright © 2025 Google
# SPDX-License-Identifier: MIT

libvirtgpu_kumquat = static_library(
  'virtgpu_kumquat',
  'lib.rs',
  gnu_symbol_visibility : 'hidden',
  rust_abi : 'rust',
  rust_args : rust_global_args,
  link_with: [libmesa_protocols, libmesa_rust_util],
  dependencies: [dep_mesa3d_util]
)
