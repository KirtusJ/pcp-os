//! Starting the machine's other processors, and parking them (feature `processors`; stage 2 of
//! using every processor). Each is brought from reset to 64-bit mode on the kernel's own map, given
//! its own segment table, task state segment and stacks, set to the same protections as the first,
//! asked who it is, and then left idling: halted, with interrupts on, waiting for a message from
//! another processor (stage 3). None runs a program yet. The kernel's code runs on one processor at
//! a time, whichever holds its one lock; see "one lock around the kernel" below.
//!
//! The order of a start is the one the processor makers give: INIT, ten milliseconds, a start-up
//! message naming a page below one mebibyte, and a second one if the first is not answered. One is
//! started at a time. One that does not answer is sent INIT again, which returns it to waiting, and
//! no further one is tried.
use super::*;
core::arch::global_asm!(include_str!("others.s"), options(att_syntax));
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

/// The most other processors: one fewer than the kernel will run on.
pub const OTHERS: usize = processors::MOST - 1;
const STACK_SIZE: usize = 32 * 1024;
/// How many pages `protect` leaves unmapped for them: one below each of every processor's four stacks.
pub const GUARDS: usize = OTHERS * 4;
/// What each other processor has to itself: its segment table and task state segment on one page,
/// which `protect` maps read-only; three stacks for the exceptions that must not depend on the stack
/// in use; and the stack it runs on. Below each stack is a page `protect` leaves unmapped.
#[repr(C, align(4096))]
struct Area {
    segments: Segments,
    faults: [u8; 3 * FAULT_SLOT],
    stack: [u8; 4096 + STACK_SIZE],
}
const AREA: usize = core::mem::size_of::<Area>();
struct Areas(core::cell::UnsafeCell<[u8; OTHERS * AREA]>);
// Safety: `prepare` writes the segment pages once, before `protect` and before any other processor
// runs; afterwards each area is used only by the processor it belongs to.
unsafe impl Sync for Areas {}
#[repr(align(4096))]
struct Aligned(Areas);
static AREAS: Aligned = Aligned(Areas(core::cell::UnsafeCell::new([0; OTHERS * AREA])));

fn area(slot: usize) -> usize {
    AREAS.0 .0.get() as usize + slot * AREA
}

fn inside(addr: usize) -> Option<usize> {
    let base = area(0);
    (addr >= base && addr < base + OTHERS * AREA).then(|| (addr - base) % AREA)
}

/// Whether `addr` is a page left unmapped below one of the other processors' stacks.
pub fn guard(addr: usize) -> bool {
    matches!(inside(addr), Some(at) if at >= 4096 && ((at - 4096) % FAULT_SLOT == 0 && at < 4096 + 3 * FAULT_SLOT || at == 4096 + 3 * FAULT_SLOT))
}

/// Whether `addr` is the page of one of their segment tables.
pub fn segments(addr: usize) -> bool {
    inside(addr) == Some(0)
}

/// Fill in every other processor's segment table and task state segment. Called once, before `protect`
/// makes those pages read-only.
pub fn prepare() {
    for slot in 0..OTHERS {
        let base = area(slot);
        // Safety: static storage used for nothing else, written here before anything reads it.
        unsafe {
            let seg = base as *mut Segments;
            let tss = core::ptr::addr_of_mut!((*seg).tss) as *mut u32;
            for n in 0..3usize {
                let top = (base + 4096 + n * FAULT_SLOT + FAULT_SLOT) as u64;
                tss.add(9 + 2 * n).write(top as u32);
                tss.add(10 + 2 * n).write((top >> 32) as u32);
            }
            let top = (base + AREA) as u64;
            tss.add(1).write(top as u32);
            tss.add(2).write((top >> 32) as u32);
            tss.add(25).write(104 << 16);
            let at = tss as u64;
            *core::ptr::addr_of_mut!((*seg).gdt) = [0, 0x00AF_9B00_0000_FFFF, 0x00CF_9300_0000_FFFF, 0x00CF_F300_0000_FFFF, 0x00AF_FB00_0000_FFFF,
                                                    103 | (at & 0xFF_FFFF) << 16 | 0x89 << 40 | (at >> 24 & 0xFF) << 56, at >> 32];
        }
    }
}

extern "C" {
    static ap_tramp_start: u8;
    static ap_tramp_end: u8;
    static ap_fix_gdt: u8;
    static ap_fix_pm: u8;
    static ap_fix_cr3: u8;
    static ap_fix_lm: u8;
    static mut ap_stack_top: u64;
}

