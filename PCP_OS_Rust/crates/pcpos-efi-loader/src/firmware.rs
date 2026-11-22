//! Unsafe calls are confined to firmware services and already owned physical pages.
//! Single threaded up to the one-way Multiboot transfer.
use crate::{
    cmdline::Line,
    elf::Elf,
    memory::{self, Map, Owned, Pages, Range, Span, PAGE},
    payload::{self, Media},
    Result, NAMES,
};
use core::{
    fmt::{self, Write},
    mem::{size_of, take},
    ptr, slice,
};
use pcpos_model_media::Manifest;
mod config {
    include!(env!("PCPOS_EFI_CONFIGURATION"));
}
#[repr(C)]
#[derive(Default)]
struct Mode {
    base: u64,
    size: u64,
    width: u32,
    height: u32,
    stride: u32,
    format: u32,
}
extern "C" {
    fn fw_watchdog() -> usize;
    fn fw_print(s: *const u16);
    fn fw_root(out: *mut usize) -> usize;
    fn fw_open(parent: usize, name: *const u16, out: *mut usize) -> usize;
    fn fw_read(f: usize, n: *mut usize, data: *mut u8) -> usize;
    fn fw_close(f: usize) -> usize;
    fn fw_map(
        len: *mut usize,
        buf: *mut u8,
        key: *mut usize,
        step: *mut usize,
        version: *mut u32,
    ) -> usize;
    fn fw_claim(pages: usize, address: *mut u64) -> usize;
    fn fw_free(addr: u64, pages: usize) -> usize;
    fn fw_exit(key: usize) -> usize;
    fn fw_acpi_table(modern: u32) -> u64;
    fn fw_display(out: *mut Mode) -> i32;
    fn transfer(info: usize, entry: usize, sp: usize) -> !;
    static transfer_end: u8;
}
const ERROR: usize = 1usize << 63;
fn ok(s: usize, msg: &'static str) -> Result<()> {
    if s & ERROR == 0 {
        Ok(())
    } else {
        Err(msg)
    }
}
struct Console;
impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for part in s.as_bytes().chunks(95) {
            let mut chars = [0u16; 96];
            for (o, &i) in chars.iter_mut().zip(part) {
                *o = i as u16
            }
            unsafe { fw_print(chars.as_ptr()) }
        }
        Ok(())
    }
}
struct Services {
    root: usize,
    folder: usize,
    cleanup_error: bool,
}
fn open(parent: usize, name: &[u8]) -> Result<usize> {
    if name.len() >= 32 || !name.iter().all(|c| c.is_ascii() && *c != 0) {
        return Err("Invalid internal file name.");
    }
    let mut path = [0u16; 32];
    for (o, &i) in path.iter_mut().zip(name) {
        *o = i as u16
    }
    let mut f = 0;
    ok(
        unsafe { fw_open(parent, path.as_ptr(), &mut f) },
        "A required file is missing or unreadable.",
    )?;
    if f == 0 {
        return Err("Firmware returned no file handle.");
    }
    Ok(f)
}
fn read(f: usize, buf: &mut [u8]) -> Result<usize> {
    let mut n = buf.len();
    ok(
        unsafe { fw_read(f, &mut n, buf.as_mut_ptr()) },
        "A required file could not be read to its expected length.",
    )?;
    if n > buf.len() {
        return Err("Firmware exceeded the requested file length.");
    }
    Ok(n)
}
fn close(f: usize) -> Result<()> {
    if f == 0 {
        return Ok(());
    }
    ok(
        unsafe { fw_close(f) },
        "A required file could not be closed.",
    )
}
impl Services {
    fn open_volume(&mut self) -> Result<()> {
        ok(
            unsafe { fw_root(&mut self.root) },
            "The loader could not open its own boot volume.",
        )?;
        if self.root == 0 {
            return Err("The loader could not open its own boot volume.");
        }
        self.folder = open(self.root, b"PCPOS")
            .map_err(|_| "The PCPOS folder is missing or unreadable on the boot volume.")?;
        Ok(())
    }
    fn close_volume(&mut self) -> Result<()> {
        let folder = close(take(&mut self.folder));
        let root = close(take(&mut self.root));
        self.cleanup_error |= folder.is_err() || root.is_err();
        folder.and(root)
    }
}
impl Media for Services {
    type File = usize;
    fn open(&mut self, name: &[u8]) -> Result<usize> {
        open(self.folder, name)
    }
    fn read(&mut self, file: usize, buf: &mut [u8]) -> Result<usize> {
        read(file, buf)
    }
    fn close(&mut self, file: usize) -> Result<()> {
        let r = close(file);
        self.cleanup_error |= r.is_err();
        r
    }
    fn progress(&mut self, done: u64, total: u64) {
        let _ = write!(Console, "Verified {done} of {total} model bytes.\r\n");
    }
}
impl Pages for Services {
    fn claim(&mut self, s: Span) -> Result<()> {
        let mut addr = s.start;
        ok(
            unsafe { fw_claim((s.size / PAGE) as usize, &mut addr) },
            "Firmware could not reserve the requested memory.",
        )?;
        if addr != s.start {
            self.cleanup_error |= unsafe { fw_free(addr, (s.size / PAGE) as usize) } & ERROR != 0;
            return Err("Firmware returned a different memory address.");
        }
        Ok(())
    }
    fn free(&mut self, s: Span) -> Result<()> {
        ok(
            unsafe { fw_free(s.start, (s.size / PAGE) as usize) },
            "Firmware could not release allocated pages.",
        )
    }
}
#[repr(align(8))]
struct Scratch([u8; 256 * 1024]);
impl Scratch {
    fn snapshot(&mut self) -> Result<(Map<'_>, usize)> {
        let mut n = self.0.len();
        let (mut key, mut step, mut ver) = (0, 0, 0);
        ok(
            unsafe { fw_map(&mut n, self.0.as_mut_ptr(), &mut key, &mut step, &mut ver) },
            "The firmware memory map could not be read.",
        )?;
        let b = self
            .0
            .get(..n)
            .ok_or("The firmware memory map exceeds the loader limit.")?;
        Ok((Map::new(b, step, ver)?, key))
    }
}
#[repr(C, align(16))]
struct Stack([u8; 16384]);
#[repr(C)]
struct Boot {
    info: [u32; 32],
    modules: [[u32; 4]; 3],
    names: [[u8; 20]; 3],
    line: [u8; 1024],
    ranges: [Range; 65536 / 24],
    stack: Stack,
}
fn prepare(b: &mut Boot, inputs: &[Span; 4], pieces: &[Span]) -> Result<()> {
    b.info[0] = 4 | 8 | 0x40;
    b.info[4] = b.line.as_ptr() as u32;
    b.info[5] = 3;
    b.info[6] = b.modules.as_ptr() as u32;
    b.info[12] = b.ranges.as_ptr() as u32;
    for (i, name) in [b"tailorbird".as_slice(), b"model-worker", b"model.manifest"]
        .into_iter()
        .enumerate()
    {
        b.names[i][..name.len()].copy_from_slice(name);
        b.modules[i] = [
            inputs[i + 1].start as u32,
            (inputs[i + 1].start + config::SIZES[i + 1] as u64) as u32,
            b.names[i].as_ptr() as u32,
            0,
        ];
    }
    let mut g = Mode::default();
    if unsafe { fw_display(&mut g) } == 1
        && g.format == 1
        && g.width > 0
        && g.height > 0
        && g.stride >= g.width
    {
        if let Some(pitch) = g.stride.checked_mul(4) {
            let size = u64::from(pitch) * u64::from(g.height);
            if g.base != 0
                && g.base % 4 == 0
                && size <= g.size
                && g.base.checked_add(size).is_some()
            {
                b.info[0] |= 1 << 12;
                b.info[22] = g.base as u32;
                b.info[23] = (g.base >> 32) as u32;
                b.info[24] = pitch;
                b.info[25] = g.width;
                b.info[26] = g.height;
                b.info[27] = u32::from_le_bytes([32, 1, 16, 8]);
                b.info[28] = u32::from_le_bytes([8, 8, 0, 8]);
            }
        }
    }
    let mut l = Line::new(&mut b.line);
    let err = "The kernel command line is too long.";
    write!(l,"image={} auto=tailorbird then=menu propose_image={} worker_for={} worker_image={} model_manifest={} model_memory=",config::MEASUREMENT,config::UI,config::UI,config::WORKER,config::MANIFEST).map_err(|_|err)?;
    let mut left = config::MODEL;
    for (i, p) in pieces.iter().enumerate() {
        let n = left.min(p.size);
        write!(
            l,
            "{}{:#x}:{:#x}",
            if i == 0 { "" } else { "," },
            p.start,
            n
        )
        .map_err(|_| err)?;
        left -= n;
    }
    if left != 0 {
        return Err("The kernel model command line is incomplete.");
    }
    l.append_rsdp(|modern| unsafe { fw_acpi_table(u32::from(modern)) })
        .map_err(|_| err)?;
    Ok(())
}
/// Caller holds the firmware allocation exclusively; initialize it before producing Rust references.
unsafe fn clear(s: Span) {
    ptr::write_bytes(s.start as *mut u8, 0, s.size as usize)
}
fn low(s: &mut Services, owned: &mut Owned, map: &mut Scratch, n: usize) -> Result<Span> {
    let size = (n as u64).next_multiple_of(PAGE);
    let span = memory::low(map.snapshot()?.0, config::END, size)?;
    owned.claim(s, span)?;
    unsafe { clear(span) };
    Ok(span)
}
fn boot(fw: &mut Services, owned: &mut Owned, map: &mut Scratch, exiting: &mut bool) -> Result<()> {
    ok(
        unsafe { fw_watchdog() },
        "Firmware could not disable the boot watchdog for model verification.",
    )?;
    if transfer as *const () as usize >= 1 << 32 || ptr::addr_of!(transfer_end) as usize > 1 << 32 {
        return Err(
            "Firmware placed the execution transition above its supported 32-bit address range.",
        );
    }
    let needed = config::MODEL.next_multiple_of(PAGE);
    let (plan,n)=memory::high_plan(map.snapshot()?.0,needed).map_err(|available|{
        let _=write!(Console,"Model memory required: {needed} bytes. Available in at most eight high ranges: {available} bytes.\r\n");
        "There is not enough usable high memory for this model."
    })?;
    let pieces = &plan[..n];
    for &p in pieces {
        owned.claim(fw, p)?;
    }
    let kernel = Span {
        start: config::BASE,
        size: config::END - config::BASE,
    };
    if !map
        .snapshot()?
        .0
        .iter()
        .any(|d| d.free() && d.span.start <= kernel.start && d.span.end() >= kernel.end())
    {
        return Err("The kernel's linked physical range is unavailable on this machine.");
    }
    owned.claim(fw, kernel)?;
    unsafe { clear(kernel) };
    let bs = low(fw, owned, map, size_of::<Boot>())?;
    let b = unsafe { &mut *(bs.start as *mut Boot) };
    let mut inputs = [Span::default(); 4];
    for (p, &size) in inputs.iter_mut().zip(&config::SIZES) {
        *p = low(fw, owned, map, size)?;
    }
    fw.open_volume()?;
    for (i, p) in inputs.iter().enumerate() {
        let bytes = unsafe { slice::from_raw_parts_mut(p.start as *mut u8, config::SIZES[i]) };
        if let Err(e) = payload::pinned(fw, NAMES[i].as_bytes(), bytes, config::PINS[i]) {
            let _ = write!(Console, "File: {}\r\n", NAMES[i]);
            return Err(e);
        }
    }
    let source = unsafe { slice::from_raw_parts(inputs[0].start as *const u8, config::SIZES[0]) };
    let elf = Elf::parse(source)?;
    if elf.base != kernel.start || elf.end != kernel.end() {
        return Err("Pinned kernel layout changed.");
    }
    for seg in elf.segments() {
        unsafe {
            ptr::copy_nonoverlapping(
                source[seg.offset..].as_ptr(),
                seg.start as *mut u8,
                seg.file,
            )
        }
    }
    owned.release(fw, inputs[0].start)?;
    let bytes = unsafe { slice::from_raw_parts(inputs[3].start as *const u8, config::SIZES[3]) };
    let manifest =
        Manifest::parse(bytes, config::PINS[3]).map_err(|_| "Invalid pinned model manifest.")?;
    if manifest.model_bytes() != config::MODEL {
        return Err("Pinned model geometry changed.");
    }
    payload::inventory(&manifest, |buf| read(fw.folder, buf))?;
    let mut dest: [&mut [u8]; 8] = core::array::from_fn(|_| &mut [][..]);
    for (d, &p) in dest.iter_mut().zip(pieces) {
        unsafe {
            clear(p);
            *d = slice::from_raw_parts_mut(p.start as *mut u8, p.size as usize)
        }
    }
    payload::load_model(fw, &manifest, &mut dest[..n])?;
    prepare(b, &inputs, pieces)?;
    fw.close_volume()?;
    let _ = write!(Console, "Model verified. Starting PCP OS.\r\n");
    memory::exit_with_map(
        || {
            let (snapshot, key) = map.snapshot()?;
            let ranges = memory::translate(snapshot, pieces, &mut b.ranges)?;
            b.info[11] = (ranges * size_of::<Range>()) as u32;
            Ok(key)
        },
        |key| {
            *exiting = true;
            match unsafe { fw_exit(key) } {
                0 => Ok(true),
                s if s == (ERROR | 2) => Ok(false),
                _ => Err("Firmware could not complete the handoff."),
            }
        },
    )?;
    unsafe {
        transfer(
            bs.start as usize,
            elf.entry as usize,
            b.stack.0.as_ptr().add(b.stack.0.len()) as usize,
        )
    }
}
#[no_mangle]
pub extern "C" fn loader_main() -> usize {
    static mut MAP: Scratch = Scratch([0; 256 * 1024]);
    let mut owned = Owned::default();
    let mut fw = Services {
        root: 0,
        folder: 0,
        cleanup_error: false,
    };
    let mut exiting = false;
    let Err(message) = boot(
        &mut fw,
        &mut owned,
        unsafe { &mut *ptr::addr_of_mut!(MAP) },
        &mut exiting,
    ) else {
        return ERROR | 1;
    };
    let closed = exiting || fw.close_volume().is_ok();
    let released = owned.rollback(&mut fw) && closed && !fw.cleanup_error;
    if !exiting {
        let _ = write!(
            Console,
            "PCP OS could not start. {message}\r\n{}\r\n",
            if released {
                "Loader resources released."
            } else {
                "Firmware reported an error while releasing resources."
            }
        );
    }
    ERROR | if released { 1 } else { 7 }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop()
    }
}
