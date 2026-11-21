# Firmware calls during USB loader initialization

B3 inspected the default Q35 USB build from PCP OS commit `d92f5f630a1d452466113a61252f135219143506`. Its EFI executable has SHA-256 `90d49523c00f2f61fcb552fcb46c26e42e7ba144294d1d15ef14c04a008408f8`. The loader returns `EFI_UNSUPPORTED` when its TCG check fails. On the accepted VM path, it calls GNU-EFI `InitializeLib` before requesting the memory map. [Loader entry and guard](https://github.com/KirtusJ/pcp-os/blob/d92f5f630a1d452466113a61252f135219143506/examples/firmware-owned-uefi/loader.c#L160-L172)

I compared the linked ELF’s complete text and read-only data sections with the corresponding sections of the EFI executable. They were byte-identical. I inspected the calls in the linked initialization routine and ran the same GNU-EFI archive against two host-side service tables. In one, the firmware language variable was absent. In the other, it contained `eng`. A read of a missing variable is still a request.

| Observed in both host cases | Count |
|---|---:|
| Loaded Image protocol lookup | 1 |
| Firmware variable reads | 2, in the order `EFIDebug`, `Lang` |
| Unicode Collation protocol handle search | 1 |
| Pool allocations and frees | 2 of each |
| Firmware variable writes | 0 |

These are library calls from a simulated initialization. They do not measure what the EliteBook firmware does internally or exhaust the possible responses of every firmware implementation. The fixture returned no Unicode Collation handles. A hardware-targeted build needs its own firmware-call inventory, including link-time dependencies. Its limits should account for variable reads, protocol queries and temporary pool allocations. The current VM fixture also emits debug bytes to port `0xe9` after its TCG guard passes. Its fixed-allocation refusal path requests shutdown. The behavior a physical candidate presents on refusal remains an open acceptance check. [Debug output](https://github.com/KirtusJ/pcp-os/blob/d92f5f630a1d452466113a61252f135219143506/examples/firmware-owned-uefi/loader.c#L31-L39) · [Allocation refusal](https://github.com/KirtusJ/pcp-os/blob/d92f5f630a1d452466113a61252f135219143506/examples/firmware-owned-uefi/loader.c#L188-L194)

## Reproduction

On an x86-64 Linux host with a C compiler and the matching GNU-EFI installation, invoke `sh run.sh /path/to/usr /existing/output/directory`. The harness uses local functions for UEFI calls. Compare the two lines with [trace.txt](trace.txt).

| Checked input | SHA-256 |
|---|---|
| GNU-EFI libefi.a | `a3c6cfe519bf1850622f8c01854ee3b6473c7774795b97f4a693878e22233498` |
| EFI and linked ELF .text | `ab470afc948b0737fa433ffa07d2a1612b853cf7a1725439b00fa13184dc769b` |
| EFI and linked ELF .rodata | `c2adc35bd851efc180bf6765847f429c0bb8ad8a1b8dfbd31129a423251a3271` |
