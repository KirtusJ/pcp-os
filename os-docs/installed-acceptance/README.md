# Installed recovery and desktop acceptance

This check boots the same raw, already-installed PCP OS disk three times under x86-64 QEMU with OVMF. It targets releases whose manifest selects the recovery entry by default and whose boot menu also offers Tailorbird and the offline Qwen diagnostic.

| Boot | Operator input | Evidence required |
| --- | --- | --- |
| Recovery | Wait for the default, then power down from the launcher | Native amd64 EFI handoff, long mode, matched kernel measurement, empty launcher, clean guest exit |
| Desktop | Select the desktop, open Tailorbird, then leave it | Persistent store mounted, manifest-matched Tailorbird executable, separate Qwen worker admitted, local MCP connected, model ready, clean application and guest exit |
| Qwen diagnostic | Select the offline diagnostic | Manifest-matched worker runs the five input token IDs, returns the expected three output IDs in order, finishes, and exits successfully |

Use Python 3.11 or newer, QEMU with x86 TCG, matching OVMF code and variable templates, and the release manifest for the disk. Supply a fresh results directory. The operator supplies the three expected token IDs from a separate reference run on the same model. The diagnostic prompt uses token IDs 17, 488, 220, 17, 284. The recorded Qwen2.5-Coder-0.5B fixture produced 220, 19, 198 for this prompt.

```sh
python3 os-docs/installed-acceptance/check.py \
  --disk /qa/installed.raw --manifest /qa/release.json \
  --ovmf-code /usr/share/OVMF/OVMF_CODE_4M.fd \
  --ovmf-vars /usr/share/OVMF/OVMF_VARS_4M.fd \
  --expected-tokens 220 19 198 --results /qa/readback-01
```

To isolate a failed path, pass `--modes recovery`, `--modes desktop`, or `--modes diagnostic`. The JSON result lists the selected paths. The transcript acceptance rules can be checked with `python3 -m unittest discover -s os-docs/installed-acceptance`.

Each boot gets a fresh firmware variable store and disposable disk writes. The runner compares the whole disk with the release manifest before boot, then checks the disk again after success or failure. It saves the guest serial stream, QEMU standard error, and a JSON result identifying the exact inputs and completed boots. The runner atomically replaces `result.json` before and after each boot. A forced stop leaves the last checkpoint marked `in_progress`. Its `boots` entries identify completed paths, and `active_mode` identifies any boot still underway at that checkpoint. The runner records `complete` only after all selected paths pass and the final disk digest matches. An orderly failure records `failed` and the available error details. On a host under memory pressure, `--ram-directory` places temporary model-guest memory in a private directory on the chosen filesystem. Allow enough free space for the selected guest memory size. `--timeout` sets a deadline for each boot.

The desktop and diagnostic boots exercise distinct paths. The diagnostic measures one fixed inference. Interactive model responses, model access to the Enterprise packet, updates to stored applications, physical installation, and performance on a physical GPU need their own measurements.

[QEMU invocation and snapshot controls](https://www.qemu.org/docs/master/system/invocation.html) · [QEMU disk images](https://www.qemu.org/docs/master/system/images.html)

## Read the installed Qwen model and license

On a Linux host with Python 3.11 or later, util-linux `partx` and GNU mtools, run the following against an offline raw factory installation and its matching release manifest.

```sh
python3 os-docs/installed-acceptance/check_model_partition.py \
  --disk /qa/installed.raw --manifest /qa/release.json
```

The checker opens the image read-only, compares its whole-disk SHA-256 with the manifest, validates the primary and backup GPT headers and both entry arrays (including their CRCs and matching identities), and locates the uniquely named GPT partition `PCP OS models` on the release's 512-byte-sector image. It reads both files through the FAT filesystem under the manifest's Qwen revision and prints their byte counts and hashes as JSON. It exits unsuccessfully if either file differs or the partition cannot be identified uniquely. Use this check for the factory disk before user changes. Its result covers installed content. Boot selection and model execution have their own checks above.

The model-content fixture tests use `mformat`, `mmd` and `mcopy` to create disposable volumes and run through the same file-reading path. The six GPT-integrity fixtures use Python's standard library alone, including corrupt secondary metadata and self-consistent but divergent partition arrays. Run `python3 -m unittest discover -v -s os-docs/installed-acceptance` and check for skipped tests if the tools are unavailable. [GNU mtools manual](https://www.gnu.org/s/mtools/manual/mtools.pdf) · [UEFI GPT layout](https://uefi.org/specs/UEFI/2.9_A/05_GUID_Partition_Table_Format.html)
