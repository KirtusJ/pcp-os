#![no_std]
#![no_main]
//! A test program for `examples/vector-state/check.py`. Built two ways (LEAVE=1 or not). It asks the processor
//! whether the 256-bit registers may be used; if so it reads the last of them first (what a program
//! finds there when it starts), then fills it, sleeps so that the kernel runs in between, and reads it
//! again. The first build ends leaving the register full, for the second to look for.
use core::arch::asm;
use pcp_game_guest::{exit, log, sleep_us};

const LEAVE: bool = option_env!("LEAVE").is_some();

fn read() -> [u8; 32] {
    let mut bytes = [0xAAu8; 32];
    // Safety: only run after the processor has said these instructions and registers are on.
    unsafe { asm!("vmovups [{}], ymm15", in(reg) bytes.as_mut_ptr(), options(nostack)) };
    bytes
}

fn say(words: &[u8], value: bool) {
    log(words);
    log(if value { b"1\n" } else { b"0\n" });
}

pcp_game_guest::entry!(probe_main);
#[no_mangle]
pub extern "C" fn probe_main() -> ! {
    let features = core::arch::x86_64::__cpuid(1).ecx;
    // The whole of the processor maker's test, in its order: the operating system has turned on the
    // register that says which states it keeps (bit 27); the processor has AVX (bit 28); and that
    // register says the 128-bit and 256-bit states are both kept. Only then is a 256-bit instruction
    // run. (AVX2 and FMA are further bits, which this program neither tests nor uses.)
    let offered = features & (1 << 27) != 0 && features & (1 << 28) != 0;
    let kept = offered && {
        let (low, _high): (u32, u32);
        // Safety: XGETBV is allowed here because bit 27 says so.
        unsafe { asm!("xgetbv", in("ecx") 0u32, out("eax") low, out("edx") _high, options(nomem, nostack)) };
        low & 0b110 == 0b110
    };
    if !kept {
        log(b"vector: none\n");
        exit(0);
    }
    say(b"vector: found clean ", read() == [0u8; 32]);
    // Safety: as above. Every bit of the register is set, and the compiler is told the register is
    // written. (This program is built for a target without vector instructions, so the compiler
    // itself never uses the register between here and the readings below.)
    unsafe { asm!("vcmpps ymm15, ymm15, ymm15, 0x0f", out("ymm15") _, options(nostack)) };
    say(b"vector: set ", read() == [0xFFu8; 32]);
    let mut slept = 0;
    while slept < 30 {
        sleep_us(10_000);
        slept += 1;
    }
    say(b"vector: kept ", read() == [0xFFu8; 32]);
    if !LEAVE {
        // Safety: as above; the one register this program wrote is emptied, and declared.
        unsafe { asm!("vxorps ymm15, ymm15, ymm15", out("ymm15") _, options(nostack)) };
    }
    log(if LEAVE { b"vector: left full\n" } else { b"vector: done\n" });
    exit(0);
}
