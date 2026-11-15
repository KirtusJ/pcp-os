# Ultra Omega Barrel

Play the episode-one campaign with the Omega boss, husks and mini barrels. Start a new game with Enter. Arrows move and turn, Shift runs, Alt strafes, Ctrl fires, Space operates switches and doors, and number keys select weapons. Tab opens the automap. Type `idclev18` during play to enter the final encounter.

~~~sh
cargo run --locked --release --manifest-path store/native/Cargo.toml -- KERNEL.elf32 ultra-omega-barrel /path/to/freedoom1.wad
~~~

The original authoring bundle `Ultra_Omega_Barrel.zip` had SHA-256 `52f7280835720221e376f7a9e40af108f13c3d6d5a2d0fca4c0616aa84bdb4f0`. The Store includes its native Omega guest, original campaign effects and music, and complete corresponding engine source. The environmental barrel graphics in this edition come from the player's base. The spelling of `-omega` matters when launching the guest directly. [Supported data, play and build instructions](../doom/)
