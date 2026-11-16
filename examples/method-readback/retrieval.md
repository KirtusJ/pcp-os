# Run a retrieval check with either Enterprise adapter

The standalone Build 22 text and OS adapters include the seven Specifications and the defined-terms table. Start the participant with `/pcp:calibrate` (text) or `/pcp-os:calibrate` (OS). The assessor's released question generator is available in this PCP OS checkout. Python 3 and the following local files are sufficient to generate a retrieval prompt and score the answers offline.

1. Obtain the [Build 22 text adapter](https://github.com/KirtusJ/pcp-claude/tree/3187dc226cd9f51b81a940f7d986bc124b26bc12). It provides the packet's lookup program. Use the method directory from the adapter being assessed, either that text adapter or the [Build 22 OS adapter](https://github.com/KirtusJ/pcp-os-claude/tree/c0fbde739b4cc3bc409878d901de0974702c506e).
2. In a directory private to the assessor and outside the participant's workspace, choose a new output path. From the PCP OS repository run the following, substituting local paths. The preparer checks the manifest and each required file against Build 22 before writing. It copies the two released programs, seven Specifications, defined terms and manifest into the new directory. The bundled Build 19 Enterprise packet predates this command.

```sh
python3 -I -B examples/method-readback/prepare-retrieval.py \
  --method /path/to/installed-plugin/method \
  --lookup /path/to/pcp-claude/method/TOOLS/acr.py \
  --out /private/assessor/retrieval
cd /private/assessor/retrieval
```

3. Choose an integer seed privately for this run. Keep the seed, items file and answer key with the assessor. Give the participant only the contents of `prompt.txt`. Use a participant environment without access to the assessor’s directory. Separate working directories under the same account can still be readable.

```sh
python3 -I -B TOOLS/calibrate.py items --seed "$seed" --out items.json --key key.json
python3 -I -B TOOLS/calibrate.py prompt items.json > prompt.txt
```

4. Preserve the participant's original JSON reply as `answers.json`. Score it on the assessor's side. Select `full` when the participant can inspect the Specifications and terms, `kernel` when it can inspect only the kernel extracts, or `none` when it has no method text in reach. Record the actual model, host, delivery, extent, date and counts with the result.

```sh
python3 -I -B TOOLS/calibrate.py score items.json key.json answers.json --extent full
```

The scoring command reports retrieval on these questions. Application requires separately administered items and an independent assessor. The full matching Implementation packet supplies those probes. [Build 22 Core §14.4](https://github.com/KirtusJ/pcp-os-claude/blob/c0fbde739b4cc3bc409878d901de0974702c506e/method/MODULES/01_Core/SOURCE/Specification.md#144-calibration) · [Python isolated mode](https://docs.python.org/3/using/cmdline.html#cmdoption-I)
