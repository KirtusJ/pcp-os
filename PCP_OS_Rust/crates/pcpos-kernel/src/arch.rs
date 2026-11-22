//! Everything that touches the machine directly. This is the only module of the kernel in which `unsafe`
//! is allowed; the rest of the kernel is written against the safe functions it exports.
#![allow(unsafe_code)]
use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

core::arch::global_asm!(include_str!("boot.s"), options(att_syntax));

fn outb(port: u16, value: u8) {
    // Safety: writing a byte to an I/O port has no memory effect; the ports used are the serial port and the exit device.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) }
}

fn inb(port: u16) -> u8 {
    let value: u8;
    // Safety: reading a byte from an I/O port has no memory effect.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags)) }
    value
}

/// Read a byte from a device port.
/// # Safety
/// The caller must own the device behind the port: a port write can start a transfer that changes memory.
pub unsafe fn port_in8(port: u16) -> u8 {
    inb(port)
}
/// Write a byte to a device port. Safety: as [`port_in8`].
pub unsafe fn port_out8(port: u16, value: u8) {
    outb(port, value)
}
/// Read a 16-bit word from a device port. Safety: as [`port_in8`].
pub unsafe fn port_in16(port: u16) -> u16 {
    let value: u16;
    asm!("in ax, dx", in("dx") port, out("ax") value, options(nomem, nostack, preserves_flags));
    value
}
/// Write a 16-bit word to a device port. Safety: as [`port_in8`].
pub unsafe fn port_out16(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack, preserves_flags))
}
/// Read a 32-bit word from a device port. Safety: as [`port_in8`].
pub unsafe fn port_in32(port: u16) -> u32 {
    let value: u32;
    asm!("in eax, dx", in("dx") port, out("eax") value, options(nomem, nostack, preserves_flags));
    value
}
/// Write a 32-bit word to a device port. Safety: as [`port_in8`].
pub unsafe fn port_out32(port: u16, value: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack, preserves_flags))
}

// This optional polling device uses COM2 exclusively. COM1 remains the verification console.
#[cfg(feature = "game-link")]
const GAME_UART: u16 = 0x2f8;

#[cfg(feature = "game-link")]
pub fn game_link_init() -> bool {
    let p = GAME_UART;
    // A scratch register readback detects an absent UART without touching its peer.
    outb(p + 7, 0x5a);
    if inb(p + 7) != 0x5a { return false; }
    outb(p + 1, 0); outb(p + 3, 0x80);
    outb(p, 1); outb(p + 1, 0); outb(p + 3, 3);
    outb(p + 2, 7); outb(p + 4, 3);
    true
}
#[cfg(feature = "game-link")]
pub fn game_link_read() -> Option<u8> {
    (inb(GAME_UART + 5) & 1 != 0).then(|| inb(GAME_UART))
}
#[cfg(feature = "game-link")]
pub fn game_link_write(byte: u8) -> bool {
    if inb(GAME_UART + 5) & 0x20 == 0 { return false; }
    outb(GAME_UART, byte); true
}

const COM1: u16 = 0x3F8;

/// Whether a serial port answered at the usual address and is still taking bytes.
static SERIAL: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Prepare the first serial port, if there is one: 115200 baud, 8 bits, no parity, one stop bit.
///
/// Most laptops have no such port. Nothing is known about what answers at its address before it is
/// tried, so it is tried in the way that changes least: first by reading (a line-status register that
/// reads all ones is no port), then by writing two values to its scratch register, which exists to hold
/// a byte and does nothing else, and reading each back. Only a port that kept both is set up and written
/// to. Where there is none, the report is not written anywhere and nothing waits for it.
pub fn serial_init() {
    use core::sync::atomic::Ordering;
    if inb(COM1 + 5) == 0xFF {
        return;
    }
    for value in [0x5Au8, 0xA5] {
        outb(COM1 + 7, value);
        if inb(COM1 + 7) != value {
            return;
        }
    }
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80);
    outb(COM1, 0x01);
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);
    outb(COM1 + 2, 0xC7);
    outb(COM1 + 4, 0x0B);
    SERIAL.store(true, Ordering::SeqCst);
}

/// Whether the report is being written to a serial port.
pub fn serial_present() -> bool {
    SERIAL.load(core::sync::atomic::Ordering::SeqCst)
}

/// Write one byte to the serial port, waiting until the port can take it.
pub fn serial_put(byte: u8) {
    use core::sync::atomic::Ordering;
    if !SERIAL.load(Ordering::SeqCst) {
        return;
    }
    // Bounded: a port that never becomes ready costs the rest of the report, not the machine. It is
    // not written to, and not waited for, again.
    for _ in 0..1_000_000u32 {
        let status = inb(COM1 + 5);
        // A status of all ones is no port, here as at the start: a port that was there and has gone
        // is not written to (B3).
        if status == 0xFF {
            break;
        }
        if status & 0x20 != 0 {
            outb(COM1, byte);
            return;
        }
    }
    SERIAL.store(false, Ordering::SeqCst);
}

/// Whether the processor says it is run by an emulator or a hypervisor (CPUID leaf 1, ECX bit 31). Hardware
/// never sets this bit; QEMU, Bochs and VirtualBox do unless told not to.
pub fn emulated() -> bool {
    // Safety: CPUID leaf 1 exists on every 64-bit processor and reading it changes nothing.
    unsafe { core::arch::x86_64::__cpuid(1).ecx & (1 << 31) != 0 }
}

/// Stop the machine. Under an emulator with the exit device this ends the emulator with a status, and
/// otherwise its power device is asked to switch off. On hardware none of those ports is written: they are
/// not a power interface there and what answers at them is not known, so the processor is only halted.
pub fn stop(ok: bool) -> ! {
    if emulated() {
        outb(0xf4, if ok { 0x10 } else { 0x11 });
        // Safety: these three ports are the ones QEMU, Bochs and VirtualBox answer for power, and the
        // processor has said it is emulated.
        unsafe {
            port_out16(0x604, 0x2000);
            port_out16(0xB004, 0x2000);
            port_out16(0x4004, 0x3400);
        }
    }
    loop {
        // Safety: halting the processor with interrupts disabled touches no memory.
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) }
    }
}

/// A file the loader placed in memory beside the kernel: where it lies and the name it was given.
#[derive(Clone, Debug)]
pub struct Module {
    pub start: usize,
    pub end: usize,
    pub name: alloc::string::String,
    /// What followed the name on the line the loader was given: arguments for it, if it is a program.
    pub arguments: alloc::string::String,
    /// False if it lies where the kernel cannot map it read-only; such a module is reported and not used.
    pub usable: bool,
}

/// A display mode the loader had already set, if it said so.
#[derive(Clone, Copy, Debug)]
pub struct LoaderDisplay {
    pub address: usize,
    pub pitch: u32,
    pub width: u32,
    pub height: u32,
    pub bits: u8,
}

mod boot_text;
mod frame_map;
mod framebuffer;
pub mod processors;
#[cfg(feature = "processors")]
pub mod others;

/// Everything the loader said, copied out of its own structures while they can still be read.
#[derive(Clone, Debug, Default)]
pub struct BootInfo {
    /// Memory below and above one mebibyte, in kibibytes.
    pub memory: Option<(u32, u32)>,
    /// The complete Multiboot memory map. An advertised but unreadable map stays empty and yields no pool.
    memory_map: Option<alloc::vec::Vec<u8>>,
    incomplete_modules: bool,
    pub command_line: alloc::string::String,
    command_line_exact: bool,
    pub modules: alloc::vec::Vec<Module>,
    pub display: Option<LoaderDisplay>,
    /// The processors the firmware's tables list, if they could be read and add up.
    pub processors: Option<processors::Found>,
    /// Where the tables were looked for, and if nothing came of it, why.
    pub processors_note: &'static str,
    /// Where the root pointer was taken to be, if anywhere.
    pub processors_root: Option<u64>,
}

impl BootInfo {
    /// A page below one mebibyte that the loader's map gives as free, to start another processor from.
    #[cfg(feature = "processors")]
    pub fn start_page(&self) -> Option<u64> {
        self.memory_map.as_deref().and_then(processors::low_free_page)
    }

    #[cfg(any(feature = "guest-ethernet", feature = "vm-store", feature = "proposals", feature = "companion", feature = "model-memory"))]
    pub(crate) fn exact_command_line(&self) -> Option<&str> {
        self.command_line_exact.then_some(self.command_line.as_str())
    }
}

/// Read what a Multiboot loader handed over. Called once, before [`protect`], while the first gibibyte is
/// still mapped as the loader-time map left it. Every address is checked to lie inside that gibibyte, every
/// count and length is bounded, and everything is copied: nothing of the loader's is referred to afterwards.
pub fn boot_info(magic: u32, info: u32) -> BootInfo {
    let mut out = BootInfo::default();
    const LIMIT: usize = 1 << 30;
    if magic != 0x2BADB002 || info == 0 || info as usize > LIMIT - 120 {
        return out; // not a Multiboot loader, or a structure outside what is mapped at this point
    }
    // Safety: a Multiboot loader passes the address of its information structure, which lies in the
    // identity-mapped first gibibyte; each word read is defined by the Multiboot specification, and every
    // address taken from it is checked against the mapped gibibyte before it is followed.
    unsafe {
        let word = |at: usize| ((info as usize + at) as *const u32).read_unaligned();
        let text = |addr: usize, most: usize| boot_text::read(most, |i| {
            let at = addr.checked_add(i)?;
            (addr != 0 && at < LIMIT).then(|| (at as *const u8).read())
        });
        let flags = word(0);
        if flags & 1 != 0 {
            out.memory = Some((word(4), word(8)));
        }
        if flags & 4 != 0 {
            (out.command_line, out.command_line_exact) = text(word(16) as usize, boot_text::LINE);
        } else {
            out.command_line_exact = true; // The loader supplied no arguments.
        }
        if flags & 8 != 0 {
            let (count, at) = (word(20) as usize, word(24) as usize);
            out.incomplete_modules = count > 16 || (count > 0 && at == 0);
            for n in 0..count.min(16) {
                let e = at + n * 16;
                if at == 0 || e + 16 > LIMIT {
                    out.incomplete_modules = true;
                    break;
                }
                let r = |o: usize| ((e + o) as *const u32).read_unaligned() as usize;
                let (start, end) = (r(0), r(4));
                let (full, _) = text(r(8), boot_text::NAME);
                // The loader gives the path it was told; a program is known here by its last component.
                let (first, rest) = match full.split_once(' ') {
                    Some((f, r)) => (f, r.trim()),
                    None => (full.as_str(), ""),
                };
                let name = first.rsplit('/').next().unwrap_or("").into();
                out.modules.push(Module { start, end, name, arguments: rest.into(), usable: false });
            }
        }
        if flags & 0x40 != 0 {
            let (len, at) = (word(44) as usize, word(48) as usize);
            let data = if at != 0 && at < LIMIT && len <= LIMIT - at && len <= frame_map::MAX_MAP_BYTES {
                // The complete interval is inside the bootstrap identity mapping. Copy before protect replaces it.
                core::slice::from_raw_parts(at as *const u8, len).to_vec()
            } else {
                alloc::vec::Vec::new()
            };
            out.memory_map = Some(data);
        }
        if flags & 0x1000 != 0 {
            let addr = ((info as usize + 88) as *const u64).read_unaligned();
            let (pitch, width, height) = (word(96), word(100), word(104));
            let bits = ((info as usize + 108) as *const u8).read();
            let kind = ((info as usize + 109) as *const u8).read();
            // Where it lies is judged when it is mapped (`map_loader_display`), with the reason said if refused.
            if kind == 1 && addr != 0 {
                out.display = Some(LoaderDisplay { address: addr as usize, pitch, width, height, bits });
            }
        }
        // The firmware's tables of processors. They are read here, while the loader-time map still
        // reaches the first four gibibytes; only memory is read, and everything is copied.
        // Where the root pointer is said to be decides which memory a table may be read from. One the
        // loader named (a UEFI start) is believed only in what the map gives as the firmware's own
        // tables. One looked for where a BIOS puts it lies in memory such firmware calls reserved, so
        // there the rule is that nothing is read from memory the map gives as free.
        let named = if out.command_line_exact { processors::named_root(&out.command_line) } else { Err(()) };
        let by_loader = matches!(named, Ok(Some(_)));
        let map = out.memory_map.clone().unwrap_or_default();
        let mut read = |at: u64, len: usize| -> Option<alloc::vec::Vec<u8>> {
            let end = at.checked_add(len as u64)?;
            let allowed = if by_loader { processors::inside_table_memory(&map, at, len) } else { processors::outside_free_memory(&map, at, len) };
            // Address zero is never a table's, and a slice may not begin there.
            (at != 0 && end <= 4 << 30 && len <= 1 << 20 && allowed).then(|| core::slice::from_raw_parts(at as usize as *const u8, len).to_vec())
        };
        let root = match named {
            Ok(Some(at)) => {
                out.processors_note = "at the address the loader named";
                Some(at)
            }
            Err(()) => {
                out.processors_note = "the loader's line is cut short, or its acpi_rsdp= is not the one form taken";
                None
            }
            Ok(None) => {
                let area = (((0x40E as *const u16).read_unaligned() as u64) << 4, 1024u64);
                let found = [area, (0xE_0000, 0x2_0000)].into_iter().filter(|(start, _)| *start >= 0x400).find_map(|(start, length)| {
                    (start..start + length).step_by(16).find(|at| read(*at, 36).is_some_and(|b| processors::root_holds(&b)))
                });
                out.processors_note = if found.is_some() { "found where a BIOS puts it" } else { "no root pointer where a BIOS puts it, and the loader named none" };
                found
            }
        };
        out.processors_root = root;
        if let Some(root) = root {
            match processors::find(root, &mut read) {
                Ok(found) => out.processors = Some(found),
                Err(why) => {
                    out.processors_note = match why {
                        processors::Refused::Unreadable => if by_loader { "a table lies outside what can be read at start, or outside the firmware's own table memory" } else { "a table lies outside what can be read at start, or in memory given as free" },
                        processors::Refused::NoRoot => "the root pointer does not hold",
                        processors::Refused::BadTable => "a table's signature, length or checksum is wrong",
                        processors::Refused::NoProcessorTable => "the firmware lists no table of processors",
                        processors::Refused::BadEntry => "the table of processors has a malformed entry",
                        processors::Refused::BadProcessors => "the table of processors names one twice, none, or no controller",
                    }
                }
            }
        }
    }
    out
}

