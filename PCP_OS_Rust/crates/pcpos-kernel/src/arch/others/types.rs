//! What is done with a started processor whose memory-type registers are not the first's. Pure, so
//! that it is tested on a host (tests/test_types.rs).

/// The registers that say how each range of memory is treated, with their values, and how many.
pub const TYPE_REGISTERS: usize = 2 * 32 + 11 + 2;
pub type Types = ([(u32, u64); TYPE_REGISTERS], usize);

/// The page-attribute table, which is each processor's own.
pub const TABLE: u32 = 0x277;

/// What to do with a processor that has just been started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// It holds what the first holds.
    Same,
    /// Its ranges are the first's and only its page-attribute table is not: it is given the table.
    GiveTable,
    /// Its range registers are not the first's, in value or in which there are: nothing of its is
    /// written, and no processor is given work.
    Hold,
}

/// Ranges are never rewritten on a started processor (A3). The makers' procedure for changing them
/// has every processor of the machine stop using memory together while it is done, and here the
/// first goes on running; and a processor that lists other registers than the first would be
/// written a register it does not have. The table is another matter: it is the processor's own,
/// read only by it, and the first's was changed by this kernel, so every started processor differs
/// there and is given it.
pub fn decide(first: &Types, mine: &Types) -> Decision {
    let ranges = |t: &Types| -> Types {
        let mut out = ([(0, 0); TYPE_REGISTERS], 0);
        for &entry in t.0[..t.1].iter().filter(|(register, _)| *register != TABLE) {
            out.0[out.1] = entry;
            out.1 += 1;
        }
        out
    };
    let table = |t: &Types| t.0[..t.1].iter().find(|(register, _)| *register == TABLE).map(|&(_, value)| value);
    if ranges(first) != ranges(mine) {
        Decision::Hold
    } else if table(first) != table(mine) {
        // Both have a table or neither has: a processor without one where the first has one cannot
        // be given it, and is held.
        if table(mine).is_some() { Decision::GiveTable } else { Decision::Hold }
    } else {
        Decision::Same
    }
}
