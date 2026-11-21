# B3 read-back of the compact desktop at 3d345bf

The [two-program tester CD](https://github.com/KirtusJ/pcp-os/actions/runs/44903207380/artifacts/14706389905) was built from `3d345bf68df5b64cf8345dfa6754c7d98046b01d`, tree `5f4d222650cc3c76293f632cfd70749313c06363`. The hosted-kernel runs extracted that disk’s ELF and booted it directly in QEMU to select a framebuffer size. Its SHA-256 is `fb1a0e5902e5a4dfe84c85f3e791cc98ac6aa2389ac24c7ddbd1d165d5951a7e`.

| Record | What the guest showed |
| --- | --- |
| [Recent tasks and widgets](recent-and-widgets.zip) | Seven pins, three completed Queue Desk sessions and all widgets at 640×480 and 320×200. The 640×480 display kept all three tasks, memory, session files, selected identity and keys. |
| [Selected pins](selected-pins.tar.xz) | All fourteen pins reachable at both sizes. At 320×200, two 64-byte filenames with the same binary and a common prefix differed only in the Home position line. The corresponding Launcher and Details frames matched pixel for pixel. |
| [Launch feedback](launch-feedback.zip) | Locally built baseline and candidate kernels, the 32-byte runnable control, refusals under 33-, 63- and 64-byte names, native frames, serials and replay. The candidate exposes the refusal and distinguishes the tested suffixes. [Reference patch](compact-launch-feedback.patch) |
| [Files and paging](files.tar.xz) | Six hosted-kernel guests and six local candidate guests. At 320×200 a nonempty Files view clipped the total, and paged lists lost the Esc hint. Returning to Home worked in all twelve guests. The candidate keeps both labels readable and rejects altered frames that lose either. [Reference patch](compact-files.patch) |

The patches are separate candidates for Z3's review. Results labelled “hosted kernel” use the ELF from the Actions artifact. Results labelled “candidate” use the particular local build recorded with that test.