/// Addresses the linker script defines: the extent of code, of read-only data, and the end of the kernel.
pub fn layout() -> [usize; 5] {
    extern "C" {
        static __text_start: u8;
        static __text_end: u8;
        static __rodata_start: u8;
        static __rodata_end: u8;
        static __kernel_end: u8;
    }
    // Only the addresses of the linker symbols are taken; nothing is read through them.
    [core::ptr::addr_of!(__text_start) as usize, core::ptr::addr_of!(__text_end) as usize,
     core::ptr::addr_of!(__rodata_start) as usize, core::ptr::addr_of!(__rodata_end) as usize,
     core::ptr::addr_of!(__kernel_end) as usize]
}

/// The symbol `boot.s` calls. It exists only to hand over to the safe part of the kernel.
#[no_mangle]
pub extern "C" fn kernel_main(magic: u32, info: u32) -> ! {
    crate::kernel_start(magic, info)
}

/// The control registers that say which protections are on: (CR0, CR4, EFER).
pub fn protection_registers() -> (u64, u64, u64) {
    let (cr0, cr4): (u64, u64);
    let (lo, hi): (u32, u32);
    // Safety: reading control registers and a model-specific register in ring 0 has no side effect.
    unsafe {
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        asm!("rdmsr", in("ecx") 0xC000_0080u32, out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
    }
    (cr0, cr4, ((hi as u64) << 32) | lo as u64)
}

#[cfg(not(feature = "uefi-demo-heap"))]
const HEAP_SIZE: usize = 16 * 1024 * 1024;
#[cfg(feature = "uefi-demo-heap")]
const HEAP_SIZE: usize = 4 * 1024 * 1024;

#[repr(align(4096))]
struct HeapArea(core::cell::UnsafeCell<[u8; HEAP_SIZE]>);
// Safety: the area is handed out in disjoint pieces by the allocator below and never accessed as a whole.
unsafe impl Sync for HeapArea {}
static HEAP: HeapArea = HeapArea(core::cell::UnsafeCell::new([0; HEAP_SIZE]));
/// Bytes of the area that have ever been carved into blocks.
static CARVED: AtomicUsize = AtomicUsize::new(0);
/// Bytes in blocks that are in use now.
static LIVE: AtomicUsize = AtomicUsize::new(0);

/// Block sizes are powers of two, from 16 bytes to 8 MiB.
const CLASSES: usize = 20;
struct FreeLists(core::cell::UnsafeCell<[usize; CLASSES]>);
// Safety: read and written only with interrupts off, on the one processor there is.
unsafe impl Sync for FreeLists {}
static FREE: FreeLists = FreeLists(core::cell::UnsafeCell::new([0; CLASSES]));

/// Refuse interrupts and say whether they were allowed, so that the caller can put things back as they were.
fn interrupts_save_off() -> bool {
    let flags: u64;
    // Safety: reads the flags through the stack and clears the interrupt flag; no memory besides that stack slot.
    unsafe { asm!("pushfq", "pop {}", "cli", out(reg) flags) };
    flags & 0x200 != 0
}

fn interrupts_restore(were_on: bool) {
    if were_on {
        // Safety: interrupts were allowed when the matching `interrupts_save_off` ran.
        unsafe { asm!("sti", options(nomem, nostack)) }
    }
}

fn class_of(layout: Layout) -> Option<usize> {
    if layout.align() > 4096 {
        return None;
    }
    let need = layout.size().max(layout.align()).max(16).checked_next_power_of_two()?;
    let class = need.trailing_zeros() as usize - 4;
    if class < CLASSES { Some(class) } else { None }
}

/// The kernel's allocator. Every block is a power of two in size and is aligned to its size (to a page, if
/// it is larger than one). A freed block goes on the list of its size and is handed out again from there;
/// blocks are never split or joined, so a block's extent never changes once it is carved. Exhaustion
/// returns null, which stops the kernel. Interrupts are off while a list is changed: an interrupt handler
/// allocates too.
pub struct Heap;

// Safety: a block is either on exactly one free list or handed out to exactly one caller; carving only ever
// moves forward through the area; every block lies inside HEAP and is aligned as described above.
unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let class = match class_of(layout) {
            Some(c) => c,
            None => return core::ptr::null_mut(),
        };
        let size = 16usize << class;
        let on = interrupts_save_off();
        let lists = FREE.0.get();
        let head = (*lists)[class];
        let block = if head != 0 {
            (*lists)[class] = (head as *const usize).read();
            head
        } else {
            let base = HEAP.0.get() as usize;
            let align = size.min(4096);
            let start = (base + CARVED.load(Ordering::Relaxed) + align - 1) & !(align - 1);
            if start - base + size > HEAP_SIZE {
                0
            } else {
                CARVED.store(start - base + size, Ordering::Relaxed);
                start
            }
        };
        if block != 0 {
            LIVE.fetch_add(size, Ordering::Relaxed);
        }
        interrupts_restore(on);
        block as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(class) = class_of(layout) {
            let on = interrupts_save_off();
            let lists = FREE.0.get();
            (ptr as *mut usize).write((*lists)[class]);
            (*lists)[class] = ptr as usize;
            LIVE.fetch_sub(16usize << class, Ordering::Relaxed);
            interrupts_restore(on);
        }
    }
}

/// Bytes of the heap in use now, and bytes of it ever carved into blocks.
pub fn heap_used() -> (usize, usize) {
    (LIVE.load(Ordering::Relaxed), CARVED.load(Ordering::Relaxed))
}

// ---- memory protection: writable or executable, never both -----------------------------------------
const P: u64 = 1;
const W: u64 = 2;
const U: u64 = 4;
const HUGE: u64 = 0x80;
const NX: u64 = 1 << 63;
/// A software bit in a leaf entry of a program's tables: the page is device memory, not a frame the space owns.
const NOT_OWNED: u64 = 1 << 9;
/// A page whose protection never changes: what it was mapped as, it stays.
const SEALED: u64 = 1 << 10;
const ADDRESS: u64 = 0x000f_ffff_ffff_f000;
const MIB2: usize = 2 << 20;
/// 4 KiB pages for the first 128 MiB, which holds the whole kernel image and the files the loader placed after it.
const SMALL_TABLES: usize = 64;
/// Tables kept for the two ends of a loader's framebuffer: where it begins or ends inside a 2 MiB stretch
/// that has no small table, that stretch is mapped page by page from one of these.
const SPARE_TABLES: usize = 2;
/// Top table, second-level table, the third-level table of each of the first four gibibytes, the small
/// tables, and the spare ones.
const TABLE_COUNT: usize = 6 + SMALL_TABLES + SPARE_TABLES;
static SPARE_USED: AtomicUsize = AtomicUsize::new(0);

#[repr(align(4096))]
struct Tables(core::cell::UnsafeCell<[[u64; 512]; TABLE_COUNT]>);
// Safety: written by `protect`, before anything else uses them, and by `map_device`, which only ever adds
// entries for device memory; otherwise only the processor reads them.
unsafe impl Sync for Tables {}
static TABLES: Tables = Tables(core::cell::UnsafeCell::new([[0; 512]; TABLE_COUNT]));

/// Where the image begins: the loader places it at one mebibyte (linker.ld).
const IMAGE_START: usize = 1 << 20;

/// The memory the kernel hands out as frames: [start, end), both multiples of 2 MiB. Zero until `protect`.
static POOL_START: AtomicUsize = AtomicUsize::new(0);
static POOL_END: AtomicUsize = AtomicUsize::new(0);
/// Memory above four gibibytes that programs' own pages may be taken from: up to eight [start, end)
/// ranges of whole 2 MiB pages, mapped like the frame memory below (each address at itself, writable,
/// never executable, never reachable from user mode). Written once by `protect`.
static HIGH: Single<([(usize, usize); frame_map::HIGH_RANGES], usize)> = Single::new(([(0, 0); frame_map::HIGH_RANGES], 0));
/// The third-level tables of the sixty-four gibibytes above four, used only for those ranges.
#[repr(align(4096))]
struct HighTables(core::cell::UnsafeCell<[[u64; 512]; 64]>);
// Safety: written by `protect`, before anything else uses them; afterwards only the processor reads them.
unsafe impl Sync for HighTables {}
static HIGH_TABLES: HighTables = HighTables(core::cell::UnsafeCell::new([[0; 512]; 64]));
/// The loader's files, as mapped read-only: up to sixteen [start, end) ranges. Written once by `protect`.
static MODULES: Single<([(usize, usize); 16], usize)> = Single::new(([(0, 0); 16], 0));
/// Device memory mapped by `map_device`: up to four [start, end) ranges.
static DEVICES: Single<([(usize, usize); 4], usize)> = Single::new(([(0, 0); 4], 0));
/// For each of those ranges, the bytes inside it that the kernel itself may write: all of it for a device
/// whose memory was mapped whole, the picture alone for a loader's framebuffer, whose last page can hold
/// bytes past the picture's end.
static DEVICE_BYTES: Single<[(usize, usize); 4]> = Single::new([(0, 0); 4]);

