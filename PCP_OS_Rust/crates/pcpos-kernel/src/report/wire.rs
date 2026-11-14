//! Write COM2 observations directly as JSON Lines or versioned CBOR records.
use core::fmt::{self, Write};
use pcpos_core::{boot_report::{BootId, BOOT, SESSION, STOP}, machine::System, r1};

#[derive(Clone, Copy)]
pub enum Format {
    Json,
    Cbor,
    Cbor2,
    Cbor3(BootId),
}

pub enum Message<'a> {
    Boot { image: &'a str, matches_expected: Option<bool> },
    Session { passed: u32, failed: u32, system: Option<System> },
    Stop { passed: u32, failed: u32, replay: bool, events: u64, head: &'a str },
}

struct SystemName(Option<System>);

impl fmt::Display for SystemName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(system) => write!(f, r#""{}""#, system.name()),
            None => f.write_str("null"),
        }
    }
}

struct Json<F>(F);

impl<F: FnMut(&[u8])> Write for Json<F> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        (self.0)(s.as_bytes());
        Ok(())
    }
}

fn boolean(value: bool) -> u8 {
    if value { 0xf5 } else { 0xf4 }
}

fn emit_system(system: Option<System>, emit: &mut impl FnMut(&[u8])) {
    match system {
        Some(s) => r1::emit_text(s.name(), emit),
        None => emit(&[0xf6]),
    }
}

impl Message<'_> {
    pub fn write(&self, format: Format, mut emit: impl FnMut(&[u8])) {
        match format {
            Format::Json => {
                // Dynamic text is a kernel-computed lowercase digest or a System enum name.
                // Json's write_str always succeeds; the byte sink handles the actual UART.
                let mut out = Json(emit);
                match self {
                    Self::Boot { image, matches_expected } => {
                        let comparison = match matches_expected {
                            None => "null", Some(true) => "true", Some(false) => "false",
                        };
                        writeln!(out, r#"{{"event":"boot","image":"{image}","match":{comparison},"v":1}}"#)
                    }
                    Self::Session { passed, failed, system } => {
                        let admit = *failed == 0;
                        let system = SystemName(*system);
                        writeln!(out, r#"{{"admit":{admit},"event":"session","fail":{failed},"pass":{passed},"system":{system},"v":1}}"#)
                    }
                    Self::Stop { passed, failed, replay, events, head } => {
                        writeln!(out, r#"{{"event":"stop","events":{events},"fail":{failed},"head":"{head}","pass":{passed},"replay":{replay},"v":1}}"#)
                    }
                }.expect("infallible byte sink");
            }
            Format::Cbor => {
                // The R1 key for "event" is 70 (0x18 0x46). The other keys are
                // short text, ordered by encoded bytes. No runtime key sort.
                match self {
                    Self::Boot { image, matches_expected } => {
                        emit(b"\xa4\x18\x46\x64boot\x61v\x01\x65image");
                        r1::emit_text(image, &mut emit);
                        emit(b"\x65match");
                        emit(&[matches_expected.map(boolean).unwrap_or(0xf6)]);
                    }
                    Self::Session { passed, failed, system } => {
                        emit(b"\xa6\x18\x46\x67session\x61v\x01\x64fail");
                        r1::emit_uint(u64::from(*failed), &mut emit);
                        emit(b"\x64pass");
                        r1::emit_uint(u64::from(*passed), &mut emit);
                        emit(b"\x65admit");
                        emit(&[boolean(*failed == 0)]);
                        emit(b"\x66system");
                        emit_system(*system, &mut emit);
                    }
                    Self::Stop { passed, failed, replay, events, head } => {
                        emit(b"\xa7\x18\x46\x64stop\x61v\x01\x64fail");
                        r1::emit_uint(u64::from(*failed), &mut emit);
                        emit(b"\x64head");
                        r1::emit_text(head, &mut emit);
                        emit(b"\x64pass");
                        r1::emit_uint(u64::from(*passed), &mut emit);
                        emit(b"\x66events");
                        r1::emit_uint(*events, &mut emit);
                        emit(b"\x66replay");
                        emit(&[boolean(*replay)]);
                    }
                }
            }
            Format::Cbor2 | Format::Cbor3(_) => {
                if let Format::Cbor3(id) = format {
                    emit(&id.prefix());
                }
                self.write_positional(emit);
            }
        }
    }

    fn write_positional(&self, mut emit: impl FnMut(&[u8])) {
        match self {
            Self::Boot { image, matches_expected } => {
                emit(&BOOT.prefix());
                r1::emit_text(image, &mut emit);
                emit(&[matches_expected.map(boolean).unwrap_or(0xf6)]);
            }
            Self::Session { passed, failed, system } => {
                emit(&SESSION.prefix());
                r1::emit_uint(u64::from(*passed), &mut emit);
                r1::emit_uint(u64::from(*failed), &mut emit);
                emit_system(*system, &mut emit);
                emit(&[boolean(*failed == 0)]);
            }
            Self::Stop { passed, failed, replay, events, head } => {
                emit(&STOP.prefix());
                r1::emit_uint(u64::from(*passed), &mut emit);
                r1::emit_uint(u64::from(*failed), &mut emit);
                emit(&[boolean(*replay)]);
                r1::emit_uint(*events, &mut emit);
                r1::emit_text(head, &mut emit);
            }
        }
    }
}
