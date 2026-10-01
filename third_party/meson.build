# Copyright 2025 Magma-GPU project
# SPDX-License-Identifier: MIT

subdir('mesa3d/src/virtio/magma-gpu-rs/lib')
if with_magma
  subdir('mesa3d/src/virtio/magma-gpu-rs/magma/headers')
  subdir('mesa3d/src/virtio/magma-gpu-rs/magma')
  dep_rutabaga_gfx += dep_libmagma
endif
