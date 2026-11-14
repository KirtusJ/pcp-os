# Published vectors

Written by `pcpos-vectors` from the native implementation, except where noted. Other implementations are tested against these files.

| File | What it is | What it is for |
| --- | --- | --- |
| `scenario_inputs.jsonl` | 82 events offered to an empty record, in order | The input of the scenario |
| `register_scenario.jsonl`, `.expected.json` | The register those inputs produce through the native guards, and its head | Every kind of event; replays with no problem |
| `guard_cases.jsonl` | Each scenario event offered at every position, with mutations of its body and other actors: accepted or refused, the exit code and clause, and the body written. 17,113 cases, 6.3 MB, SHA-256 `b37c859041f66065413d0a5d07824614b370acc451b681abbe08bbabd88e3e33`. **Generated, and not kept in the source archive:** `cargo run --release --bin pcpos-vectors -- vectors` writes it, and the build does so before any test reads it | Agreement between implementations |
| `machines.tsv` | The eight state machines as complete tables | Lookup and comparison |
| `canonical.json` | Values and their one byte form | The byte form |
| `register_reference.jsonl`, `.expected.json` | Four events written by the Python implementation with text, nesting and control characters | The chain and the byte form only. Its actor is not registered, so the guards report two problems in it; it is not a guard vector |
| `register_fphpm_exercise.jsonl`, `.expected.json` | Thirteen events written by the Python implementation on 12 November 2026: one focused self-check of research baseline Z2 (792 files) | A register from real use. It predates baseline Z3 and says nothing about it. Replays with no problem |
