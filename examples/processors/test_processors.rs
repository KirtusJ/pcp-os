//! The host tests of the kernel's reading of the firmware's processor table:
//!     rustc --edition=2021 --test -C overflow-checks=on examples/processors/test_processors.rs -o t && ./t
#![allow(dead_code)]
#[path = "../../PCP_OS_Rust/crates/pcpos-kernel/src/arch/processors.rs"]
mod processors;
