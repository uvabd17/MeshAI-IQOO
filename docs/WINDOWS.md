# MeshAI on Windows

`meshd` is one Rust binary (`meshd.exe`) that serves the admin panel, the control plane for
phones and the OpenAI-compatible `/v1` endpoint. Everything platform-specific goes through
`sysinfo` (memory, process control, hostname) and `local-ip-address`, so the Linux and Windows
builds are the same code. The parts that differ are listed at the end.

## What you need on the Windows machine

| Item | Why | Where |
|---|---|---|
| Rust (stable, MSVC or GNU toolchain) | build `meshd.exe` | https://rustup.rs |
| protoc — **not needed**: `protoc-bin-vendored` ships one | control-plane protobufs | (crate) |
| llama.cpp Windows binaries built with `GGML_RPC=ON`: `llama-server.exe`, `ggml-rpc-server.exe`, `llama-bench.exe` and their DLLs | host / worker / bench processes | `scripts/windows/setup.ps1` downloads the pinned release, or build from `third_party/llama.cpp` with CMake |
| GGUF models | the catalog | `scripts/windows/setup.ps1 -Models` or the admin's Models page |

## Build and run

```powershell
# from the repo root, in PowerShell
scripts\windows\setup.ps1            # llama.cpp binaries → third_party\llama.cpp\build-host\bin
cargo build --release --manifest-path desktop\Cargo.toml
scripts\windows\run-meshd.ps1        # opens the firewall for :7070/:8080 (asks for admin once) and starts meshd --lan
```

Then open `http://localhost:8080/admin/?token=<printed token>`. Pair the phone from **Pair phone**
exactly as on Linux; the phone dials the address shown in the QR, so pick the interface that
faces the phone (`POST /api/pair/offer {"host": "<ip>"}`, or the admin's host field).

Cross-compiling from Linux instead: `rustup target add x86_64-pc-windows-gnu`, install
`gcc-mingw-w64-x86-64`, then `scripts/build-windows.sh` produces `desktop/target/x86_64-pc-windows-gnu/release/meshd.exe`.
(`cargo check --target x86_64-pc-windows-gnu` passes without the linker; it is run in `scripts/verify`.)

## Windows Firewall and links

- `meshd --lan` listens on `0.0.0.0:8080` (token required) and `0.0.0.0:7070` (control plane,
  paired devices only). `run-meshd.ps1` adds inbound rules for both; remove them with `-Remove`.
- Private link options, same as Linux: the laptop's mobile hotspot (Settings → Network → Mobile
  hotspot), the phone's hotspot, or USB tethering (the phone appears as an "RNDIS" adapter).
- The RPC data plane (`:50052` on the phone) is dialled *from* the laptop, so no extra inbound rule.

## Platform differences (all handled in code)

| Concern | Linux | Windows |
|---|---|---|
| free memory | `sysinfo` `available_memory()` | same |
| what a stop frees (D024 credit) | `RssAnon+RssShmem` from `/proc/<pid>/status` | `sysinfo` process memory (private working set) |
| stop a child | SIGTERM, then SIGKILL after 3 s | `TerminateProcess` (no gentler signal exists); llama-server writes nothing on exit so this is safe |
| is a pid alive | `sysinfo` | same |
| pairing secrets file mode | `0600` | NTFS ACL of the user profile (`state\paired.json` is under the user's directory) |
| binaries | `llama-server`, `ggml-rpc-server` | same names; Rust appends `.exe` |
| open the admin | `xdg-open` (by the user) | `Start-Process` in `run-meshd.ps1` |

## Not yet done on Windows

- Nothing has been *executed* on a Windows machine in this repo yet: the port is `cargo check`
  clean for `x86_64-pc-windows-gnu` and the platform calls are covered by `sysinfo`, but the
  Definition of Done requires a run. Do that first on a Windows laptop: `scripts/verify` there,
  then `scripts/qa/run-guards.sh` under Git Bash.
- A Windows service wrapper (NSSM or `sc.exe`) is optional; `run-meshd.ps1` is a foreground run.
