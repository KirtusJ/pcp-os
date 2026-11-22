# Processor startup baseline for issue 396

Director A3 ran five private, diskless Q35 BIOS boots under QEMU 10.0.13 TCG. The input was the queue-desk tester CD from master commit `2f7d9d4315fdc485c6e0693e325643456993e1df`. Each boot used a read-only ISO, no NIC, the ordinary clock, 64 MiB RAM and the qemu64 CPU model.

| Sockets × cores × threads | APIC IDs from QEMU | Kernel checks at startup / shutdown |
| --- | --- | --- |
| 1 × 1 × 1 | 0 | 71 / 72 |
| 1 × 2 × 1 | 0, 1 | 71 / 72 |
| 1 × 4 × 1 | 0, 1, 2, 3 | 71 / 72 |
| 1 × 8 × 1 | 0, 1, 2, 3, 4, 5, 6, 7 | 71 / 72 |
| 2 × 3 × 1 | 0, 1, 2, 4, 5, 6 | 71 / 72 |

The check reads the `apic-id` QOM property for each processor returned by `query-cpus-fast`. The six-vCPU fixture distinguishes a correct processor count from a correct identity set. The guest reported its pinned kernel measurement in each boot. F10 and a fresh Enter ended each guest with QMP reason `guest-shutdown` and exit 0. The serial footer still lists a second processor as absent. Secondary execution, parking and performance remain unmeasured on this input.

The ISO SHA-256 is `53af1322c98f0f20e23645909135d41f1b878ada301c9f6f3b6a946a02643d39`. The kernel measurement is `b151a16572913d8d26196187931933bd592fd9d67bfa9f151a498d9689c55f7d`. The two download archive digests matched the GitHub metadata before extraction. [Tester CD](https://github.com/KirtusJ/pcp-os/actions/runs/45028850462/artifacts/14757390957) · [Build receipt](https://github.com/KirtusJ/pcp-os/actions/runs/45028850462/artifacts/14757107002)

Each directory contains the original serial bytes and the corresponding QMP observations. To repeat the sparse-ID case, extract the ISO and `first/receipt.json` from the linked artifacts and run the command below. Use a new output directory for each run.

```sh
python3 check.py --iso pcp-os-tester.iso --receipt first/receipt.json \
    --topology 6,sockets=2,cores=3,threads=1 --count 6 --output replay-q35-6
```

The hosted PCP OS recursion helper ran the five emulator checks under one parent. The supervisor reopened the five results and verified their serial hashes after worker exit. An initial attempt using a local QMP socket stopped at host startup because socket creation was unavailable. The recorded boots used QMP over standard input and output.
