# PCP OS Store

The Store catalog lists the instruments a person or agent can run with the Rust host. Each name below is accepted by `pcpos instrument` and the MCP tool `acr_instrument`. Run `pcpos instrument list` to see what your own binary includes. A packaged release can differ from a build of this checkout.

## Science and mathematics

| Instrument | When to use it | Input fields |
| --- | --- | --- |
| `mathematics.exact` | Evaluate rational arithmetic or test an equality or inequality exactly. | `expression` |
| `mathematics.protected_bins` | Compare squared norms and odd coordinate gcds for corresponding bins of two integer columns. [Worked example and limits](../PCP_OS_Rust/README.md#checking-a-signed-column-in-protected-bins) | `scale_exponent`, `source`, `target` |
| `mathematics.statement` | Compare a named Lean statement with a pinned digest and screen for trivial forms. Run Lean itself to check the proof. | `source`, `theorem`, optional `pinned` |
| `science.order` | Check which of two artifacts first entered a particular register. | `earlier`, `later` (SHA-256 digests), plus the register |
| `statistics.grim` | Test whether a rounded mean is possible for the given count of integer observations. | `mean` (text preserving reported precision), `n` |
| `statistics.percent` | Test whether a reported percentage is possible for an integer count out of a stated total. | `percent` (text preserving reported precision), `total` |
| `engineering.units` | Convert compatible physical units and compare at the precision of the second quantity. | `from`, `to` (for example, `"1 m"` and `"100 cm"`) |

## Integrity and review

| Instrument | When to use it | Input fields |
| --- | --- | --- |
| `cryptography.digest` | Compute the SHA-256 of supplied text and optionally compare a claimed digest. | `text`, optional `claimed` |
| `quality.coverage` | Find claims without findings in a commissioned review and list recorded checks its findings do not cite. | `review` (label), plus the register |
| `core.calibration` | Check the counts, thresholds and claimed result in a Calibration Record. | `record` (object) |

## Run an instrument

With `pcpos` on your path, these commands list the catalog and check a small calculation. The JSON example uses Bash or PowerShell 7.3+ native argument quoting.

```sh
pcpos instrument list
pcpos instrument mathematics.exact '{"expression":"1/3 + 1/6 = 1/2"}'
```

The calculation prints `TRUE` and returns exit code 0. A completed check whose predicate fails returns 1. An unreadable input or an operation the instrument cannot perform returns 2. For the two register-dependent instruments, pass the register file as the final CLI argument. Terminal runs print their results.

In an MCP session call `acr_instrument` with the arguments below to run the same calculation. Set `instrument` to `"list"` to discover names. The example uses `record: false` for a read-only calculation. With recording enabled and an open review, the host can enter the execution as a check in the selected register. [CLI and MCP implementation](../PCP_OS_Rust/crates/pcpos-host/src/pcpos.rs)

```json
{"instrument":"mathematics.exact","input":{"expression":"1/3 + 1/6 = 1/2"},"record":false}
```

## Applications, modules and packages

| Looking for | Where it lives | How it is delivered |
| --- | --- | --- |
| Built-in instruments above | [Rust catalog and dispatcher](../PCP_OS_Rust/crates/pcpos-host/src/instruments.rs) and [per-instrument implementations](../PCP_OS_Rust/crates/pcpos-host/src/instruments/) | Build or obtain a host containing the instrument. The CLI and MCP share the same implementation. |
| Separately installable Store apps | [Package entry format and verifier](../PCP_OS_Rust/crates/pcpos-pkg/src/lib.rs) and [transaction model](../PCP_OS_Rust/crates/pcpos-pkg/src/txn.rs) | This checkout has no published app packages or end-user remote install command. The library verifies supplied bytes and models install, remove and restore in memory. |
| ACR method modules | [Module catalog in the released method](https://github.com/KirtusJ/pcp-os-claude/blob/master/method/REFERENCE/Module_Catalog.csv) | Inspect a method tree with `pcpos modules <method-directory>`. The host also accepts a local module specification through `pcpos module add`. |
| Assistant plugins | [Claude plugin marketplace manifest in the release repository](https://github.com/KirtusJ/pcp/blob/master/PCP_1.0_Plugins/.claude-plugin/marketplace.json) | The assistant's plugin manager handles these packages. |
| Programs on the kernel boot screen | [Kernel launcher and program builds](../PCP_OS_Rust/crates/pcpos-kernel/README.md) | The bootloader supplies program images, including the probe and the Doom build target. |

### Proposed research tools

The [incident-count bounds instrument](https://github.com/KirtusJ/pcp-os/pull/83) and [shared Science and Mathematics SVG plotter](https://github.com/KirtusJ/pcp-os/pull/86) have separate review branches. Follow the pull requests for their examples and integration status.

### Maintaining this catalog

For a built-in instrument, add its Rust implementation and dispatch entry, cover its accepted inputs and refusal cases, and document its name, purpose and inputs in the same change. The [catalog check](../PCP_OS_Rust/crates/pcpos-host/tests/store_catalog.rs) compares the documented names with a freshly built host. Add a separately distributed app to its own listing when its package and installation path can be exercised. [Rust contribution policy](../RUST_PRIMACY.md)
