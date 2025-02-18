// Copyright 2023 The arm-gic Authors.
// This project is dual-licensed under Apache 2.0 and MIT terms.
// See LICENSE-APACHE and LICENSE-MIT for details.

//! Driver for the Arm Generic Interrupt Controller version 3 (or 4).

pub mod registers;

use self::registers::{Gicd, GicdCtlr, Gicr, GicrCtlr, Sgi, Waker};
use crate::sysreg::{
    read_icc_iar1_el1, write_icc_ctlr_el1, write_icc_eoir1_el1, write_icc_igrpen0_el1,
    write_icc_igrpen1_el1, write_icc_pmr_el1, write_icc_sgi1r_el1, write_icc_sre_el1,
};
use crate::{IntId, Trigger};
use core::hint::spin_loop;
use registers::Typer;
use thiserror::Error;

/// The offset in bytes from `RD_base` to `SGI_base`.
const SGI_OFFSET: usize = 0x10000;

/// An error which may be returned from operations on a GIC Redistributor.
#[derive(Error, Debug, Clone, Copy, Eq, PartialEq)]
pub enum GICRError {
    #[error("Redistributor has already been notified that the connected core is awake")]
    AlreadyAwake,
}

/// Modifies `nth` bit of memory pointed by `registers`.
///
/// # Safety
///
/// The caller must ensure that `registers` is a valid pointer for volatile reads and writes.
unsafe fn modify_bit(registers: *mut [u32], nth: usize, set_bit: bool) {
    let reg_num: usize = nth / 32;

    let bit_num: usize = nth % 32;
    let bit_mask: u32 = 1 << bit_num;

    // SAFETY: `registers` is guaranteed to be
    // a valid pointer for volatile reads and writes
    // and `reg_num` does not exceed `*registers` length.
    unsafe {
        let reg_ptr = &raw mut (*registers)[reg_num];
        let old_value = reg_ptr.read_volatile();

        let new_value: u32 = if set_bit {
            old_value | bit_mask
        } else {
            old_value & !bit_mask
        };

        reg_ptr.write_volatile(new_value);
    }
}

/// Sets `nth` bit of memory pointed by `registers`.
///
/// # Safety
///
/// The caller must ensure that `registers` is a valid
/// pointer for volatile reads and writes.
unsafe fn set_bit(registers: *mut [u32], nth: usize) {
    modify_bit(registers, nth, true);
}

/// Clears `nth` bit of memory pointed by `registers`.
///
/// # Safety
///
/// The caller must ensure that `registers` is a valid
/// pointer for volatile reads and writes.
unsafe fn clear_bit(registers: *mut [u32], nth: usize) {
    modify_bit(registers, nth, false);
}

/// Driver for an Arm Generic Interrupt Controller version 3 (or 4).
#[derive(Debug)]
pub struct GicV3 {
    gicd: *mut Gicd,
    gicr_base: *mut Gicr,
    /// The number of CPU cores, and hence redistributors.
    cpu_count: usize,
    /// The offset in bytes between the start of redistributor frames.
    gicr_stride: usize,
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
    pub unsafe fn new(
        gicd: *mut u64,
        gicr_base: *mut u64,
        cpu_count: usize,
        gicr_stride: usize,
    ) -> Self {
        assert_eq!(gicr_stride % 0x20000, 0);
        Self {
            gicd: gicd as _,
            gicr_base: gicr_base as _,
            cpu_count,
            gicr_stride,
        }
    }

    /// Enables system register access, marks the given CPU core as awake, and sets some basic
    /// configuration.
    ///
    /// `cpu` should be the linear index of the CPU core as used by the GIC redistributor.
    ///
    /// If the core is already marked as awake this will not return any error.
    ///
    /// This disables the use of `ICC_PMR_EL1` as a hint for interrupt distribution, configures a
    /// write to an EOI register to also deactivate the interrupt, and configures preemption groups
    /// for group 0 and group 1 interrupts separately.
    pub fn init_cpu(&mut self, cpu: usize) {
        // Enable system register access.
        write_icc_sre_el1(0x01);

        // Ignore error in case core is already awake.
        let _ = self.redistributor_mark_core_awake(cpu);

        // Disable use of `ICC_PMR_EL1` as a hint for interrupt distribution, configure a write to
        // an EOI register to also deactivate the interrupt, and configure preemption groups for
        // group 0 and group 1 interrupts separately.
        write_icc_ctlr_el1(0);
    }

    /// Initialises the GIC and marks the given CPU core as awake.
    ///
    /// `cpu` should be the linear index of the CPU core as used by the GIC redistributor.
    pub fn setup(&mut self, cpu: usize) {
        self.init_cpu(cpu);

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
            for cpu in 0..self.cpu_count {
                (&raw mut (*self.sgi_ptr(cpu)).igroupr0).write_volatile(0xffffffff);
            }
            // Put all SPIs into non-secure group 1.
            for i in 1..32 {
                (&raw mut (*self.gicd).igroupr[i]).write_volatile(0xffffffff);
            }
        }

