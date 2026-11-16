"""Add tick observations only to a separate diagnostic kernel."""
from pathlib import Path
import sys

if sys.argv[2] != "trace":
    sys.exit(0)
src=Path(sys.argv[1])
def insert(name, anchor, text):
    p=src/name
    s=p.read_text()
    assert s.count(anchor)==1,(name,anchor)
    p.write_text(s.replace(anchor,text+anchor))
insert("user.rs", "    dev::keys_forget();\n    let _ = dev::stop_requested(); // a stop asked for before the program existed",'''    if program.name == "sleeper_long" { crate::log(format_args!("SLEEP-TRACE admitted {}", arch::ticks())); }
''')
insert("user.rs", "                        p.waiting = waiting;",'''                        if p.name == "sleeper_long" && matches!(waiting, Waiting::Until(_)) { crate::log(format_args!("SLEEP-TRACE blocked {}", now)); }
''')
insert("dev.rs","            out8(0x64, 0xD2);",'''            if matches!(s.pacing, KeyPacing::Sleeping { .. }) && s.next == 0 { crate::log(format_args!("SLEEP-TRACE first-key {}", ticks)); }
''')
insert("user.rs","        p.end = Some(end);",'''        if p.name == "sleeper_long" && end == End::Stopped { crate::log(format_args!("SLEEP-TRACE stop {}", now)); }
''')