/// Replace the loader-time mapping, in which the first gibibyte was writable and executable, by one that
/// maps what the kernel uses and nothing else. The kernel image: code is executable and not writable;
/// read-only data and the processor's own tables are neither; all else is writable and not executable; the
/// page below each stack is absent. The loader's files: readable, and nothing more. The memory above them,
/// which the kernel hands out as frames: writable and never executable. Nothing else is mapped: not the low
/// mebibyte, not memory the loader did not call usable. Every address is mapped at itself, once, so no
/// second address reaches the kernel's code. Returns the extent of the frame memory.
pub fn protect(boot: &mut BootInfo) -> (usize, usize) {
    let [text_start, text_end, ro_start, ro_end, end] = layout();
    let small_limit = SMALL_TABLES * MIB2;
    let mut top = end;
    let mut ranges = ([(0usize, 0usize); 16], 0usize);
    for m in boot.modules.iter_mut() {
        m.usable = m.start % 4096 == 0 && m.start >= end && m.end > m.start && m.end <= small_limit && ranges.1 < 16;
        if m.usable {
            ranges.0[ranges.1] = (m.start, (m.end + 4095) & !4095);
            ranges.1 += 1;
            top = top.max((m.end + 4095) & !4095);
        }
    }
    // Even a module that cannot be mapped read-only may occupy RAM the loader still describes as usable.
    const LIMIT: usize = 1 << 30;
    for m in &boot.modules {
        if m.start < LIMIT && m.start < m.end {
            top = top.max(m.end.min(LIMIT));
        }
    }
    let (pool_start, pool_end) = if boot.incomplete_modules || boot.modules.iter().any(|m| m.end < m.start) {
        (0, 0)
    } else if let Some(map) = &boot.memory_map {
        frame_map::pool(map, top)
    } else {
        // A loader which supplied only the legacy contiguous size retains the previous 2 MiB margin.
        let ram_end = boot.memory.map(|(_, upper)| (IMAGE_START + upper as usize * 1024).saturating_sub(MIB2)).unwrap_or(0);
        let first = top.div_ceil(MIB2) * MIB2;
        let last = ram_end.min(LIMIT) / MIB2 * MIB2;
        if first < last { (first, last) } else { (0, 0) }
    };
    // Safety: TABLES is static, page-aligned storage used for nothing else; the new mapping is an identity
    // map in which every address the kernel is using stays valid (code stays executable, the stack and all
    // data stay writable) when CR3 is replaced. MODULES is written here once, before interrupts are on.
    unsafe {
        let t = TABLES.0.get() as *mut [u64; 512];
        let (pml4, pdpt) = (t, t.add(1));
        (*pml4)[0] = pdpt as u64 | P | W;
        for g in 0..4usize {
            (*pdpt)[g] = t.add(2 + g) as u64 | P | W;
        }
        let pd = t.add(2);
        for i in 0..512usize {
            let at = i * MIB2;
            (*pd)[i] = if at >= pool_start && at < pool_end {
                at as u64 | P | W | NX | HUGE
            } else if i < SMALL_TABLES {
                t.add(6 + i) as u64 | P | W
            } else {
                0
            };
        }
        assert!(end <= small_limit, "the kernel image does not fit the finely mapped region");
        let guards = guard_pages();
        for page in IMAGE_START >> 12..end >> 12 {
            let addr = page << 12;
            #[cfg(feature = "processors")]
            if others::guard(addr) {
                continue; // not mapped: the page below a stack of another processor
            }
            if guards.contains(&addr) {
                continue; // not mapped: the page below a stack
            }
            #[cfg(feature = "processors")]
            let theirs = others::segments(addr);
            #[cfg(not(feature = "processors"))]
            let theirs = false;
            let flags = if addr == interrupt_table() || addr == segment_tables() || theirs {
                P | NX // the processor's own tables: it reads them and nothing writes them again
            } else if addr >= text_start && addr < text_end {
                P
            } else if addr >= ro_start && addr < ro_end {
                P | NX
            } else {
                P | W | NX
            };
            (*t.add(6 + page / 512))[page % 512] = addr as u64 | flags;
        }
        for &(start, stop) in &ranges.0[..ranges.1] {
            for page in start >> 12..stop >> 12 {
                (*t.add(6 + page / 512))[page % 512] = (page << 12) as u64 | P | NX;
            }
        }
        // Memory above four gibibytes, where the loader's map shows any and the pool below exists: mapped
        // by the same rule as the pool, in 2 MiB leaves, from tables that are part of the kernel image.
        // The map the pools are made from. With the emulator fixture, memory the line says the
        // loader marked is added to it as the model's kind first, as a loader would have handed it.
        #[cfg(feature = "model-memory-fixture")]
        let marked = fixture_map(boot);
        #[cfg(feature = "model-memory-fixture")]
        let map = marked.as_ref().or(boot.memory_map.as_ref());
        #[cfg(not(feature = "model-memory-fixture"))]
        let map = boot.memory_map.as_ref();
        // Memory the loader marked in its map as holding a model, and named on its line, is decided
        // here. Such memory is never part of the pool, named or not: the map's kind keeps it out.
        #[cfg(feature = "model-memory")]
        MODEL.with(|m| *m = model_named(boot, map));
        let high = match (map, pool_start < pool_end) {
            (Some(map), true) => frame_map::high(map),
            _ => ([(0, 0); frame_map::HIGH_RANGES], 0),
        };
        let high_tables = HIGH_TABLES.0.get() as *mut [u64; 512];
        for &(start, stop) in &high.0[..high.1] {
            let mut at = start;
            while at < stop {
                let (gibibyte, slot) = (at >> 30, (at >> 21) & 511);
                let table = high_tables.add(gibibyte - 4);
                (*pdpt)[gibibyte] = table as u64 | P | W;
                (*table)[slot] = at as u64 | P | W | NX | HUGE;
                at += MIB2;
            }
        }
        HIGH.with(|h| *h = high);
        MODULES.with(|m| *m = ranges);
        POOL_START.store(pool_start, Ordering::SeqCst);
        POOL_END.store(pool_end, Ordering::SeqCst);
        asm!("mov cr3, {}", in(reg) pml4 as u64, options(nostack, preserves_flags));
    }
    (pool_start, pool_end)
}

/// What the loader's line and map say about memory holding a model.
#[cfg(feature = "model-memory")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    /// The line names none.
    None,
    /// The line names memory the loader's map marks as a model's: out of the pool, mapped nowhere by the kernel.
    Held { extents: [(u64, u64); frame_map::MODEL_EXTENTS], count: usize, bytes: u64 },
    /// The line names memory that is not given as a model's, and why.
    Refused(&'static str),
}

#[cfg(feature = "model-memory")]
static MODEL: Single<Model> = Single::new(Model::None);

/// The memory kept for a model, as decided once at start.
#[cfg(feature = "model-memory")]
pub fn model() -> Model {
    let on = interrupts_save_off();
    let model = MODEL.with(|m| *m);
    interrupts_restore(on);
    model
}

#[cfg(feature = "model-memory")]
fn model_named(boot: &BootInfo, map: Option<&alloc::vec::Vec<u8>>) -> Model {
    // A line that was cut may have lost part of what it names; nothing is taken from it.
    let Some(line) = boot.exact_command_line() else {
        return if boot.command_line.contains("model_memory=") { Model::Refused("the loader's line was cut") } else { Model::None };
    };
    let mut named = line.split_whitespace().filter_map(|word| word.strip_prefix("model_memory="));
    let Some(word) = named.next() else { return Model::None };
    if named.next().is_some() {
        return Model::Refused("model_memory is given more than once");
    }
    let Some((extents, count)) = frame_map::model_extents(word) else {
        return Model::Refused("model_memory is not address:bytes pieces in hexadecimal");
    };
    let Some(map) = map else { return Model::Refused("the loader gave no memory map") };
    match frame_map::model_allowed(map, &extents[..count]) {
        Ok(bytes) => Model::Held { extents, count, bytes },
        Err(why) => Model::Refused(why),
    }
}

/// FOR THE EMULATOR ONLY. An emulator's own loader cannot mark memory in its map as a model's, as
/// the loader stage on a real machine does. With this feature the line's `fixture_model_ram=` (the
/// same form as `model_memory=`) stands for that marking: its pieces, in whole pages, are added to
/// a copy of the map as the model's kind, and everything after reads the copy exactly as it would
/// read a loader's map. No release is built with it, and the kernel says at start when it is in use.
#[cfg(feature = "model-memory-fixture")]
fn fixture_map(boot: &BootInfo) -> Option<alloc::vec::Vec<u8>> {
    let mut map = boot.memory_map.clone()?;
    let word = boot.exact_command_line()?.split_whitespace().find_map(|w| w.strip_prefix("fixture_model_ram="))?;
    let (extents, count) = frame_map::model_extents(word)?;
    for &(base, bytes) in &extents[..count] {
        let whole = bytes.checked_add(4095)? / 4096 * 4096;
        map.extend_from_slice(&20u32.to_le_bytes());
        map.extend_from_slice(&base.to_le_bytes());
        map.extend_from_slice(&whole.to_le_bytes());
        map.extend_from_slice(&frame_map::MODEL_RAM.to_le_bytes());
    }
    Some(map)
}

/// Whether the emulator fixture marked any memory in this boot.
#[cfg(feature = "model-memory-fixture")]
pub fn model_fixture_used(boot: &BootInfo) -> bool {
    fixture_map(boot).is_some_and(|map| Some(&map) != boot.memory_map.as_ref())
}

/// The frame memory: [start, end).
pub fn pool() -> (usize, usize) {
    (POOL_START.load(Ordering::Relaxed), POOL_END.load(Ordering::Relaxed))
}

/// The ranges of memory above four gibibytes that are mapped as frame memory, and how many there are.
pub fn high_pools() -> ([(usize, usize); frame_map::HIGH_RANGES], usize) {
    HIGH.with(|h| *h)
}

fn in_high(addr: usize, size: usize) -> bool {
    let (ranges, count) = high_pools();
    ranges[..count].iter().any(|&(start, end)| addr >= start && addr + size <= end)
}

fn in_pool(frame: usize) -> bool {
    let (start, end) = pool();
    frame % 4096 == 0 && ((frame >= start && frame < end) || in_high(frame, 4096))
}

/// Fill one frame of the pool with zeros. False if the address is not a frame of the pool.
pub fn frame_zero(frame: usize) -> bool {
    if !in_pool(frame) {
        return false;
    }
    // Safety: the frame lies in the pool, which is mapped writable at its own address and holds no part of the kernel.
    unsafe { core::ptr::write_bytes(frame as *mut u8, 0, 4096) };
    true
}

/// Whether one frame of the pool holds nothing but zeros. False if the address is not a frame of the pool.
#[cfg(feature = "high-memory-demo")]
pub fn frame_is_zero(frame: usize) -> bool {
    // Safety: the frame lies in the pool, which is mapped readable at its own address.
    in_pool(frame) && unsafe { core::slice::from_raw_parts(frame as *const u8, 4096) }.iter().all(|b| *b == 0)
}

/// Copy bytes into one frame of the pool. False if the frame is not of the pool or the bytes would not fit.
pub fn frame_write(frame: usize, offset: usize, data: &[u8]) -> bool {
    if !in_pool(frame) || offset > 4096 || data.len() > 4096 - offset {
        return false;
    }
    // Safety: the destination is `data.len()` bytes inside one pool frame; the source is a kernel slice, which cannot lie in the pool.
    unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), (frame + offset) as *mut u8, data.len()) };
    true
}

/// Publish one aligned little-endian halfword in an exclusively reserved DMA frame.
#[cfg(feature = "virtio-tx-probe")]
pub fn frame_publish_u16(frame: usize, offset: usize, value: u16) -> bool {
    if !in_pool(frame) || offset > 4094 || offset % 2 != 0 {
        return false;
    }
    // Safety: the halfword is aligned, lies inside a mapped writable pool frame,
    // and the transmitter retains the frame until the device confirms reset.
    unsafe { core::ptr::write_volatile((frame + offset) as *mut u16, value.to_le()) };
    true
}

/// The bytes of a file the loader placed in memory, if the kernel mapped it. They stay mapped, read-only, for the life of the kernel.
pub fn module_bytes(m: &Module) -> Option<&'static [u8]> {
    if !m.usable {
        return None;
    }
    let known = MODULES.with(|r| r.0[..r.1].iter().any(|&(s, e)| m.start >= s && m.end <= e));
    // Safety: the range was mapped readable by `protect` and nothing maps it writable.
    if known { Some(unsafe { core::slice::from_raw_parts(m.start as *const u8, m.end - m.start) }) } else { None }
}

/// Map device memory, such as a display's, at its own address: writable and never executable. The memory
/// must lie between one and four gibibytes and begin and end on a multiple of 2 MiB. False if it does
/// not, or if four devices are mapped already.
pub fn map_device(phys: usize, len: usize) -> bool {
    let end = match phys.checked_add(len) {
        Some(e) => e,
        None => return false,
    };
    if phys < 1 << 30 || end > 4 << 30 || phys % MIB2 != 0 || len % MIB2 != 0 || len == 0 {
        return false;
    }
    #[cfg(feature = "processors")]
    if others::controller_stretch(phys, end) {
        return false; // the interrupt controller's own page is there
    }
    let on = interrupts_save_off();
    let ok = DEVICES.with(|d| {
        if d.1 == 4 {
            return false;
        }
        d.0[d.1] = (phys, end);
        DEVICE_BYTES.with(|b| b[d.1] = (phys, end));
        d.1 += 1;
        true
    });
    if ok {
        // Safety: the third-level tables of gibibytes one to three are static storage of TABLES; the entries
        // written were absent, so no translation that was in use changes.
        unsafe {
            let t = TABLES.0.get() as *mut [u64; 512];
            let mut at = phys;
            while at < end {
                (*t.add(2 + (at >> 30)))[(at >> 21) & 511] = at as u64 | P | W | NX | HUGE;
                at += MIB2;
            }
        }
    }
    interrupts_restore(on);
    ok
}

