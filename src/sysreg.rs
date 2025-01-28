// Copyright 2023 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

#[cfg(test)]
#[macro_use]
pub mod fake;
#[cfg(all(not(test), target_arch = "aarch64"))]
#[macro_use]
mod aarch64;

read_sysreg!(icc_iar1_el1, read_icc_iar1_el1);

write_sysreg!(icc_ctlr_el1, write_icc_ctlr_el1);
write_sysreg!(icc_eoir1_el1, write_icc_eoir1_el1);
write_sysreg!(icc_igrpen0_el1, write_icc_igrpen0_el1);
write_sysreg!(icc_igrpen1_el1, write_icc_igrpen1_el1);
write_sysreg!(icc_pmr_el1, write_icc_pmr_el1);
write_sysreg!(icc_sgi1r_el1, write_icc_sgi1r_el1);
write_sysreg!(icc_sre_el1, write_icc_sre_el1);
