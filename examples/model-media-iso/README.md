# Master a UEFI model-media ISO

Start with an unpartitioned FAT image containing the x86-64 removable-media loader at
`EFI/BOOT/BOOTX64.EFI` and the ordinary files it expects. For a preloaded build,
place the packer's `model.manifest` and numbered shards where the loader opens them.
The loader or launch configuration must already have the independently trusted
manifest digest. This tool packages the supplied bytes. The firmware and worker
remain responsible for admission and verification.

On Linux with Python 3.11 or newer, mtools, and xorriso, run

```sh
python3 examples/model-media-iso/build.py prepared.fat model.iso
```

The destination must be a new file on a filesystem that supports hard links. The
builder snapshots the FAT file and works in the output directory. It extracts the
ordinary files, masters the ISO, then reads back its Joliet names, directory entries,
file lengths and SHA-256 digests before publishing the result. It prints the ISO
size and SHA-256. Identical input FAT bytes and tool versions give the same ISO
with fixed dates and a disk GUID derived from the FAT image. The 5,010 MiB test
volume needs roughly 21 GB free in the output directory, including the output.

## Media layout

- Optical firmware can find the FAT volume through the El Torito entry.
- Raw USB firmware can find those same FAT bytes through the GPT EFI System
  Partition. xorriso assigns both references to the same boot image.
- A file-copy writer sees the ordinary files in the ISO tree. The FAT container
  and boot catalog are hidden from both Rock Ridge and Joliet, keeping the
  multi-gigabyte container out of the files copied to a FAT32 stick.

The ordinary file data appears twice in the ISO. With a 5,027,783,488-byte
synthetic model and a 5,010 MiB FAT32 volume, the ISO took 10,282,047,488 bytes.
That exceeds the capacity of an 8 GB stick in raw mode and of an 8.5 GB dual-layer DVD.

For large boot images xorriso records zero in the 16-bit El Torito sector-count
field. In a tested OVMF build the available optical partition then depends on
the volume size in the ISO primary volume descriptor. Appending the ESP beyond
that boundary left zero blocks available. Placing it inside the ISO volume
covers it without patching the catalog or calculating sector addresses.
[UEFI media access](https://uefi.org/specs/UEFI/2.11/13_Protocols_Media_Access.html) ·
[EDK II optical partition sizing](https://github.com/tianocore/edk2/blob/edk2-stable202502/MdeModulePkg/Universal/Disk/PartitionDxe/ElTorito.c#L233-L246) ·
[xorrisofs boot and partition options](https://www.gnu.org/software/xorriso/man_1_xorrisofs.html).

The reference boots use q35 and OVMF with a diagnostic EFI program and synthetic
data. Physical firmware, Rufus itself and inference through the integrated worker
require separate trials.