/// The one page of the interrupt controller's registers, where they are reached through memory.
#[repr(align(4096))]
struct ControllerTable(core::cell::UnsafeCell<[u64; 512]>);
// Safety: written once by `start`, with interrupts off; afterwards only the processor reads it.
unsafe impl Sync for ControllerTable {}
static CONTROLLER_TABLE: ControllerTable = ControllerTable(core::cell::UnsafeCell::new([0; 512]));
/// The address of that page once mapped; zero before, and where the registers are reached otherwise.
static CONTROLLER: AtomicUsize = AtomicUsize::new(0);

/// Whether `addr` is the page of the interrupt controller's registers, mapped by `start`.
pub fn controller_page(addr: usize) -> bool {
    addr != 0 && CONTROLLER.load(Ordering::Relaxed) == addr
}

/// Whether [start, end) touches the 2 MiB stretch that page is in, which no device may then be mapped over.
pub fn controller_stretch(start: usize, end: usize) -> bool {
    let at = CONTROLLER.load(Ordering::Relaxed) & !(MIB2 - 1);
    at != 0 && start < at + MIB2 && at < end
}

fn rdmsr(msr: u32) -> u64 {
    let (low, high): (u32, u32);
    // Safety: the callers read only registers that every processor with the feature they checked has.
    unsafe { asm!("rdmsr", in("ecx") msr, out("eax") low, out("edx") high, options(nomem, nostack, preserves_flags)) };
    (high as u64) << 32 | low as u64
}

/// How the interrupt controller's registers are reached: at an address in memory, or (where firmware
/// left the processor in the extended mode) as registers of the processor itself.
#[derive(Clone, Copy)]
enum Reach {
    Memory(usize),
    Registers,
}

impl Reach {
    /// Send `message` to the processor whose controller is `to`, and wait until it has left.
    fn send(self, to: u32, message: u32) -> bool {
        match self {
            // Safety: the page is mapped by `start`, and these are the two words of the interrupt
            // command register.
            Reach::Memory(at) => unsafe {
                ((at + 0x310) as *mut u32).write_volatile(to << 24);
                ((at + 0x300) as *mut u32).write_volatile(message);
                for _ in 0..10_000_000u32 {
                    if ((at + 0x300) as *const u32).read_volatile() & (1 << 12) == 0 {
                        return true;
                    }
                    core::hint::spin_loop();
                }
                false
            },
            Reach::Registers => {
                wrmsr(0x830, (to as u64) << 32 | message as u64);
                true
            }
        }
    }
}

/// Who the processor running this is, and how its identity divides into a core and a thread of it:
/// the identity, how many of its low bits tell threads of one core apart, and whether that is known.
fn myself() -> (u32, u32, bool) {
    // Safety: CPUID has no memory effect.
    {
        if core::arch::x86_64::__cpuid(0).eax >= 0xB {
            let level = core::arch::x86_64::__cpuid_count(0xB, 0);
            if level.ebx != 0 && level.ecx >> 8 & 0xFF == 1 {
                return (level.edx, level.eax & 31, true);
            }
        }
        (core::arch::x86_64::__cpuid(1).ebx >> 24, 0, false)
    }
}

/// What a processor says it can do, as far as the kernel's settings depend on it. Bits that follow a
/// setting and not the processor (whether the system has turned XSAVE or protection keys on) are left out.
fn abilities() -> [u32; 7] {
    // Safety: CPUID has no memory effect.
    unsafe {
        let one = core::arch::x86_64::__cpuid(1);
        let seven = if core::arch::x86_64::__cpuid(0).eax >= 7 { core::arch::x86_64::__cpuid_count(7, 0) } else { core::mem::zeroed() };
        let wide = core::arch::x86_64::__cpuid(0x8000_0001);
        [one.ecx & !(1 << 27), one.edx, seven.ebx, seven.ecx & !(1 << 4), seven.edx, wide.ecx, wide.edx]
    }
}

/// How a processor is told to treat each range of memory (cached, uncached, write-combining): the
/// registers that say so, with their values, and how many there are. Every processor of a machine
/// must hold the same. Firmware sets them on all, and on a real processor a start-up message leaves
/// them alone; an emulator's processor can come back with them cleared, and firmware is not always
/// right, which is why the usual practice, followed here, is that each processor started is given
/// the first one's when its own differ.
const TYPE_REGISTERS: usize = 2 * 32 + 11 + 2;
type Types = ([(u32, u64); TYPE_REGISTERS], usize);

