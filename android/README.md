# android/ — MeshAI worker (Phase 1)

Kotlin + Jetpack Compose. minSdk 30, arm64-v8a only. See `docs/MESHAI.md` §10.

Native pieces to be placed here:
- `libllama.so` / `libggml*.so` built by `scripts/android-build-llama.sh` (+ `libggml-opencl.so` for Tier A/S).
- `libmeshcore.so` — Rust control plane via `cargo-ndk`, Kotlin bindings via UniFFI.
- Worker thread calls `ggml_backend_rpc_start_server(endpoint, cache_dir, n_threads, n_devices, devices[])` with an explicit device list.
- Foreground service type `connectedDevice`; `PARTIAL_WAKE_LOCK`; `WIFI_MODE_FULL_LOW_LATENCY`; `FLAG_KEEP_SCREEN_ON` on the dashboard.
- Every prebuilt `.so` must be 16 KB page-aligned (Play requirement since Nov 2025).
