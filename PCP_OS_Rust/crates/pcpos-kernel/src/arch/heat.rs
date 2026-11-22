//! How far a processor is from the temperature at which it protects itself, as the processor
//! reports it. No I/O here: the caller reads the two values and this says what they mean.
//!
//! An Intel processor with a digital sensor reports, for the core asked, how many degrees it is
//! below its highest junction temperature (TjMax), whether it is limiting itself now, and whether
//! its critical-temperature status is set. The processor acts on heat by itself, kernel or no kernel.
//!
//! The distance is NOT how far the processor is from slowing itself. Firmware can set the point at
//! which the processor begins to limit itself some degrees below TjMax, and that setting is not
//! read here, so the real headroom may be smaller than the distance by an amount this code does not
//! know (Director B3, from Intel's datasheet for the 10th generation). For that reason nothing
//! here says whether a processor should be given work: this only reports.

/// What a processor says about its heat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heat {
    /// Degrees below TjMax. The processor may begin to limit itself before this reaches zero.
    pub below_highest: u8,
    /// Its thermal status is set: it is limiting itself now.
    pub limiting: bool,
    /// It has limited itself at some time since this was last cleared (nothing here clears it).
    pub has_limited: bool,
    /// Its critical-temperature status is set. What follows from that is the machine's doing and is not predicted here.
    pub critical: bool,
}

/// Whether the processor has the sensor: bit 0 of EAX of CPUID leaf 6. `highest` is the highest
/// leaf the processor has.
pub fn offered(highest: u32, leaf_six_eax: u32) -> bool {
    highest >= 6 && leaf_six_eax & 1 != 0
}

/// Read the status value (the register IA32_THERM_STATUS). `None` if the processor says the
/// reading is not valid.
pub fn read(status: u64) -> Option<Heat> {
    if status & (1 << 31) == 0 {
        return None;
    }
    Some(Heat { below_highest: (status >> 16 & 0x7F) as u8, limiting: status & 1 != 0, has_limited: status & 2 != 0, critical: status & (1 << 4) != 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sensor_is_asked_for_only_where_the_processor_has_one() {
        assert!(offered(6, 1) && offered(0x1B, 0x77));
        assert!(!offered(5, 1)); // the leaf is not there: what it would say means nothing
        assert!(!offered(6, 0x76)); // the leaf is there and the sensor is not (an emulator's processor)
    }

    #[test]
    fn the_status_is_read_field_by_field() {
        assert_eq!(read(0), None);
        assert_eq!(read(0x0000_0000_7FFF_FFFF), None); // everything but the bit that says it is valid
        assert_eq!(read(0x8832_0000), Some(Heat { below_highest: 50, limiting: false, has_limited: false, critical: false })); // a cool core: 50 below
        assert_eq!(read(0x8800_0003), Some(Heat { below_highest: 0, limiting: true, has_limited: true, critical: false }));
        assert_eq!(read(0x8805_0002), Some(Heat { below_highest: 5, limiting: false, has_limited: true, critical: false }));
        assert_eq!(read(0x8800_0013), Some(Heat { below_highest: 0, limiting: true, has_limited: true, critical: true }));
        assert_eq!(read(0x80FF_0000).map(|h| h.below_highest), Some(127)); // seven bits and no more
        assert_eq!(read(0xFFFF_FFFF_8000_0000 | 20 << 16).map(|h| h.below_highest), Some(20)); // the upper half is not read
    }
}