fn in_device(phys: usize) -> bool {
    DEVICES.with(|d| d.0[..d.1].iter().any(|&(s, e)| phys >= s && phys < e))
}

/// Whether [phys, end) lies wholly inside the bytes of one device that the kernel may write.
fn in_device_bytes(phys: usize, end: usize) -> bool {
    let count = DEVICES.with(|d| d.1);
    DEVICE_BYTES.with(|b| b[..count].iter().any(|&(s, e)| phys >= s && end <= e))
}

/// Map the framebuffer the loader described, if it may be mapped: exactly the pages the picture occupies,
/// writable and never executable, each at its own address. Whether it may be is `framebuffer::decide`,
/// given the loader's memory map and every range this kernel uses; nothing that is already mapped is
/// changed, and if any page of it is, nothing is mapped at all. Returns the bytes mapped, or why not.
pub fn map_loader_display(boot: &BootInfo) -> Result<usize, &'static str> {
    let l = boot.display.ok_or("the loader described none")?;
    if boot.memory_map.as_ref().is_some_and(|m| m.is_empty()) {
        return Err("the loader's memory map could not be read");
    }
    let [_, _, _, _, image_end] = layout();
    let mut protected: alloc::vec::Vec<(usize, usize)> = alloc::vec![(IMAGE_START, image_end), pool()];
    protected.extend(boot.modules.iter().filter(|m| m.end > m.start).map(|m| (m.start, (m.end + 4095) & !4095)));
    let memory_top = boot.memory.map(|(_, upper)| IMAGE_START as u64 + upper as u64 * 1024);
    let window = framebuffer::decide(l.address as u64, l.pitch, l.width, l.height, l.bits, boot.memory_map.as_deref(), memory_top, &protected)?;
    let (first, last) = window.pages;
    let on = interrupts_save_off();
    let done = (|| {
        if DEVICES.with(|d| d.1 == 4 || d.0[..d.1].iter().any(|&(s, e)| s < last && first < e)) {
            return Err("device memory is already mapped there, or four devices are");
        }
        // Safety: TABLES is this kernel's own static storage. The first pass only reads it. The second
        // writes entries the first found absent, so no translation in use changes; a spare table is
        // zeroed before the entry that points to it is written.
        unsafe {
            let t = TABLES.0.get() as *mut [u64; 512];
            let stretches = || (first / MIB2 * MIB2..last).step_by(MIB2);
            let whole = |stretch: usize| first <= stretch && stretch + MIB2 <= last;
            let mut spare_needed = 0;
            for stretch in stretches() {
                let entry = (*t.add(2 + (stretch >> 30)))[(stretch >> 21) & 511];
                if entry & P == 0 {
                    spare_needed += !whole(stretch) as usize;
                } else if entry & HUGE != 0 {
                    return Err("part of it is already mapped");
                } else {
                    let table = (entry & ADDRESS) as *const [u64; 512];
                    for page in (stretch.max(first)..(stretch + MIB2).min(last)).step_by(4096) {
                        if (*table)[(page >> 12) & 511] != 0 {
                            return Err("part of it is already mapped");
                        }
                    }
                }
            }
            let used = SPARE_USED.load(Ordering::SeqCst);
            if used + spare_needed > SPARE_TABLES {
                return Err("it would need more page tables than are kept for it");
            }
            let mut next = used;
            for stretch in stretches() {
                let directory = t.add(2 + (stretch >> 30));
                let slot = (stretch >> 21) & 511;
                if (*directory)[slot] & P == 0 {
                    if whole(stretch) {
                        (*directory)[slot] = stretch as u64 | P | W | NX | HUGE;
                        continue;
                    }
                    let table = t.add(6 + SMALL_TABLES + next);
                    (*table) = [0; 512];
                    (*directory)[slot] = table as u64 | P | W;
                    next += 1;
                }
                let table = ((*directory)[slot] & ADDRESS) as *mut [u64; 512];
                for page in (stretch.max(first)..(stretch + MIB2).min(last)).step_by(4096) {
                    (*table)[(page >> 12) & 511] = page as u64 | P | W | NX;
                }
            }
            SPARE_USED.store(next, Ordering::SeqCst);
        }
        DEVICES.with(|d| {
            d.0[d.1] = (first, last);
            DEVICE_BYTES.with(|b| b[d.1] = window.bytes);
            d.1 += 1;
        });
        Ok(last - first)
    })();
    interrupts_restore(on);
    done
}

/// Whether a program could be given `pages` whole pages of device memory from `phys`: the question
/// `map_device` asks, with nothing mapped.
pub fn device_grantable(phys: usize, pages: usize) -> bool {
    let count = DEVICES.with(|d| d.1);
    DEVICE_BYTES.with(|b| b[..count].iter().any(|&bytes| framebuffer::grant(bytes, phys, pages)))
}

/// Write 32-bit words into mapped device memory. False if any of them would fall outside it.
pub fn device_write32(phys: usize, words: &[u32]) -> bool {
    let end = match phys.checked_add(words.len() * 4) {
        Some(e) => e,
        None => return false,
    };
    if phys % 4 != 0 || words.is_empty() || !in_device_bytes(phys, end) {
        return false;
    }
    // Safety: the whole destination lies inside one device range mapped writable by `map_device`.
    unsafe { core::ptr::copy_nonoverlapping(words.as_ptr(), phys as *mut u32, words.len()) };
    true
}

/// Fill mapped device memory with one 32-bit word. False if any of it would fall outside the device.
pub fn device_fill32(phys: usize, count: usize, word: u32) -> bool {
    let end = match count.checked_mul(4).and_then(|n| phys.checked_add(n)) {
        Some(e) => e,
        None => return false,
    };
    if phys % 4 != 0 || !in_device_bytes(phys, end) {
        return false;
    }
    // Safety: as `device_write32`.
    unsafe {
        let p = phys as *mut u32;
        for i in 0..count {
            p.add(i).write_volatile(word);
        }
    }
    true
}

/// What the processor's own page tables say about the kernel's address space, read back from them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapAudit {
    /// 4 KiB pages mapped inside the kernel image.
    pub image_pages: u32,
    /// 4 KiB pages mapped inside the loader's files, and how many of those are writable or executable.
    pub module_pages: u32,
    pub module_pages_not_read_only: u32,
    /// Bytes mapped inside the frame memory and inside device memory, and how many leaves of either are executable or reachable from user mode.
    pub pool_bytes: usize,
    pub device_bytes: usize,
    pub pool_or_device_executable: u32,
    /// Pages of the interrupt controller's registers (feature `processors`), and how many are executable.
    pub controller_pages: u32,
    /// Leaves that are both writable and executable, anywhere.
    pub writable_and_executable: u32,
    /// Leaves that map something that is none of the four things above.
    pub outside: u32,
    /// Leaves or tables marked reachable from user mode.
    pub user_reachable: u32,
    /// Leaves that map an address to a different address.
    pub not_identity: u32,
    pub code_executable_not_writable: bool,
    pub read_only_data_neither: bool,
    pub data_writable_not_executable: bool,
    pub address_zero_unmapped: bool,
    pub low_mebibyte_unmapped: bool,
    /// Entries of the top table other than the first.
    pub other_top_entries: u32,
}

/// Walk the kernel's own page tables and classify every leaf.
pub fn kernel_map_audit() -> MapAudit {
    let [text_start, _te, ro_start, _re, end] = layout();
    let (pool_start, pool_end) = pool();
    let modules = MODULES.with(|m| *m);
    let mut a = MapAudit { address_zero_unmapped: true, low_mebibyte_unmapped: true, ..MapAudit::default() };
    let mut table_user = 0u32; // tables, as opposed to leaves, that are marked for user mode
    let mut classify = |addr: usize, size: usize, e: u64| {
        if e & W != 0 && e & NX == 0 {
            a.writable_and_executable += 1;
        }
        if e & U != 0 {
            a.user_reachable += 1;
        }
        if (e & ADDRESS) as usize != addr {
            a.not_identity += 1;
        }
        if addr == 0 {
            a.address_zero_unmapped = false;
        }
        if addr < IMAGE_START {
            a.low_mebibyte_unmapped = false;
        }
        if addr >= IMAGE_START && addr + size <= end {
            a.image_pages += 1;
        } else if modules.0[..modules.1].iter().any(|&(s, t)| addr >= s && addr + size <= t) {
            a.module_pages += 1;
            if e & W != 0 || e & NX == 0 {
                a.module_pages_not_read_only += 1;
            }
        } else if (addr >= pool_start && addr + size <= pool_end) || in_high(addr, size) {
            a.pool_bytes += size;
            if e & NX == 0 {
                a.pool_or_device_executable += 1;
            }
        } else if in_device(addr) && in_device(addr + size - 1) {
            a.device_bytes += size;
            if e & NX == 0 {
                a.pool_or_device_executable += 1;
            }
        } else {
            #[cfg(feature = "processors")]
            if size == 4096 && others::controller_page(addr) {
                a.controller_pages += 1;
                if e & NX == 0 {
                    a.pool_or_device_executable += 1;
                }
                return;
            }
            a.outside += 1;
        }
    };
    // Safety: reads the kernel's own tables, which are static storage and are only ever added to.
    unsafe {
        let pml4 = TABLES.0.get() as *const u64;
        for i in 1..512usize {
            if pml4.add(i).read() & P != 0 {
                a.other_top_entries += 1;
            }
        }
        let pdpt = (pml4.read() & ADDRESS) as *const u64;
        for g in 0..512usize {
            let e3 = pdpt.add(g).read();
            if e3 & P == 0 {
                continue;
            }
            if e3 & U != 0 {
                table_user += 1;
            }
            if e3 & HUGE != 0 {
                classify(g << 30, 1 << 30, e3);
                continue;
            }
            let pd = (e3 & ADDRESS) as *const u64;
            for i in 0..512usize {
                let e2 = pd.add(i).read();
                if e2 & P == 0 {
                    continue;
                }
                if e2 & HUGE != 0 {
                    classify(g << 30 | i << 21, MIB2, e2);
                    continue;
                }
                if e2 & U != 0 {
                    table_user += 1;
                }
                let pt = (e2 & ADDRESS) as *const u64;
                for k in 0..512usize {
                    let e1 = pt.add(k).read();
                    if e1 & P != 0 {
                        classify(g << 30 | i << 21 | k << 12, 4096, e1);
                    }
                }
            }
        }
    }
    a.user_reachable += table_user;
    let leaf = |addr: usize| -> Option<u64> {
        // Safety: as above.
        unsafe {
            let mut table = TABLES.0.get() as *const u64;
            for level in (0..4).rev() {
                let e = table.add((addr >> (12 + 9 * level)) & 511).read();
                if e & P == 0 {
                    return None;
                }
                if level == 0 || e & HUGE != 0 {
                    return Some(e);
                }
                table = (e & ADDRESS) as *const u64;
            }
            None
        }
    };
    let is = |addr: usize, w: bool, x: bool| matches!(leaf(addr), Some(e) if (e & W != 0) == w && (e & NX == 0) == x);
    a.code_executable_not_writable = is(text_start, false, true);
    a.read_only_data_neither = is(ro_start, false, false);
    a.data_writable_not_executable = is(core::ptr::addr_of!(CARVED) as usize, true, false);
    a
}

/// The pages left unmapped below each stack: the kernel stack, the three stacks the processor switches to
/// for exceptions that must not depend on the stack that was in use, the stack for entries from user mode,
/// and the stack the processor idles on.
pub fn guard_pages() -> [usize; 6] {
    let base = FAULT_STACKS.0.get() as usize;
    [stack_guard(), base, base + FAULT_SLOT, base + 2 * FAULT_SLOT, ENTRY_STACK.0.get() as usize, IDLE_STACK.0.get() as usize]
}

