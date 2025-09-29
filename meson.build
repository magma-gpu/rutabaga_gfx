# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'rutabaga_gfx',
  ['rust', 'c'],
  meson_version: '>=1.3.0',
  license : 'BSD-3-Clause AND MIT',
  version: '0.1.71',
  meson_version: '>=1.3.0',
  default_options : [
    'rust_std=2021',
    'build.rust_std=2021',
  ],
)

rust = import('rust')
null_dep = dependency('', required : false)

features = get_option('features')
with_ffi = get_option('ffi')
with_kumquat = get_option('kumquat')
with_tests = get_option('build-tests')

with_gbm = features.contains('gbm')
with_gfxstream = features.contains('gfxstream')
with_virgl_renderer = features.contains('virgl_renderer')

rutabaga_args = []
dep_rutabaga_gfx = []

if with_gbm
  error('rutabaga_gfx does not work with upstream gbm yet')
  dep_gbm = dependency('gbm', version: '>= 21.1.1')
  rutabaga_args += ['--cfg', 'feature="gbm"']
  dep_rutabaga_gfx += dep_gbm
endif

if with_gfxstream
  dep_gfxstream = dependency('gfxstream_backend', version: '>= 0.1.2')
  rutabaga_args += ['--cfg', 'feature="gfxstream"']
  dep_rutabaga_gfx += dep_gfxstream
endif

if with_virgl_renderer
  dep_virgl = dependency('virglrenderer', version: '>= 1.0.0')
  rutabaga_args += ['--cfg', 'feature="virgl_renderer"']
  dep_rutabaga_gfx += virgl_renderer
endif

if with_kumquat
  rutabaga_args += ['--cfg', 'fence_passing_option1']
endif

subdir('third_party')
subdir('src')

if with_ffi
  subdir('ffi')
endif

if with_kumquat
  subdir('kumquat')
endif
