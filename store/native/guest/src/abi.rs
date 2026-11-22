use core::arch::asm;

pub fn call(n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let r: isize;
    unsafe {
        asm!("syscall",inlateout("rax") n=>r,in("rdi") a,in("rsi") b,in("rdx") c,in("r10") d,in("r8") e,in("r9") f,lateout("rcx") _,lateout("r11") _,options(nostack));
    }
    r
}
pub fn open(path: &[u8], flags: usize) -> isize {
    call(2, path.as_ptr() as usize, flags, 0, 0, 0, 0)
}
pub fn ioctl(fd: isize, req: usize, arg: usize) -> isize {
    call(16, fd as usize, req, arg, 0, 0, 0)
}
pub fn log(s: &[u8]) {
    call(1, 1, s.as_ptr() as usize, s.len(), 0, 0, 0);
}
pub fn exit(n: usize) -> ! {
    call(60, n, 0, 0, 0, 0, 0);
    loop {
        core::hint::spin_loop()
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    log(b"PCP game panic\n");
    exit(3)
}
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}
#[no_mangle]
unsafe extern "C" fn memset(dst: *mut u8, b: i32, n: usize) -> *mut u8 {
    let byte = b as u8;
    let mut ptr = dst;
    asm!("rep stosq", inout("rdi") ptr, inout("rcx") n/8=>_, in("rax") 0x0101010101010101u64*u64::from(byte), options(nostack));
    asm!("rep stosb", inout("rdi") ptr=>_, inout("rcx") n%8=>_, in("al") byte, options(nostack));
    dst
}
#[no_mangle]
unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let (mut to, mut from) = (dst, src);
    asm!("rep movsq", inout("rdi") to, inout("rsi") from, inout("rcx") n/8=>_, options(nostack));
    asm!("rep movsb", inout("rdi") to=>_, inout("rsi") from=>_, inout("rcx") n%8=>_, options(nostack));
    dst
}
/// Compare `n` bytes as unsigned values: negative, zero or positive as the first that differs is
/// smaller in `left`, absent, or larger. A length of zero reads neither. The compiler emits calls to
/// this (and to `bcmp`) for comparisons of slices; a guest built for the Linux target with no C
/// library has no other definition. The reads are volatile so that the compiler does not turn
/// this loop back into a call to the routine it is.
///
/// # Safety
/// Both must be readable for `n` bytes.
#[no_mangle]
unsafe extern "C" fn memcmp(left: *const u8, right: *const u8, n: usize) -> i32 {
    for at in 0..n {
        let (a, b) = (left.add(at).read_volatile(), right.add(at).read_volatile());
        if a != b {
            return i32::from(a) - i32::from(b);
        }
    }
    0
}
/// Zero if `n` bytes are the same, something else if not.
///
/// # Safety
/// Both must be readable for `n` bytes.
#[no_mangle]
unsafe extern "C" fn bcmp(left: *const u8, right: *const u8, n: usize) -> i32 {
    memcmp(left, right, n)
}
