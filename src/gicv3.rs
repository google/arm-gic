// Copyright 2023 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

//! Driver for the Arm Generic Interrupt Controller version 3 (or 4).

pub mod registers;

use self::registers::{GicdCtlr, Waker, GICD, GICR, SGI};
use crate::sysreg::{
    read_icc_iar1_el1, write_icc_ctlr_el1, write_icc_eoir1_el1, write_icc_igrpen1_el1,
    write_icc_pmr_el1, write_icc_sgi1r_el1, write_icc_sre_el1,
};
use core::{
    fmt::{self, Debug, Formatter},
    hint::spin_loop,
    mem::size_of,
};

/// The offset in bytes from `RD_base` to `SGI_base`.
const SGI_OFFSET: usize = 0x10000;

/// An interrupt ID.
#[derive(Copy, Clone, Eq, Ord, PartialOrd, PartialEq)]
pub struct IntId(u32);

impl IntId {
    /// Special interrupt ID returned when running at EL3 and the interrupt should be handled at
    /// S-EL2 or S-EL1.
    pub const SPECIAL_SECURE: Self = Self(1020);

    /// Special interrupt ID returned when running at EL3 and the interrupt should be handled at
    /// (non-secure) EL2 or EL1.
    pub const SPECIAL_NONSECURE: Self = Self(1021);

    /// Special interrupt ID returned when the interrupt is a non-maskable interrupt.
    pub const SPECIAL_NMI: Self = Self(1022);

    /// Special interrupt ID returned when there is no pending interrupt of sufficient priority for
    /// the current security state and interrupt group.
    pub const SPECIAL_NONE: Self = Self(1023);

    /// The maximum number of SPIs which may be supported.
    pub const MAX_SPI_COUNT: u32 = Self::SPECIAL_START - Self::SPI_START;

    /// The number of Software Generated Interrupts.
    pub const SGI_COUNT: u32 = Self::PPI_START - Self::SGI_START;

    /// The number of (non-extended) Private Peripheral Interrupts.
    pub const PPI_COUNT: u32 = Self::SPI_START - Self::PPI_START;

    /// The maximum number of extended Private Peripheral Interrupts which may be supported.
    pub const MAX_EPPI_COUNT: u32 = Self::EPPI_END - Self::EPPI_START;

    /// The maximum number of extended Shared Peripheral Interrupts which may be supported.
    pub const MAX_ESPI_COUNT: u32 = Self::ESPI_END - Self::ESPI_START;

    /// The ID of the first Software Generated Interrupt.
    const SGI_START: u32 = 0;

    /// The ID of the first Private Peripheral Interrupt.
    const PPI_START: u32 = 16;

    /// The ID of the first Shared Peripheral Interrupt.
    const SPI_START: u32 = 32;

    /// The first special interrupt ID.
    const SPECIAL_START: u32 = 1020;

    /// One more than the last special interrupt ID.
    const SPECIAL_END: u32 = 1024;

    /// The first extended Private Peripheral Interrupt.
    const EPPI_START: u32 = 1056;

    /// One more than the last extended Private Peripheral Interrupt.
    const EPPI_END: u32 = 1120;

    /// The first extended Shared Peripheral Interrupt.
    const ESPI_START: u32 = 4096;

    /// One more than the last extended Shared Peripheral Interrupt.
    const ESPI_END: u32 = 5120;

    /// The first Locality-specific Peripheral Interrupt.
    const LPI_START: u32 = 8192;

    /// Returns the interrupt ID for the given Software Generated Interrupt.
    pub const fn sgi(sgi: u32) -> Self {
        assert!(sgi < Self::SGI_COUNT);
        Self(Self::SGI_START + sgi)
    }

    /// Returns the interrupt ID for the given Private Peripheral Interrupt.
    pub const fn ppi(ppi: u32) -> Self {
        assert!(ppi < Self::PPI_COUNT);
        Self(Self::PPI_START + ppi)
    }

