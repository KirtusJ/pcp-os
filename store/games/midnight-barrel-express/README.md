# Midnight Barrel Express

Cross the couplings toward the locomotive. Face the port interlock on the left of the cargo car and the starboard interlock on the right of the tender. Tap Space at each, then take the brass brake handle in the locomotive. Hold Space for the service brake and release it to coast. Compare the signal distance with the distance needed to stop. Finish within 13 metres of the signal. Tap Space to retry a missed approach.

~~~sh
cargo run --locked --release --manifest-path store/native/Cargo.toml -- KERNEL.elf32 midnight-barrel-express /path/to/freedoom1.wad
~~~

Recovered from `Midnight_Barrel_Express.zip`, SHA-256 `80a21ac27d4a6c8486b0a91a1585c5b2e542576a84ba78f27ee178b9c10c4bbd`. The package supplies the E1M9 train, crew, cab instruments, arrival ticket, music, sounds and native guest. Direct guest options are `-omega -express -warp 1 9`. [Supported data, play and source build](../doom/)
