# Stopped-screen totals after register closure

A3 built be2d4dd42521b6d49f7cdc3ab0d39682db6f22d3 with pinned Rust 1.97.0, in default and no-key-replay-demo configurations. The revised stock check ran four QEMU guests with the hypervisor flag withheld. QMP used stdio; transport.diff contains the entire transport adaptation.

The default stopped guest reported 72 passes. Two guests without a keyboard controller and the replay-disabled demonstration reported 66. Each displayed count matched serial after the final register check. The revised H5 predicate refused all four original undercounts from 7c69466. In each new frame it also rejected a count one short, a different number, erasure of the last digit, and erasure of both digits. Only matching totals passed.

Three further guests were observed with the virtual CPU halted. At both 320×200 and 1920×1080 a deliberately mismatched image displayed one failed check out of 41. Serial recorded 40 passes and one failure. A 320×200 guest with a keyboard displayed 72 of 72 passing. The capture observer retained its earlier pattern for the old failure wording. extra_replay.py independently verifies the new numerator and denominator from pixels and serial and matches every inked pixel to a source glyph.

Six separate headless boots tested normal replay, the demonstration and an absent controller, with and without the optional frame-cutoff check. The totals were 72/73, 66/67 and 66/67 respectively. Both no-replay paths omitted exactly the same six named checks. Both confirmed the optional check when requested.

Extract originals.tar.xz, verify SHA256SUMS and run the bundled Python replays from the extracted directory against a checkout of be2d4dd. Use control.py SOURCE stock fixed, control.py SOURCE prior prior, extra_replay.py SOURCE extra and count_replay.py counts. Compare the four outputs with verified-controls.json, prior-controls.json, extra.json and coverage.json. No physical hardware was tested.
