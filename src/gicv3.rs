// Copyright 2023 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

//! Driver for the Arm Generic Interrupt Controller version 3 (or 4).

mod registers;

use self::registers::{GicdCtlr, Waker, GICD, GICR, SGI};
use crate::sysreg::{read_sysreg, write_sysreg};
use core::{
    fmt::{self, Debug, Formatter},
    hint::spin_loop,
    mem::size_of,
    ptr::{addr_of, addr_of_mut},
};

/// The offset in bytes from `RD_base` to `SGI_base`.
const SGI_OFFSET: usize = 0x10000;

/// An interrupt ID.
#[derive(Copy, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub struct IntId(u32);

impl IntId {
    /// The ID of the first Software Generated Interrupt.
    const SGI_START: u32 = 0;

    /// The ID of the first Private Peripheral Interrupt.
    const PPI_START: u32 = 16;

    /// The ID of the first Shared Peripheral Interrupt.
    const SPI_START: u32 = 32;

    /// The first special interrupt ID.
    const SPECIAL_START: u32 = 1020;

    /// Returns the interrupt ID for the given Software Generated Interrupt.
    pub const fn sgi(sgi: u32) -> Self {
        assert!(sgi < Self::PPI_START);
        Self(Self::SGI_START + sgi)
    }

    /// Returns the interrupt ID for the given Private Peripheral Interrupt.
    pub const fn ppi(ppi: u32) -> Self {
        assert!(ppi < Self::SPI_START - Self::PPI_START);
        Self(Self::PPI_START + ppi)
    }

    /// Returns the interrupt ID for the given Shared Peripheral Interrupt.
    pub const fn spi(spi: u32) -> Self {
        assert!(spi < Self::SPECIAL_START);
        Self(Self::SPI_START + spi)
    }

    /// Returns whether this interrupt ID is for a Software Generated Interrupt.
    fn is_sgi(self) -> bool {
        self.0 < Self::PPI_START
    }

    /// Returns whether this interrupt ID is private to a core, i.e. it is an SGI or PPI.
    fn is_private(self) -> bool {
        self.0 < Self::SPI_START
    }
}

impl Debug for IntId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        if self.0 < Self::PPI_START {
            write!(f, "SGI {}", self.0 - Self::SGI_START)
        } else if self.0 < Self::SPI_START {
            write!(f, "PPI {}", self.0 - Self::PPI_START)
        } else if self.0 < Self::SPECIAL_START {
            write!(f, "SPI {}", self.0 - Self::SPI_START)
        } else {
            write!(f, "Special IntId {}", self.0)
        }
    }
}

impl From<IntId> for u32 {
    fn from(intid: IntId) -> Self {
        intid.0
    }
}

/// Driver for an Arm Generic Interrupt Controller version 3 (or 4).
#[derive(Debug)]
pub struct GicV3 {
    gicd: *mut GICD,
    gicr: *mut GICR,
    sgi: *mut SGI,
}

impl GicV3 {
    /// Constructs a new instance of the driver for a GIC with the given distributor and
    /// redistributor base addresses.
    ///
    /// # Safety
    ///
    /// The given base addresses must point to the GIC distributor and redistributor registers
    /// respectively. These regions must be mapped into the address space of the process as device
    /// memory, and not have any other aliases, either via another instance of this driver or
    /// otherwise.
    pub unsafe fn new(gicd: *mut u64, gicr: *mut u64) -> Self {
        Self {
            gicd: gicd as _,
            gicr: gicr as _,
            sgi: gicr.wrapping_add(SGI_OFFSET / size_of::<u64>()) as _,
        }
    }

    /// Initialises the GIC.
    pub fn setup(&mut self) {
        // Safe because writing to this system register doesn't access memory in any way.
        unsafe {
            // Enable system register access.
            write_sysreg!(icc_sre_el1, 0x01);
        }

        // Safe because we know that `self.gicr` is a valid and unique pointer to the registers of a
        // GIC redistributor interface.
        unsafe {
            // Mark this CPU core as awake, and wait until the GIC wakes up before continuing.
            let mut waker = addr_of!((*self.gicr).waker).read_volatile();
            waker -= Waker::PROCESSOR_SLEEP;
            addr_of_mut!((*self.gicr).waker).write_volatile(waker);

            while addr_of!((*self.gicr).waker)
                .read_volatile()
                .contains(Waker::CHILDREN_ASLEEP)
            {
                spin_loop();
            }
        }

        // Safe because writing to this system register doesn't access memory in any way.
        unsafe {
            // Disable use of `ICC_PMR_EL1` as a hint for interrupt distribution, configure a write
            // to an EOI register to also deactivate the interrupt, and configure preemption groups
            // for group 0 and group 1 interrupts separately.
            write_sysreg!(icc_ctlr_el1, 0);
        }

        // Safe because we know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface.
        unsafe {
            // Enable affinity routing and non-secure group 1 interrupts.
            addr_of_mut!((*self.gicd).ctlr)
                .write_volatile(GicdCtlr::ARE_S | GicdCtlr::EnableGrp1NS);
        }

        // Safe because we know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            // Put all SGIs and PPIs into non-secure group 1.
            addr_of_mut!((*self.sgi).igroupr0).write_volatile(0xffffffff);
            // Put all SPIs into non-secure group 1.
            for i in 0..32 {
                addr_of_mut!((*self.gicd).igroupr[i]).write_volatile(0xffffffff);
            }
        }

