# Changelog

## Unreleased

### Breaking changes

- `IntId` and `Trigger` moved to top-level module, as they are shared with GICv2 driver.
- Added support for multiple cores. `GicV3::new` now takes an array of redistributor base addresses,
  and various other method take a cpu index.

### Bugfixes

- Fixed `GicV3::setup` not to write to GICD IGROUPR[0].

### Improvements

- Added more interrupt types to `IntId`, and public constants for number of each type.
- Added constants to `IntId` for special interrupt IDs.
- Added methods to read type register and its fields.
- Added `set_group`, `redistributor_mark_core_awake` and other methods to `GicV3`.
- Added support for GICv2 in a separate `GicV2` driver.

## 0.1.2

### Bugfixes

- Changed `irouter` and `irouter_e` fields of `GICD` to use u64, to match GIC specification.

### Improvements

- Made `gicv3::registers` module public and added methods to `GicV3` to get pointers to registers.

## 0.1.1

### Improvements

- Implemented `Send` and `Sync` for `GicV3`.

## 0.1.0

Initial version, with basic support for GICv3 (and 4) on aarch64.
