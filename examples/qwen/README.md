# Read a PCP register with a local language model

Qwen runs on your computer under llama.cpp. The [example adapter](status.py) asks Qwen which tool to call, sends one `acr_status` request to the Rust host, then gives the host's reply to Qwen. The host reads a temporary copy of the repository's [13-event exercise](../../PCP_OS_Rust/vectors/register_fphpm_exercise.expected.json). This example runs offline once you have the tools and weights.

## Run the example

1. From the repository root, build the Rust host. With Cargo's default target directory, the Windows executable is `PCP_OS_Rust/target/debug/pcpos.exe`.

   ```sh
   cargo build --manifest-path PCP_OS_Rust/Cargo.toml --locked -p pcpos-host --bin pcpos
   ```

2. Obtain llama.cpp for your platform. Keep its libraries alongside `llama-completion`. Download the instruction-tuned model file `qwen2.5-0.5b-instruct-q4_k_m.gguf`. The tested Windows x64 CPU build was `b12387`. The model file is 491,400,032 bytes with SHA-256 `74a4da8c9fdbcd15bd1f6d01d621410d31c6fc00986f5eb687824e7b93d7a9db`. [llama.cpp release](https://github.com/ggml-org/llama.cpp/releases/tag/b12387) · [Qwen's pinned weights](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/blob/9217f5db79a29953eb74d5343926648285ec7e67/qwen2.5-0.5b-instruct-q4_k_m.gguf)

3. Run the adapter with Python 3.9 or newer. Substitute the paths to your runner and weights. On Linux or macOS, use the appropriate executables without the `.exe` suffix.

   ```sh
   python examples/qwen/status.py --pcpos PCP_OS_Rust/target/debug/pcpos.exe --runner "PATH/llama-completion.exe" --model "PATH/qwen2.5-0.5b-instruct-q4_k_m.gguf"
   ```

The Windows CPU run printed `Model requested acr_status`. The Rust reply reported 13 events and the head prefix `836469bc0d389369`. Qwen then answered `13`. The adapter checks that the copied register bytes are unchanged and prints the Rust reply alongside the generated answer so you can check it yourself. The runner loads the model twice, once for each turn, and exits after each one.

## Connect an existing MCP client

Configure the MCP client to launch your compiled `pcpos` with the argument `mcp` over standard input and output. Set `PCPOS_ROOT` to the workspace, `PCPOS_REGISTER` to the register path relative to it, and `PCPOS_ACTOR` to the participant label for that session. The client can list the host's tools after initialization. For a read-only connection, expose only `acr_status` with empty arguments, as this example does. The host also implements operations that can write to a register. [Rust MCP server](../../PCP_OS_Rust/crates/pcpos-host/src/pcpos.rs) · [MCP standard-I/O transport](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports)

The model runtime produces text. The client decides which requests to forward. The Rust host checks the register and enforces its event rules. The [native PCP-OS guest](../../PCP_OS_Rust/crates/pcpos-kernel/) is a separate execution environment. This replay runs both inference and the Rust host on the operating system running Python.