        // Safe because writing to this system register doesn't access memory in any way.
        unsafe {
            // Enable non-secure group 1.
            write_sysreg!(icc_igrpen1_el1, 0x00000001);
        }
    }

    /// Enables or disables the interrupt with the given ID.
    pub fn enable_interrupt(&mut self, intid: IntId, enable: bool) {
        let index = (intid.0 / 32) as usize;
        let bit = 1 << (intid.0 % 32);

        // Safe because we know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            if enable {
                addr_of_mut!((*self.gicd).isenabler[index]).write_volatile(bit);
                if intid.is_private() {
                    addr_of_mut!((*self.sgi).isenabler0).write_volatile(bit);
                }
            } else {
                addr_of_mut!((*self.gicd).icenabler[index]).write_volatile(bit);
                if intid.is_private() {
                    addr_of_mut!((*self.sgi).icenabler0).write_volatile(bit);
                }
            }
        }
    }

    /// Enables all interrupts.
    pub fn enable_all_interrupts(&mut self, enable: bool) {
        for i in 0..32 {
            // Safe because we know that `self.gicd` is a valid and unique pointer to the registers
            // of a GIC distributor interface.
            unsafe {
                if enable {
                    addr_of_mut!((*self.gicd).isenabler[i]).write_volatile(0xffffffff);
                } else {
                    addr_of_mut!((*self.gicd).icenabler[i]).write_volatile(0xffffffff);
                }
            }
        }
        // Safe because we know that `self.sgi` is a valid and unique pointer to the SGI and PPI
        // registers of a GIC redistributor interface.
        unsafe {
            if enable {
                addr_of_mut!((*self.sgi).isenabler0).write_volatile(0xffffffff);
            } else {
                addr_of_mut!((*self.sgi).icenabler0).write_volatile(0xffffffff);
            }
        }
    }

    /// Sets the priority mask for the current CPU core.
    ///
    /// Only interrupts with a higher priority (numerically lower) will be signalled.
    pub fn set_priority_mask(min_priority: u8) {
        // Safe because writing to this system register doesn't access memory in any way.
        unsafe {
            write_sysreg!(icc_pmr_el1, min_priority.into());
        }
    }

    /// Sets the priority of the interrupt with the given ID.
    ///
    /// Note that lower numbers correspond to higher priorities; i.e. 0 is the highest priority, and
    /// 255 is the lowest.
    pub fn set_interrupt_priority(&mut self, intid: IntId, priority: u8) {
        // Safe because we know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            // Affinity routing is enabled, so use the GICR for SGIs and PPIs.
            if intid.is_private() {
                addr_of_mut!((*self.sgi).ipriorityr[intid.0 as usize]).write_volatile(priority);
            } else {
                addr_of_mut!((*self.gicd).ipriorityr[intid.0 as usize]).write_volatile(priority);
            }
        }
    }

    /// Configures the trigger type for the interrupt with the given ID.
    pub fn set_trigger(&mut self, intid: IntId, trigger: Trigger) {
        let index = (intid.0 / 16) as usize;
        let bit = 1 << (((intid.0 % 16) * 2) + 1);

        // Safe because we know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            // Affinity routing is enabled, so use the GICR for SGIs and PPIs.
            let register = if intid.is_private() {
                addr_of_mut!((*self.sgi).icfgr[index])
            } else {
                addr_of_mut!((*self.gicd).icfgr[index])
            };
            let v = register.read_volatile();
            register.write_volatile(match trigger {
                Trigger::Edge => v | bit,
                Trigger::Level => v & !bit,
            });
        }
    }

    /// Sends a software-generated interrupt (SGI) to the given cores.
    pub fn send_sgi(intid: IntId, target: SgiTarget) {
        assert!(intid.is_sgi());

        let sgi_value = match target {
            SgiTarget::All => {
                let irm = 0b1;
                (u64::from(intid.0 & 0x0f) << 24) | (irm << 40)
            }
            SgiTarget::List {
                affinity3,
                affinity2,
                affinity1,
                target_list,
            } => {
                let irm = 0b0;
                u64::from(target_list)
                    | (u64::from(affinity1) << 16)
                    | (u64::from(intid.0 & 0x0f) << 24)
                    | (u64::from(affinity2) << 32)
                    | (irm << 40)
                    | (u64::from(affinity3) << 48)
            }
        };

        // Safe because writing to this system register doesn't access memory in any way.
        unsafe {
            write_sysreg!(icc_sgi1r_el1, sgi_value);
        }
    }

    /// Gets the ID of the highest priority signalled interrupt, and acknowledges it.
    ///
    /// Returns `None` if there is no pending interrupt of sufficient priority.
    pub fn get_and_acknowledge_interrupt() -> Option<IntId> {
        // Safe because reading this system register doesn't access memory in any way.
        let intid = unsafe { read_sysreg!(icc_iar1_el1) } as u32;
        if intid == IntId::SPECIAL_START {
            None
        } else {
            Some(IntId(intid))
        }
    }

    /// Informs the interrupt controller that the CPU has completed processing the given interrupt.
    /// This drops the interrupt priority and deactivates the interrupt.
    pub fn end_interrupt(intid: IntId) {
        // Safe because writing to this system register doesn't access memory in any way.
        unsafe { write_sysreg!(icc_eoir1_el1, intid.0.into()) }
    }
}

/// The trigger configuration for an interrupt.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Trigger {
    /// The interrupt is edge triggered.
    Edge,
    /// The interrupt is level triggered.
    Level,
}

/// The target specification for a software-generated interrupt.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SgiTarget {
    /// The SGI is routed to all CPU cores except the current one.
    All,
    /// The SGI is routed to the CPU cores matching the given affinities and list.
    List {
        affinity3: u8,
        affinity2: u8,
        affinity1: u8,
        target_list: u16,
    },
}
