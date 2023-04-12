// Copyright 2023 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

use bitflags::bitflags;

bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct GicdCtlr: u32 {
        const RWP = 1 << 31;
        const nASSGIreq = 1 << 8;
        const E1NWF = 1 << 7;
        const DS = 1 << 6;
        const ARE_NS = 1 << 5;
        const ARE_S = 1 << 4;
        const EnableGrp1S = 1 << 2;
        const EnableGrp1NS = 1 << 1;
        const EnableGrp0 = 1 << 0;
    }
}

/// GIC Distributor registers.
#[allow(clippy::upper_case_acronyms)]
#[repr(C, align(8))]
pub struct GICD {
    /// Distributor control register.
    pub ctlr: GicdCtlr,
    /// Interrupt controller type register.
    pub typer: u32,
    /// Distributor implementer identification register.
    pub iidr: u32,
    /// Interrupt controller type register 2.
    pub typer2: u32,
    /// Error reporting status register.
    pub statusr: u32,
    _reserved0: [u32; 3],
    /// Implementation defined registers.
    pub implementation_defined: [u32; 8],
    /// Set SPI register.
    pub setspi_nsr: u32,
    _reserved1: u32,
    /// Clear SPI register.
    pub clrspi_nsr: u32,
    _reserved2: u32,
    /// Set SPI secure register.
    pub setspi_sr: u32,
    _reserved3: u32,
    /// Clear SPI secure register.
    pub clrspi_sr: u32,
    _reserved4: [u32; 9],
    /// Interrupt group registers.
    pub igroupr: [u32; 32],
    /// Interrupt set-enable registers.
    pub isenabler: [u32; 32],
    /// Interrupt clear-enable registers.
    pub icenabler: [u32; 32],
    /// Interrupt set-pending registers.
    pub ispendr: [u32; 32],
    /// Interrupt clear-pending registers.
    pub icpendr: [u32; 32],
    /// Interrupt set-active registers.
    pub isactiver: [u32; 32],
    /// Interrupt clear-active registers.
    pub icactiver: [u32; 32],
    /// Interrupt priority registers.
    pub ipriorityr: [u8; 1024],
    /// Interrupt processor targets registers.
    pub itargetsr: [u32; 256],
    /// Interrupt configuration registers.
    pub icfgr: [u32; 64],
    /// Interrupt group modifier registers.
    pub igrpmodr: [u32; 32],
    _reserved5: [u32; 32],
    /// Non-secure access control registers.
    pub nsacr: [u32; 64],
    /// Software generated interrupt register.
    pub sigr: u32,
    _reserved6: [u32; 3],
    /// SGI clear-pending registers.
    pub cpendsgir: [u32; 4],
    /// SGI set-pending registers.
    pub spendsgir: [u32; 4],
    _reserved7: [u32; 20],
    /// Non-maskable interrupt registers.
    pub inmir: [u32; 32],
    /// Interrupt group registers for extended SPI range.
    pub igroupr_e: [u32; 32],
    _reserved8: [u32; 96],
    /// Interrupt set-enable registers for extended SPI range.
    pub isenabler_e: [u32; 32],
    _reserved9: [u32; 96],
    /// Interrupt clear-enable registers for extended SPI range.
    pub icenabler_e: [u32; 32],
    _reserved10: [u32; 96],
    /// Interrupt set-pending registers for extended SPI range.
    pub ispendr_e: [u32; 32],
    _reserved11: [u32; 96],
    /// Interrupt clear-pending registers for extended SPI range.
    pub icpendr_e: [u32; 32],
    _reserved12: [u32; 96],
    /// Interrupt set-active registers for extended SPI range.
    pub isactive_e: [u32; 32],
    _reserved13: [u32; 96],
    /// Interrupt clear-active registers for extended SPI range.
    pub icactive_e: [u32; 32],
    _reserved14: [u32; 224],
    /// Interrupt priority registers for extended SPI range.
    pub ipriorityr_e: [u8; 1024],
    _reserved15: [u32; 768],
    /// Extended SPI configuration registers.
    pub icfgr_e: [u32; 64],
    _reserved16: [u32; 192],
    /// Interrupt group modifier registers for extended SPI range.
    pub igrpmodr_e: [u32; 32],
    _reserved17: [u32; 96],
    /// Non-secure access control registers for extended SPI range.
    pub nsacr_e: [u32; 32],
    _reserved18: [u32; 288],
    /// Non-maskable interrupt registers for extended SPI range.
    pub inmr_e: [u32; 32],
    _reserved19: [u32; 2400],
    /// Interrupt routing registers.
    pub irouter: [u32; 1975],
    _reserved20: [u32; 9],
    /// Interrupt routing registers for extended SPI range.
    pub irouter_e: [u32; 2048],
    _reserved21: [u32; 2048],
    /// Implementation defined registers.
    pub implementation_defined2: [u32; 4084],
    /// ID registers.
    pub id_registers: [u32; 12],
}

