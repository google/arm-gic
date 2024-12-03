// Copyright 2023 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use core::arch::asm;

/// Generates a safe public function named `$function_name` to read the system register `$sysreg`.
///
/// This should only be used for system registers which are indeed safe to read.
macro_rules! read_sysreg {
    ($sysreg:ident, $function_name:ident) => {
        pub fn $function_name() -> u64 {
            let value;
            unsafe {
                asm!(
                    concat!("mrs {value}, ", stringify!($sysreg)),
                    options(nostack),
                    value = out(reg) value,
                );
            }
            value
        }
    };
}

/// Generates a safe public function named `$function_name` to write to the system register
/// `$sysreg`.
///
/// This should only be used for system registers which are indeed safe to write.
macro_rules! write_sysreg {
    ($sysreg:ident, $function_name:ident) => {
        pub fn $function_name(value: u64) {
            unsafe {
                asm!(
                    concat!("msr ", stringify!($sysreg), ", {value}"),
                    options(nostack),
                    value = in(reg) value,
                );
            }
        }
    };
}

read_sysreg!(icc_iar1_el1, read_icc_iar1_el1);

write_sysreg!(icc_ctlr_el1, write_icc_ctlr_el1);
write_sysreg!(icc_eoir1_el1, write_icc_eoir1_el1);
write_sysreg!(icc_igrpen1_el1, write_icc_igrpen1_el1);
write_sysreg!(icc_pmr_el1, write_icc_pmr_el1);
write_sysreg!(icc_sgi1r_el1, write_icc_sgi1r_el1);
write_sysreg!(icc_sre_el1, write_icc_sre_el1);
