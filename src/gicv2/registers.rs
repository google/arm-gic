// Copyright 2025 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.
use bitflags::bitflags;

bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct GicdCtlr: u32 {
        const EnableGrp1 = 1 << 1;
        const EnableGrp0 = 1 << 0;
    }
}

#[repr(C, align(8))]
pub struct GICD {
    /// Distributor Control Register
    pub ctlr: GicdCtlr,
    /// Interrupt Controller Type Register
    pub typer: u32,
    /// Distributor Implementer Identification Register.
    pub iidr: u32,
    _reserved_0: [u32; 0x1D],
    /// Interrupt Group Registers
    pub igroupr: [u32; 0x20],
    /// Interrupt Set-Enable Registers.
    pub isenabler: [u32; 0x20],
    /// Interrupt Clear-Enable Registers.
    pub icenabler: [u32; 0x20],
    /// Interrupt Set-Pending Registers.
    pub ispendr: [u32; 0x20],
    /// Interrupt Clear-Pending Registers.
    pub icpendr: [u32; 0x20],
    /// Interrupt Set-Active Registers.
    pub isactiver: [u32; 0x20],
    /// Interrupt Clear-Active Registers.
    pub icactiver: [u32; 0x20],
    /// Interrupt Priority Registers.
    pub ipriorityr: [u32; 0x100],
    /// Interrupt Processor Targets Registers.
    pub itargetsr: [u32; 0x100],
    /// Interrupt Configuration Registers.
    pub icfgr: [u32; 0x40],
    _reserved_1: [u32; 0x80],
    /// Software Generated Interrupt Register.
    pub sgir: u32,
}

#[repr(C, align(8))]
pub struct GICC {
    /// CPU Interface Control Register.
    pub ctlr: u32,
    /// Interrupt Priority Mask Register.
    pub pmr: u32,
    /// Binary Point Register.
    pub bpr: u32,
    /// Interrupt Acknowledge Register.
    pub iar: u32,
    /// End of Interrupt Register.
    pub eoir: u32,
    /// Running Priority Register.
    pub rpr: u32,
    /// Highest Priority Pending Interrupt Register.
    pub hppir: u32,
    /// Aliased Binary Point Register
    pub abpr: u32,
    /// Aliased Interrupt Acknwoledge Register
    pub aiar: u32,
    /// Aliased End of Interrupt Register
    pub aeoir: u32,
    /// Aliased Highest Priority Pending Interrupt Register
    pub ahppir: u32,
    _reserved_0: [u32; 0x34],
    /// CPU Interface Identification Register.
    pub iidr: u32,
    _reserved_1: [u32; 0x3C0],
    /// Deactivate Interrupt Register.
    pub dir: u32,
}
