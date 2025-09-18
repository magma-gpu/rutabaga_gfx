project(
  'unicode-ident-1-rs',
  'rust',
  meson_version: '>=1.3.0',
  version: '1.0.12',
  license: '(MIT OR Apache-2.0) AND Unicode-DFS-2016',
)

lib = static_library(
  'unicode_ident',
  'src/lib.rs',
  override_options : ['rust_std=2018', 'build.rust_std=2018'],
  rust_abi : 'rust',
)