    /// Returns the interrupt ID for the given Shared Peripheral Interrupt.
    pub const fn spi(spi: u32) -> Self {
        assert!(spi < Self::MAX_SPI_COUNT);
        Self(Self::SPI_START + spi)
    }

    /// Returns the interrupt ID for the given extended Private Peripheral Interrupt.
    pub const fn eppi(eppi: u32) -> Self {
        assert!(eppi < Self::MAX_EPPI_COUNT);
        Self(Self::EPPI_START + eppi)
    }

    /// Returns the interrupt ID for the given extended Shared Peripheral Interrupt.
    pub const fn espi(espi: u32) -> Self {
        assert!(espi < Self::MAX_ESPI_COUNT);
        Self(Self::ESPI_START + espi)
    }

    /// Returns the interrupt ID for the given Locality-specific Peripheral Interrupt.
    pub const fn lpi(lpi: u32) -> Self {
        Self(Self::LPI_START + lpi)
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
        } else if self.0 < Self::SPECIAL_END {
            write!(f, "Special IntId {}", self.0)
        } else if self.0 < Self::EPPI_START {
            write!(f, "Reserved IntId {}", self.0)
        } else if self.0 < Self::EPPI_END {
            write!(f, "EPPI {}", self.0 - Self::EPPI_START)
        } else if self.0 < Self::ESPI_START {
            write!(f, "Reserved IntId {}", self.0)
        } else if self.0 < Self::ESPI_END {
            write!(f, "ESPI {}", self.0 - Self::ESPI_START)
        } else if self.0 < Self::LPI_START {
            write!(f, "Reserved IntId {}", self.0)
        } else {
            write!(f, "LPI {}", self.0 - Self::LPI_START)
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
        // Enable system register access.
        write_icc_sre_el1(0x01);

        // SAFETY: We know that `self.gicr` is a valid and unique pointer to the registers of a
        // GIC redistributor interface.
        unsafe {
            // Mark this CPU core as awake, and wait until the GIC wakes up before continuing.
            let mut waker = (&raw const (*self.gicr).waker).read_volatile();
            waker -= Waker::PROCESSOR_SLEEP;
            (&raw mut (*self.gicr).waker).write_volatile(waker);

            while (&raw const (*self.gicr).waker)
                .read_volatile()
                .contains(Waker::CHILDREN_ASLEEP)
            {
                spin_loop();
            }
        }

        // Disable use of `ICC_PMR_EL1` as a hint for interrupt distribution, configure a write to
        // an EOI register to also deactivate the interrupt, and configure preemption groups for
        // group 0 and group 1 interrupts separately.
        write_icc_ctlr_el1(0);

        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface.
        unsafe {
            // Enable affinity routing and non-secure group 1 interrupts.
            (&raw mut (*self.gicd).ctlr).write_volatile(GicdCtlr::ARE_S | GicdCtlr::EnableGrp1NS);
        }

        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            // Put all SGIs and PPIs into non-secure group 1.
            (&raw mut (*self.sgi).igroupr0).write_volatile(0xffffffff);
            // Put all SPIs into non-secure group 1.
            for i in 1..32 {
                (&raw mut (*self.gicd).igroupr[i]).write_volatile(0xffffffff);
            }
        }

