// Copyright 2023 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

//! Raw register access for the GICv3.

use super::IntId;
use bitflags::bitflags;
use core::{
    cmp::min,
    fmt::{self, Debug, Formatter},
};
use safe_mmio::fields::{ReadPure, ReadPureWrite, WriteOnly};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[repr(transparent)]
#[derive(Copy, Clone, Eq, FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq)]
pub struct GicdCtlr(u32);

bitflags! {
    impl GicdCtlr: u32 {
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

impl Debug for GicdCtlr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "GicdCtlr(")?;
        bitflags::parser::to_writer(self, &mut *f)?;
        write!(f, ")")?;
        Ok(())
    }
}
#[repr(transparent)]
#[derive(Copy, Clone, Eq, FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq)]
pub struct GicrCtlr(u32);

bitflags! {
    impl GicrCtlr: u32 {
        const UWP = 1 << 31;
        const DPG1S = 1 << 26;
        const DPG1NS = 1 << 25;
        const DPG0 = 1 << 24;
        const RWP = 1 << 3;
        const IR = 1 << 2;
        const CES = 1 << 1;
        const EnableLPIs = 1 << 0;
    }
}

impl Debug for GicrCtlr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "GicrCtlr(")?;
        bitflags::parser::to_writer(self, &mut *f)?;
        write!(f, ")")?;
        Ok(())
    }
}

/// This register controls the powerup sequence of the Redistributors.
///
/// It is implemented only for GIC-600 and GIC-700
/// and is used to power on/off the redistributors.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq)]
pub struct GicrPwrr(u32);

bitflags! {
    impl GicrPwrr: u32 {
        /// (RDGPO)
        /// This bit indicates:
        /// 0 = GCI (GIC Cluster Interface) is powered up and can be accessed.
        /// 1 = It is safe to power down the GCI.
        const RedistributorGroupPoweredOff = 1 << 3;

        /// (RDGPD)
        /// This bit indicates the intentional power state of the GCI:
        /// 0 = Intend to power up
        /// 1 = Intend to power down
        /// The GCI has reached its intentional power state when RDGPD = RDGPO.
        const RedistributorGroupPowerDown = 1 << 2;

        /// (RDAG)
        /// Setting this bit to 1 applies the RDPD value to all Redistributors
        /// on the same GCI.
        /// If the RDPD value cannot be applied to all cores in the group,
        /// then the GIC ignores this request.
        const RedistributorApplyGroup = 1 << 1;

        /// (RDPD)
        /// 0 = Redistributor is powered up and can be accessed.
        /// 1 = The core permits the Redistributor to be powered down.
        const RedistributorPowerDown = 1 << 0;
    }
}

/// This register provides information about
/// the implementer and revision of the Redistributor.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq)]
pub struct GicrIidr(u32);

impl GicrIidr {
    pub const MODEL_ID_ARM_GIC_600: u32 = 0x0200043b;
    pub const MODEL_ID_ARM_GIC_600AE: u32 = 0x0300043b;
    pub const MODEL_ID_ARM_GIC_700: u32 = 0x0400043b;

    /// Returns model ID of the redistributor.
    pub fn model_id(self) -> u32 {
        const PRODUCT_ID_MASK: u32 = 0xff << 24;
        const IMPLEMENTER_MASK: u32 = 0xfff;
        self.0 & (PRODUCT_ID_MASK | IMPLEMENTER_MASK)
    }
}

/// Interrupt controller redistributor type register value.
#[derive(Clone, Copy, Debug, Eq, FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq)]
#[repr(transparent)]
pub struct GicrTyper(u64);

impl GicrTyper {
    /// The identity of the PE associated with this Redistributor
    ///
    /// ret[0] provides Aff0, the Affinity level 0 value for the Redistributor.
    /// ret[1] provides Aff1, the Affinity level 1 value for the Redistributor.
    /// ret[2] provides Aff2, the Affinity level 2 value for the Redistributor.
    /// ret[3] provides Aff3, the Affinity level 3 value for the Redistributor.
    fn affinity_value(self) -> [u8; 4] {
        ((self.0 >> 32) as u32).to_le_bytes()
    }