fn memory_types() -> Types {
    let features = core::arch::x86_64::__cpuid(1).edx;
    let mut out: Types = ([(0, 0); TYPE_REGISTERS], 0);
    let mut keep = |register: u32| {
        out.0[out.1] = (register, rdmsr(register));
        out.1 += 1;
    };
    if features & (1 << 12) != 0 {
        let has = rdmsr(0xFE);
        for n in 0..(has & 0xFF).min(32) as u32 {
            keep(0x200 + 2 * n);
            keep(0x201 + 2 * n);
        }
        if has & (1 << 8) != 0 {
            for register in [0x250, 0x258, 0x259, 0x268, 0x269, 0x26A, 0x26B, 0x26C, 0x26D, 0x26E, 0x26F] {
                keep(register);
            }
        }
        keep(0x2FF); // which kind is the default, and whether the registers above are in force: written last
    }
    if features & (1 << 16) != 0 {
        keep(0x277);
    }
    out
}

/// Give this processor the first one's memory types, by the procedure the processor makers give for
/// changing them: caches put in no-fill mode and emptied, translations forgotten, the registers
/// turned off, written, and turned on again by the last write, caches and translations emptied again.
/// The caller has interrupts off and restores CR0 afterwards.
///
/// # Safety
/// The processor must be one whose registers are the same kind and number as the first's (its
/// abilities equal), running nothing else, and `types` what the first processor holds.
unsafe fn take_memory_types(types: &Types) {
    let mut cr0: u64;
    asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    cr0 = (cr0 | 1 << 30) & !(1 << 29);
    asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));
    asm!("wbinvd", "mov {t}, cr3", "mov cr3, {t}", t = out(reg) _, options(nostack, preserves_flags));
    if types.0[..types.1].iter().any(|(register, _)| *register == 0x2FF) {
        wrmsr(0x2FF, rdmsr(0x2FF) & !(1 << 11));
    }
    for &(register, value) in &types.0[..types.1] {
        if register != 0x2FF {
            wrmsr(register, value);
        }
    }
    if let Some(&(_, value)) = types.0[..types.1].iter().find(|(register, _)| *register == 0x2FF) {
        wrmsr(0x2FF, value);
    }
    asm!("wbinvd", "mov {t}, cr3", "mov cr3, {t}", t = out(reg) _, options(nostack, preserves_flags));
}

/// What the first processor is set to, for the others to be set the same.
static FIRST_TYPES: Single<Types> = Single::new(([(0, 0); TYPE_REGISTERS], 0));
static FIRST_CR0: AtomicU64 = AtomicU64::new(0);
static FIRST_CR4: AtomicU64 = AtomicU64::new(0);
static FIRST_ABILITIES: Single<[u32; 7]> = Single::new([0; 7]);
/// Which slot the processor being started takes, and who it should say it is.
static SLOT: AtomicUsize = AtomicUsize::new(0);
static EXPECTED: AtomicU32 = AtomicU32::new(0);
/// How many have answered, and what each said: its identity, the thread bits, and what was wrong if anything.
static ANSWERED: AtomicUsize = AtomicUsize::new(0);
const UNKNOWN_THREADS: u64 = 1 << 40;
const NOT_ITSELF: u64 = 1 << 41;
const DIFFERENT: u64 = 1 << 42;
const GIVEN_TYPES: u64 = 1 << 43;
static SAID: [AtomicU64; OTHERS] = [const { AtomicU64::new(0) }; OTHERS];