        // Enable non-secure group 1.
        write_icc_igrpen1_el1(0x00000001);
    }

    /// Enables or disables the interrupt with the given ID.
    pub fn enable_interrupt(&mut self, intid: IntId, enable: bool) {
        let index = (intid.0 / 32) as usize;
        let bit = 1 << (intid.0 % 32);

        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            if enable {
                (&raw mut (*self.gicd).isenabler[index]).write_volatile(bit);
                if intid.is_private() {
                    (&raw mut (*self.sgi).isenabler0).write_volatile(bit);
                }
            } else {
                (&raw mut (*self.gicd).icenabler[index]).write_volatile(bit);
                if intid.is_private() {
                    (&raw mut (*self.sgi).icenabler0).write_volatile(bit);
                }
            }
        }
    }

    /// Enables all interrupts.
    pub fn enable_all_interrupts(&mut self, enable: bool) {
        for i in 0..32 {
            // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers
            // of a GIC distributor interface.
            unsafe {
                if enable {
                    (&raw mut (*self.gicd).isenabler[i]).write_volatile(0xffffffff);
                } else {
                    (&raw mut (*self.gicd).icenabler[i]).write_volatile(0xffffffff);
                }
            }
        }
        // SAFETY: We know that `self.sgi` is a valid and unique pointer to the SGI and PPI
        // registers of a GIC redistributor interface.
        unsafe {
            if enable {
                (&raw mut (*self.sgi).isenabler0).write_volatile(0xffffffff);
            } else {
                (&raw mut (*self.sgi).icenabler0).write_volatile(0xffffffff);
            }
        }
    }

    /// Sets the priority mask for the current CPU core.
    ///
    /// Only interrupts with a higher priority (numerically lower) will be signalled.
    pub fn set_priority_mask(min_priority: u8) {
        write_icc_pmr_el1(min_priority.into());
    }

    /// Sets the priority of the interrupt with the given ID.
    ///
    /// Note that lower numbers correspond to higher priorities; i.e. 0 is the highest priority, and
    /// 255 is the lowest.
    pub fn set_interrupt_priority(&mut self, intid: IntId, priority: u8) {
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            // Affinity routing is enabled, so use the GICR for SGIs and PPIs.
            if intid.is_private() {
                (&raw mut (*self.sgi).ipriorityr[intid.0 as usize]).write_volatile(priority);
            } else {
                (&raw mut (*self.gicd).ipriorityr[intid.0 as usize]).write_volatile(priority);
            }
        }
    }

    /// Configures the trigger type for the interrupt with the given ID.
    pub fn set_trigger(&mut self, intid: IntId, trigger: Trigger) {
        let index = (intid.0 / 16) as usize;
        let bit = 1 << (((intid.0 % 16) * 2) + 1);

        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            // Affinity routing is enabled, so use the GICR for SGIs and PPIs.
            let register = if intid.is_private() {
                (&raw mut (*self.sgi).icfgr[index])
            } else {
                (&raw mut (*self.gicd).icfgr[index])
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

        write_icc_sgi1r_el1(sgi_value);
    }

    /// Gets the ID of the highest priority signalled interrupt, and acknowledges it.
    ///
    /// Returns `None` if there is no pending interrupt of sufficient priority.
    pub fn get_and_acknowledge_interrupt() -> Option<IntId> {
        let intid = read_icc_iar1_el1() as u32;
        if intid == IntId::SPECIAL_START {
            None
        } else {
            Some(IntId(intid))
        }
    }

    /// Informs the interrupt controller that the CPU has completed processing the given interrupt.
    /// This drops the interrupt priority and deactivates the interrupt.
    pub fn end_interrupt(intid: IntId) {
        write_icc_eoir1_el1(intid.0.into())
    }

    /// Returns a raw pointer to the GIC distributor registers.
    ///
    /// This may be used to read and write the registers directly for functionality not yet
    /// supported by this driver.
    pub fn gicd_ptr(&mut self) -> *mut GICD {
        self.gicd
    }

    /// Returns a raw pointer to the GIC redistributor registers.
    ///
    /// This may be used to read and write the registers directly for functionality not yet
    /// supported by this driver.
    pub fn gicr_ptr(&mut self) -> *mut GICR {
        self.gicr
    }

    /// Returns a raw pointer to the GIC redistributor SGI and PPI registers.
    ///
    /// This may be used to read and write the registers directly for functionality not yet
    /// supported by this driver.
    pub fn sgi_ptr(&mut self) -> *mut SGI {
        self.sgi
    }
}

// SAFETY: The GIC interface can be accessed from any CPU core.
unsafe impl Send for GicV3 {}

// SAFETY: Any operations which change state require `&mut GicV3`, so `&GicV3` is fine to share.
unsafe impl Sync for GicV3 {}

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