    /// MPIDR value for the corresponding core.
    ///
    /// This should be used to discover Redistributor order
    /// with respect to the chosen linear core ID.
    pub fn core_mpidr(self) -> u64 {
        let affinity_value = self.affinity_value();

        u64::from_le_bytes([
            affinity_value[0],
            affinity_value[1],
            affinity_value[2],
            0,
            affinity_value[3],
            0,
            0,
            0,
        ])
    }

    /// Returns the value of the PPInum field.
    fn ppi_num(self) -> u32 {
        ((self.0 >> 27) & 0b11111) as u32
    }

    /// Returns the maximum number of Extended PPI interrupt IDs supported.
    pub fn max_eppi_count(self) -> u32 {
        32 * self.ppi_num()
    }

    /// Returns a unique ID for the PE associated with this redistributor.
    pub fn processor_number(self) -> u16 {
        (self.0 >> 8) as u16
    }

    /// Returns whether MPAM is supported.
    pub fn mpam_supported(self) -> bool {
        self.0 & (1 << 6) != 0
    }

    /// Returns whether the redistributor supports Disable Processor Group.
    pub fn disable_processor_group_supported(self) -> bool {
        self.0 & (1 << 5) != 0
    }

    /// This redistributor is the last redistributor on the chip.
    pub fn last_redistributor(self) -> bool {
        self.0 & (1 << 4) != 0
    }

    /// Returns whether direct injection of LPIs is supported.
    pub fn direct_lpis_supported(self) -> bool {
        self.0 & (1 << 3) != 0
    }

    /// Returns whether VPENDBASER.Dirty is supported.
    pub fn dirty_supported(self) -> bool {
        self.0 & (1 << 2) != 0
    }

    /// Returns whether virtual LPIs are supported.
    pub fn virtual_lpis_supported(self) -> bool {
        self.0 & (1 << 1) != 0
    }

    /// Returns whether physical LPIs are supported.
    pub fn physical_lpis_supported(self) -> bool {
        self.0 & (1 << 0) != 0
    }
}

/// Interrupt controller type register value.
#[derive(Clone, Copy, Debug, Eq, FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq)]
#[repr(transparent)]
pub struct Typer(u32);

impl Typer {
    /// Returns the value of the ESPI_range field.
    fn espi_range(self) -> u32 {
        self.0 >> 27
    }

    /// Returns the highest supported Extended SPI interrupt ID.
    pub fn max_espi(self) -> IntId {
        IntId::espi(32 * self.espi_range() + 31)
    }

    /// Returns the range of affinity level 0 values supported for targeted SGIs.
    pub fn range_selector_support(self) -> RangeSelectorSupport {
        if self.0 & (1 << 26) == 0 {
            RangeSelectorSupport::AffZero16
        } else {
            RangeSelectorSupport::AffZero256
        }
    }

    /// Returns whether 1 of N SPI interrupts are supported.
    pub fn one_of_n_supported(self) -> bool {
        self.0 & (1 << 25) == 0
    }

    /// Returns whether the GICD supports nonzero values for affinity level 3.
    pub fn affinity_3_supported(self) -> bool {
        self.0 & (1 << 24) != 0
    }

    /// Returns the number of interrupt ID bits supported.
    pub fn id_bits(self) -> u32 {
        ((self.0 >> 19) & 0b11111) + 1
    }

    /// Returns whether Direct Virtual LPI injection is supported.
    pub fn dvi_supported(self) -> bool {
        self.0 & (1 << 18) != 0
    }

    /// Returns whether LPIs are supported.
    pub fn lpis_supported(self) -> bool {
        self.0 & (1 << 17) != 0
    }