/// Ask the processor which further protections it offers, and turn on each one it does: supervisor-mode
/// execution prevention, supervisor-mode access prevention, and user-mode instruction prevention. They
/// bind the kernel itself from the moment user memory exists. Returns (offered, enabled) as bit sets in
/// the positions of control register 4: bit 20, bit 21, bit 11.
pub fn harden() -> (u64, u64) {
    const SMEP: u64 = 1 << 20;
    const SMAP: u64 = 1 << 21;
    const UMIP: u64 = 1 << 11;
    // Safety: CPUID is available on every processor that reached long mode and has no memory effect; the
    // bits set in CR4 are ones the processor has just said it implements; no user-accessible page is
    // mapped, so no access the kernel makes changes meaning.
    unsafe {
        let mut offered = 0;
        if core::arch::x86_64::__cpuid(0).eax >= 7 {
            let r = core::arch::x86_64::__cpuid_count(7, 0);
            if r.ebx & (1 << 7) != 0 {
                offered |= SMEP;
            }
            if r.ebx & (1 << 20) != 0 {
                offered |= SMAP;
            }
            if r.ecx & (1 << 2) != 0 {
                offered |= UMIP;
            }
        }
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= offered;
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        (offered, cr4 & (SMEP | SMAP | UMIP))
    }
}

/// The floating-point and vector registers of one program, as the processor saves them: the 512 bytes
/// of FXSAVE, and where the processor keeps wider registers (AVX) the 64-byte header and 256 bytes that
/// XSAVE adds for them. 832 bytes are used at most; the rest is never written or read.
#[repr(C, align(64))]
#[derive(Clone)]
pub struct FpuArea([u8; 1024]);

/// Whether the wider registers are on: set once by `fpu_init`, before any program exists.
static WIDE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// What XSAVE keeps here: the x87 registers, the 128-bit registers and the upper halves of the 256-bit
/// ones. Nothing wider is turned on, so nothing wider can hold a program's data.
const XSAVE_KEPT: u32 = 0b111;

/// Whether programs may use the 256-bit vector registers on this processor (the kernel then saves them).
pub fn wide_vectors() -> bool {
    WIDE.load(core::sync::atomic::Ordering::Relaxed)
}

struct FpuTemplate(core::cell::UnsafeCell<FpuArea>);
// Safety: written once by `fpu_init`, before any program exists; read-only afterwards.
unsafe impl Sync for FpuTemplate {}
static FPU_CLEAN: FpuTemplate = FpuTemplate(core::cell::UnsafeCell::new(FpuArea([0; 1024])));

/// Turn on the floating-point and vector instructions for programs, and keep a copy of their state as it
/// is when nothing has used them. The kernel itself is compiled without those instructions and never
/// touches the registers, so they hold exactly what the last program to run left in them until the kernel
/// saves and replaces them.
pub fn fpu_init() {
    // Safety: every processor that offers long mode offers FXSAVE and SSE2; the bits set say the kernel
    // saves that state, which it does (`fpu_save`, `fpu_restore`). The template is written here once.
    unsafe {
        let mut cr0: u64;
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        cr0 = (cr0 & !(1 << 2)) | (1 << 1) | (1 << 5); // no emulation; monitor the coprocessor; native error reporting
        asm!("mov cr0, {}", in(reg) cr0, options(nomem, nostack, preserves_flags));
        let mut cr4: u64;
        asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
        cr4 |= (1 << 9) | (1 << 10); // FXSAVE and vector exceptions are handled by this kernel
        asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
        // Where the processor offers XSAVE and AVX, the 256-bit registers are turned on as well and
        // kept with XSAVE; a processor without them keeps the 128-bit state with FXSAVE as before.
        let features = core::arch::x86_64::__cpuid(1).ecx;
        let wide = features & (1 << 26) != 0 && features & (1 << 28) != 0
            && core::arch::x86_64::__cpuid_count(0xD, 0).eax & XSAVE_KEPT == XSAVE_KEPT
            && core::arch::x86_64::__cpuid_count(0xD, 2).ebx == 576 && core::arch::x86_64::__cpuid_count(0xD, 2).eax == 256;
        if wide {
            asm!("mov {}, cr4", out(reg) cr4, options(nomem, nostack, preserves_flags));
            cr4 |= 1 << 18; // XSAVE and the register that says which states are on
            asm!("mov cr4, {}", in(reg) cr4, options(nomem, nostack, preserves_flags));
            asm!("xsetbv", in("ecx") 0u32, in("eax") XSAVE_KEPT, in("edx") 0u32, options(nomem, nostack, preserves_flags));
            WIDE.store(true, core::sync::atomic::Ordering::Relaxed);
        }
        let mxcsr: u32 = 0x1F80;
        asm!("fninit", "ldmxcsr [{}]", in(reg) &mxcsr, options(nostack));
        if wide {
            // The template is zero, which XRSTOR reads as "every register in its first state"; saving
            // over it records that state with the processor's own header.
            asm!("xrstor64 [{}]", in(reg) FPU_CLEAN.0.get(), in("eax") XSAVE_KEPT, in("edx") 0u32, options(nostack));
            asm!("xsave64 [{}]", in(reg) FPU_CLEAN.0.get(), in("eax") XSAVE_KEPT, in("edx") 0u32, options(nostack));
        } else {
            asm!("fxsave64 [{}]", in(reg) FPU_CLEAN.0.get(), options(nostack));
        }
    }
}

impl FpuArea {
    /// The state of a program that has not yet used the registers: nothing of any other program is in it.
    pub fn clean() -> FpuArea {
        // Safety: the template is never written after `fpu_init`.
        unsafe { (*FPU_CLEAN.0.get()).clone() }
    }
}

/// Save the floating-point and vector registers as they are now.
pub fn fpu_save(area: &mut FpuArea) {
    // Safety: the area is 1024 bytes aligned to 64; FXSAVE writes 512 and XSAVE, for the three states
    // that are on, 832 (checked against the processor's own figures in `fpu_init`).
    unsafe {
        if wide_vectors() {
            asm!("xsave64 [{}]", in(reg) area as *mut FpuArea, in("eax") XSAVE_KEPT, in("edx") 0u32, options(nostack))
        } else {
            asm!("fxsave64 [{}]", in(reg) area as *mut FpuArea, options(nostack))
        }
    }
}

/// Load the floating-point and vector registers. An area holds only what `fpu_save` or `fpu_init` wrote, so the processor accepts it.
pub fn fpu_restore(area: &FpuArea) {
    // Safety: see above. Every state that is on is loaded from the area, so nothing of the program
    // that ran before stays in any register a program can read.
    unsafe {
        if wide_vectors() {
            asm!("xrstor64 [{}]", in(reg) area as *const FpuArea, in("eax") XSAVE_KEPT, in("edx") 0u32, options(nostack))
        } else {
            asm!("fxrstor64 [{}]", in(reg) area as *const FpuArea, options(nostack))
        }
    }
}

/// Only with the `vector-state-demo` feature, and only at start-up, before any program exists: fill a
/// 256-bit register with one pattern and save it, fill it with another and save that, then load each
/// saved state back and read the register. Two saved states that are not the first state, through the
/// same `fpu_save` and `fpu_restore` a switch between programs uses. Says what it found on the log.
#[cfg(feature = "vector-state-demo")]
pub fn vector_state_demo() {
    if !wide_vectors() {
        crate::log(format_args!("vector demo: this processor has no 256-bit registers to keep\n"));
        return;
    }
    let fill = |pattern: &[u8; 32]| {
        // Safety: the wide registers are on (checked above); the register written is declared.
        unsafe { asm!("vmovups ymm15, [{}]", in(reg) pattern.as_ptr(), out("ymm15") _, options(nostack)) }
    };
    let read = || {
        let mut bytes = [0u8; 32];
        // Safety: as above; only memory of this function is written.
        unsafe { asm!("vmovups [{}], ymm15", in(reg) bytes.as_mut_ptr(), options(nostack)) };
        bytes
    };
    let (mut first, mut second) = ([0u8; 32], [0u8; 32]);
    for i in 0..32 {
        first[i] = 0x11 + i as u8;
        second[i] = 0xE0 - i as u8;
    }
    let (mut saved_first, mut saved_second) = (alloc::boxed::Box::new(FpuArea::clean()), alloc::boxed::Box::new(FpuArea::clean()));
    fill(&first);
    fpu_save(&mut saved_first);
    fill(&second);
    fpu_save(&mut saved_second);
    fpu_restore(&saved_first);
    let got_first = read();
    fpu_restore(&saved_second);
    let got_second = read();
    fpu_restore(&saved_first);
    let again = read();
    // The registers are left as a program expects to find them.
    fpu_restore(&FpuArea::clean());
    let clean = read();
    crate::log(format_args!("vector demo: first state returned {}, second state returned {}, first again {}, left clean {}\n",
        (got_first == first) as u8, (got_second == second) as u8, (again == first) as u8, (clean == [0u8; 32]) as u8));
}

fn wrmsr(msr: u32, value: u64) {
    // Safety: the callers write only registers whose meaning is fixed and whose value they have checked.
    unsafe { asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nomem, nostack, preserves_flags)) }
}

/// Let programs reach the kernel with the `syscall` instruction. Interrupts are masked on the way in, and
/// the entry in `boot.s` builds the same frame as an interrupt does.
pub fn syscall_init() {
    extern "C" {
        fn syscall_entry();
        static mut syscall_kernel_rsp: u64;
    }
    let (lo, hi): (u32, u32);
    // Safety: reading the extended-feature register has no side effect; the stack word is written once, here, before any program runs.
    unsafe {
        asm!("rdmsr", in("ecx") 0xC000_0080u32, out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
        core::ptr::addr_of_mut!(syscall_kernel_rsp).write((ENTRY_STACK.0.get() as usize + 4096 + ENTRY_STACK_SIZE) as u64);
    }
    wrmsr(0xC000_0080, (((hi as u64) << 32) | lo as u64) | 1); // enable the instruction
    wrmsr(0xC000_0081, 0x0010_0008_0000_0000); // the kernel's segments on entry (0x08, 0x10); the return pair is unused: the kernel returns with iretq
    wrmsr(0xC000_0082, syscall_entry as usize as u64);
    wrmsr(0xC000_0084, 0x4_4700); // on entry clear: interrupts, direction, trap, nested task, alignment check
}

/// Set the base of the FS segment, which a program's C library uses to find its per-thread data.
/// False if the address is not one a program may hold.
pub fn set_user_fs(base: u64) -> bool {
    if base != 0 && ((base as usize) < USER_BASE || (base as usize) >= USER_BASE + (1 << 39)) {
        return false;
    }
    wrmsr(0xC000_0100, base);
    true
}

/// The processor's count of cycles since it started. Not a clock: its rate is whatever the machine's is.
pub fn cycles() -> u64 {
    let (lo, hi): (u32, u32);
    // Safety: no side effect.
    unsafe { asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags)) };
    ((hi as u64) << 32) | lo as u64
}

/// Sixteen bytes for a program's own use as a seed, and whether the processor's random number generator
/// supplied them. Where it did not, they are derived from the cycle counter and the clock and are not random.
pub fn seed() -> ([u8; 16], bool) {
    let mut out = [0u8; 16];
    // Safety: CPUID has no side effect; RDRAND is executed only if the processor says it has it.
    let hardware = unsafe {
        if core::arch::x86_64::__cpuid(1).ecx & (1 << 30) != 0 {
            let mut ok = true;
            for half in 0..2 {
                let mut v: u64 = 0;
                let mut good: u8 = 0;
                for _ in 0..16 {
                    asm!("rdrand {}", "setc {}", out(reg) v, out(reg_byte) good, options(nomem, nostack));
                    if good != 0 {
                        break;
                    }
                }
                ok &= good != 0;
                out[half * 8..half * 8 + 8].copy_from_slice(&v.to_le_bytes());
            }
            ok
        } else {
            false
        }
    };
    if !hardware {
        let (y, mo, d, h, mi, s) = clock();
        let a = cycles() ^ ((y as u64) << 40 | (mo as u64) << 32 | (d as u64) << 24 | (h as u64) << 16 | (mi as u64) << 8 | s as u64);
        let b = cycles().rotate_left(29) ^ a.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        out[..8].copy_from_slice(&a.to_le_bytes());
        out[8..].copy_from_slice(&b.to_le_bytes());
    }
    (out, hardware)
}

/// The address of the unmapped page below the kernel stack.
pub fn stack_guard() -> usize {
    extern "C" {
        static boot_stack_guard: u8;
    }
    core::ptr::addr_of!(boot_stack_guard) as usize
}

/// The kernel's code and read-only data as they are in memory, for measuring the running image.
pub fn image() -> &'static [u8] {
    let [text_start, _te, _rs, ro_end, _end] = layout();
    // Safety: the range is the kernel's own code and read-only data, mapped readable for the life of the
    // kernel and never written after loading.
    unsafe { core::slice::from_raw_parts(text_start as *const u8, ro_end - text_start) }
}

// ---- exceptions and interrupts ----------------------------------------------------------------------

