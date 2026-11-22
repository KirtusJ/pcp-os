//! Host tests of what is done with a started processor whose memory types are not the first's. From this crate's folder:
//!   rustc --edition=2021 --test tests/test_types.rs -o /tmp/test-types && /tmp/test-types
#[path = "../src/arch/others/types.rs"]
mod types;
use types::{decide, Decision, Types, TABLE, TYPE_REGISTERS};

fn held(registers: &[(u32, u64)]) -> Types {
    let mut out: Types = ([(0, 0); TYPE_REGISTERS], 0);
    for &entry in registers {
        out.0[out.1] = entry;
        out.1 += 1;
    }
    out
}

const FIRST: &[(u32, u64)] = &[(0x200, 0x8000_0000), (0x201, 0xF_8000_0800), (0x2FF, 0xC06), (TABLE, 0x0007_0406_0007_0106)];

#[test]
fn a_processor_that_holds_the_same_is_left_alone() {
    assert_eq!(decide(&held(FIRST), &held(FIRST)), Decision::Same);
}

#[test]
fn one_that_differs_only_in_its_table_is_given_the_table() {
    let mut mine = FIRST.to_vec();
    mine[3].1 = 0x0007_0406_0007_0406;
    assert_eq!(decide(&held(FIRST), &held(&mine)), Decision::GiveTable);
}

#[test]
fn one_whose_ranges_differ_in_value_is_held_whatever_its_table() {
    // What this emulator returns after a restart: every range register cleared.
    let cleared = [(0x200, 0), (0x201, 0), (0x2FF, 0), (TABLE, 0x0007_0406_0007_0406)];
    assert_eq!(decide(&held(FIRST), &held(&cleared)), Decision::Hold);
    let same_table = [(0x200, 0), (0x201, 0), (0x2FF, 0), FIRST[3]];
    assert_eq!(decide(&held(FIRST), &held(&same_table)), Decision::Hold);
}

#[test]
fn one_that_lists_fewer_range_registers_is_held() {
    // A3's case: equal values where both have a register, but this processor has no variable pair.
    let fewer = [(0x2FF, 0xC06), FIRST[3]];
    assert_eq!(decide(&held(FIRST), &held(&fewer)), Decision::Hold);
}

#[test]
fn one_that_lists_more_range_registers_is_held() {
    let more = [FIRST[0], FIRST[1], (0x202, 0), (0x203, 0), FIRST[2], FIRST[3]];
    assert_eq!(decide(&held(FIRST), &held(&more)), Decision::Hold);
}

#[test]
fn one_without_a_table_where_the_first_has_one_is_held_and_never_written() {
    assert_eq!(decide(&held(FIRST), &held(&FIRST[..3])), Decision::Hold);
}

#[test]
fn where_no_processor_has_range_registers_only_the_table_is_compared() {
    let first = [FIRST[3]];
    let mine = [(TABLE, 0x0007_0406_0007_0406)];
    assert_eq!(decide(&held(&first), &held(&mine)), Decision::GiveTable);
    assert_eq!(decide(&held(&first), &held(&first)), Decision::Same);
}
