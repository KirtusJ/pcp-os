//! Native, single-process PCP OS resident worker.
#![no_std]
#![no_main]

use core::arch::{asm, global_asm};
use core::fmt::{self, Write};

#[allow(dead_code)]
#[path = "../../../PCP_OS_Rust/crates/pcpos-core/src/machine.rs"]
mod machine;
mod model;

#[cfg(written_trial)]
#[path = "../written/answer.rs"]
mod answer;

const PARENT: [usize; 25] = [
    0, 0, 1, 1, 1, 2, 2, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 7, 8, 8, 9, 10, 11, 12,
];
static mut CONTEXT: model::Context = model::Context::new();

global_asm!(
    ".global _start",
    "_start:",
    "mov rdi, rsp",
    "and rsp, -16",
    "call worker_main",
    "ud2"
);

fn syscall(n: usize, a: usize, b: usize, c: usize) -> isize {
    let result: isize;
    unsafe {
        asm!("syscall", inlateout("rax") n => result, in("rdi") a, in("rsi") b, in("rdx") c,
             lateout("rcx") _, lateout("r11") _, options(nostack));
    }
    result
}

fn fail() -> ! {
    unsafe {
        asm!("syscall", in("rax") 60usize, in("rdi") 1usize, options(noreturn));
    }
}

struct Console;
impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut bytes = s.as_bytes();
        while !bytes.is_empty() {
            let n = syscall(1, 1, bytes.as_ptr() as usize, bytes.len());
            if n <= 0 || n as usize > bytes.len() {
                return Err(fmt::Error);
            }
            bytes = &bytes[n as usize..];
        }
        Ok(())
    }
}

// The loader supplies NUL-terminated argv entries. Only bounded decimal IDs are accepted.
unsafe fn number(ptr: *const u8) -> Option<usize> {
    let mut n = 0usize;
    for i in 0..8 {
        let byte = *ptr.add(i);
        if byte == 0 {
            return (i > 0).then_some(n);
        }
        if i == 7 || !byte.is_ascii_digit() {
            return None;
        }
        n = n * 10 + usize::from(byte - b'0');
    }
    None
}

#[no_mangle]
extern "C" fn worker_main(stack: *const usize) -> ! {
    let (node, incoming, salt) = unsafe {
        if *stack != 4 {
            fail();
        }
        let arg = |i: usize| number(*stack.add(i + 1) as *const u8).unwrap_or_else(|| fail());
        (arg(1), arg(2), arg(3))
    };
    if !(1..PARENT.len()).contains(&node) || !(1..1_000_003).contains(&incoming) || salt >= 16 {
        fail();
    }
    let ticket = (incoming * 31 + node * 13 + salt + 1) % 1_000_003;
    let model = model::Model::load();
    // One loader entry and one thread own the entire inference context.
    let ctx = unsafe { &mut *core::ptr::addr_of_mut!(CONTEXT) };
    let reply = |ctx: &mut model::Context, prompt: usize| {
        let [a, b, c] = model.predict(ctx, prompt);
        writeln!(Console, "LM tokens {a} {b} {c}").unwrap();
    };
    let parent = PARENT[node];
    writeln!(
        Console,
        "NODE {node} PARENT {parent} INPUT {incoming} TICKET {ticket}"
    )
    .unwrap();
    reply(ctx, (node + incoming + salt) % 16);
    let required = machine::MINIMUM_GATES;
    if machine::gate_step(required, 0, 4) != Err(machine::GateRefusal::Before(0))
        || machine::gate_step(required, 0, 0) != Ok(1)
    {
        fail();
    }
    writeln!(Console, "GATE 1").unwrap();
    for (child, &owner) in PARENT.iter().enumerate().skip(2) {
        if owner == node {
            writeln!(Console, "SPAWN {child} PARENT {node} TICKET {ticket}").unwrap();
        }
    }
    let fd = syscall(2, b"/dev/kbd\0".as_ptr() as usize, 0, 0);
    if fd < 0 {
        fail();
    }
    loop {
        writeln!(Console, "READY {node}").unwrap();
        loop {
            let mut key = [0u8; 2];
            if syscall(0, fd as usize, key.as_mut_ptr() as usize, key.len()) != 2 {
                fail();
            }
            #[cfg(written_trial)]
            if key == [18, 0] {
                answer::respond(node, &model);
                break;
            }
            if key[1] == 0 && (2..=17).contains(&key[0]) {
                let prompt = usize::from(key[0] - 2);
                writeln!(Console, "CHALLENGE {prompt}").unwrap();
                reply(ctx, prompt);
                break;
            }
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    fail()
}

// The freestanding ELF supplies the two memory operations LLVM emits for slice copies and fills.
#[cfg(not(written_trial))]
global_asm!(
    ".text",
    ".global memcpy",
    "memcpy:",
    "mov rax, rdi",
    "mov rcx, rdx",
    "rep movsb",
    "ret",
    ".global memset",
    "memset:",
    "mov r8, rdi",
    "mov eax, esi",
    "mov rcx, rdx",
    "rep stosb",
    "mov rax, r8",
    "ret"
);
