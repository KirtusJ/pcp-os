#[allow(dead_code)]
#[path="kernel-stock/PCP_OS_Rust/crates/pcpos-kernel/src/arch/processors.rs"]
mod processors;
fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("capture folder"));
    let address = u64::from_str_radix(&std::env::args().nth(2).expect("RSDP in hexadecimal"),16).unwrap();
    let mut read = |at: u64, len: usize| {
        std::fs::read(root.join(format!("acpi-{at:016x}-{len}.bin"))).ok()
    };
    let found = processors::find(address, &mut read).expect("firmware table replay");
    let ids: Vec<_> = found.processors.iter().map(|p| p.apic).collect();
    assert_eq!(ids,[0,1,2,4,5,6]);
    assert_eq!((found.beyond,found.disabled),(0,0));
    println!("APIC identities {ids:?}, controller {:#x}, beyond {}, disabled {}", found.controller, found.beyond, found.disabled);
}
