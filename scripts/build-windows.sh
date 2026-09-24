#!/usr/bin/env bash
# Cross-compile meshd for Windows from Linux.
#   rustup target add x86_64-pc-windows-gnu
#   sudo apt install gcc-mingw-w64-x86-64        (the linker; `cargo check` needs no linker)
# Output: desktop/target/x86_64-pc-windows-gnu/release/meshd.exe
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
rustup target list --installed | grep -q x86_64-pc-windows-gnu || rustup target add x86_64-pc-windows-gnu
if ! command -v x86_64-w64-mingw32-gcc >/dev/null; then
  echo "x86_64-w64-mingw32-gcc missing: sudo apt install gcc-mingw-w64-x86-64  (type-check only: cargo check --target x86_64-pc-windows-gnu)"; exit 2
fi
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
cargo build --release --target x86_64-pc-windows-gnu --manifest-path desktop/Cargo.toml
ls -la desktop/target/x86_64-pc-windows-gnu/release/meshd.exe
