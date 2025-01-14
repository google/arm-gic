# Arm Generic Interrupt Controller driver

[![crates.io page](https://img.shields.io/crates/v/arm-gic.svg)](https://crates.io/crates/arm-gic)
[![docs.rs page](https://docs.rs/arm-gic/badge.svg)](https://docs.rs/arm-gic)

This crate provides a Rust driver for the Arm Generic Interrupt Controller version 3 or 4 (GICv3 and
GICv4) as well as verison 2.

Because of large technical differences between the version 2 and version 3/4 Generic Interrupt Controllers
they have been separated in different modules. Use the one appropriate for your hardware. The interfaces are
largely compatible. Only differences when dispatching software-generated interrupts should be considered.
Look at the ARM-Manuals for further details.

Currently it only supports AArch64. Patches are welcome to add support for AArch32 and other GIC
versions.

This is not an officially supported Google product.

## License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

If you want to contribute to the project, see details of
[how we accept contributions](CONTRIBUTING.md).
