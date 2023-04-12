// Copyright 2023 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

//! Driver for the Arm Generic Interrupt Controller version 3 or 4, on aarch64.
//!
//! This top level module contains functions that are not specific to any particular interrupt
//! controller, as support for other GIC versions may be added in future.
//!
//! # Example
//!
//! ```
//! use arm_gic::{
//!     gicv3::{GicV3, IntId, SgiTarget},
//!     irq_enable,
//! };
//!
//! // Base addresses of the GICv3 distributor and redistributor.
//! const GICD_BASE_ADDRESS: *mut u64 = 0x800_0000 as _;
//! const GICR_BASE_ADDRESS: *mut u64 = 0x80A_0000 as _;
//!
//! // Initialise the GIC.
//! let mut gic = unsafe { GicV3::new(GICD_BASE_ADDRESS, GICR_BASE_ADDRESS) };
//! gic.setup();
//!
//! // Configure an SGI and then send it to ourself.
//! let sgi_intid = IntId::sgi(3);
//! GicV3::set_priority_mask(0xff);
//! gic.set_interrupt_priority(sgi_intid, 0x80);
//! gic.enable_interrupt(sgi_intid, true);
//! irq_enable();
//! GicV3::send_sgi(
//!     sgi_intid,
//!     SgiTarget::List {
//!         affinity3: 0,
//!         affinity2: 0,
//!         affinity1: 0,
//!         target_list: 0b1,
//!     },
//! );
//! ```

#![no_std]

pub mod gicv3;
mod sysreg;

use core::arch::asm;

/// Disables debug, SError, IRQ and FIQ exceptions.
pub fn irq_disable() {
    // Safe because writing to this system register doesn't access memory in any way.
    unsafe {
        asm!("msr DAIFSet, #0xf", options(nomem, nostack));
    }
}

/// Enables debug, SError, IRQ and FIQ exceptions.
pub fn irq_enable() {
    // Safe because writing to this system register doesn't access memory in any way.
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
