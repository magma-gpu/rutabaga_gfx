# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

librutabaga_gfx_ffi = library(
  'rutabaga_gfx_ffi',
  'src/lib.rs',
  dependencies: dep_rutabaga_gfx,
  link_with: [librutabaga_gfx, libmesa_rust_util],
  rust_abi: 'c',
  install: true,
)

pkg = import('pkgconfig')
pkg.generate(
  libraries: '-L${libdir} -lrutabaga_gfx_ffi',
  name: 'rutabaga_gfx_ffi',
  version: meson.project_version(),
  description: 'C FFI bindings to Rutabaga VGI',
)

rutabaga_gfx_ffi_h = files('src/include/rutabaga_gfx_ffi.h')
install_headers(rutabaga_gfx_ffi_h,
                subdir: 'rutabaga_gfx')

if with_tests
  inc_ffi = include_directories('src/include')
  subdir('src/tests')
endif