/// What `isr_common` in `boot.s` leaves on the stack, lowest address first.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Frame {
    regs: [u64; 15],
    /// The vector number.
    pub vector: u64,
    /// The error code, or zero where the processor supplies none.
    pub error: u64,
    /// The instruction that was interrupted or that faulted.
    pub rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

impl Frame {
    /// The first frame of a program in user mode: ring 3 segments, interrupts allowed, nothing in any register.
    pub fn user(entry: usize, stack_top: usize) -> Frame {
        Frame { regs: [0; 15], vector: 0, error: 0, rip: entry as u64, cs: 0x23, rflags: 0x202, rsp: stack_top as u64, ss: 0x1B }
    }

    /// True if the processor was in user mode when this frame was made.
    pub fn from_user(&self) -> bool {
        self.cs & 3 == 3
    }

    /// What the processor runs when no program is ready: the idle loop, in the kernel, on a stack of its own, with interrupts allowed.
    pub fn idle() -> Frame {
        extern "C" {
            fn idle_loop();
        }
        Frame { regs: [0; 15], vector: 0, error: 0, rip: idle_loop as usize as u64, cs: 0x08, rflags: 0x202,
                rsp: (IDLE_STACK.0.get() as usize + 4096 + IDLE_STACK_SIZE) as u64, ss: 0x10 }
    }

    /// A system call's number and its two arguments: RAX, RDI, RSI.
    pub fn call(&self) -> (u64, u64, u64) {
        (self.regs[14], self.regs[8], self.regs[9])
    }

    /// A call made with the `syscall` instruction: its number (RAX) and six arguments (RDI, RSI, RDX, R10, R8, R9).
    pub fn linux_call(&self) -> (u64, [u64; 6]) {
        (self.regs[14], [self.regs[8], self.regs[9], self.regs[12], self.regs[5], self.regs[7], self.regs[6]])
    }

    /// Run the `syscall` instruction again when the program is next resumed: for a call that must wait.
    /// The instruction is two bytes long and RAX still holds the call's number.
    pub fn retry_call(&mut self) {
        self.rip -= 2;
    }

    /// The answer to a system call, which the program finds in RAX.
    pub fn answer(&mut self, value: u64) {
        self.regs[14] = value;
    }
}

/// Why the processor left user mode, or was asked to enter it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trap {
    /// The kernel asks to run the programs that are ready (vector 129, from ring 0 only).
    Enter,
    /// The timer interrupted a program.
    Timer,
    /// A program made a system call through the first gate (vector 128): the five calls of the built-in programs.
    Call,
    /// A program made a system call with the `syscall` instruction.
    Syscall,
    /// A device interrupted: something that was waiting may be able to go on.
    Device,
    /// A program did something the processor refuses.
    Fault { vector: u64, error: u64, address: u64 },
}

const NONE: u64 = u64::MAX;
static PROBE_ARMED: AtomicBool = AtomicBool::new(false);
static PROBE_VECTOR: AtomicU64 = AtomicU64::new(NONE);
static PROBE_ERROR: AtomicU64 = AtomicU64::new(0);
static TICKS: AtomicU64 = AtomicU64::new(0);

extern "C" {
    static isr_table: [u64; 51];
    static probe_resume: u8;
    fn probe_write(addr: *mut u8, value: u8);
    fn probe_read(addr: *const u8) -> u32;
    fn probe_exec(addr: *const u8);
}

/// Entered from `isr_common` for every exception and interrupt.
#[no_mangle]
pub extern "C" fn exception_entry(frame: &mut Frame) {
    let user = frame.from_user();
    match frame.vector {
        32 => {
            let ticks = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
            outb(0x20, 0x20); // end of interrupt
            crate::dev::tick(ticks);
            crate::user::trap(frame, Trap::Timer, user);
        }
        v @ 33..=47 => {
            let line = (v - 32) as u8;
            // Lines 7 and 15 can fire with nothing behind them; the controller says whether one is really in service.
            if line == 7 || line == 15 {
                let port = if line == 7 { 0x20 } else { 0xA0 };
                outb(port, 0x0B);
                if inb(port) & 0x80 == 0 {
                    if line == 15 {
                        outb(0x20, 0x20);
                    }
                    return;
                }
            }
            crate::dev::interrupt(line);
            if line >= 8 {
                outb(0xA0, 0x20);
            }
            outb(0x20, 0x20);
            crate::user::trap(frame, Trap::Device, user);
        }
        128 if user => crate::user::trap(frame, Trap::Call, true),
        129 if !user => crate::user::trap(frame, Trap::Enter, false),
        130 if user => crate::user::trap(frame, Trap::Syscall, true),
        // Anything else a program in user mode causes ends that program, and only that program.
        v if user => {
            let cr2: u64;
            // Safety: reading CR2 has no side effect.
            unsafe { asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags)) };
            crate::user::trap(frame, Trap::Fault { vector: v, error: frame.error, address: cr2 }, true)
        }
        // A probe is one memory access: it can raise a page fault or a protection fault and nothing else.
        // Anything else that arrives while a probe is armed is not the probe's, and stops the kernel.
        v if (v == 14 || v == 13) && PROBE_ARMED.swap(false, Ordering::SeqCst) => {
            PROBE_VECTOR.store(v, Ordering::SeqCst);
            PROBE_ERROR.store(frame.error, Ordering::SeqCst);
            frame.rip = core::ptr::addr_of!(probe_resume) as u64;
        }
        v => {
            let cr2: u64;
            // Safety: reading CR2 has no side effect.
            unsafe { asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags)) };
            crate::fault(v, frame.error, frame.rip, cr2)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Gate {
    low: u16,
    selector: u16,
    ist: u8,
    kind: u8,
    mid: u16,
    high: u32,
    zero: u32,
}

#[repr(align(4096))] // exactly one page, which `protect` maps read-only
struct Idt(core::cell::UnsafeCell<[Gate; 256]>);
// Safety: written once by `interrupts_init` before interrupts are possible; afterwards only the processor reads it.
unsafe impl Sync for Idt {}
static IDT: Idt = Idt(core::cell::UnsafeCell::new([Gate { low: 0, selector: 0, ist: 0, kind: 0, mid: 0, high: 0, zero: 0 }; 256]));

/// Three exceptions must be deliverable whatever state the stack in use is in: a double fault (the usual
/// cause is that the stack is exhausted), a non-maskable interrupt and a machine check (either can arrive
/// at any instruction). Each has a stack of its own, and each stack has an unmapped page below it.
const FAULT_STACK_SIZE: usize = 16 * 1024;
const FAULT_SLOT: usize = 4096 + FAULT_STACK_SIZE;
#[repr(align(4096))]
struct FaultStacks(core::cell::UnsafeCell<[u8; 3 * FAULT_SLOT]>);
// Safety: used only by the processor, as the stacks of those three handlers.
unsafe impl Sync for FaultStacks {}
static FAULT_STACKS: FaultStacks = FaultStacks(core::cell::UnsafeCell::new([0; 3 * FAULT_SLOT]));

/// The stack the processor switches to when an interrupt, an exception or a system call arrives from user
/// mode. One is enough: the kernel finishes each entry before it returns, and there is one processor.
/// An unmapped page lies below it.
const ENTRY_STACK_SIZE: usize = 64 * 1024;
#[repr(align(4096))]
struct EntryStack(core::cell::UnsafeCell<[u8; 4096 + ENTRY_STACK_SIZE]>);
// Safety: used only by the processor, as the stack of entries from user mode.
unsafe impl Sync for EntryStack {}
static ENTRY_STACK: EntryStack = EntryStack(core::cell::UnsafeCell::new([0; 4096 + ENTRY_STACK_SIZE]));

/// The stack the processor is on while no program is ready. An interrupt that arrives then is handled on
/// it, so it is as large as the entry stack. An unmapped page lies below it.
const IDLE_STACK_SIZE: usize = 64 * 1024;
#[repr(align(4096))]
struct IdleStack(core::cell::UnsafeCell<[u8; 4096 + IDLE_STACK_SIZE]>);
// Safety: used only by the processor, as the stack of the idle loop.
unsafe impl Sync for IdleStack {}
static IDLE_STACK: IdleStack = IdleStack(core::cell::UnsafeCell::new([0; 4096 + IDLE_STACK_SIZE]));

/// The segment table and the task state segment, alone on one page, which `protect` maps read-only.
#[repr(C, align(4096))]
struct Segments {
    gdt: [u64; 7],
    tss: [u32; 26],
}
struct SegmentPage(core::cell::UnsafeCell<Segments>);
// Safety: written once by `interrupts_init`, before `protect`; afterwards only the processor reads it.
unsafe impl Sync for SegmentPage {}
static SEGMENTS: SegmentPage = SegmentPage(core::cell::UnsafeCell::new(Segments { gdt: [0; 7], tss: [0; 26] }));

/// The address of the page that holds the segment table and the task state segment.
pub fn segment_tables() -> usize {
    SEGMENTS.0.get() as usize
}

/// The address of the interrupt table, which `protect` maps read-only.
pub fn interrupt_table() -> usize {
    IDT.0.get() as usize
}

#[repr(C, packed)]
struct TablePointer {
    limit: u16,
    base: u64,
}

/// Install handlers for every vector, so that any exception is reported and stops the kernel instead of
/// resetting the machine, and prepare the interrupt controller and a timer of about 100 ticks a second.
/// Interrupts stay off until [`interrupts_on`].
pub fn interrupts_init() {
    // Safety: the table is static storage used for nothing else, filled before it is loaded; the stubs are
    // the code in boot.s whose addresses isr_table lists; the selector is the kernel code segment.
    unsafe {
        let idt = IDT.0.get();
        for v in 0..256usize {
            let stub = match v {
                0..=47 => isr_table[v],
                128 => isr_table[48],
                129 => isr_table[49],
                _ => isr_table[50],
            };
            let ist = match v {
                8 => 1,  // double fault
                2 => 2,  // non-maskable interrupt
                18 => 3, // machine check
                _ => 0,
            };
            // Vector 128 is the one gate a program in user mode may call. Every other vector is the processor's or the kernel's.
            let kind = if v == 128 { 0xEE } else { 0x8E };
            (*idt)[v] = Gate { low: stub as u16, selector: 0x08, ist, kind, mid: (stub >> 16) as u16, high: (stub >> 32) as u32, zero: 0 };
        }
        // Segment table with a task state segment, whose only use here is to name the three stacks.
        let seg = SEGMENTS.0.get();
        let tss = core::ptr::addr_of_mut!((*seg).tss) as *mut u32;
        for n in 0..3usize {
            let top = (FAULT_STACKS.0.get() as usize + n * FAULT_SLOT + FAULT_SLOT) as u64;
            tss.add(9 + 2 * n).write(top as u32); // interrupt stack n+1, at byte 36 + 8n
            tss.add(10 + 2 * n).write((top >> 32) as u32);
        }
        let entry_top = (ENTRY_STACK.0.get() as usize + 4096 + ENTRY_STACK_SIZE) as u64;
        tss.add(1).write(entry_top as u32); // the stack for entries from user mode, at byte 4
        tss.add(2).write((entry_top >> 32) as u32);
        tss.add(25).write(104 << 16); // no I/O permission map: a program in user mode reaches no device port
        let base = tss as u64;
        let gdt = core::ptr::addr_of_mut!((*seg).gdt);
        // The "accessed" bit of each segment is set here, so that the processor never needs to write the
        // table again and the page can be read-only. Loading the task register marks the task busy: that
        // write happens below, before `protect`.
        // Kernel code and data (0x08, 0x10), user data and code (0x18, 0x20; used as 0x1B and 0x23), the task state segment (0x28).
        *gdt = [0, 0x00AF_9B00_0000_FFFF, 0x00CF_9300_0000_FFFF, 0x00CF_F300_0000_FFFF, 0x00AF_FB00_0000_FFFF,
                103 | (base & 0xFF_FFFF) << 16 | 0x89 << 40 | (base >> 24 & 0xFF) << 56, base >> 32];
        let g = TablePointer { limit: 55, base: gdt as u64 };
        asm!("lgdt [{}]", in(reg) &g, options(readonly, nostack, preserves_flags));
        asm!("ltr {0:x}", in(reg) 0x28u16, options(nostack, preserves_flags));
        let p = TablePointer { limit: (core::mem::size_of::<[Gate; 256]>() - 1) as u16, base: idt as u64 };
        asm!("lidt [{}]", in(reg) &p, options(readonly, nostack, preserves_flags));
    }
    // The two interrupt controllers: vectors 32 to 47, everything masked except the timer and the line that
    // joins the second controller to the first. A driver unmasks its own line (`interrupt_line_on`).
    for (port, value) in [(0x20, 0x11), (0xA0, 0x11), (0x21, 0x20), (0xA1, 0x28), (0x21, 4), (0xA1, 2), (0x21, 1), (0xA1, 1), (0x21, 0xFA), (0xA1, 0xFF)] {
        outb(port, value);
    }
    // The interval timer: 1193182 Hz divided by 1193, which is 1000.15 ticks a second.
    outb(0x43, 0x36);
    outb(0x40, (1193u16 & 0xff) as u8);
    outb(0x40, (1193u16 >> 8) as u8);
}