    /// Returns whether message-based interrupts are supported.
    pub fn mpis_supported(self) -> bool {
        self.0 & (1 << 16) != 0
    }

    /// Returns the number of LPIs supported.
    pub fn num_lpis(self) -> u32 {
        let num_lpis = (self.0 >> 11) & 0b11111;
        if num_lpis == 0 {
            (1u32 << self.id_bits()).saturating_sub(8192)
        } else {
            2 << num_lpis
        }
    }

    /// Returns whether the GIC supports two security states.
    pub fn has_security_extension(self) -> bool {
        self.0 & (1 << 10) != 0
    }

    /// Returns whether the non-maskable interrupt property is supported.
    pub fn nmi_supported(self) -> bool {
        self.0 & (1 << 9) != 0
    }

    /// Returns whether the extended SPI range is implemented.
    pub fn espi_supported(self) -> bool {
        self.0 & (1 << 8) != 0
    }

    /// Returns the number of CPU cores supported when affinity routing is disabled.
    pub fn num_cpus(self) -> u32 {
        (self.0 >> 5) & 0b111
    }

    /// Returns the number of SPIs supported.
    pub fn num_spis(self) -> u32 {
        let it_lines = self.0 & 0b11111;
        min(32 * it_lines, IntId::MAX_SPI_COUNT)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RangeSelectorSupport {
    /// The IRI supports targeted SGIs with affinity level 0 values up to 15.
    AffZero16,
    /// The IRI supports targeted SGIs with affinity level 0 values up to 255.
    AffZero256,
}

/// GIC Distributor registers.
#[repr(C, align(8))]
pub struct Gicd {
    /// Distributor control register.
    pub ctlr: ReadPureWrite<GicdCtlr>,
    /// Interrupt controller type register.
    pub typer: ReadPure<Typer>,
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
    pub igroupr: [ReadPureWrite<u32>; 32],
    /// Interrupt set-enable registers.
    pub isenabler: [ReadPureWrite<u32>; 32],
    /// Interrupt clear-enable registers.
    pub icenabler: [ReadPureWrite<u32>; 32],
    /// Interrupt set-pending registers.
    pub ispendr: [u32; 32],
    /// Interrupt clear-pending registers.
    pub icpendr: [u32; 32],
    /// Interrupt set-active registers.
    pub isactiver: [u32; 32],
    /// Interrupt clear-active registers.
    pub icactiver: [u32; 32],
    /// Interrupt priority registers.
    pub ipriorityr: [ReadPureWrite<u8>; 1024],
    /// Interrupt processor targets registers.
    pub itargetsr: [u32; 256],
    /// Interrupt configuration registers.
    pub icfgr: [ReadPureWrite<u32>; 64],
    /// Interrupt group modifier registers.
    pub igrpmodr: [ReadPureWrite<u32>; 32],
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
    pub igroupr_e: [ReadPureWrite<u32>; 32],
    _reserved8: [u32; 96],
    /// Interrupt set-enable registers for extended SPI range.
    pub isenabler_e: [ReadPureWrite<u32>; 32],
    _reserved9: [u32; 96],
    /// Interrupt clear-enable registers for extended SPI range.
    pub icenabler_e: [ReadPureWrite<u32>; 32],
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
    pub ipriorityr_e: [ReadPureWrite<u8>; 1024],
    _reserved15: [u32; 768],
    /// Extended SPI configuration registers.
    pub icfgr_e: [ReadPureWrite<u32>; 64],
    _reserved16: [u32; 192],
    /// Interrupt group modifier registers for extended SPI range.
    pub igrpmodr_e: [ReadPureWrite<u32>; 32],
    _reserved17: [u32; 96],
    /// Non-secure access control registers for extended SPI range.
    pub nsacr_e: [u32; 32],
    _reserved18: [u32; 288],
    /// Non-maskable interrupt registers for extended SPI range.
    pub inmr_e: [u32; 32],
    _reserved19: [u32; 2400],
    /// Interrupt routing registers.
    pub irouter: [u64; 988],
    _reserved20: [u32; 8],
    /// Interrupt routing registers for extended SPI range.
    pub irouter_e: [u64; 1024],
    _reserved21: [u32; 2048],
    /// Implementation defined registers.
    pub implementation_defined2: [u32; 4084],
    /// ID registers.
    pub id_registers: [u32; 12],
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, FromBytes, Immutable, IntoBytes, KnownLayout, PartialEq)]
pub struct Waker(u32);

impl Debug for Waker {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Waker(")?;
        bitflags::parser::to_writer(self, &mut *f)?;
        write!(f, ")")?;
        Ok(())
    }
}
bitflags! {
    impl Waker: u32 {
        const CHILDREN_ASLEEP = 1 << 2;
        const PROCESSOR_SLEEP = 1 << 1;
    }
}

/// GIC Redistributor, SGI and PPI registers.
#[repr(C, align(8))]
pub struct GicrSgi {
    pub gicr: Gicr,
    pub sgi: Sgi,
}

/// GIC Redistributor registers.
#[repr(C, align(8))]
pub struct Gicr {
    /// Redistributor control register.
    pub ctlr: ReadPureWrite<GicrCtlr>,
    /// Implementer identification register.
    pub iidr: ReadPure<GicrIidr>,
    /// Redistributor type register.
    pub typer: ReadPure<GicrTyper>,
    /// Error reporting status register.
    pub statusr: ReadPureWrite<u32>,
    /// Redistributor wake register.
    pub waker: ReadPureWrite<Waker>,
    /// Report maximum PARTID and PMG register.
    pub mpamidr: ReadPure<u32>,
    /// Set PARTID and PMG register.
    pub partidr: ReadPureWrite<u32>,
    /// Implementation defined registers.
    pub implementation_defined1: u32,
    /// Redistributor power register (implemented in GIC-600 and GIC-700).
    pub pwrr: ReadPureWrite<GicrPwrr>,
    /// Implementation defined registers.
    pub implementation_defined2: [u32; 6],
    /// Set LPI pending register.
    pub setlprir: WriteOnly<u64>,
    /// Clear LPI pending register.
    pub clrlpir: WriteOnly<u64>,
    _reserved0: [u32; 8],
    /// Redistributor properties base address register.
    pub propbaser: ReadPureWrite<u64>,
    /// Redistributor LPI pending table base address register.
    pub pendbaser: ReadPureWrite<u64>,
    _reserved1: [u32; 8],
    /// Redistributor invalidate LPI register.
    pub invlpir: WriteOnly<u64>,
    _reserved2: u64,
    /// Redistributor invalidate all register.
    pub invallr: WriteOnly<u64>,
    _reserved3: u64,
    /// Redistributor synchronize register.
    pub syncr: ReadPure<u32>,
    _reserved4: [u32; 15],
    /// Implementation defined registers.
    pub implementation_defined3: u64,
    _reserved5: u64,
    /// Implementation defined registers.
    pub implementation_defined4: u64,
    _reserved6: [u32; 12218],
    /// Implementation defined registers.
    pub implementation_defined5: [u32; 4084],
    /// ID registers.
    pub id_registers: [ReadPure<u32>; 12],
}

/// GIC Redistributor SGI and PPI registers.
#[repr(C, align(8))]
pub struct Sgi {
    _reserved0: [u32; 32],
    /// Interrupt group register 0.
    pub igroupr0: ReadPureWrite<u32>,
    /// Interrupt group registers for extended PPI range.
    pub igroupr_e: [ReadPureWrite<u32>; 2],
    _reserved1: [u32; 29],
    /// Interrupt set-enable register 0.
    pub isenabler0: ReadPureWrite<u32>,
    /// Interrupt set-enable registers for extended PPI range.
    pub isenabler_e: [ReadPureWrite<u32>; 2],
    _reserved2: [u32; 29],
    /// Interrupt clear-enable register 0.
    pub icenabler0: ReadPureWrite<u32>,
    /// Interrupt clear-enable registers for extended PPI range.
    pub icenabler_e: [ReadPureWrite<u32>; 2],
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
    pub ipriorityr: [ReadPureWrite<u8>; 32],
    /// Interrupt priority registers for extended PPI range.
    pub ipriorityr_e: [ReadPureWrite<u8>; 64],
    _reserved8: [u32; 488],
    /// SGI configuration register, PPI configuration register and extended PPI configuration
    /// registers.
    pub icfgr: [ReadPureWrite<u32>; 6],
    _reserved9: [u32; 58],
    /// Interrupt group modifier register 0.
    pub igrpmodr0: ReadPureWrite<u32>,
    /// Interrupt group modifier registers for extended PPI range.
    pub igrpmodr_e: [ReadPureWrite<u32>; 2],
    _reserved10: [u32; 61],
    /// Non-secure access control register.
    pub nsacr: ReadPureWrite<u32>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_espi() {
        assert_eq!(Typer(0xffffffff).max_espi().0, IntId::ESPI_END - 1);
    }

