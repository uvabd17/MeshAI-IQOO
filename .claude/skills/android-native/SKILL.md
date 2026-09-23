---
name: android-native
description: Android worker engineering rules for MeshAI — cpusets and thread affinity, foreground-service types and time caps, thermal headroom, battery, memory headroom and low-memory killer, Wi-Fi locks, model files via SAF fd, 16 KB page alignment, embedding the ggml RPC server in-process. Loads automatically when editing android/; use when touching the worker, profiler, or native bridge.
paths: android/**
user-invocable: false
---

# Android worker — native rules

Read [references/apis.md](references/apis.md) for exact API names and constraints before writing code. The rules that follow are the non-obvious ones that bite.

1. **Big cores come from being top-app.** The visible Activity puts the process in the `top-app` cpuset (all cores). Background = little cores only. Keep the dashboard Activity visible (`FLAG_KEEP_SCREEN_ON`) while a plan is active; then pin ggml worker threads inside that cpuset with `sched_setaffinity` via `syscall`. Read `/proc/self/status → Cpus_allowed_list` and show it on the dashboard.
2. **Foreground service type is `connectedDevice`**, never `dataSync` / `mediaProcessing` (6 h per 24 h cap on Android 15+). Prerequisite is satisfied by holding `CHANGE_WIFI_STATE`. `specialUse` is the fallback and needs a manifest justification property.
3. **The RPC server runs in-process and blocks.** `ggml_backend_rpc_start_server(endpoint, cache_dir, n_threads, n_devices, devices[])` runs the accept loop — call it on its own thread, pass an explicit `devices[]` (CPU, optionally OpenCL) so backend selection is ours (sidesteps llama.cpp #11957). Bind to the paired link's interface only, after pairing, and stop it when the plan ends.
4. **Memory headroom is a hard limit.** Report `ActivityManager.MemoryInfo.availMem` and `onTrimMemory` levels; the desktop decides. Keep 1–2 GB headroom until Phase 0 measures the real kill threshold. `mlock` is not available at these sizes; rely on mmap + headroom + the RPC `-c` tensor cache.
5. **Thermal is a forecast, not a status.** Use `PowerManager.getThermalHeadroom(forecastSeconds)` (API 30) for scheduling; `addThermalStatusListener` only for the coarse dashboard badge.
6. **Model files by fd.** A user-picked file (SAF `ACTION_OPEN_DOCUMENT`) yields an fd; pass `/proc/self/fd/<n>` as the path to llama.cpp — mmap works through it, no `MANAGE_EXTERNAL_STORAGE`. App-private `getExternalFilesDir()` is the default cache location. Verify by SHA-256 against the catalog hash.
7. **16 KB pages.** NDK r28+ aligns by default; every prebuilt `.so` (OpenCL loader, GenieX AAR) must be checked with `llvm-readelf -l` → LOAD `Align 0x4000`.
8. **Wi-Fi low-latency lock is foreground-only.** `WifiManager.WIFI_MODE_FULL_LOW_LATENCY` + `PARTIAL_WAKE_LOCK` while a plan is active; release both when it ends.
9. **Phone decides nothing.** It reports (`Telemetry`) and obeys (`Plan`). Scheduling logic lives in `meshcore` so the admin web can show *why*.
10. **Compute path order:** CPU (`dotprod+i8mm`, `GGML_OPENMP=OFF`) → OpenCL Adreno for prefill (decode may be slower than CPU on 8 Elite class — measure) → GenieX/Hexagon only on Tier S and only for whole-model-alone mode.