/// Where another processor arrives from `ap_entry` in others.s: in 64-bit mode on the kernel's map, on
/// its own stack, interrupts off, with the stub's segments. It never returns and never runs anything else.
#[no_mangle]
extern "C" fn ap_main() -> ! {
    let slot = SLOT.load(Ordering::SeqCst);
    let base = area(slot);
    let (who, threads, known) = myself();
    let mut said = who as u64 | (threads as u64) << 32 | if known { 0 } else { UNKNOWN_THREADS };
    if who != EXPECTED.load(Ordering::SeqCst) {
        said |= NOT_ITSELF;
    }
    // The first processor wrote these before any other was started and does not write them again.
    let mut same = FIRST_ABILITIES.with(|first| *first == abilities());
    if same && FIRST_TYPES.with(|first| *first != memory_types()) {
        // Safety: its abilities equal the first's, it runs nothing else, and interrupts are off.
        unsafe { FIRST_TYPES.with(|first| take_memory_types(first)) };
        said |= GIVEN_TYPES;
        same = FIRST_TYPES.with(|first| *first == memory_types());
    }
    // Safety: the segment table and task state segment are this processor's own, filled by `prepare`;
    // loading the task register writes one bit into that read-only page, which is why write
    // protection is not yet on; the interrupt table is the one every processor shares, whose three
    // special stacks are named by each processor's own task state segment. The control registers
    // are set to the first processor's values only if this one says it can do everything that one can.
    unsafe {
        let g = TablePointer { limit: 55, base: base as u64 };
        let p = TablePointer { limit: (core::mem::size_of::<[Gate; 256]>() - 1) as u16, base: IDT.0.get() as u64 };
        asm!("lgdt [{g}]",
             "push 0x08",
             "lea {t}, [rip + 2f]",
             "push {t}",
             "retfq",
             "2:",
             "mov ds, {d:x}",
             "mov es, {d:x}",
             "mov ss, {d:x}",
             "ltr {r:x}",
             "lidt [{p}]",
             g = in(reg) &g, p = in(reg) &p, t = out(reg) _, d = in(reg) 0x10u16, r = in(reg) 0x28u16);
        if same {
            asm!("mov cr4, {}", in(reg) FIRST_CR4.load(Ordering::SeqCst), options(nomem, nostack, preserves_flags));
            if wide_vectors() {
                asm!("xsetbv", in("ecx") 0u32, in("eax") XSAVE_KEPT, in("edx") 0u32, options(nomem, nostack, preserves_flags));
            }
            asm!("mov cr0, {}", in(reg) FIRST_CR0.load(Ordering::SeqCst), options(nomem, nostack, preserves_flags));
        } else {
            said |= DIFFERENT;
            let mut cr0: u64;
            asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
            cr0 |= 1 << 16; // write protection at the least
            asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));
        }
        // Forget every translation made on the way here, the start-up page's among them.
        asm!("mov {t}, cr3", "mov cr3, {t}", t = out(reg) _, options(nostack, preserves_flags));
    }
    SAID[slot].store(said, Ordering::SeqCst);
    extern "C" {
        fn ap_idle() -> !;
    }
    if said & (NOT_ITSELF | DIFFERENT) != 0 {
        ANSWERED.fetch_add(1, Ordering::SeqCst);
        loop {
            // Safety: no memory effect. A processor that is not as the first is left halted with
            // interrupts off and is asked nothing: only a non-maskable interrupt or a reset ends the
            // halt, and the first is handled on this processor's own stack and comes back here.
            unsafe { asm!("cli", "hlt", options(nomem, nostack)) }
        }
    }
    listen();
    ANSWERED.fetch_add(1, Ordering::SeqCst);
    // Safety: the loop in others.s, which allows interrupts and halts; it does not return. This
    // processor does not hold the kernel's lock here and touches nothing shared; a message takes
    // the lock on its way in like any other entry.
    unsafe { ap_idle() }
}

// ---- one lock around the kernel -------------------------------------------------------------------
// The kernel is written for one processor: its shared values rely on nothing else running while
// interrupts are off. That stays true with several processors because of one rule: a processor runs
// the kernel's code only while it holds this lock. It is taken on the way in from outside (a program,
// or idling) and given up on the way back out; an interrupt that arrives while the kernel is already
// running on that processor finds it held and changes nothing.

/// Zero when free, otherwise one more than the number of the processor that holds it. The first
/// processor holds it from the start: it begins in the kernel.
static LOCK: AtomicUsize = AtomicUsize::new(1);
/// Whether each processor holds it. Written only by that processor, with interrupts off.
static FIRST_HOLDS: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(true);
static OTHER_HOLDS: [core::sync::atomic::AtomicBool; OTHERS] = [const { core::sync::atomic::AtomicBool::new(false) }; OTHERS];

/// Which processor this is: 0 for the first, otherwise one more than its slot. Told by the stack it
/// is on, since each other processor's stacks lie in its own area and nothing a program can set is asked.
pub fn cpu() -> usize {
    let at: usize;
    // Safety: reads the stack pointer.
    unsafe { asm!("mov {}, rsp", out(reg) at, options(nomem, nostack, preserves_flags)) };
    let base = area(0);
    if at > base && at <= base + OTHERS * AREA { (at - 1 - base) / AREA + 1 } else { 0 }
}

fn holds(cpu: usize) -> &'static core::sync::atomic::AtomicBool {
    if cpu == 0 { &FIRST_HOLDS } else { &OTHER_HOLDS[cpu - 1] }
}