    #[test]
    fn num_lpis() {
        // num_LPIs is 0, no IDbits means no LPIs.
        assert_eq!(Typer(0).num_lpis(), 0);
        // num_LPIs is 0, 13 IDbits means no LPIs.
        assert_eq!(Typer(12 << 19).num_lpis(), 0);
        // num_LPIs is 0, 14 IDbits means 2**13 LPIs.
        assert_eq!(Typer(13 << 19).num_lpis(), 1 << 13);
        // num_LPIs is 0, 15 IDbits means 2**13 LPIs.
        assert_eq!(Typer(13 << 19).num_lpis(), 1 << 13);

        // num_LPIs is specified.
        assert_eq!(Typer(1 << 11).num_lpis(), 4);
        assert_eq!(Typer(2 << 11).num_lpis(), 8);
        assert_eq!(Typer(16 << 11).num_lpis(), 1 << 17);
    }

    #[test]
    fn gicr_size() {
        // The size of the Gicr struct should match the offset from `RD_base` to `SGI_base`.
        assert_eq!(size_of::<Gicr>(), 0x10000);
    }

    #[test]
    fn gicr_typer_affinity() {
        let gicr_typer = GicrTyper(0x12_34_56_78_c0ffeeee);

        // Level 0 is 0x78, Level 1 is 0x56, etc.
        let expected_affinity_values = [0x78, 0x56, 0x34, 0x12];
        assert_eq!(gicr_typer.affinity_value(), expected_affinity_values);

        // Affinity Level 3 is shifted (8 bit gap).
        let expected_core_mpidr = 0x00_00_00_12_00_34_56_78;
        assert_eq!(gicr_typer.core_mpidr(), expected_core_mpidr);
    }

    #[test]
    fn gicr_typer_flags() {
        // This flag is bit 0.
        assert!(!GicrTyper(0b0000000).physical_lpis_supported());
        assert!(GicrTyper(0b0000001).physical_lpis_supported());

        // This flag is bit 1.
        assert!(!GicrTyper(0b0000000).virtual_lpis_supported());
        assert!(GicrTyper(0b0000010).virtual_lpis_supported());

        // This flag is bit 3.
        assert!(!GicrTyper(0b0000000).direct_lpis_supported());
        assert!(GicrTyper(0b0001000).direct_lpis_supported());

        // This flag is bit 4.
        assert!(!GicrTyper(0b0000000).last_redistributor());
        assert!(GicrTyper(0b0010000).last_redistributor());

        // This flag is bit 5.
        assert!(!GicrTyper(0b0000000).disable_processor_group_supported());
        assert!(GicrTyper(0b0100000).disable_processor_group_supported());
    }
}
