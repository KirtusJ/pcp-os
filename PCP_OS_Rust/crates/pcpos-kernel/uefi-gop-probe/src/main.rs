#![no_std]
#![no_main]
//! Read-only default EFI GOP probe. Optional disposable-fixture mode change
//! requires explicit --features exercise-mode-switch during BUILD; this is
//! deliberately never enabled by default or linked into the PCP OS kernel.

use core::{ffi::c_void, mem::{size_of, transmute}, ptr};
use pcpos_gop_probe::{
    BootServices, EfiStatus, FreePoolFn, GOP_GUID, GraphicsOutput,
    GraphicsOutputMode, LocateProtocolFn, ModeInformation, SimpleTextOutput,
    SystemTable, BOOT_SERVICES_SIGNATURE, BS_FREE_POOL, BS_LOCATE_PROTOCOL,
    SYSTEM_TABLE_SIGNATURE, observed, safe_alternate_candidate,
};

const MAX_LISTED_MODES: u32 = 64;
const EFI_NOT_FOUND: usize = (1usize << 63) | 14;
const EFI_INVALID_PARAMETER: usize = (1usize << 63) | 2;
const EFI_UNSUPPORTED: usize = (1usize << 63) | 3;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop { core::hint::spin_loop(); }
}

/// Fixed-size UTF-16 console row, no allocator or format macros.
struct Line {
    units: [u16; 256],
    len: usize,
}
impl Line {
    fn new() -> Self { Self { units: [0; 256], len: 0 } }
    fn clear(&mut self) { self.len = 0; self.units[0] = 0; }
    fn text(&mut self, s: &str) {
        for c in s.bytes() {
            if self.len + 3 >= self.units.len() { break; }
            self.units[self.len] = c as u16;
            self.len += 1;
        }
    }
    fn number(&mut self, mut n: u64) {
        let mut digits = [0u8; 20];
        let mut pos = digits.len();
        loop {
            pos -= 1; digits[pos] = (n % 10) as u8 + b'0';
            n /= 10;
            if n == 0 { break; }
        }
        for c in &digits[pos..] { self.text(core::str::from_utf8(core::slice::from_ref(c)).unwrap_or("?")); }
    }
    fn hex(&mut self, n: u64) {
        self.text("0x");
        let chars = b"0123456789abcdef";
        for i in (0..16).rev() {
            let nibble = ((n >> (i*4)) & 15) as usize;
            let c = chars[nibble] as char;
            let mut tmp = [0u8; 4];
            self.text(c.encode_utf8(&mut tmp));
        }
    }
    /// Caller guarantees console is the valid UEFI boot-services ConOut.
    unsafe fn send(&mut self, con: *mut SimpleTextOutput) {
        self.text("\r\n");
        self.units[self.len] = 0;
        if !con.is_null() {
            let _ = unsafe { ((*con).output_string)(con, self.units.as_ptr()) };
        }
        self.clear();
    }
}

unsafe fn show_mode(con: *mut SimpleTextOutput, label: &str, gop: *mut GraphicsOutput) {
    if gop.is_null() { return; }
    let m = unsafe { (*gop).mode };
    let mut line = Line::new();
    line.text("PCPOS_GOP_PROBE_V1 ");
    line.text(label);
    if m.is_null() {
        line.text(" MODE_NULL");
        unsafe { line.send(con) };
        return;
    }
    let mode = unsafe { &*m };
    line.text(" mode="); line.number(mode.mode as u64);
    line.text(" max="); line.number(mode.max_mode as u64);
    line.text(" fbase="); line.hex(mode.framebuffer_base);
    line.text(" fbytes="); line.number(mode.framebuffer_size as u64);
    unsafe { line.send(con) };
    let info = mode.info;
    if info.is_null() || mode.size_of_info < size_of::<ModeInformation>() {
        line.text("PCPOS_GOP_PROBE_V1 "); line.text(label);
        line.text(" INFO_MISSING_OR_SHORT");
        unsafe { line.send(con) };
        return;
    }
    let i = unsafe { ptr::read_unaligned(info) };
    line.text("PCPOS_GOP_PROBE_V1 "); line.text(label);
    line.text(" width="); line.number(i.horizontal_resolution as u64);
    line.text(" height="); line.number(i.vertical_resolution as u64);
    line.text(" pitch_pixels="); line.number(i.pixels_per_scan_line as u64);
    line.text(" format="); line.number(i.pixel_format as u64);
    line.text(" version="); line.number(i.version as u64);
    unsafe { line.send(con) };
    line.text("PCPOS_GOP_PROBE_V1 "); line.text(label);
    line.text(" masks="); line.hex(i.pixel_information.red as u64);
    line.text(","); line.hex(i.pixel_information.green as u64);
    line.text(","); line.hex(i.pixel_information.blue as u64);
    line.text(","); line.hex(i.pixel_information.reserved as u64);
    unsafe { line.send(con) };
}

