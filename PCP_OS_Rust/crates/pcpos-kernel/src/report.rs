//! Opt-in boot health on COM2. COM1 carries diagnostics and application output.
mod wire;

use crate::arch;
use pcpos_core::{ledger::Ledger, machine::System};
use wire::{Format, Message};

pub struct Report(Format);

impl Report {
    pub fn open(command_line: &str) -> Option<Self> {
        let option = command_line.split_whitespace().find_map(|word| word.strip_prefix("report="))?;
        let report = match option {
            "json" => Self(Format::Json),
            "cbor" => Self(Format::Cbor),
            "cbor2" => Self(Format::Cbor2),
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