/// Timer ticks in a second, to the nearest whole number.
pub const TICKS_PER_SECOND: u64 = 1000;

/// Let one of the sixteen device lines interrupt. Called by the driver of the device on that line.
pub fn interrupt_line_on(line: u8) {
    if line < 16 {
        let on = interrupts_save_off();
        let port = if line < 8 { 0x21 } else { 0xA1 };
        outb(port, inb(port) & !(1 << (line & 7)));
        interrupts_restore(on);
    }
}

/// Run `f` with interrupts refused, and then allow them again if they were allowed before.
pub fn without_interrupts<R>(f: impl FnOnce() -> R) -> R {
    let on = interrupts_save_off();
    let r = f();
    interrupts_restore(on);
    r
}

/// Stop the processor until the next interrupt. For kernel code that waits with interrupts allowed.
pub fn wait_for_interrupt() {
    // Safety: no memory effect; an interrupt resumes execution after the instruction.
    unsafe { asm!("hlt", options(nomem, nostack)) }
}

/// Allow interrupts.
pub fn interrupts_on() {
    // Safety: handlers for every vector are installed.
    unsafe { asm!("sti", options(nomem, nostack)) }
}

/// Refuse interrupts.
pub fn interrupts_off() {
    // Safety: no memory effect.
    unsafe { asm!("cli", options(nomem, nostack)) }
}

/// Timer ticks since interrupts were first allowed.
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Wait until the tick count reaches `target`, giving up after a bounded number of polls so that a timer
/// that never fires is a failed check and not a hang. Returns whether the target was reached.
pub fn wait_until(target: u64) -> bool {
    for _ in 0..2_000_000_000u64 {
        if ticks() >= target {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// What a probe found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    /// The processor refused the access with a page fault; the fault's error code is given.
    Refused(u64),
    /// The processor raised a different exception; its vector is given.
    Other(u64),
    /// The access went through.
    Allowed,
}

fn probe(run: impl FnOnce()) -> Probe {
    PROBE_VECTOR.store(NONE, Ordering::SeqCst);
    PROBE_ARMED.store(true, Ordering::SeqCst);
    run();
    PROBE_ARMED.store(false, Ordering::SeqCst);
    match PROBE_VECTOR.load(Ordering::SeqCst) {
        NONE => Probe::Allowed,
        14 => Probe::Refused(PROBE_ERROR.load(Ordering::SeqCst)),
        v => Probe::Other(v),
    }
}

/// Try to write to an address the byte it already holds, so that nothing changes even if the write is allowed.
pub fn probe_write_same(addr: usize) -> Probe {
    // Safety: the address is mapped readable (the callers pass code and read-only data of the kernel); the
    // write stores the value just read, and a refused write is resumed by the exception handler.
    probe(|| unsafe {
        let p = addr as *mut u8;
        probe_write(p, core::ptr::read_volatile(p))
    })
}

/// Try to read one byte.
pub fn probe_read_at(addr: usize) -> Probe {
    // Safety: a read has no effect if allowed, and a refused read is resumed by the exception handler.
    probe(|| unsafe {
        probe_read(addr as *const u8);
    })
}

struct RetCell(core::cell::UnsafeCell<[u8; 16]>);
// Safety: never written after initialization.
unsafe impl Sync for RetCell {}
static RET_IN_DATA: RetCell = RetCell(core::cell::UnsafeCell::new([0xC3; 16])); // sixteen "return" instructions, in writable data

/// Try to execute an instruction that lies in writable data. The instruction is a bare return, so that
/// nothing happens even if execution is allowed.
pub fn probe_execute_data() -> Probe {
    // Safety: see above; a refused fetch is resumed by the exception handler.
    probe(|| unsafe { probe_exec(RET_IN_DATA.0.get() as *const u8) })
}

/// Try to execute an instruction placed in a frame of the pool. The instruction is a bare return, so that
/// nothing happens even if execution is allowed. The frame must be one the caller holds.
pub fn probe_execute_at(frame: usize) -> Probe {
    if !frame_write(frame, 0, &[0xC3]) {
        return Probe::Other(0);
    }
    // Safety: see `probe_execute_data`; the target is a pool frame holding a return instruction.
    probe(|| unsafe { probe_exec(frame as *const u8) })
}

/// One register of the machine's clock. The index is written with its top bit clear, as is usual: that
/// bit masks the non-maskable interrupt, and this kernel leaves it able to arrive (it has a handler for it)
/// instead of switching it off for good as a side effect of reading the time.
fn cmos(reg: u8) -> u8 {
    outb(0x70, reg & 0x7F);
    inb(0x71)
}

/// The time of the machine's clock as (year, month, day, hour, minute, second). The clock is whatever the
/// machine was set to; nothing authenticates it.
pub fn clock() -> (u32, u32, u32, u32, u32, u32) {
    for _ in 0..1_000_000 {
        if cmos(0x0A) & 0x80 == 0 {
            break;
        }
    }
    let b = cmos(0x0B);
    let fix = |v: u8| -> u32 { if b & 4 == 0 { ((v >> 4) * 10 + (v & 15)) as u32 } else { v as u32 } };
    (2000 + fix(cmos(9)), fix(cmos(8)), fix(cmos(7)), fix(cmos(4) & 0x7f), fix(cmos(2)), fix(cmos(0)))
}

/// Recurse until the stack is exhausted. For the overflow demonstration only: it ends in `crate::fault`.
#[cfg(feature = "overflow-demo")]
pub fn exhaust_stack(depth: u64) -> u64 {
    let pad = [depth; 64];
    core::hint::black_box(&pad);
    exhaust_stack(depth + 1) + pad[(depth % 64) as usize]
}

/// Read one byte with no probe armed. For the fault demonstration only: at an unmapped address this ends
/// in `crate::fault`, which reports and stops.
#[cfg(feature = "fault-demo")]
pub fn read_unguarded(addr: usize) {
    // Safety: deliberately faults; the exception handler stops the kernel.
    unsafe {
        probe_read(addr as *const u8);
    }
}

// ---- user mode: address spaces and entry --------------------------------------------------------------

/// State that the kernel and its entry from user mode share.
pub struct Single<T>(core::cell::UnsafeCell<T>);
// Safety: there is one processor, and every use that changes the value is made with interrupts off (inside
// an entry from user mode, or in kernel code that has turned them off), so no two uses overlap. `with` is
// never called from inside `with`. A value written once before interrupts are first allowed may be read at any time.
unsafe impl<T> Sync for Single<T> {}
impl<T> Single<T> {
    pub const fn new(value: T) -> Self {
        Single(core::cell::UnsafeCell::new(value))
    }

    /// Use the value. Interrupts must be off, and the closure must not use the same value again.
    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        // Safety: see the Sync implementation above.
        unsafe { f(&mut *self.0.get()) }
    }
}

/// Where a program's memory begins: the second 512 GiB of the address space, which the kernel never uses.
pub const USER_BASE: usize = 0x80_0000_0000;
/// Where it ends (exclusive).
pub const USER_END: usize = USER_BASE + (1 << 39);
/// A built-in program's one page of data.
pub const USER_DATA: usize = USER_BASE + 0x10000;
/// The top of a built-in program's stack: one page, with an absent page below it.
pub const USER_STACK_TOP: usize = USER_BASE + 0x21000;

/// One program's address space: its own top-level table, in which the kernel is mapped for the kernel
/// only and the program's pages are mapped for the program. Every table and every page of the program is
/// a frame of the pool that this space owns, except pages of device memory, which are marked as not owned.
pub struct Space {
    root: usize,
    /// For a built-in program: its code, data and stack frames.
    three: [usize; 3],
}

fn current_root() -> usize {
    let cr3: u64;
    // Safety: reading CR3 has no side effect.
    unsafe { asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags)) };
    (cr3 & ADDRESS) as usize
}

impl Space {
    /// An address space with nothing in it for a program.
    pub fn empty() -> Option<Space> {
        let root = crate::mem::take()?;
        // Safety: `root` is a zeroed pool frame owned by this space; the kernel's second-level table is only read by the processor.
        unsafe {
            let kernel_pdpt = (TABLES.0.get() as *mut [u64; 512]).add(1) as u64;
            *(root as *mut u64) = kernel_pdpt | P | W; // the kernel: present in every space, and never with the user bit
        }
        Some(Space { root, three: [0; 3] })
    }

    /// The space of a built-in program: `code` (at most one page) at [`USER_BASE`], executable and not
    /// writable; a page of data and a page of stack, writable and not executable.
    pub fn new(code: &[u8]) -> Option<Space> {
        if code.is_empty() || code.len() > 4096 {
            return None;
        }
        let mut s = Space::empty()?;
        // Map each frame as it is acquired. A successful mapping transfers ownership to `s`;
        // the current, not-yet-mapped frame is the only one that release cannot reach.
        for (i, addr) in [USER_BASE, USER_DATA, USER_STACK_TOP - 0x1000].into_iter().enumerate() {
            let Some(frame) = crate::mem::take() else {
                s.release();
                return None;
            };
            if !s.map(addr, frame, i != 0, i == 0) {
                crate::mem::give(frame);
                s.release();
                return None;
            }
            s.three[i] = frame;
        }
        frame_write(s.three[0], 0, code);
        Some(s)
    }

    /// The physical pages a built-in program was given: code, data, stack.
    pub fn frames(&self) -> [usize; 3] {
        self.three
    }

    /// The leaf entry for a user address, making the tables on the way if asked to.
    fn slot(&self, addr: usize, create: bool) -> Option<*mut u64> {
        if addr < USER_BASE || addr >= USER_END {
            return None;
        }
        // Safety: every table reached is a pool frame this space owns, mapped at its own address in the
        // kernel; index 1 of the top table is the only one followed, so the kernel's tables are never reached.
        unsafe {
            let mut table = self.root as *mut u64;
            for level in (1..4).rev() {
                let e = table.add((addr >> (12 + 9 * level)) & 511);
                if *e & P == 0 {
                    if !create {
                        return None;
                    }
                    *e = crate::mem::take()? as u64 | P | W | U;
                }
                table = (*e & ADDRESS) as *mut u64;
            }
            Some(table.add((addr >> 12) & 511))
        }
    }

    fn flush(&self, addr: usize) {
        if current_root() == self.root {
            // Safety: drops one cached translation; no memory effect.
            unsafe { asm!("invlpg [{}]", in(reg) addr, options(nostack, preserves_flags)) }
        }
    }

    /// Give the program one frame of the pool at a page of its address space: readable, and writable or
    /// executable as asked, never both. False if the address is not the program's to have, is taken, or
    /// the frame is not of the pool, or no memory is left for the tables.
    pub fn map(&mut self, addr: usize, frame: usize, write: bool, exec: bool) -> bool {
        if addr % 4096 != 0 || (write && exec) || !in_pool(frame) {
            return false;
        }
        match self.slot(addr, true) {
            // Safety: the slot is a leaf entry of this space's own tables.
            Some(e) => unsafe {
                if *e & P != 0 {
                    return false;
                }
                *e = frame as u64 | P | U | if write { W } else { 0 } | if exec { 0 } else { NX };
                true
            },
            None => false,
        }
    }

    /// Give the program whole pages of mapped device memory, one after another from `addr`: readable and
    /// writable, never executable. Every byte of every page must be memory of one device that the kernel
    /// itself may write, and every page of the program's must be free; if not, nothing is mapped. The
    /// space does not own the pages and does not return them to the pool.
    pub fn map_device(&mut self, addr: usize, phys: usize, pages: usize) -> bool {
        let count = DEVICES.with(|d| d.1);
        let granted = DEVICE_BYTES.with(|b| b[..count].iter().any(|&bytes| framebuffer::grant(bytes, phys, pages)));
        if addr % 4096 != 0 || !granted {
            return false;
        }
        // First every slot is found, which makes any tables it needs, and seen to be free.
        for n in 0..pages {
            match self.slot(addr + n * 4096, true) {
                // Safety: the slot is a leaf entry of this space's own tables.
                Some(e) if unsafe { *e } & P == 0 => {}
                _ => return false,
            }
        }
        for n in 0..pages {
            match self.slot(addr + n * 4096, true) {
                // Safety: as above; the pass before found this slot and found it free.
                Some(e) => unsafe { *e = (phys + n * 4096) as u64 | P | U | W | NX | NOT_OWNED },
                None => return false,
            }
        }
        true
    }

