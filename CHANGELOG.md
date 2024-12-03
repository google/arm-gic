# Changelog

## Unreleased

### Improvements

- Added more interrupt types to `IntId`, and public constants for number of each type.

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
