# Copyright 2025 Magma-GPU project
# SPDX-License-Identifier: MIT

subdir('mesa3d/src/util/rust')
subdir('mesa3d/src/virtio/protocols')
subdir('mesa3d/src/virtio/virtgpu_kumquat')

if host_machine.system() == 'linux' or host_machine.system() == 'windows'
  subdir('mesa3d/src/magma/headers')
  subdir('mesa3d/src/magma')
endif
