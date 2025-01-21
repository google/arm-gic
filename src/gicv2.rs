// Copyright 2024 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

//! Driver for the Arm Generic Interrupt Controller version 2.

mod registers;

use self::registers::{GicdCtlr, GICC, GICD};
use core::{
    fmt::{self, Debug, Formatter},
    ptr::{addr_of, addr_of_mut},
};

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

/// Driver for an Arm Generic Interrupt Controller version 2.
#[derive(Debug)]
pub struct GicV2 {
    gicd: *mut GICD,
    gicc: *mut GICC,
}

impl GicV2 {
    /// Constructs a new instance of the driver for a GIC with the given distributor and
    /// controller base addresses.
    ///
    /// # Safety
    ///
    /// The given base addresses must point to the GIC distributor and redistributor registers
    /// respectively. These regions must be mapped into the address space of the process as device
    /// memory, and not have any other aliases, either via another instance of this driver or
    /// otherwise.
    pub unsafe fn new(gicd: *mut u64, gicc: *mut u64) -> Self {
        Self {
            gicd: gicd as _,
            gicc: gicc as _,
        }
    }

    fn max_irqs(&self) -> u32 {
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface.
        unsafe { ((addr_of!((*self.gicd).typer).read_volatile() as u32 & 0b11111) + 1) * 32 }
    }

    /// Initialises the GIC.
    pub fn setup(&mut self) {
        unsafe {
            addr_of_mut!((*self.gicd).ctlr).write_volatile(GicdCtlr::EnableGrp1);
            for i in 0..32 {
                addr_of_mut!((*self.gicd).igroupr[i]).write_volatile(0xffffffff);
            }

            addr_of_mut!((*self.gicc).ctlr).write_volatile(0b1);
            addr_of_mut!((*self.gicc).pmr).write_volatile(0xff);
        }
    }

    /// Enables or disables the interrupt with the given ID.
    pub fn enable_interrupt(&mut self, intid: IntId, enable: bool) {
        let index = (intid.0 / 32) as usize;
        let bit = 1 << (intid.0 % 32);

        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface.
        if enable {
            unsafe {
                addr_of_mut!((*self.gicd).isenabler[index]).write_volatile(bit);
                if (addr_of!((*self.gicd).isenabler[index]).read_volatile() & bit) == 0 {
                    panic!("Couldn't enable interrupt {}", intid.0);
                }
            }
        } else {
            unsafe {
                addr_of_mut!((*self.gicd).icenabler[index]).write(bit);
            }
        }
    }

    /// Enables all interrupts.
    pub fn enable_all_interrupts(&mut self, enable: bool) {
        for i in 0..32 {
            // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers
            // of a GIC distributor interface.
            if enable {
                unsafe {
                    addr_of_mut!((*self.gicd).isenabler[i]).write_volatile(0xffffffff);
                }
            } else {
                unsafe {
                    addr_of_mut!((*self.gicd).icenabler[i]).write_volatile(0xffffffff);
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
            addr_of_mut!((*self.gicc).pmr).write_volatile(min_priority as u32);
        }
    }

    /// Sets the priority of the interrupt with the given ID.
    ///
    /// Note that lower numbers correspond to higher priorities; i.e. 0 is the highest priority, and
    /// 255 is the lowest.
    pub fn set_interrupt_priority(&mut self, intid: IntId, priority: u8) {
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface.
        let idx = intid.0 as usize / 4;
        let priority = (priority as u32) << (8 * (intid.0 % 4));
        unsafe {
            addr_of_mut!((*self.gicd).ipriorityr[idx]).write_volatile(priority);
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
            let register = addr_of_mut!((*self.gicd).icfgr[index]);
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
            SgiTarget::All => (u32::from(intid.0 & 0x0f)) | (0xff << 16),
            SgiTarget::List {
                target_list_filter,
                target_list,
            } => {
                u32::from(intid.0 & 0xf)
                    | u32::from(
                        match target_list_filter {
                            SgiTargetListFilter::CPUTargetList => 0b00 as u32,
                            SgiTargetListFilter::ForwardOthersOnly => 0b01 as u32,
                            SgiTargetListFilter::ForwardSelfOnly => 0b10 as u32,
                            _ => panic!("Invalid target_list_filter passed to SGI generation."),
                        } << 24,
                    )
                    | u32::from(((target_list & 0xff) as u32) << 16)
                    | (1u32 << 15)
            }
        };

        // SAFETY: As guaranteed by the user, the gicd is a valid pointer to a GIC distributor
        // which always contains the sgir register.
        unsafe {
            addr_of_mut!((*self.gicd).sgir).write_volatile(sgi_value);
        }
    }

    /// Gets the ID of the highest priority signalled interrupt, and acknowledges it.
    ///
    /// Returns `None` if there is no ptending interrupt of sufficient priority.
    pub fn get_and_acknowledge_interrupt(&mut self) -> Option<IntId> {
        // SAFETY: This memory access is guaranteed by the user passing along a valid GICD address.
        unsafe {
            let intid = addr_of_mut!((*self.gicc).aiar).read_volatile() as u32;

            if intid == IntId::SPECIAL_START {
                None
            } else {
                Some(IntId(intid))
            }
        }
    }

    /// Informs the interrupt controller that the CPU has completed processing the given interrupt.
    /// This drops the interrupt priority and deactivates the interrupt.
    pub fn end_interrupt(&mut self, intid: IntId) {
        // SAFETY: The gicc is a valid pointer as guaranteed by the user. The aeoir register is always present.
        unsafe {
            addr_of_mut!((*self.gicc).aeoir).write_volatile(intid.0);
        }
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
        target_list_filter: SgiTargetListFilter,
        target_list: u16,
    },
}
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SgiTargetListFilter {
    CPUTargetList,
    ForwardOthersOnly,
    ForwardSelfOnly,
    Reserved,
}
