// Copyright 2023 the authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

#![no_std]

pub mod gicv3;
mod sysreg;

use core::arch::asm;

/// Disables debug, SError, IRQ and FIQ exceptions.
pub fn irq_disable() {
    // Safe because writing to this system register doesn't access memory in any
    // way.
    unsafe {
        asm!("msr DAIFSet, #0xf", options(nomem, nostack));
    }
}

/// Enables debug, SError, IRQ and FIQ exceptions.
pub fn irq_enable() {
    // Safe because writing to this system register doesn't access memory in any
    // way.
    unsafe {
        asm!("msr DAIFClr, #0xf", options(nomem, nostack));
    }
}

/// Waits for an interrupt.
pub fn wfi() {
    // Safe because this doesn't access memory in any way.
    unsafe {
        asm!("wfi", options(nomem, nostack));
    }
}