fn acquire(cpu: usize) {
    #[cfg(feature = "processors-unlocked-demo")]
    if DEMO_UNLOCKED.load(Ordering::Relaxed) {
        return; // the control: nothing is waited for, so two processors can be in at once
    }
    while LOCK.compare_exchange_weak(0, cpu + 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }
}

fn release() {
    LOCK.store(0, Ordering::Release);
}

#[cfg(feature = "processors-unlocked-demo")]
static DEMO_UNLOCKED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// What `enter` found, for `leave`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Entered {
    /// The processor already held the lock: the kernel was interrupted in its own work.
    Held,
    /// Taken on the way in from outside.
    FromOutside,
    /// Taken on the way in from kernel code that had given it up for a moment (`relax`).
    FromRelaxed,
}

/// On every entry to the kernel, before anything else. Interrupts are off.
pub fn enter(from_outside: bool) -> Entered {
    let me = cpu();
    if holds(me).load(Ordering::Relaxed) {
        return Entered::Held;
    }
    acquire(me);
    holds(me).store(true, Ordering::Relaxed);
    if from_outside { Entered::FromOutside } else { Entered::FromRelaxed }
}

/// On every way out, after everything else. `to_outside` is where the entry returns to, which is not
/// always where it came from: the kernel may have switched to a program, to idling, or back to itself.
pub fn leave(entered: Entered, to_outside: bool) {
    if to_outside || entered == Entered::FromRelaxed {
        let me = cpu();
        holds(me).store(false, Ordering::Relaxed);
        release();
    }
}

/// Give the lock up for a moment so that another processor can have it, and take it again. For
/// kernel code that waits for another processor.
pub fn relax() {
    let on = interrupts_save_off();
    let me = cpu();
    holds(me).store(false, Ordering::Relaxed);
    release();
    core::hint::spin_loop();
    acquire(me);
    holds(me).store(true, Ordering::Relaxed);
    interrupts_restore(on);
}

// ---- a message from one processor to another -----------------------------------------------------

/// How the controller is reached, for the processors started: 0 not yet, 1 as registers, else the address.
static REACH: AtomicUsize = AtomicUsize::new(0);
/// How many others are parked, and each one's identity.
static PARKED: AtomicUsize = AtomicUsize::new(0);
static IDENTITY: [AtomicU32; OTHERS] = [const { AtomicU32::new(0) }; OTHERS];
/// What each is asked to do next (a function's address, or zero), and how many it has done.
static ASKED: [AtomicUsize; OTHERS] = [const { AtomicUsize::new(0) }; OTHERS];
static DONE: [AtomicU64; OTHERS] = [const { AtomicU64::new(0) }; OTHERS];

fn reach() -> Option<Reach> {
    match REACH.load(Ordering::SeqCst) {
        0 => None,
        1 => Some(Reach::Registers),
        at => Some(Reach::Memory(at)),
    }
}

/// The entry in others.s for vector 240.
pub fn message_stub() -> u64 {
    extern "C" {
        fn isr_message();
    }
    isr_message as usize as u64
}

/// Whether `at` is in the loop another processor idles in.
pub fn idling(at: usize) -> bool {
    extern "C" {
        fn ap_idle() -> !;
        fn ap_idle_end();
    }
    at >= ap_idle as usize && at < ap_idle_end as usize
}

/// Turn this processor's interrupt controller on, so that a message can reach it. Nothing else is
/// let through: its local lines stay masked as a restart left them.
fn listen() {
    match reach() {
        // Safety: the page is mapped by `start`; 0xF0 is the register that turns the controller on
        // and names the vector for an interrupt with nothing behind it (255, which has a handler).
        Some(Reach::Memory(at)) => unsafe { ((at + 0xF0) as *mut u32).write_volatile(0x1FF) },
        Some(Reach::Registers) => wrmsr(0x80F, 0x1FF),
        None => {}
    }
}

/// A message arrived on this processor (vector 240): say it was taken, and do what was asked.
pub fn message() {
    match reach() {
        // Safety: as above; 0xB0 is the register that ends an interrupt.
        Some(Reach::Memory(at)) => unsafe { ((at + 0xB0) as *mut u32).write_volatile(0) },
        Some(Reach::Registers) => wrmsr(0x80B, 0),
        None => {}
    }
    let me = cpu();
    if me == 0 {
        return;
    }
    let asked = ASKED[me - 1].swap(0, Ordering::SeqCst);
    if asked != 0 {
        // Safety: only `ask` stores here, and only the address of a `fn(usize)`.
        let work: fn(usize) = unsafe { core::mem::transmute(asked) };
        work(me);
        DONE[me - 1].fetch_add(1, Ordering::SeqCst);
    }
}

