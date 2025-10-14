# Copyright 2025 Magma-GPU
# SPDX-License-Identifier: MIT

project(
  'serde_json-1-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '1.0.145',
  license: 'MIT OR Apache-2.0',
)

serde = subproject('serde-1-rs').get_variable('lib')
serde_core = subproject('serde_core-1-rs').get_variable('lib')
serde_derive = subproject('serde_derive-1-rs').get_variable('lib')
ryu = subproject('ryu-1-rs').get_variable('lib')
itoa = subproject('itoa-1-rs').get_variable('lib')
memchr = subproject('memchr-2-rs').get_variable('lib')

rust_args = ['--cfg=feature="default"', '--cfg=feature="std"']

if host_machine.cpu_family() == 'x86_64' or host_machine.cpu_family() == 'aarch64':
    rust_args += '--cfg=fast_arithmetic="64"'
elif host_machine.cpu_family() == 'x86' or host_machine.cpu_family() == 'arm'
    rust_args += '--cfg=fast_arithmetic="32"'
endif

lib = static_library(
  'serde_json',
  'src/lib.rs',
  override_options : ['rust_std=2021', 'build.rust_std=2021'],
  rust_abi : 'rust',
  rust_args: rust_args,
  link_with: [serde, serde_core, serde_derive, ryu, itoa, memchr],
)

dep_serde_json = declare_dependency(
  link_with: [lib, serde, serde_core, serde_derive, ryu, itoa, memchr]
)
