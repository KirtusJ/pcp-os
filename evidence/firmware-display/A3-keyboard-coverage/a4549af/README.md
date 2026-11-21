# Keyboard coverage and the stopped screen at a4549af

The default tester image was built at a4549af9c9ead717d32bcb2154b66fdf2445cd3c with the pinned Rust compiler. The demonstration kernel used the same source and compiler with no-key-replay-demo. All boots were virtual.

The six headless combinations cover ordinary replay, the demonstration with input available, and an absent i8042 controller, each with and without the optional frame-cutoff test. Compare individual named records, including repetitions. Relative to ordinary replay, the demonstration and absent controller both omit exactly the same six key-dependent checks. The optional check added one passing record in every configuration. The independent stock replay test also passed.

The four graphical guests show the actual F5 wording on 320×200 and 1920×1080 with ordinary replay or the demonstration. All four opened Queue Desk, typed and saved a label, closed the program, and confirmed shutdown. Screenshots and serial logs are included. Counts at startup and at final register closure are distinct.

The six additional halted guests with the hypervisor bit withheld confirm the remaining stopped-screen count error. Four successful guests show one fewer passing check than serial. Two intentionally mismatched images display their one failed check correctly. stop_replay.py checks the actual font on every stopped-screen pixel and the CPU halt flag.

To replay, extract originals.tar.xz, check SHA256SUMS, and run count_replay.py counts, screen_replay.py /path/to/a4549af screens and stop_replay.py /path/to/a4549af stop using Python. The first script requires only the standard library. The two screen replays use the pinned repository and NumPy. Compare their output with coverage.json, screens.json and stopped.json.
