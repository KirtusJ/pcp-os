# Run a retrieval check with either Enterprise adapter

The standalone Build 22 text and OS adapters include the seven Specifications and the defined-terms table. Start the participant with `/pcp:calibrate` (text) or `/pcp-os:calibrate` (OS). The assessor's released question generator is available in this PCP OS checkout. Python 3, Git and the following local files are sufficient to generate a retrieval prompt and score the answers offline.

1. Obtain the [Build 22 text adapter](https://github.com/KirtusJ/pcp-claude/tree/3187dc226cd9f51b81a940f7d986bc124b26bc12). It provides the packet's lookup program. Use the method directory from the adapter being assessed, either that text adapter or the [Build 22 OS adapter](https://github.com/KirtusJ/pcp-os-claude/tree/c0fbde739b4cc3bc409878d901de0974702c506e).
2. In a directory private to the assessor and outside the participant's workspace, choose a new output path. From the PCP OS repository run the following, substituting local paths. The preparer checks the manifest and each required file against Build 22 before writing. For a new measurement, select the reach-checked scorer. The preparer independently pins that program and copies it with the released lookup program, seven Specifications, defined terms and manifest into the new directory. The bundled Build 19 Enterprise packet predates this command.

```sh
python3 -I -B examples/method-readback/prepare-retrieval.py \
  --method /path/to/installed-plugin/method \
  --scorer reach-checked \
  --lookup /path/to/pcp-claude/method/TOOLS/acr.py \
  --out /private/assessor/retrieval
cd /private/assessor/retrieval
```

3. Choose an integer seed privately for this run. Keep the seed, items file and answer key with the assessor. Give the participant only the contents of `prompt.txt`. Use a participant environment without access to the assessor’s directory. Separate working directories under the same account can still be readable.

```sh
python3 -I -B TOOLS/calibrate.py items --seed "$seed" --out items.json --key key.json
python3 -I -B TOOLS/calibrate.py prompt items.json > prompt.txt
```

4. Preserve the participant's original JSON reply as `answers.json`. With the reach-checked scorer, choose `full` for access to the Specifications and terms, `kernel` for access only to kernel extracts, or `none` for no method text in reach. Record the actual model, host, delivery, extent, date and counts with the result.

```sh
python3 -I -B TOOLS/calibrate.py score items.json key.json answers.json --extent full
```

Record the scorer variant with the result. The reach-checked scorer is a PCP OS source correction, SHA-256 `3138c84bc5db7daeb09d9ebfca4f9dcbb75c8b6083473fd74bfbe9c164048c49`. For historical replay, omit `--scorer reach-checked`. The preparer always verifies the original program against the Build 22 manifest. For a new run it applies the [source correction](retrieval-reach.patch) in a temporary directory and checks the resulting digest before use. Both modes work offline in a shallow checkout or source export. The original Build 22 scorer credits an exact quotation of a provision even when the provision was out of reach. Review those historical rows before using a restricted-extent aggregate. [Executed Build 22 reproduction](https://github.com/KirtusJ/pcp-os/issues/187#issuecomment-6920146289)

The scoring command reports retrieval on these questions. Application requires separately administered items and an independent assessor. The full matching Implementation packet supplies those probes. [Build 22 Core §14.4](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/MODULES/01_Core/SOURCE/Specification.md#144-calibration) · [Python isolated mode](https://docs.python.org/3/using/cmdline.html#cmdoption-I)
