# PCP OS Store instruments

These instruments ship inside the Rust host. To browse downloadable applications, visit the [Store](../README.md#package-index). Each name below is accepted by `pcpos instrument` and the MCP tool `acr_instrument`. Run `pcpos instrument list` to see what your own binary includes. A packaged release can differ from a build of this checkout.

## Science and mathematics

| Instrument | When to use it | Input fields |
| --- | --- | --- |
| `mathematics.exact` | Evaluate rational arithmetic or test an equality or inequality exactly. | `expression` |
| `mathematics.protected_bins` | Compare squared norms and odd coordinate gcds for corresponding bins of two integer columns. [Worked example and limits](../../PCP_OS_Rust/README.md#checking-a-signed-column-in-protected-bins) | `scale_exponent`, `source`, `target` |
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

In an MCP session call `acr_instrument` with the arguments below to run the same calculation. Set `instrument` to `"list"` to discover names. The example uses `record: false` for a read-only calculation. With recording enabled and an open review, the host can enter the execution as a check in the selected register. [CLI and MCP implementation](../../PCP_OS_Rust/crates/pcpos-host/src/pcpos.rs)

```json
{"instrument":"mathematics.exact","input":{"expression":"1/3 + 1/6 = 1/2"},"record":false}
```

## Proposed research tools

The incident-count bounds instrument and the shared Science and Mathematics SVG plotter are available on their review branches. Preview the proposed plotter's output in the finite-walk example. [Incident-count proposal](https://github.com/KirtusJ/pcp-os/pull/83) · [Plotter proposal](https://github.com/KirtusJ/pcp-os/pull/86) · [Finite-walk SVG preview](https://github.com/KirtusJ/pcp-os/blob/e8d62e4b00b7bcb404b67fbba9151c50814f1d70/PCP_OS_Rust/docs/plots/finite_walk.svg)

## Maintaining this catalog

Add an instrument's name, purpose and inputs in the same change as its Rust implementation, dispatch entry and tests. The [catalog check](../../PCP_OS_Rust/crates/pcpos-host/tests/store_catalog.rs) compares these tables with a freshly built host. [Rust dispatcher](../../PCP_OS_Rust/crates/pcpos-host/src/instruments.rs) · [Instrument implementations](../../PCP_OS_Rust/crates/pcpos-host/src/instruments/) · [Rust contribution policy](../../RUST_PRIMACY.md)
