# Director A3 read-back for PCP OS #312

**Decision: hold `f77f8aef17b2da9f69e2d6538af7868946b1d553`.** Compact Home now keeps the selected program’s full name, size and digest above the footer at capacity. It still tells the operator “Enter starts” and returns to an identical screen when task admission refuses the selected program.

## What the guest showed

In a diskless 640×480 QEMU PC guest, I pinned the same 54,256-byte Queue Desk ELF under different ASCII filenames. With wrapped filenames present, ten pins fit. The complete eleventh-pin refusal was visible. All four selections showed the matching name, size, digest and footer.

| Filename length | Home identity | Result of pressing Enter |
| --- | --- | --- |
| 32 bytes | One row | Queue Desk opened, emitted its ready marker, and exited 0 on Escape. |
| 33 bytes | One row | The guest logged a task admission refusal. The Home framebuffer was byte-identical before and after Enter. |
| 63 and 64 bytes | Two rows each | The first and last pins showed both lines and their digest. Both launch attempts received the same refusal with byte-identical Home frames. |

The same executable gave the same four launch outcomes on the withdrawn `04a2954`. At that revision’s 13-pin capacity, the card also overlapped the footer. The 32-byte task-label limit is unchanged from master. For this desktop change, a truthful indication at the selected pin or a visible refusal after Enter would address the open issue. [Task label limit](https://github.com/KirtusJ/pcp-os/blob/f77f8aef17b2da9f69e2d6538af7868946b1d553/PCP_OS_Rust/crates/pcpos-core/src/task.rs#L137) · [Compact Home](https://github.com/KirtusJ/pcp-os/blob/f77f8aef17b2da9f69e2d6538af7868946b1d553/PCP_OS_Rust/crates/pcpos-kernel/src/session.rs#L541)

I also decoded B3’s two original 640×480 seven-pin widget frames. The older frame shows three completed Queue sessions and memory. The candidate shows two sessions and a blank row above the footer. Memory is absent. These are B3’s guest runs. [A3’s frame read-back](B3-frame-readback.json) · [B3’s paired original evidence](https://github.com/KirtusJ/pcp-os/blob/263ba1c13ca8bb6447941a3650c9b948c766935f/evidence/desktop-display-311/B3-compact-small-and-widgets-f77f8ae.zip)

Two other fresh guest runs on this kernel passed. At 1920×1080, three distinct programs launched and returned from both bar and dock. At 640×480, the Files view showed files 16–30 after Page Down and 16–31 after Down, including the continuation of filename 16. All 31 files reopened. These checks used QEMU software emulation. Physical hardware and UEFI are outside this read-back.

## Full native suite

The full native suite exited 1. It passed 19 of 20 test groups and 107 of 108 assertions. The sole failed assertion required the recorded Doom demo to end DONE. The guest recorded FAILED after a page fault on exit. Interactive Doom, the four-minute soak, and the no-display case passed. The test runner used 240 seconds of host time for the soak. [Doom issue #310](https://github.com/KirtusJ/pcp-os/issues/310)

Both hosted tester-CD artifacts from this workflow contained byte-identical kernels. I verified the four ZIP digests, both CD receipts, their source commit and Git tree, and the kernel bytes. Kernel SHA-256 `7d1c7b9a97e32d6e0662b33cbfde84a16f9487bb386f974d39019678dd827123`. [Hosted build](https://github.com/KirtusJ/pcp-os/actions/runs/44901276095)

I used QEMU 10.0.13 with software emulation and the suite from the specified commit. The invocation below used full mode with `PCPOS_TESTS_ONLY` unset. The adapter carries QMP over stdio because this executor refused a Unix-domain socket. The transport diff is included.

```sh
PCPOS_ADAPTER_CHECKOUT="$PWD/source" python3 -u run_stdio.py "$PWD/kernel.elf32" "$PWD/programs" "$PWD/data" "$PWD/full"
```

[Machine-readable suite result](full-suite-readback.json) · [Suite arguments, per-assertion results, serial logs, screenshots and audio hashes](full-suite.zip) · [Display frames, decoded rows, inputs and replay scripts](display-evidence.tar.xz) · [Home after refusal](compact-home-after-refusal.png) · [Queue Desk running under the 32-byte name](queue-desk-running.png)

To check the compact frames, unpack the display archive and run `python3 verify_display.py SOURCE exact-display previous` with a checkout of the stated commit. The raw P6 captures and original serial logs are included for both revisions.
