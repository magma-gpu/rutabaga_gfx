# Changelog

## [v0.1.71](https://github.com/magma-gpu/rutabaga_gfx/tree/v0.1.71)

### API and dependencies

- `rutabaga_gfx`
  - `RutabagaBuilder`
    - `capset_mask` and `fence_handler` are required when creating builder
    - `default_component` is now set via an optional builder function
    - `server_descriptor` is now set via an optional builder function
  - `Rutabaga`
    - `guest_cpu_mappable` removed from `ResourceInfo3D` and made into getter function
    - `query(..)` function renamed to `resource3d_info(..)`
  - rust features
    - `minigbm` feature renamed to `gbm` feature
    - `gfxstream_stub` feature removed
    - `x` feature removed
  - `anyhow`, `tempfile` dependencies removed
  - added dependency on `mesa3d_util`
- `mesa3d_util`
  - Good enough for projects that depend on `Cargo.toml` only
  - If you want to package to distro, ping `magma-gpu` team first
- `mesa3d_protocols` + `kumquat_virtio`
  - Under active development
  - Do not depend in `Cargo.toml` or package into a distro

### Development

- Github migration large complete
- Github CI/CD
- Initial stub Magma context type
- Support for vendored Mesa3D crates
- Improved and more accurate documentation
