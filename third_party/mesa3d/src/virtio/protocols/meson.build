# Copyright © 2024 Google
# SPDX-License-Identifier: MIT

libmesa_protocols = static_library(
  'mesa3d_protocols',
  'lib.rs',
  gnu_symbol_visibility : 'hidden',
  rust_abi : 'rust',
  rust_args : rust_global_args,
  link_with: [libmesa_rust_util],
  dependencies: dep_mesa3d_util
)
