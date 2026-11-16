# Supervise resident PCP OS inference workers

This Linux QA runner boots 24 distinct x86-64 PCP OS guests and keeps their numerical workers resident. Each parent emits the child requests. The local QEMU supervisor verifies the requests and starts their virtual machines. Repeated challenges check that a worker can still perform inference after its children start. Each VM has 32 MiB of guest RAM, a 4 MiB TCG translation cache, a virtual keyboard, and networking disabled. Allow additional host memory for the emulator and firmware.

## Build and run the public fixture

The guest runs a Rust implementation of the 260K-parameter TinyStories transformer. Each key selects a two-token prompt (BOS followed by token 300 through 315). The guest generates three tokens by greedy decoding on every request. The builder compares all 48 output tokens against the pinned upstream C implementation before producing the worker ELF. Each guest also exercises the PCP core transition for the first required gate and its out-of-order refusal.

From the repository root on x86-64 Linux, use a C compiler, Python 3.9+, Rust with the `x86_64-unknown-none` target and `rust-src`, LLVM's `ld.lld`, GNU `objcopy`, and QEMU. Use a fresh directory for the fixture and for each supervisor run. These commands retrieve the two external inputs. The builder checks both SHA-256 digests before invoking either compiler.

```sh
mkdir -p inputs
curl -fL https://huggingface.co/karpathy/tinyllamas/resolve/0bd21da7698eaf29a0d7de3992de8a46ef624add/stories260K/stories260K.bin -o inputs/stories260K.bin
curl -fL https://raw.githubusercontent.com/karpathy/llama2.c/350e04fe35433e6d2941dce5a1f53308f87058eb/run.c -o inputs/run.c
rustup target add x86_64-unknown-none
rustup component add rust-src
python3 examples/resident-workers/fixture/build.py \
  --model inputs/stories260K.bin --reference inputs/run.c --out inputs/resident
PCP_OS_Rust/crates/pcpos-kernel/run.sh build
python3 examples/resident-workers/supervise.py \
  --kernel PCP_OS_Rust/crates/pcpos-kernel/target/pcpos-kernel.elf32 \
  --worker inputs/resident/resident.elf --oracle inputs/resident/oracle.json \
  --run inputs/run-01 --once
```

The checkpoint SHA-256 is `b0a507e7ad0f626624f17112325e66691f9076d622e1d3274d103d00299f2696`. The reference source SHA-256 is `9c4f2d5c6ae01b71726d1cc37530d71e60bff0ec7cc012565f16a43c1ca658bd`. The guest embeds the checkpoint. The oracle runs on the host and its outputs stay outside the guest ELF. [Model release](https://huggingface.co/karpathy/tinyllamas/tree/main/stories260K) · [Upstream reference](https://github.com/karpathy/llama2.c/tree/350e04fe35433e6d2941dce5a1f53308f87058eb)

## Supervise a cohort

Use `--qemu` and `--firmware` for a nonstandard QEMU installation. Supply `--once` for a single cycle followed by shutdown. The default interval between complete challenge cycles is 60 seconds. Interrupt or terminate the supervisor for orderly shutdown.

The supervisor checks the measured kernel, admitted worker, argument values, the parent ticket and the exact child list on startup. At each subsequent cycle it queries the VM UUID, sends a fresh key, consumes only new serial bytes, compares the response to the supplied reference, and checks all running states again. QMP requests and replies carry matching IDs. [QMP specification](https://www.qemu.org/docs/master/interop/qmp-spec.html)

Inspect `events.jsonl` for admissions, completed cycles, failures and shutdowns. `readback.json` records the last completed cycle with its time, identities and serial digests. Each completed `readback` event also retains the full record for that round. To verify an older serial digest after later challenges or branch repairs, hash the first `serial_bytes` raw bytes of the named `serial_file`. Earlier runs that recorded only a digest in the event log require a separately retained snapshot for that round. Serial files and QEMU diagnostics remain available when startup or a later cycle fails. The supervisor reaps the QEMU processes it started when it exits normally, encounters a failed check, or handles an interrupt or termination signal. Forced host termination and gaps between observations can interrupt coverage. A completed readback measures fixed-prompt inference and topology. Protocol comprehension, unrestricted text generation, nested hardware virtualization and daily-driver fitness require separate observations.

For a busy host, set `--startup-timeout` and `--response-timeout` in seconds. One startup attempt has a 120-second overall deadline for the QMP handshake, serial boot output and identity checks. Each monitor or challenge response defaults to 60 seconds. Set an optional `--startup-idle-timeout` to fail earlier if the boot serial stops growing while the guest is working toward READY. New bytes reset only the idle timer. The overall deadline still applies. For example, `--startup-timeout 900 --startup-idle-timeout 120` permits a slow, progressing boot while detecting a silent guest earlier. The default failure policy stops the cohort. Use `--repairs N` for up to N branch restarts after transport loss, emulator exit or a timeout. The supervisor logs the interruption, stops that node and its descendants, then admits fresh VMs after rechecking the parent. Unaffected siblings keep their identities. Attempts have separate logs. An identity, topology or inference mismatch fails the check. Replacement preserves logical placement and starts a new period of observation.
