# Doom game archive

Download an original ZIP from the table below. Each game includes its own launcher, engine source, and notices.

| Game | Inside the game | Original archive |
| --- | --- | --- |
| Ultra Omega Barrel | Nine-map campaign beginning in The Loading Bay. Includes Hangar.gif and The-Last-Press.mp4. | [Ultra_Omega_Barrel.zip](games/Ultra_Omega_Barrel.zip) |
| Barrel Bowl | Ten-frame barrel bowling at Phobos Lanes. Also includes its own copy of the Ultra Omega campaign and launcher. | [Barrel_Bowl.zip](games/Barrel_Bowl.zip) |
| Midnight Barrel Express | Ride the Phobos Railway, release two interlocks, and brake at the signal for Deimos Junction. | [Midnight_Barrel_Express.zip](games/Midnight_Barrel_Express.zip) |
| Forklift Certified | At Dock 07, draw Lucky Seven into three cargo stacks and recover the rocket launcher. | [Forklift_Certified.zip](games/Forklift_Certified.zip) |
| Doom Sports / Phobos After Dark | The Night Shift Open and Classic Course, with solo and two-player hotseat golf. The last Night Shift green rides a freight car. | [Doom_Sports_Phobos_After_Dark.zip](games/Doom_Sports_Phobos_After_Dark.zip) |

## First Doom game memory

[Doom_Game_Memory.zip](memory/Doom_Game_Memory.zip) preserves the original slot-0 save named **TOOCLOSE**, the session record, and its timing script. The save header identifies Doom version 109, episode 1, map 1. The session record separately lists the nine maps played, including the secret map. Its restore notes identify the game build and shareware IWAD used at the time.

## Using an archived copy

Compare your ZIP with [SHA256SUMS](SHA256SUMS), then extract it and open its `PLAY.txt` for the launcher and controls. Each bundle calls for Python 3, x86 QEMU, and your original Doom Shareware 1.9 `doom1.wad`.

The game executables are x86-64 ELFs. Each launcher boots a dedicated QEMU guest using the bundled `pcpos-kernel.elf32` and supplies the game and assembled data at startup. Compatibility with the [UEFI daily driver](../../../examples/release-media/daily-driver/README.md) and the [native Store launcher](../../native/README.md) is unverified.

The [file inventory](inventory.json) lists the byte length and SHA-256 of each ZIP and each ZIP member, including the save.
