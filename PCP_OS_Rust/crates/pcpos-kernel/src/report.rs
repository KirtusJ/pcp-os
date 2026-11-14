//! Opt-in boot health on COM2. COM1 carries diagnostics and application output.
mod wire;

use crate::arch;
use pcpos_core::{boot_report::BootId, ledger::Ledger, machine::System};
use wire::{Format, Message};

pub struct Report(Format);

impl Report {
    pub fn open(command_line: &str, command_line_complete: bool) -> Option<Self> {
        let option = command_line.split_whitespace().find_map(|word| word.strip_prefix("report="))?;
        let report = match option {
            "json" => Self(Format::Json),
            "cbor" => Self(Format::Cbor),
            "cbor2" => Self(Format::Cbor2),
            "cbor3" => {
                if !command_line_complete {
                    crate::log(format_args!("kernel reporting unavailable: incomplete cbor3 command line"));
                    return None;
                }
                let mut ids = command_line.split_whitespace().filter_map(|word| word.strip_prefix("boot_id="));
                match (ids.next().and_then(BootId::parse), ids.next()) {
                    (Some(id), None) => Self(Format::Cbor3(id)),
                    _ => {
                        crate::log(format_args!("kernel reporting unavailable: cbor3 requires exactly one boot_id of 32 lowercase hex digits"));
                        return None;
                    }
                }
            }
            _ => return None,
        };
        if !arch::report_port_init() {
            crate::log(format_args!("kernel reporting unavailable: COM2 did not respond"));
            return None;
        }
        Some(report)
    }

    pub fn boot(&self, image: &str, matches_expected: Option<bool>) {
        Message::Boot { image, matches_expected }.write(self.0, arch::report_bytes);
    }

    pub fn session(&self, passed: u32, failed: u32, system: Option<System>) {
        Message::Session { passed, failed, system }.write(self.0, arch::report_bytes);
    }

    pub fn stop(&self, passed: u32, failed: u32, replay: bool, ledger: &Ledger) {
        Message::Stop { passed, failed, replay, events: ledger.len(), head: ledger.head() }
            .write(self.0, arch::report_bytes);
    }
}
