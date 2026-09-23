# desktop/ — meshd (Phase 1)

Rust, single binary for Linux (primary) and Windows. See `docs/ARCHITECTURE-v3-native-mesh.md` §4.

Planned crates: `tokio`, `prost` (proto/mesh.proto), `snow` (Noise XX pairing), `gguf` (planner input), `axum` (JSON API + /v1 proxy + /admin), `qrcode`, `sysinfo`.
Subcommands: `meshd serve | pair | plan <model.gguf> | bench`.
Owns the `llama-server --rpc …` child process; restarts it on replan.