/// Ask a parked processor to run `work` in the kernel, under the lock like all kernel code. False if
/// there is no such processor, it still has something asked of it, or the message could not be sent.
pub fn ask(slot: usize, work: fn(usize)) -> bool {
    if slot >= PARKED.load(Ordering::SeqCst) || ASKED[slot].compare_exchange(0, work as usize, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return false;
    }
    let on = interrupts_save_off();
    let sent = reach().is_some_and(|r| r.send(IDENTITY[slot].load(Ordering::SeqCst), 240));
    interrupts_restore(on);
    sent
}

/// The check of the lock: every processor at once adds one to the same count, `rounds` times each,
/// giving the lock up and taking it again around every addition. The count is an ordinary value, read
/// and written back with a pause between, so two processors in the kernel together would lose
/// additions. Returns (the count, what it must be, whether every processor finished in time).
/// Interrupts must be on: time is told by the timer.
pub fn count_together(rounds: u64) -> (u64, u64, bool) {
    static COUNT: Single<u64> = Single::new(0);
    static ROUNDS: AtomicU64 = AtomicU64::new(0);
    fn add(_: usize) {
        for _ in 0..ROUNDS.load(Ordering::SeqCst) {
            relax();
            COUNT.with(|count| {
                // Safety: a plain value read and written through its own address.
                let now = unsafe { core::ptr::read_volatile(count) };
                for _ in 0..8 {
                    core::hint::spin_loop();
                }
                unsafe { core::ptr::write_volatile(count, now + 1) };
            });
        }
    }
    let parked = PARKED.load(Ordering::SeqCst);
    COUNT.with(|count| *count = 0);
    ROUNDS.store(rounds, Ordering::SeqCst);
    let before: alloc::vec::Vec<u64> = DONE[..parked].iter().map(|d| d.load(Ordering::SeqCst)).collect();
    #[cfg(feature = "processors-unlocked-demo")]
    DEMO_UNLOCKED.store(true, Ordering::SeqCst);
    let mut asked = true;
    for slot in 0..parked {
        asked &= ask(slot, add);
    }
    add(0);
    let until = ticks() + 20 * TICKS_PER_SECOND;
    let finished = |slot: usize| DONE[slot].load(Ordering::SeqCst) > before[slot];
    while !(0..parked).all(finished) && ticks() < until {
        relax();
    }
    let all = asked && (0..parked).all(finished);
    #[cfg(feature = "processors-unlocked-demo")]
    DEMO_UNLOCKED.store(false, Ordering::SeqCst);
    (COUNT.with(|count| *count), (parked as u64 + 1) * rounds, all)
}

/// How starting the others went.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Started {
    /// How many others the firmware lists (beyond the one the kernel runs on) that this kernel would start.
    pub listed: usize,
    /// How many answered, in 64-bit mode on their own stacks, and are parked.
    pub parked: usize,
    /// Of those, how many said they were someone else than the one addressed, and how many cannot do
    /// what the first can, or still hold other memory types than it after being given its (those are left with write protection and nothing more).
    pub not_itself: usize,
    pub different: usize,
    /// How many came up holding other memory types than the first and were given the first's.
    pub given_types: usize,
    /// How many were not tried because their identity needs the extended mode and the machine is not in it.
    pub out_of_reach: usize,
    /// Whether one did not answer (it was returned to waiting, and no further one was tried).
    pub silent: bool,
    /// Threads in all, the first included, and the cores they are on, where every processor could say.
    pub threads: usize,
    pub cores: Option<usize>,
    /// Whether the controller's registers are reached through memory (one page is then mapped for them)
    /// or, in the extended mode, as registers of the processor.
    pub through_memory: bool,
}

fn small_entry(page: usize) -> *mut u64 {
    // Safety of the callers' writes: the first small table of TABLES covers the first 2 MiB.
    unsafe { (TABLES.0.get() as *mut [u64; 512]).add(6) as *mut u64 }.wrapping_add(page >> 12)
}

fn forget(addr: usize) {
    // Safety: drops one translation from this processor's cache; no memory effect.
    unsafe { asm!("invlpg [{}]", in(reg) addr, options(nostack, preserves_flags)) }
}