/// This method is explicitly a *standalone* application probe, not the
/// OS-controlled EFI loader callsite. If the optional mode-switch fixture is
/// built, the probe reports its own identity instead of claiming PCP OS
/// selected the mode or matched a physical monitor's EDID.
unsafe fn app(table: *mut SystemTable) -> EfiStatus {
    if table.is_null() { return EFI_INVALID_PARAMETER; }
    let st = unsafe { &*table };
    if st.hdr.signature != SYSTEM_TABLE_SIGNATURE || st.con_out.is_null() {
        return EFI_INVALID_PARAMETER;
    }
    let con = st.con_out;
    let bs_ptr = st.boot_services;
    if bs_ptr.is_null() { return EFI_INVALID_PARAMETER; }
    let bs = unsafe { &*bs_ptr };
    if bs.hdr.signature != BOOT_SERVICES_SIGNATURE
       || (bs.hdr.header_size as usize) < size_of::<BootServices>()
    {
        return EFI_INVALID_PARAMETER;
    }
    let locate: LocateProtocolFn = unsafe { transmute(bs.function_slots[BS_LOCATE_PROTOCOL]) };
    let free_pool: FreePoolFn = unsafe { transmute(bs.function_slots[BS_FREE_POOL]) };
    let mut interface: *mut c_void = ptr::null_mut();
    let status = unsafe { locate(&GOP_GUID, ptr::null_mut(), &mut interface) };
    let mut line = Line::new();
    if status != 0 || interface.is_null() {
        line.text("PCPOS_GOP_PROBE_V1 NO_GOP status=");
        line.hex(status as u64);
        unsafe { line.send(con) };
        return if status != 0 { status } else { EFI_NOT_FOUND };
    }
    let gop = interface.cast::<GraphicsOutput>();
    let mode = unsafe { (*gop).mode };
    if mode.is_null() { return EFI_UNSUPPORTED; }
    line.text("PCPOS_GOP_PROBE_V1 STANDALONE first_gop=");
    line.hex(gop as usize as u64);
    line.text(" no_edid_association");
    unsafe { line.send(con) };
    unsafe { show_mode(con, "BEFORE", gop) };
    let original = unsafe { (*mode).mode };
    let max_modes = unsafe { (*mode).max_mode };
    let bounded = core::cmp::min(max_modes, MAX_LISTED_MODES);
    #[cfg(feature="exercise-mode-switch")]
    let mut candidate: Option<u32> = None;

    for index in 0..bounded {
        let mut size = 0usize;
        let mut ptr_info: *mut ModeInformation = ptr::null_mut();
        let result = unsafe { ((*gop).query_mode)(gop, index, &mut size, &mut ptr_info) };
        line.text("PCPOS_GOP_PROBE_V1 QUERY index=");
        line.number(index as u64);
        line.text(" status="); line.hex(result as u64);
        if result == 0 && !ptr_info.is_null() {
            if size >= size_of::<ModeInformation>() {
                let info = unsafe { ptr::read_unaligned(ptr_info) };
                line.text(" width="); line.number(info.horizontal_resolution as u64);
                line.text(" height="); line.number(info.vertical_resolution as u64);
                line.text(" pitch="); line.number(info.pixels_per_scan_line as u64);
                line.text(" format="); line.number(info.pixel_format as u64);
                #[cfg(feature="exercise-mode-switch")]
                if candidate.is_none() && safe_alternate_candidate(original, max_modes, index, info) {
                    candidate = Some(index);
                }
            } else {
                line.text(" SHORT_INFO");
            }
        }
        unsafe { line.send(con) };
        // QueryMode allocates its Info with AllocatePool; return the pool
        // allocation even when the reported byte size is too short.
        if !ptr_info.is_null() {
            let freed = unsafe { free_pool(ptr_info.cast()) };
            if freed != 0 {
                line.text("PCPOS_GOP_PROBE_V1 FREE_POOL_FAILED ");
                line.hex(freed as u64);
                unsafe { line.send(con) };
                return freed;
            }
        }

    }
    #[cfg(feature="exercise-mode-switch")]
    {
        if let Some(target) = candidate {
            line.text("PCPOS_GOP_PROBE_V1 STANDALONE_FIXTURE_SET_MODE target=");
            line.number(target as u64);
            unsafe { line.send(con) };
            let changed = unsafe { ((*gop).set_mode)(gop, target) };
            line.text("PCPOS_GOP_PROBE_V1 SET_MODE_STATUS ");
            line.hex(changed as u64);
            unsafe { line.send(con) };
            unsafe { show_mode(con, "AFTER", gop) };
            // Best-effort restoring of initial mode is not a guarantee.
            // This opt-in experimental binary must only run on a disposable
            // emulator and never precede a real OS loader on shared disks.
            let restored = unsafe { ((*gop).set_mode)(gop, original) };
            line.text("PCPOS_GOP_PROBE_V1 RESTORE_STATUS ");
            line.hex(restored as u64);
            unsafe { line.send(con) };
            unsafe { show_mode(con, "RESTORED", gop) };
            if changed != 0 { return changed; }
            return restored;
        } else {
            line.text("PCPOS_GOP_PROBE_V1 NO_ALTERNATE_MODE");
            unsafe { line.send(con) };
        }
    }
    // In the normal build, *zero* SetMode calls. No ExitBootServices call.
    line.text("PCPOS_GOP_PROBE_V1 END READ_ONLY");
    unsafe { line.send(con) };
    0
}

#[no_mangle]
pub extern "efiapi" fn efi_main(_image: usize, system_table: *mut SystemTable) -> EfiStatus {
    unsafe { app(system_table) }
}