        // Enable group 1 for the current security state.
        Self::enable_group1(true);
    }

    /// Enables or disables group 0 interrupts.
    pub fn enable_group0(enable: bool) {
        write_icc_igrpen0_el1(if enable { 0x01 } else { 0x00 });
    }

    /// Enables or disables group 1 interrupts for the current security state.
    pub fn enable_group1(enable: bool) {
        write_icc_igrpen1_el1(if enable { 0x01 } else { 0x00 });
    }

    /// Enables or disables the interrupt with the given ID.
    ///
    /// If it is an SGI or PPI then the CPU core on which to enable it must also be specified;
    /// otherwise this is ignored and may be `None`.
    pub fn enable_interrupt(&mut self, intid: IntId, cpu: Option<usize>, enable: bool) {
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        let (isenabler, icenabler): (*mut [u32], *mut [u32]) = unsafe {
            if intid.is_private() {
                (
                    &raw mut (*self.sgi_ptr(cpu.unwrap())).isenabler0 as *mut [u32; 1],
                    &raw mut (*self.sgi_ptr(cpu.unwrap())).icenabler0 as *mut [u32; 1],
                )
            } else {
                (
                    &raw mut (*self.gicd).isenabler,
                    &raw mut (*self.gicd).icenabler,
                )
            }
        };

        // SAFETY: We know that `isenabler` and `icenabler` are valid and unique pointers
        // to the registers of GIC distributor or redistributor interface.
        unsafe {
            if enable {
                set_bit(isenabler, intid.0 as usize);
            } else {
                set_bit(icenabler, intid.0 as usize);
            }
        }
    }

    /// Enables or disables all interrupts on all CPU cores.
    pub fn enable_all_interrupts(&mut self, enable: bool) {
        for i in 1..32 {
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
        for cpu in 0..self.cpu_count {
            // SAFETY: We know that `self.sgis` are valid and unique pointers to the SGI and PPI
            // registers of a GIC redistributor interface.
            unsafe {
                if enable {
                    (&raw mut (*self.sgi_ptr(cpu)).isenabler0).write_volatile(0xffffffff);
                } else {
                    (&raw mut (*self.sgi_ptr(cpu)).icenabler0).write_volatile(0xffffffff);
                }
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
    pub fn set_interrupt_priority(&mut self, intid: IntId, cpu: Option<usize>, priority: u8) {
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            // Affinity routing is enabled, so use the GICR for SGIs and PPIs.
            if intid.is_private() {
                (&raw mut (*self.sgi_ptr(cpu.unwrap())).ipriorityr[intid.0 as usize])
                    .write_volatile(priority);
            } else {
                (&raw mut (*self.gicd).ipriorityr[intid.0 as usize]).write_volatile(priority);
            }
        }
    }

    /// Configures the trigger type for the interrupt with the given ID.
    pub fn set_trigger(&mut self, intid: IntId, cpu: Option<usize>, trigger: Trigger) {
        let index = (intid.0 / 16) as usize;
        let bit = 1 << (((intid.0 % 16) * 2) + 1);

        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        unsafe {
            // Affinity routing is enabled, so use the GICR for SGIs and PPIs.
            let register = if intid.is_private() {
                (&raw mut (*self.sgi_ptr(cpu.unwrap())).icfgr[index])
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

    /// Assigns the interrupt with id `intid` to interrupt group `group`.
    pub fn set_group(&mut self, intid: IntId, cpu: Option<usize>, group: Group) {
        // FIXME: For now we assume that we are running a single-core system.
        // so there's just one GICR frame and one SGI configuration.

        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface, and `self.sgi` to the SGI and PPI registers of a GIC
        // redistributor interface.
        let (igroupr, igrpmodr): (*mut [u32], *mut [u32]) = unsafe {
            if intid.is_private() {
                (
                    &raw mut (*self.sgi_ptr(cpu.unwrap())).igroupr0 as *mut [u32; 1],
                    &raw mut (*self.sgi_ptr(cpu.unwrap())).igrpmodr0 as *mut [u32; 1],
                )
            } else {
                (
                    &raw mut (*self.gicd).igroupr,
                    &raw mut (*self.gicd).igrpmodr,
                )
            }
        };

        // SAFETY: We know that `igroupr` and `igrpmodr` are valid and unique pointers
        // to the registers of GIC distributor or redistributor interface.
        unsafe {
            if let Group::Secure(sg) = group {
                clear_bit(igroupr, intid.0 as usize);
                match sg {
                    SecureIntGroup::Group1S => set_bit(igrpmodr, intid.0 as usize),
                    SecureIntGroup::Group0 => clear_bit(igrpmodr, intid.0 as usize),
                }
            } else {
                set_bit(igroupr, intid.0 as usize);
                clear_bit(igrpmodr, intid.0 as usize);
            }
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
        let intid = IntId(read_icc_iar1_el1());
        if intid == IntId::SPECIAL_NONE {
            None
        } else {
            Some(intid)
        }
    }

    /// Informs the interrupt controller that the CPU has completed processing the given interrupt.
    /// This drops the interrupt priority and deactivates the interrupt.
    pub fn end_interrupt(intid: IntId) {
        write_icc_eoir1_el1(intid.0)
    }

    /// Returns information about what the GIC implementation supports.
    pub fn typer(&self) -> Typer {
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a GIC
        // distributor interface.
        unsafe { (&raw mut (*self.gicd).typer).read_volatile() }
    }

    /// Returns a raw pointer to the GIC distributor registers.
    ///
    /// This may be used to read and write the registers directly for functionality not yet
    /// supported by this driver.
    pub fn gicd_ptr(&mut self) -> *mut Gicd {
        self.gicd
    }

    /// Returns a raw pointer to the GIC redistributor registers.
    ///
    /// This may be used to read and write the registers directly for functionality not yet
    /// supported by this driver.
    pub fn gicr_ptr(&mut self, cpu: usize) -> *mut Gicr {
        assert!(cpu < self.cpu_count);
        self.gicr_base.wrapping_byte_add(cpu * self.gicr_stride)
    }

    /// Returns a raw pointer to the GIC redistributor SGI and PPI registers.
    ///
    /// This may be used to read and write the registers directly for functionality not yet
    /// supported by this driver.
    pub fn sgi_ptr(&mut self, cpu: usize) -> *mut Sgi {
        self.gicr_ptr(cpu).wrapping_byte_add(SGI_OFFSET).cast()
    }

    fn gicd_barrier(&mut self) {
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface.
        unsafe {
            while (&raw const (*self.gicd).ctlr)
                .read_volatile()
                .contains(GicdCtlr::RWP)
            {}
        }
    }

    fn gicd_modify_control(&mut self, f: impl FnOnce(GicdCtlr) -> GicdCtlr) {
        // SAFETY: We know that `self.gicd` is a valid and unique pointer to the registers of a
        // GIC distributor interface.
        unsafe {
            let gicd_ctlr = (&raw mut (*self.gicd).ctlr).read_volatile();

            (&raw mut (*self.gicd).ctlr).write_volatile(f(gicd_ctlr));
        }

        self.gicd_barrier();
    }

    /// Clears specified bits in GIC distributor control register.
    pub fn gicd_clear_control(&mut self, flags: GicdCtlr) {
        self.gicd_modify_control(|old| old - flags);
    }

    /// Sets specified bits in GIC distributor control register.
    pub fn gicd_set_control(&mut self, flags: GicdCtlr) {
        self.gicd_modify_control(|old| old | flags);
    }

    /// Blocks until register write for the current Security state is no longer in progress.
    pub fn gicr_barrier(&mut self, cpu: usize) {
        // SAFETY: We know that `self.sgi` is a valid and unique pointer to the SGI and PPI
        // registers of a GIC redistributor interface.
        unsafe {
            while (&raw const (*self.gicr_ptr(cpu)).ctlr)
                .read_volatile()
                .contains(GicrCtlr::RWP)
            {}
        }
    }

    /// Informs the GIC redistributor that the core has awakened.
    ///
    /// Blocks until `GICR_WAKER.ChildrenAsleep` is cleared.
    pub fn redistributor_mark_core_awake(&mut self, cpu: usize) -> Result<(), GICRError> {
        // SAFETY: We know that `self.gicr` is a valid and unique pointer to
        // the GIC redistributor interface.
        unsafe {
            let mut gicr_waker = (&raw const (*self.gicr_ptr(cpu)).waker).read_volatile();

            // The WAKER_PS_BIT should be changed to 0 only when WAKER_CA_BIT is 1.
            if !gicr_waker.contains(Waker::CHILDREN_ASLEEP) {
                return Err(GICRError::AlreadyAwake);
            }

            // Mark the connected core as awake.
            gicr_waker -= Waker::PROCESSOR_SLEEP;
            (&raw mut (*self.gicr_ptr(cpu)).waker).write_volatile(gicr_waker);

            // Wait till the WAKER_CA_BIT changes to 0.
            while (&raw const (*self.gicr_ptr(cpu)).waker)
                .read_volatile()
                .contains(Waker::CHILDREN_ASLEEP)
            {
                spin_loop();
            }

            Ok(())
        }
    }
}

// SAFETY: The GIC interface can be accessed from any CPU core.
unsafe impl Send for GicV3 {}

// SAFETY: Any operations which change state require `&mut GicV3`, so `&GicV3` is fine to share.
unsafe impl Sync for GicV3 {}

/// The group configuration for an interrupt.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Group {
    Secure(SecureIntGroup),
    Group1NS,
}

/// The group configuration for an interrupt.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SecureIntGroup {
    /// The interrupt belongs to Secure Group 1.
    Group1S,
    /// The interrupt belongs to Group 0.
    Group0,
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
