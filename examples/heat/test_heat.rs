//! Host tests of the kernel's reading of a processor's heat (`arch/heat.rs`), which is pure.
//! `rustc --edition=2021 --test -C overflow-checks=on examples/heat/test_heat.rs -o heat-tests && ./heat-tests`
#[path = "../../PCP_OS_Rust/crates/pcpos-kernel/src/arch/heat.rs"]
#[allow(dead_code)]
mod heat;
