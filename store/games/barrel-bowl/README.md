# Barrel Bowl · Phobos Lanes

Aim with Left and Right. Hold Alt and tap either arrow to slide along the approach. Tap Ctrl to roll a barrel. Wait for the pins to settle before the next ball. Fallen pins stay down for the second ball. A strike earns the next two throws as a bonus, a spare earns one, and a perfect game scores 300.

~~~sh
cargo run --locked --release --manifest-path store/native/Cargo.toml -- KERNEL.elf32 barrel-bowl /path/to/freedoom1.wad
~~~

Recovered from `Barrel_Bowl.zip`, SHA-256 `cf1679a9c7d971e10f7224480e64080a5756e644e610ce2aa47148434d116f1a`. Its second copy of the Omega campaign is available as its own Store entry. This package adds the native bowling guest, the E1M9 alley, artwork, scoring logic, music and effects. Direct guest options are `-omega -bowl -warp 1 9`. [Supported data, play and source build](../doom/)
