//! The integrity core of PCP OS.
//!
//! One body of code, used unchanged by the kernel, by system services and by tools on other operating
//! systems. It needs no operating system: `no_std`, heap allocation only, no `unsafe`, one dependency
//! (a SHA-256 implementation; no cryptography is written here).
//!
//! * [`value`]  - the record data model and its two byte forms: the JSON form of record profile
//!                ACR-OS-RECORD-1, byte-identical to the Python implementation, and the draft compact form R1.
//! * [`ledger`] - an append-only, hash-chained register: appending and checking.
//! * [`cap`]    - capabilities: unforgeable references with rights that can only be reduced.
//! * [`machine`] - the state machines of ACR and of the system, as complete tables, and how they compose.
//! * [`task`]   - the table of running work behind the task manager.
//! * [`authority`] - identities, roles, groups, editions and standards: from policy to rights.
//! * [`acr`]    - ACR's mechanically checkable rules, native: the guards an event must pass, and the state
//!                derived from a register.
//!
//! A pass of any check here is a fact about bytes. It establishes nothing about whether a finding is right.
#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

extern crate alloc;

pub mod acr;
pub mod authority;
pub mod cap;
pub mod ledger;
pub mod machine;
pub mod r1;
pub mod task;
pub mod value;

use alloc::string::String;
use sha2::{Digest, Sha256};

/// Lower-case hexadecimal SHA-256 of some bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let d = Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in d.iter() {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 15) as usize] as char);
    }
    s
}

/// SHA-256 of some bytes.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

pub(crate) const HEX: &[u8; 16] = b"0123456789abcdef";
/// The chain value before the first event: 64 zeros.
pub const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
