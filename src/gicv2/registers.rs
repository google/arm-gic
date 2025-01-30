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

/// GICv2 type register value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct Typer(u32);

impl Typer {
    /// Returns the maximum number of lockable SPIs supported, from 0 to 31.
    pub fn lockable_spi_count(self) -> u32 {
        (self.0 >> 11) & 0b11111
    }

    /// Returns whether the GIC supports two security states.
    pub fn has_security_extension(self) -> bool {
        self.0 & (1 << 10) != 0
    }

    /// Returns the number of implemented CPU interfaces.
    pub fn cpu_count(self) -> u32 {
        ((self.0 >> 5) & 0b111) + 1
    }

    /// Returns the maximum number of interrupts supported.
    pub fn num_irqs(&self) -> u32 {
        ((self.0 & 0b11111) + 1) * 32
    }
}

/// GIC Distributor registers.
#[repr(C, align(8))]
pub struct Gicd {
    /// Distributor Control Register
    pub ctlr: GicdCtlr,
    /// Interrupt Controller Type Register
    pub typer: Typer,
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

/// GIC CPU interface registers.
#[repr(C, align(8))]
pub struct Gicc {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_count() {
        assert_eq!(Typer(0).cpu_count(), 1);
        assert_eq!(Typer(7 << 5).cpu_count(), 8);
    }

    #[test]
    fn it_lines() {
        assert_eq!(Typer(0).num_irqs(), 32);
        assert_eq!(Typer(0b00011).num_irqs(), 128);
        assert_eq!(Typer(0b11111).num_irqs(), 1024);
    }
}
