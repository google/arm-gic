// Copyright 2023 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

#[cfg(any(test, feature = "fakes"))]
#[macro_use]
pub mod fake;

#[cfg(all(not(any(test, feature = "fakes")), target_arch = "aarch64"))]
#[macro_use]
mod aarch64;

#[cfg(all(not(any(test, feature = "fakes")), target_arch = "arm"))]
#[macro_use]
mod aarch32;

use bitflags::bitflags;

read_sysreg32!(icc_hppir0_el1, 0, c12, c8, 2, read_icc_hppir0_el1);
read_sysreg32!(icc_hppir1_el1, 0, c12, c12, 2, read_icc_hppir1_el1);
read_sysreg32!(icc_iar0_el1, 0, c12, c8, 0, read_icc_iar0_el1);
read_sysreg32!(icc_iar1_el1, 0, c12, c12, 0, read_icc_iar1_el1);

write_sysreg32!(icc_ctlr_el1, 0, c12, c12, 4, write_icc_ctlr_el1);
write_sysreg32!(icc_eoir0_el1, 0, c12, c8, 1, write_icc_eoir0_el1);
write_sysreg32!(icc_eoir1_el1, 0, c12, c12, 1, write_icc_eoir1_el1);
write_sysreg32!(icc_igrpen0_el1, 0, c12, c12, 6, write_icc_igrpen0_el1);
write_sysreg32!(icc_igrpen1_el1, 0, c12, c12, 7, write_icc_igrpen1_el1);
write_sysreg32!(icc_igrpen1_el3, 6, c12, c12, 7, write_icc_igrpen1_el3);
write_sysreg32!(icc_pmr_el1, 0, c4, c6, 0, write_icc_pmr_el1);
write_sysreg32!(icc_sre_el1, 0, c12, c12, 5, write_icc_sre_el1, IccSre);
write_sysreg64!(icc_asgi1r_el1, 0, c12, write_icc_asgi1r_el1);
write_sysreg64!(icc_sgi0r_el1, 0, c12, write_icc_sgi0r_el1);
write_sysreg64!(icc_sgi1r_el1, 0, c12, write_icc_sgi1r_el1);

bitflags! {
    /// Type for the `icc_sre_el2` and `icc_sre_el3` registers.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct IccSre: u32 {
        /// System register enable.
        ///
        /// Enables access to the GIC CPU interface system registers.
        const SRE = 1 << 0;
        /// Disable FIQ bypass.
        const DFB = 1 << 1;
        /// Disable IRQ bypass.
        const DIB = 1 << 2;
        // TODO: Should this be on a different type? Not all registers have it.
        /// Enables lower EL access to ICC_SRE_ELn.
        const ENABLE = 1 << 3;
    }
}
