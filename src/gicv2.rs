// Copyright 2025 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

//! Driver for the Arm Generic Interrupt Controller version 2.

mod registers;

pub use self::registers::Typer;
use self::registers::{Gicc, Gicd, GicdCtlr};
use crate::{IntId, Trigger};

/// Driver for an Arm Generic Interrupt Controller version 2.
#[derive(Debug)]
pub struct GicV2 {
    gicd: *mut Gicd,
    gicc: *mut Gicc,
}

impl GicV2 {
    /// Constructs a new instance of the driver for a GIC with the given distributor and
    /// controller base addresses.
    ///
    /// # Safety
    ///
    /// The given base addresses must point to the GIC distributor and controller registers
    /// respectively. These regions must be mapped into the address space of the process as device
    /// memory, and not have any other aliases, either via another instance of this driver or
    /// otherwise.
    pub unsafe fn new(gicd: *mut u64, gicc: *mut u64) -> Self {
        Self {
            gicd: gicd as _,
            gicc: gicc as _,
        }
    }

    /// Returns information about what the GIC implementation supports.
    pub fn typer(&self) -> Typer {
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a GIC
        // distributor interface.
        unsafe { (&raw mut (*self.gicd).typer).read_volatile() }
    }

    /// Initialises the GIC.
    pub fn setup(&mut self) {
        // SAFETY: Both registers `self.gicd` and `self.gicc` are valid and unique pointers to
        // the hardware interfaces provided by the user.
        unsafe {
            (&raw mut (*self.gicd).ctlr).write_volatile(GicdCtlr::EnableGrp1);
            for i in 0..32 {
                (&raw mut (*self.gicd).igroupr[i]).write_volatile(0xffffffff);
            }

            (&raw mut (*self.gicc).ctlr).write_volatile(0b1);
            (&raw mut (*self.gicc).pmr).write_volatile(0xff);
        }
    }

    /// Enables or disables the interrupt with the given ID.
    pub fn enable_interrupt(&mut self, intid: IntId, enable: bool) -> Result<(), ()> {
        let index = (intid.0 / 32) as usize;
        let bit = 1 << (intid.0 % 32);

        if enable {
            // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
            // GIC distributor interface.
            unsafe {
                (&raw mut (*self.gicd).isenabler[index]).write_volatile(bit);
                if ((&raw const (*self.gicd).isenabler[index]).read_volatile() & bit) == 0 {
                    return Err(());
                }
            }
        } else {
            // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
            // GIC distributor interface.
            unsafe {
                (&raw mut (*self.gicd).icenabler[index]).write(bit);
            }
        }
        Ok(())
    }

    /// Enables all interrupts.
    pub fn enable_all_interrupts(&mut self, enable: bool) {
        for i in 0..32 {
            if enable {
                // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers
                // of a GIC distributor interface.
                unsafe {
                    (&raw mut (*self.gicd).isenabler[i]).write_volatile(0xffffffff);
                }
            } else {
                // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers
                // of a GIC distributor interface.
                unsafe {
                    (&raw mut (*self.gicd).icenabler[i]).write_volatile(0xffffffff);
                }
            }
        }
    }

    /// Sets the priority mask for the current CPU core.
    ///
    /// Only interrupts with a higher priority (numerically lower) will be signalled.
    pub fn set_priority_mask(&mut self, min_priority: u8) {
        // SAFETY: The existence of the PMR Register is guaranteed by the user.
        unsafe {
            (&raw mut (*self.gicc).pmr).write_volatile(min_priority as u32);
        }
    }

    /// Sets the priority of the interrupt with the given ID.
    ///
    /// Note that lower numbers correspond to higher priorities; i.e. 0 is the highest priority, and
    /// 255 is the lowest.
    pub fn set_interrupt_priority(&mut self, intid: IntId, priority: u8) {
        let idx = intid.0 as usize / 4;
        let priority = (priority as u32) << (8 * (intid.0 % 4));
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface.
        unsafe {
            (&raw mut (*self.gicd).ipriorityr[idx]).write_volatile(priority);
        }
    }

    /// Configures the trigger type for the interrupt with the given ID.
    pub fn set_trigger(&mut self, intid: IntId, trigger: Trigger) {
        let index = (intid.0 / 16) as usize;
        let bit = 1 << (((intid.0 % 16) * 2) + 1);

        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface.
        // Affinity routing is not available. So instead use the icfgr register present on all
        // GICD interfaces (present as guaranteed by the user) to set trigger modes.
        unsafe {
            let register = (&raw mut (*self.gicd).icfgr[index]);
            let v = register.read_volatile();
            register.write_volatile(match trigger {
                Trigger::Edge => v | bit,
                Trigger::Level => v & !bit,
            });
        }
    }

    /// Sends a software-generated interrupt (SGI) to the given cores.
    pub fn send_sgi(&mut self, intid: IntId, target: SgiTarget) {
        assert!(intid.is_sgi());

        let sgi_value = match target {
            SgiTarget::All => (intid.0 & 0x0f) | (0xff << 16),
            SgiTarget::List {
                target_list_filter,
                target_list,
            } => {
                (intid.0 & 0xf)
                    | match target_list_filter {
                        SgiTargetListFilter::CPUTargetList => 0b00,
                        SgiTargetListFilter::ForwardOthersOnly => 0b01,
                        SgiTargetListFilter::ForwardSelfOnly => 0b10,
                    } << 24
                    | u32::from(target_list & 0xff) << 16
                    | 1u32 << 15
            }
        };

        // SAFETY: As guaranteed by the user, the gicd is a valid pointer to a GIC distributor
        // which always contains the sgir register.
        unsafe {
            (&raw mut (*self.gicd).sgir).write_volatile(sgi_value);
        }
    }

    /// Gets the ID of the highest priority signalled interrupt, and acknowledges it.
    ///
    /// Returns `None` if there is no ptending interrupt of sufficient priority.
    pub fn get_and_acknowledge_interrupt(&mut self) -> Option<IntId> {
        let intid = IntId(
            // SAFETY: This memory access is guaranteed by the user passing along a valid GICD address.
            unsafe { (&raw mut (*self.gicc).aiar).read_volatile() },
        );
        if intid == IntId::SPECIAL_NONE {
            None
        } else {
            Some(intid)
        }
    }

    /// Informs the interrupt controller that the CPU has completed processing the given interrupt.
    /// This drops the interrupt priority and deactivates the interrupt.
    pub fn end_interrupt(&mut self, intid: IntId) {
        // SAFETY: The gicc is a valid pointer as guaranteed by the user. The aeoir register is always present.
        unsafe {
            (&raw mut (*self.gicc).aeoir).write_volatile(intid.0);
        }
    }
}

/// The target specification for a software-generated interrupt.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SgiTarget {
    /// The SGI is routed to all CPU cores except the current one.
    All,
    /// The SGI is routed to the CPU cores matching the given affinities and list.
    List {
        target_list_filter: SgiTargetListFilter,
        target_list: u16,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SgiTargetListFilter {
    CPUTargetList,
    ForwardOthersOnly,
    ForwardSelfOnly,
}
