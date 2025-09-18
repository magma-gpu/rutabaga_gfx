# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

e = executable(
  'rutabaga_gfx_ffi_test',
  'rutabaga_test.c',
  include_directories: inc_ffi,
  c_args: '-DRUTABAGA_GFX_FFI_UNSTABLE',
  link_with: librutabaga_gfx_ffi,
)

test('rutabaga_gfx_test', e)
