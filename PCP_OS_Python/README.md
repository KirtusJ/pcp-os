# PCP OS | Python implementation

**What this is.** An implementation of the PCP OS record, guards, state machines and tools in standard-library Python. It is not the product and not the authority: the native implementation in Rust is. This one is kept as a second implementation that is tested against the files the native one publishes, and as a form PCP OS can take on another operating system where only Python is at hand.

**Status.** Unreleased, unreviewed. 105 tests: 21 run and pass on Python 3.12 and 3.13 on Linux without the ACR Implementation packet (among them the comparison with the native implementation on 17,113 cases, which needs the case file the Rust workspace generates); 84 need the packet and are skipped without it. Not run on Windows. No model provider was called. Writing this implementation beside the native one found thirteen defects here, now corrected: values of the wrong type read as true or false; a boolean accepted as a number; patterns that accepted a trailing line break; an unhandled error on an undefined gate; unhandled errors on fields of the wrong type; lists and text accepted where an object was required.

**Needs.** Python 3.9 or later. The sealed ACR 1.0 Implementation archive (`ACR_1.0_Implementation.zip`) or its extracted folder beside this folder; it is not included here. Without it the tests that need the method are skipped. For the comparison with the native implementation, the Rust workspace (`PCP_OS_Rust`) beside this folder, or `PCPOS_VECTORS` set to its `vectors` directory.

**Use.**

```
python3 pcp_os.py --help
python3 pcp_os.py doctor
python3 pcp_os.py init <new directory> --principal <label>
python3 -m unittest discover -s TESTS -p "test_*.py"
```

On Windows use `py` in place of `python3`.

**Rights.** Proprietary. Copyright (c) 2026 Kirtus Justus. All rights reserved. Supplied under license only; holding a copy grants nothing. Drafted with an AI system (Claude, Anthropic) at the creator's direction; no endorsement by Anthropic is implied.
