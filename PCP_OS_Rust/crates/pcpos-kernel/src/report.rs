//! Opt-in boot health on COM2. COM1 carries diagnostics and application output.
use crate::{arch, obj, text};
use pcpos_core::{ledger::Ledger, machine::System, r1, value::Value};

pub enum Report {
    Json,
    Cbor,
}

impl Report {
    pub fn open(command_line: &str) -> Option<Self> {
        let option = command_line.split_whitespace().find_map(|word| word.strip_prefix("report="))?;
        let report = match option {
            "json" => Self::Json,
            "cbor" => Self::Cbor,
            _ => return None,
        };
        if !arch::report_port_init() {
            crate::log(format_args!("kernel reporting unavailable: COM2 did not respond"));
            return None;
        }
        Some(report)
    }

    fn send(&self, fields: &[(&str, Value)]) {
        let message = obj(fields);
        match self {
            Self::Json => {
                arch::report_bytes(&message.canonical_json());
                arch::report_bytes(b"\n");
            }
            // The encoder uses short integer keys and packs SHA-256 in 32 bytes.
            // Each definite-length CBOR item is self-delimiting.
            Self::Cbor => arch::report_bytes(&r1::encode(&message)),
        }
    }

    pub fn boot(&self, image: &str, matches_expected: Option<bool>) {
        self.send(&[
            ("v", Value::Int(1)), ("event", text("boot")), ("image", text(image)),
            ("match", matches_expected.map(Value::Bool).unwrap_or(Value::Null)),
        ]);
    }

    pub fn session(&self, passed: u32, failed: u32, system: Option<System>) {
        self.send(&[
            ("v", Value::Int(1)), ("event", text("session")),
            ("pass", Value::Int(i64::from(passed))), ("fail", Value::Int(i64::from(failed))),
            ("system", system.map(|s| text(s.name())).unwrap_or(Value::Null)),
            ("admit", Value::Bool(failed == 0)),
        ]);
    }

    pub fn stop(&self, passed: u32, failed: u32, replay_ok: bool, ledger: &Ledger) {
        self.send(&[
            ("v", Value::Int(1)), ("event", text("stop")),
            ("pass", Value::Int(i64::from(passed))), ("fail", Value::Int(i64::from(failed))),
            ("replay", Value::Bool(replay_ok)), ("events", Value::Int(ledger.len() as i64)),
            ("head", text(ledger.head())),
        ]);
    }
}