bitflags! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub struct Waker: u32 {
        const CHILDREN_ASLEEP = 1 << 2;
        const PROCESSOR_SLEEP = 1 << 1;
    }
}

/// GIC Redistributor registers.
#[allow(clippy::upper_case_acronyms)]
#[repr(C, align(8))]
pub struct GICR {
    /// Redistributor control register.
    pub ctlr: u32,
    /// Implementer identification register.
    pub iidr: u32,
    /// Redistributor type register.
    pub typer: u64,
    /// Error reporting status register.
    pub statusr: u32,
    /// Redistributor wake register.
    pub waker: Waker,
    /// Report maximum PARTID and PMG register.
    pub mpamidr: u32,
    /// Set PARTID and PMG register.
    pub partidr: u32,
    /// Implementation defined registers.
    pub implementation_defined1: [u32; 8],
    /// Set LPI pending register.
    pub setlprir: u64,
    /// Clear LPI pending register.
    pub clrlpir: u64,
    _reserved0: [u32; 8],
    /// Redistributor properties base address register.
    pub propbaser: u64,
    /// Redistributor LPI pending table base address register.
    pub pendbaser: u64,
    _reserved1: [u32; 8],
    /// Redistributor invalidate LPI register.
    pub invlpir: u64,
    _reserved2: u64,
    /// Redistributor invalidate all register.
    pub invallr: u64,
    _reserved3: u64,
    /// Redistributor synchronize register.
    pub syncr: u32,
    _reserved4: [u32; 15],
    /// Implementation defined registers.
    pub implementation_defined2: u64,
    _reserved5: u64,
    /// Implementation defined registers.
    pub implementation_defined3: u64,
    _reserved6: [u32; 12218],
    /// Implementation defined registers.
    pub implementation_defined4: [u32; 4084],
    /// ID registers.
    pub id_registers: [u32; 12],
}

/// GIC Redistributor SGI and PPI registers.
#[allow(clippy::upper_case_acronyms)]
#[repr(C, align(8))]
pub struct SGI {
    _reserved0: [u32; 32],
    /// Interrupt group register 0.
    pub igroupr0: u32,
    /// Interrupt group registers for extended PPI range.
    pub igroupr_e: [u32; 2],
    _reserved1: [u32; 29],
    /// Interrupt set-enable register 0.
    pub isenabler0: u32,
    /// Interrupt set-enable registers for extended PPI range.
    pub isenabler_e: [u32; 2],
    _reserved2: [u32; 29],
    /// Interrupt clear-enable register 0.
    pub icenabler0: u32,
    /// Interrupt clear-enable registers for extended PPI range.
    pub icenabler_e: [u32; 2],
    _reserved3: [u32; 29],
    /// Interrupt set-pending register 0.
    pub ispendr0: u32,
    /// Interrupt set-pending registers for extended PPI range.
    pub ispendr_e: [u32; 2],
    _reserved4: [u32; 29],
    /// Interrupt clear-pending register 0.
    pub icpendr0: u32,
    /// Interrupt clear-pending registers for extended PPI range.
    pub icpendr_e: [u32; 2],
    _reserved5: [u32; 29],
    /// Interrupt set-active register 0.
    pub isactiver0: u32,
    /// Interrupt set-active registers for extended PPI range.
    pub isactive_e: [u32; 2],
    _reserved6: [u32; 29],
    /// Interrupt clear-active register 0.
    pub icactiver0: u32,
    /// Interrupt clear-active registers for extended PPI range.
    pub icactive_e: [u32; 2],
    _reserved7: [u32; 29],
    /// Interrupt priority registers.
    pub ipriorityr: [u8; 32],
    /// Interrupt priority registers for extended PPI range.
    pub ipriorityr_e: [u8; 64],
    _reserved8: [u32; 488],
    /// SGI configuration register, PPI configuration register and extended PPI configuration
    /// registers.
    pub icfgr: [u32; 6],
    _reserved9: [u32; 58],
    /// Interrupt group modifier register 0.
    pub igrpmodr0: u32,
    /// Interrupt group modifier registers for extended PPI range.
    pub igrpmodr_e: [u32; 2],
    _reserved10: [u32; 61],
    /// Non-secure access control register.
    pub nsacr: u32,
    _reserved11: [u32; 95],
    /// Non-maskable interrupt register for PPIs.
    pub inmir0: u32,
    /// Non-maskable interrupt register for extended PPIs.
    pub inmir_e: [u32; 31],
    _reserved12: [u32; 11264],
    /// Implementation defined registers.
    pub implementation_defined: [u32; 4084],
    _reserved13: [u32; 12],
}
