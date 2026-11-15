# Forklift Certified · Phobos Freight

Lucky Seven has the rocket launcher. Approach a red cargo stack, let Lucky line up his charge, then sidestep the forklift. The dock plan shows you in green, the forklift in amber and three stacks in red. Clear the stacks, collect the launcher and confront the driver. Press Space after dying to retry.

~~~sh
cargo run --locked --release --manifest-path store/native/Cargo.toml -- KERNEL.elf32 forklift-certified /path/to/freedoom1.wad
~~~

Recovered from `Forklift_Certified.zip`, SHA-256 `a54d39382c40119013143ce4e72c81216b55e9d4b344991bb6e3b955041f6695`. The package supplies the loading dock, forklift, driver, permit, music, effects and native guest. Direct guest options are `-omega -forklift -warp 1 9`. [Supported data, play and source build](../doom/)
