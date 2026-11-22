//! How far a processor is from the temperature at which it protects itself, as the processor
//! reports it. No I/O here: the caller reads the two values and this says what they mean.
//!
//! An Intel processor with a digital sensor reports, for the core asked, how many degrees it is
//! below the temperature at which it begins to slow itself, whether it is slowing itself now, and
//! whether it has reached the temperature at which the machine should be shut down. The processor
//! acts on these by itself, kernel or no kernel: it slows, and at the last it stops. What the
//! kernel adds is to stop giving a hot processor work before that, and to say so. Nothing here
//! gives a temperature in degrees: that would need a further value which not every processor has,
//! and the margin is the number that matters.

/// What a processor says about its heat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heat {
    /// Degrees below the temperature at which it begins to slow itself. Zero is at it.
    pub margin: u8,
    /// It is slowing itself now.
    pub slowing: bool,
    /// It has slowed itself at some time since this was last cleared (nothing here clears it).
    pub has_slowed: bool,
    /// It is at the temperature at which the machine should be shut down.
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
    Some(Heat { margin: (status >> 16 & 0x7F) as u8, slowing: status & 1 != 0, has_slowed: status & 2 != 0, critical: status & (1 << 4) != 0 })
}

/// Whether a processor that says this should be given work. Not within `least` degrees of slowing
/// itself, not slowing itself, and not critical; and a processor that gives no valid reading while
/// claiming a sensor is not given the benefit of the doubt.
pub fn fit_for_work(heat: Option<Heat>, least: u8) -> bool {
    matches!(heat, Some(h) if !h.critical && !h.slowing && h.margin >= least)
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
        assert_eq!(read(0x8832_0000), Some(Heat { margin: 50, slowing: false, has_slowed: false, critical: false })); // a cool core: 50 below
        assert_eq!(read(0x8800_0003), Some(Heat { margin: 0, slowing: true, has_slowed: true, critical: false }));
        assert_eq!(read(0x8805_0002), Some(Heat { margin: 5, slowing: false, has_slowed: true, critical: false }));
        assert_eq!(read(0x8800_0013), Some(Heat { margin: 0, slowing: true, has_slowed: true, critical: true }));
        assert_eq!(read(0x80FF_0000).map(|h| h.margin), Some(127)); // seven bits and no more
        assert_eq!(read(0xFFFF_FFFF_8000_0000 | 20 << 16).map(|h| h.margin), Some(20)); // the upper half is not read
    }

    #[test]
    fn work_goes_only_to_a_processor_with_margin_to_spare() {
        let at = |margin, slowing, critical| Some(Heat { margin, slowing, has_slowed: false, critical });
        assert!(fit_for_work(at(50, false, false), 10) && fit_for_work(at(10, false, false), 10));
        assert!(!fit_for_work(at(9, false, false), 10));
        assert!(!fit_for_work(at(50, true, false), 10) && !fit_for_work(at(50, false, true), 10));
        assert!(!fit_for_work(None, 10));
        // Having slowed in the past does not by itself keep work away.
        assert!(fit_for_work(Some(Heat { margin: 30, slowing: false, has_slowed: true, critical: false }), 10));
    }
}
