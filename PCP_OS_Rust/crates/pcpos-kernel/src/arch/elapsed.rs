//! Elapsed time on the boot processor. The PIT still delivers scheduling events.
//! A checked ACPI HPET block provides a free-running counter independent of IRQ delivery.
//! Uses the 64-bit primary PC block. Comparator programming belongs to the interrupt source.
use super::{BootInfo, NX, P, TABLES, W};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU64, Ordering};

use super::processors::HPET_ADDRESS as PHYSICAL;

const WINDOW: usize = PHYSICAL as usize;
const UNCACHED: u64 = (1 << 3) | (1 << 4);
const PIT_TICK_NS: u64 = 999_847;
const FALLBACK: &str = "delivered PIT interrupts (elapsed time can be lost under load)";
const ADMITTED: &str = "64-bit ACPI HPET main counter";

/// Which clock elapsed time is read from now, in the words `init` returned when it chose.
pub fn source() -> &'static str {
    if active() { ADMITTED } else { FALLBACK }
}

#[repr(align(4096))]
struct TimerTables(UnsafeCell<[u64; 512]>);
// Safety: initialized once on the boot processor, before any guest can run.
unsafe impl Sync for TimerTables {}
static TIMER_TABLES: TimerTables = TimerTables(UnsafeCell::new([0; 512]));
static PERIOD_FS: AtomicU64 = AtomicU64::new(0);
static COUNTER_START: AtomicU64 = AtomicU64::new(0);
static OFFSET_NS: AtomicU64 = AtomicU64::new(0);

/// PWT and PCD together select UC on a processor without PAT. With PAT they select
/// entry 3, which firmware can change. Check its actual type before touching MMIO.
fn uncached_registers() -> bool {
    if core::arch::x86_64::__cpuid(1).edx & (1 << 16) == 0 {
        return true;
    }
    let low: u32;
    // Safety: CPUID reported the page-attribute table. Reading IA32_PAT changes no state.
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") 0x277u32, out("eax") low, out("edx") _, options(nomem, nostack, preserves_flags));
    }
    (low >> 24) == 0
}

/// Give the kernel a private, uncached view of just the timer's register page.
/// The device-grant table is for programs and does not receive this mapping.
fn map_registers() -> bool {
    // Safety: this is the existing identity-map directory for the fourth GiB. Publish a single
    // supervisor-only, uncached, non-executable leaf in a previously unused 2 MiB stretch.
    unsafe {
        let dir = (TABLES.0.get() as *mut [u64; 512]).add(2 + (WINDOW >> 30));
        let slot = (WINDOW >> 21) & 511;
        if (*dir)[slot] != 0 {
            return false;
        }
        (*TIMER_TABLES.0.get())[(WINDOW >> 12) & 511] = PHYSICAL | P | W | UNCACHED | NX;
        (*dir)[slot] = TIMER_TABLES.0.get() as u64 | P | W;
        core::arch::asm!("invlpg [{}]", in(reg) WINDOW, options(nostack, preserves_flags));
    }
    true
}

/// Once initialization is over, the kernel only reads the counter. Discard a refused mapping.
fn finish_mapping(keep: bool) {
    // Safety: boot still runs on the only active processor. Only entries installed above are changed.
    unsafe {
        let leaf = &mut (*TIMER_TABLES.0.get())[(WINDOW >> 12) & 511];
        if keep {
            *leaf &= !W;
        } else {
            *leaf = 0;
            let dir = (TABLES.0.get() as *mut [u64; 512]).add(2 + (WINDOW >> 30));
            (*dir)[(WINDOW >> 21) & 511] = 0;
        }
        core::arch::asm!("invlpg [{}]", in(reg) WINDOW, options(nostack, preserves_flags));
    }
}

pub fn active() -> bool {
    PERIOD_FS.load(Ordering::Acquire) != 0
}

pub(super) fn is_timer_leaf(addr: usize, size: usize, entry: u64) -> bool {
    addr == WINDOW
        && size == 4096
        && active()
        && entry & (P | W | NX | UNCACHED | (1 << 7)) == (P | NX | UNCACHED)
}

fn register(offset: usize) -> u32 {
    // Safety: callers have mapped the register page and pass an aligned HPET register offset.
    unsafe { ((WINDOW + offset) as *const u32).read_volatile() }
}

fn enable(value: u32) {
    // Safety: init checks the block identity and touches only the general configuration.
    unsafe { ((WINDOW + 0x10) as *mut u32).write_volatile(value) }
}

fn counter() -> u64 {
    loop {
        let upper = register(0xf4);
        let lower = register(0xf0);
        if register(0xf4) == upper {
            return (u64::from(upper) << 32) | u64::from(lower);
        }
    }
}

pub fn init(boot: &BootInfo) -> &'static str {
    let Some(id) = boot.hpet_id else {
        return FALLBACK;
    };
    let start = PHYSICAL as usize;
    // Reject an overlapping module, accepted display region or the interrupt controller’s page-table stretch.
    if boot
        .modules
        .iter()
        .any(|m| m.start < start + 4096 && m.end > start)
        || boot
            .processors
            .as_ref()
            .is_some_and(|found| found.controller >> 21 == PHYSICAL >> 21)
        || super::DEVICES.with(|ranges| {
            ranges.0[..ranges.1]
                .iter()
                .any(|&(a, b)| a < start + 4096 && b > start)
        })
        || !uncached_registers()
        || !map_registers()
    {
        return FALLBACK;
    }
    let capabilities = register(0);
    let period = u64::from(register(4));
    if capabilities != id || capabilities & (1 << 13) == 0 || !(1..=100_000_000).contains(&period) {
        finish_mapping(false);
        return FALLBACK;
    }
    let config = register(0x10);
    if config & 1 == 0 {
        let comparators = ((capabilities >> 8) & 31) as usize + 1;
        // Starting a stopped block must not route a legacy IRQ or arm someone's comparator.
        if config & 2 != 0 || (0..comparators).any(|i| register(0x100 + i * 0x20) & 4 != 0) {
            finish_mapping(false);
            return FALLBACK;
        }
        enable(config | 1);
    }
    let first = counter();
    if !(0..1024).any(|_| counter() != first) {
        if config & 1 == 0 {
            enable(config);
        }
        finish_mapping(false);
        return FALLBACK;
    }
    finish_mapping(true);
    OFFSET_NS.store(
        super::ticks().saturating_mul(PIT_TICK_NS),
        Ordering::Relaxed,
    );
    COUNTER_START.store(first, Ordering::Relaxed);
    PERIOD_FS.store(period, Ordering::Release);
    ADMITTED
}

pub fn now_ns() -> u64 {
    let period = PERIOD_FS.load(Ordering::Acquire);
    if period == 0 {
        return super::ticks().saturating_mul(PIT_TICK_NS);
    }
    let span = counter().wrapping_sub(COUNTER_START.load(Ordering::Relaxed));
    let ns = (u128::from(span) * u128::from(period)) / 1_000_000;
    OFFSET_NS
        .load(Ordering::Relaxed)
        .saturating_add(ns.min(u128::from(u64::MAX)) as u64)
}