    /// Give the program the memory kept for a model, piece after piece from `addr`, a page at a time:
    /// readable, never writable, never executable, and sealed so that no later call can change that.
    /// Only memory `model()` holds can be given. Every page of the program's must be free; if any is
    /// not, or there is no room for the tables, nothing is mapped. The space does not own the pages.
    #[cfg(feature = "model-memory")]
    pub fn map_model(&mut self, addr: usize) -> Option<usize> {
        let Model::Held { extents, count, .. } = model() else { return None };
        if addr % 4096 != 0 {
            return None;
        }
        let frames = |visit: &mut dyn FnMut(usize, u64) -> bool| -> Option<usize> {
            let mut page = 0usize;
            for &(base, bytes) in &extents[..count] {
                for n in 0..bytes.div_ceil(4096) {
                    if !visit(addr.checked_add(page.checked_mul(4096)?)?, base + n * 4096) {
                        return None;
                    }
                    page += 1;
                }
            }
            Some(page)
        };
        // First every slot is found, which makes any tables it needs, and seen to be free.
        // Safety (both passes): the slot is a leaf entry of this space's own tables.
        frames(&mut |at, _| matches!(self.slot(at, true), Some(e) if unsafe { *e } & P == 0))?;
        frames(&mut |at, frame| match self.slot(at, true) {
            Some(e) => {
                unsafe { *e = frame | P | U | NX | NOT_OWNED | SEALED };
                true
            }
            None => false,
        })
    }

    /// Take a page away from the program. Returns the frame if the space owned it, for the caller to give back to the pool.
    pub fn unmap(&mut self, addr: usize) -> Option<usize> {
        let e = self.slot(addr & !0xfff, false)?;
        // Safety: as `map`.
        let old = unsafe {
            let old = *e;
            *e = 0;
            old
        };
        self.flush(addr);
        if old & P != 0 && old & NOT_OWNED == 0 { Some((old & ADDRESS) as usize) } else { None }
    }

    /// Change what the program may do with a page it has: write or execute, never both. False if it has no such page.
    pub fn set_protection(&mut self, addr: usize, write: bool, exec: bool) -> bool {
        if write && exec {
            return false;
        }
        match self.slot(addr & !0xfff, false) {
            // Safety: as `map`.
            Some(e) => unsafe {
                if *e & P == 0 || *e & SEALED != 0 {
                    return false;
                }
                *e = (*e & (ADDRESS | NOT_OWNED)) | P | U | if write { W } else { 0 } | if exec { 0 } else { NX };
                self.flush(addr);
                true
            },
            None => false,
        }
    }

    fn leaf(&self, addr: usize) -> Option<u64> {
        let e = self.slot(addr, false)?;
        // Safety: as `map`.
        let v = unsafe { *e };
        if v & P != 0 { Some(v) } else { None }
    }

    /// The pool frame behind a page of the program's memory, if the program may read it (and write it, if asked).
    fn frame_of(&self, addr: usize, write: bool) -> Option<usize> {
        let e = self.leaf(addr)?;
        let frame = (e & ADDRESS) as usize;
        if e & U == 0 || (write && e & W == 0) || e & NOT_OWNED != 0 || !in_pool(frame) { None } else { Some(frame) }
    }

    /// What this space's tables say about a built-in program, read back from them: (pages the program can
    /// reach in its first 2 MiB, of those how many are both writable and executable, whether any kernel
    /// address is reachable from user mode, whether the page below the stack is absent).
    pub fn audit(&self) -> (u32, u32, bool, bool) {
        let (mut reach, mut both) = (0, 0);
        for page in 0..512usize {
            if let Some(e) = self.leaf(USER_BASE + (page << 12)) {
                if e & U != 0 {
                    reach += 1;
                    if e & W != 0 && e & NX == 0 {
                        both += 1;
                    }
                }
            }
        }
        (reach, both, self.kernel_reachable(), self.leaf(USER_STACK_TOP - 0x2000).is_none())
    }

    /// True if the first entry of the top table, which is the kernel's, or anything below it, is marked for user mode.
    fn kernel_reachable(&self) -> bool {
        // Safety: reads this space's top table and the kernel's own tables.
        unsafe {
            let first = *(self.root as *const u64);
            first & U != 0 || (first & ADDRESS) as usize != (TABLES.0.get() as *const [u64; 512]).add(1) as usize
        }
    }

    /// Every page of the program's, counted from the tables: (pages, of those both writable and executable,
    /// pages of device memory, whether the kernel is reachable from user mode).
    pub fn census(&self) -> (usize, usize, usize, bool) {
        let (mut pages, mut both, mut device) = (0, 0, 0);
        // Safety: walks this space's own tables below index 1 of the top table.
        unsafe {
            let top = *(self.root as *const u64).add(1);
            if top & P != 0 {
                let l3 = (top & ADDRESS) as *const u64;
                for a in 0..512 {
                    let e3 = *l3.add(a);
                    if e3 & P == 0 {
                        continue;
                    }
                    let l2 = (e3 & ADDRESS) as *const u64;
                    for b in 0..512 {
                        let e2 = *l2.add(b);
                        if e2 & P == 0 {
                            continue;
                        }
                        let l1 = (e2 & ADDRESS) as *const u64;
                        for c in 0..512 {
                            let e = *l1.add(c);
                            if e & P != 0 {
                                pages += 1;
                                if e & W != 0 && e & NX == 0 {
                                    both += 1;
                                }
                                if e & NOT_OWNED != 0 {
                                    device += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        (pages, both, device, self.kernel_reachable())
    }

    /// Copy bytes out of the program's memory. False if any of them is not memory the program itself may
    /// read. The kernel reads through its own mapping of the same frames: it never touches a user address,
    /// so the processor's supervisor access prevention stays on throughout.
    pub fn copy_in(&self, addr: usize, out: &mut [u8]) -> bool {
        let (mut at, mut done) = (addr, 0usize);
        while done < out.len() {
            let n = (4096 - (at & 0xfff)).min(out.len() - done);
            let frame = match self.frame_of(at, false) {
                Some(f) => f,
                None => return false,
            };
            // Safety: `n` bytes inside one pool frame that this space maps for its program; the destination is a kernel slice.
            unsafe { core::ptr::copy_nonoverlapping((frame + (at & 0xfff)) as *const u8, out.as_mut_ptr().add(done), n) };
            done += n;
            at = match at.checked_add(n) {
                Some(a) => a,
                None => return false,
            };
        }
        true
    }

    /// Copy bytes into the program's memory. False if any destination is not memory the program itself may write.
    pub fn copy_out(&self, addr: usize, data: &[u8]) -> bool {
        let (mut at, mut done) = (addr, 0usize);
        while done < data.len() {
            let n = (4096 - (at & 0xfff)).min(data.len() - done);
            let frame = match self.frame_of(at, true) {
                Some(f) => f,
                None => return false,
            };
            // Safety: as `copy_in`, in the other direction.
            unsafe { core::ptr::copy_nonoverlapping(data.as_ptr().add(done), (frame + (at & 0xfff)) as *mut u8, n) };
            done += n;
            at = match at.checked_add(n) {
                Some(a) => a,
                None => return false,
            };
        }
        true
    }

    /// Copy `len` bytes a program names in its own memory, at most a mebibyte. `None` if any of them is not memory it may
    /// read, or if the kernel has no memory left to copy them into.
    pub fn read(&self, addr: usize, len: usize) -> Option<alloc::vec::Vec<u8>> {
        if len > 1 << 20 {
            return None;
        }
        // The buffer's size comes from the program. If the kernel has not that much memory left, the
        // call is refused like any other that cannot be carried out: an allocation that cannot fail
        // would stop the kernel instead, on a number a program chose.
        let mut out = alloc::vec::Vec::new();
        if out.try_reserve_exact(len).is_err() {
            return None;
        }
        out.resize(len, 0);
        if self.copy_in(addr, &mut out) { Some(out) } else { None }
    }

    /// Make this the active address space.
    pub fn activate(&self) {
        if current_root() != self.root {
            // Safety: the space maps the kernel exactly as the kernel's own tables do, so every kernel address in use stays valid.
            unsafe { asm!("mov cr3, {}", in(reg) self.root as u64, options(nostack, preserves_flags)) }
        }
    }

    /// Give everything back to the pool: every frame the space owns and every table. The space must not be the active one.
    pub fn release(self) -> usize {
        assert!(current_root() != self.root, "an address space was released while the processor was using it");
        let mut freed = 0;
        // Safety: walks this space's own tables; nothing refers to them once the space is consumed.
        unsafe {
            let top = *(self.root as *const u64).add(1);
            if top & P != 0 {
                let l3 = (top & ADDRESS) as usize;
                for a in 0..512 {
                    let e3 = *(l3 as *const u64).add(a);
                    if e3 & P == 0 {
                        continue;
                    }
                    let l2 = (e3 & ADDRESS) as usize;
                    for b in 0..512 {
                        let e2 = *(l2 as *const u64).add(b);
                        if e2 & P == 0 {
                            continue;
                        }
                        let l1 = (e2 & ADDRESS) as usize;
                        for c in 0..512 {
                            let e = *(l1 as *const u64).add(c);
                            if e & P != 0 && e & NOT_OWNED == 0 {
                                crate::mem::give((e & ADDRESS) as usize);
                                freed += 1;
                            }
                        }
                        crate::mem::give(l1);
                        freed += 1;
                    }
                    crate::mem::give(l2);
                    freed += 1;
                }
                crate::mem::give(l3);
                freed += 1;
            }
        }
        crate::mem::give(self.root);
        freed + 1
    }
}

/// Make the kernel's own address space the active one, in which no user memory exists.
pub fn kernel_space() {
    // Safety: TABLES is the mapping `protect` installed; the kernel runs in it.
    unsafe { asm!("mov cr3, {}", in(reg) TABLES.0.get() as u64, options(nostack, preserves_flags)) }
}

/// True if the active address space is the kernel's own.
pub fn in_kernel_space() -> bool {
    current_root() == TABLES.0.get() as usize
}

/// Run the programs that are ready. Returns when none is left. The kernel's own state is kept by the
/// entry at vector 129 and restored by the entry that finds nothing left to run.
pub fn enter_user() {
    // Safety: vector 129 is installed, reachable from ring 0 only, and handled by `user::trap`, which
    // returns here with every register as it was.
    unsafe { asm!("int 0x81") }
}

macro_rules! user_programs {
    ($($name:ident: $start:ident, $end:ident;)*) => {
        extern "C" { $(static $start: u8; static $end: u8;)* }
        /// The bytes of a program for user mode, from `boot.s`.
        pub fn user_program(name: &str) -> Option<&'static [u8]> {
            match name {
                $(stringify!($name) => {
                    let (a, b) = (core::ptr::addr_of!($start) as usize, core::ptr::addr_of!($end) as usize);
                    // Safety: the two symbols bound one program in the kernel's read-only data.
                    Some(unsafe { core::slice::from_raw_parts(a as *const u8, b - a) })
                })*
                _ => None,
            }
        }
    };
}
user_programs! {
    hello: user_hello_start, user_hello_end;
    polite: user_polite_start, user_polite_end;
    spinner: user_spinner_start, user_spinner_end;
    read_kernel: user_read_kernel_start, user_read_kernel_end;
    write_kernel: user_write_kernel_start, user_write_kernel_end;
    privileged: user_privileged_start, user_privileged_end;
    port: user_port_start, user_port_end;
    exec_data: user_exec_data_start, user_exec_data_end;
    overflow: user_overflow_start, user_overflow_end;
    bad_calls: user_bad_calls_start, user_bad_calls_end;
    mute: user_mute_start, user_mute_end;
    hog: user_hog_start, user_hog_end;
    sleeper: user_sleeper_start, user_sleeper_end;
    sleeper_long: user_sleeper_long_start, user_sleeper_long_end;
    key_reader: user_key_reader_start, user_key_reader_end;
}