/// Start every other processor the firmware lists and park it. Interrupts must be on (time is told by
/// the timer) and no program may exist yet. `page` is a page below one mebibyte that the loader's map
/// gives as free: it holds the start-up instructions while this runs, and is cleared and unmapped
/// again before this returns. A refusal names its reason and changes nothing.
pub fn start(found: &processors::Found, page: Option<u64>) -> Result<Started, &'static str> {
    if core::arch::x86_64::__cpuid(1).edx & (1 << 9) == 0 {
        return Err("this processor has no interrupt controller of its own");
    }
    // Some firmware hands the machine over with the controller in its extended mode. No emulator's
    // firmware here does, so this demonstration puts it there, to run the way such a machine is reached.
    #[cfg(feature = "processors-extended-demo")]
    if emulated() && core::arch::x86_64::__cpuid(1).ecx & (1 << 21) != 0 && rdmsr(0x1B) & (3 << 10) == 1 << 11 {
        wrmsr(0x1B, rdmsr(0x1B) | 1 << 10);
    }
    let base = rdmsr(0x1B);
    if base & (1 << 11) == 0 {
        return Err("the processor's interrupt controller is turned off");
    }
    let extended = base & (1 << 10) != 0;
    // The way of reaching the controller in its extended mode has been run on no machine and no
    // emulator yet (the one used here does not offer the mode). Until it has, it is not used.
    #[cfg(not(feature = "processors-extended"))]
    if extended {
        return Err("firmware left the interrupt controller in its extended mode, which this kernel has not yet been proven on");
    }
    if !extended && base & ADDRESS != found.controller {
        return Err("the interrupt controller is not where the firmware's table says");
    }
    if found.controller >= 4 << 30 || found.controller < 1 << 30 {
        return Err("the interrupt controller's registers lie where this kernel maps no device");
    }
    let (me, my_threads, my_known) = myself();
    if !found.processors.iter().any(|p| p.apic == me) {
        return Err("the firmware's table does not list the processor the kernel runs on");
    }
    let Some(page) = page.map(|p| p as usize).filter(|p| *p >= 4096 && *p < 0xA_0000 && *p % 4096 == 0) else {
        return Err("the loader's map gives no free page below one mebibyte to start them from");
    };
    // Safety: the symbols are the bounds and correction points of the stub in others.s.
    let (from, length, fixes, root) = {
        let from = core::ptr::addr_of!(ap_tramp_start) as usize;
        let at = |s: *const u8| s as usize - from;
        (from, core::ptr::addr_of!(ap_tramp_end) as usize - from,
         [at(core::ptr::addr_of!(ap_fix_gdt)), at(core::ptr::addr_of!(ap_fix_pm)), at(core::ptr::addr_of!(ap_fix_lm)), at(core::ptr::addr_of!(ap_fix_cr3))],
         TABLES.0.get() as usize)
    };
    if length > 4096 || root >= 4 << 30 {
        return Err("the start-up instructions or the kernel's map do not fit where another processor begins");
    }
    #[allow(unused_mut)]
    let mut others: alloc::vec::Vec<processors::Processor> = found.processors.iter().copied().filter(|p| p.apic != me).take(OTHERS).collect();
    #[cfg(feature = "processors-silent-demo")]
    if others.len() < OTHERS {
        let nobody = found.processors.iter().map(|p| p.apic).max().unwrap_or(0) + 1;
        others.push(processors::Processor { apic: nobody, extended: false });
    }
    let mut out = Started { listed: others.len(), through_memory: !extended, ..Started::default() };
    if others.is_empty() {
        out.threads = 1;
        out.cores = my_known.then_some(1);
        return Ok(out);
    }
    let on = interrupts_save_off();
    // The controller's registers, where they are reached through memory: one page, writable, never
    // executable, not cached, in a 2 MiB stretch nothing else is mapped in.
    let reach = if extended {
        Reach::Registers
    } else {
        let at = found.controller as usize;
        // Safety: the third-level tables of gibibytes one to three are static storage of TABLES; the
        // entry written was absent, so no translation in use changes.
        let free = unsafe {
            let entry = ((TABLES.0.get() as *mut [u64; 512]).add(2 + (at >> 30)) as *mut u64).add((at >> 21) & 511);
            if at % 4096 != 0 || *entry != 0 {
                false
            } else {
                (*CONTROLLER_TABLE.0.get())[(at >> 12) & 511] = at as u64 | P | W | NX | 0x18;
                *entry = CONTROLLER_TABLE.0.get() as u64 | P | W;
                CONTROLLER.store(at, Ordering::SeqCst);
                true
            }
        };
        if !free {
            interrupts_restore(on);
            return Err("the interrupt controller's registers lie in a stretch already mapped for a device");
        }
        Reach::Memory(at)
    };
    REACH.store(match reach { Reach::Registers => 1, Reach::Memory(at) => at }, Ordering::SeqCst);
    // The start-up page: mapped writable to be filled, then executable and not writable while it is used.
    // Safety: the page is below the image, in memory the loader's map gives as free and the kernel
    // uses for nothing; the map entry is in static storage and was absent.
    unsafe {
        let mut cr: u64;
        asm!("mov {}, cr0", out(reg) cr, options(nomem, nostack, preserves_flags));
        FIRST_CR0.store(cr, Ordering::SeqCst);
        asm!("mov {}, cr4", out(reg) cr, options(nomem, nostack, preserves_flags));
        FIRST_CR4.store(cr, Ordering::SeqCst);
        FIRST_ABILITIES.with(|a| *a = abilities());
        FIRST_TYPES.with(|t| *t = memory_types());
        *small_entry(page) = page as u64 | P | W | NX;
        forget(page);
        core::ptr::write_bytes(page as *mut u8, 0, 4096);
        core::ptr::copy_nonoverlapping(from as *const u8, page as *mut u8, length);
        for (n, fix) in fixes.iter().enumerate() {
            let word = (page + fix) as *mut u32;
            word.write_unaligned(if n == 3 { root as u32 } else { word.read_unaligned() + page as u32 });
        }
        *small_entry(page) = page as u64 | P;
        forget(page);
    }
    interrupts_restore(on);

    const INIT: u32 = 0xC500;
    const INIT_RELEASED: u32 = 0x8500;
    const START: u32 = 0x4600;
    let answered = |from: usize, within: u64| {
        let until = ticks() + within;
        while ANSWERED.load(Ordering::SeqCst) == from && ticks() < until {
            wait_for_interrupt();
        }
        ANSWERED.load(Ordering::SeqCst) != from
    };
    let mut slot = 0;
    for other in &others {
        if other.extended && !extended {
            out.out_of_reach += 1;
            continue;
        }
        let before = ANSWERED.load(Ordering::SeqCst);
        SLOT.store(slot, Ordering::SeqCst);
        EXPECTED.store(other.apic, Ordering::SeqCst);
        // Safety: the word is read by the processor being started and by no one else; none is running the stub now.
        unsafe { core::ptr::addr_of_mut!(ap_stack_top).write_volatile((area(slot) + AREA) as u64) };
        let sent = reach.send(other.apic, INIT) && reach.send(other.apic, INIT_RELEASED) && {
            wait_until(ticks() + 11); // ten milliseconds, and the tick already under way
            reach.send(other.apic, START | (page >> 12) as u32)
        };
        let up = sent && (answered(before, 2) || (reach.send(other.apic, START | (page >> 12) as u32) && answered(before, 1000)));
        if !up {
            // Back to waiting, so that it can never begin later on a page that is no longer there.
            reach.send(other.apic, INIT);
            reach.send(other.apic, INIT_RELEASED);
            wait_until(ticks() + 11);
            out.silent = true;
            break;
        }
        IDENTITY[slot].store(other.apic, Ordering::SeqCst);
        slot += 1;
    }
    out.parked = slot;

    let on = interrupts_save_off();
    // Safety: every processor that used the page has left it and forgotten it (each reloads its map
    // before it answers), and one that did not answer was returned to waiting above.
    unsafe {
        *small_entry(page) = page as u64 | P | W | NX;
        forget(page);
        core::ptr::write_bytes(page as *mut u8, 0, 4096);
        *small_entry(page) = 0;
        forget(page);
    }
    interrupts_restore(on);

    let mut cores: alloc::vec::Vec<u32> = alloc::vec![me >> my_threads];
    let mut known = my_known;
    for said in SAID[..slot].iter().map(|s| s.load(Ordering::SeqCst)) {
        out.not_itself += (said & NOT_ITSELF != 0) as usize;
        out.different += (said & DIFFERENT != 0) as usize;
        out.given_types += (said & GIVEN_TYPES != 0) as usize;
        known &= said & UNKNOWN_THREADS == 0;
        let core = (said as u32) >> (said >> 32 & 31);
        if !cores.contains(&core) {
            cores.push(core);
        }
    }
    // Only if every one is as the first may any be asked to do something.
    if out.not_itself == 0 && out.different == 0 {
        PARKED.store(slot, Ordering::SeqCst);
    }
    out.threads = 1 + slot;
    out.cores = known.then_some(cores.len());
    Ok(out)
}
