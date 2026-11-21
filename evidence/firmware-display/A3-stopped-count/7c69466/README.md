# Stopped-screen count check on the integrated keyboard branch

At 7c694667e32ec7db4063a2dd929c8131d75f803c the stock stopped-screen checker accepted four QEMU guests. The default and demonstration binaries were built at that revision. QMP used stdio in this environment. transport.diff contains the transport change. The acceptance code, commands, resolutions and guest interactions were unchanged.

Each displayed total was one short of the final serial count. The 800×600 default guest showed 71 with 72 final passes. Both guests without a controller and the 800×600 demonstration showed 65 with 66 final passes. The final pass in every serial is the closed register.

control.py re-reads the original guest pixels with the stock font, reproduces the original count byte-for-byte, then changes only those glyph cells. The original, corrected, transposed, last-digit-erased and both-digits-erased frames all passed the stock stopped_problems predicate on each of the four frames. Only the corrected count matched the serial record.

Extract originals.tar.xz, verify SHA256SUMS, and run python -IB control.py /path/to/7c69466 stock. Compare output with controls.json. Earlier screenshots give the same result at the prior working head; earlier-controls.json contains that additional comparison. The two screens with intentionally failed verification remain covered by the previous six-guest archive.
