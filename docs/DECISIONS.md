# Decisions (append-only; supersede with a new entry, never edit history)

Format: **D### — title** · date · context · decision · alternatives rejected · consequences · status.

## D001 — Inference engine is upstream llama.cpp / ggml
2026-09-23 · Only engine with native Android build, RPC backend, OpenCL-Adreno (incl. 840, MXFP4), Hexagon, gpt-oss and Qwen3-MoE support in one tree. **Rejected:** prima.cpp (ZMQ, Termux-only, CUDA-only, stale fork, no gpt-oss), GenieX as base (Snapdragon-only, no multi-device), own engine (pointless). **Consequences:** track upstream master; `third_party/llama.cpp` pinned by commit in scripts. Status: accepted.

## D002 — Two planes: ggml RPC data plane (borrowed), MeshAI control plane (owned)
2026-09-23 · RPC v7 is proof-of-concept and insecure on open networks, but re-implementing tensor transport is out of scope for v1; the product value is in pairing, profiling, planning, scheduling, observability. **Decision:** tensors over ggml RPC embedded in-process on the phone with explicit `devices[]`; everything else over `proto/mesh.proto` on a Noise-encrypted TCP stream. RPC port opens only after pairing, bound to the paired link. **Rejected:** own transport in v1 (Phase 3 candidate to fix #22850 overhead). Status: accepted.

## D003 — Shared native core in Rust (`meshcore`), Kotlin shell on Android, `meshd` daemon on desktop
2026-09-23 · One implementation of protocol/planner/scheduler for Linux, Windows and Android; memory-safe networking; single static desktop binary. **Rejected:** C++ everywhere (network/crypto risk under time pressure), Kotlin Multiplatform (JVM desktop is not native), Python/FastAPI (not shippable). **Consequences:** team ramps on Rust; cargo-ndk + UniFFI for the Android binding. Status: proposed — confirm before T010 (see state/blockers.md).

## D004 — Android floor: arm64-v8a, minSdk 30, ≥8 GB, `dotprod`; tiers S/A/B
2026-09-23 · `getThermalHeadroom` is API 30; layer-split of 20B-class models needs ≥8 GB with headroom; ggml CPU path needs dotprod for acceptable speed. Tier S = 8 Elite / 8 Elite Gen 5 (GenieX NPU), A = 8 Gen 3 / D9300+ / Tensor G4+, B = any arm64 ≥8 GB with dotprod (CPU, parallel-job role). Status: accepted.

## D005 — Foreground service type `connectedDevice`; dashboard stays visible during a plan
2026-09-23 · `dataSync`/`mediaProcessing` are capped at 6 h/24 h on Android 15+; top-app cpuset is required for big cores; Wi-Fi low-latency lock is foreground-only. `specialUse` is the fallback. Status: accepted, verify on Android 15/16 device in T020.

## D006 — Phone compute order: CPU → OpenCL (prefill) → GenieX/Hexagon (Tier S, whole-model-alone only)
2026-09-23 · Community data on 8 Elite: OpenCL prefill 3.7–24× faster, decode ~17% slower than CPU on 7B; Hexagon experimental (~3.5 GB/session); NNAPI deprecated. Vulkan not planned on phone (historic hangs on 8 Elite). Status: accepted pending T005 measurement.

## D007 — Headline demo model is Qwen3-Coder-30B-A3B; gpt-oss-20b runs under a 6 GB cgroup for the "budget laptop" story
2026-09-23 · Dev laptop has 18 GB; gpt-oss-20b (12.1 GB) fits on it alone, which would make the "neither device can hold it" demo false. 30B-A3B (18.6 GB) genuinely needs the phone. Status: accepted; revisit if the loaner iQOO 15 is 12 GB (then +2 phones).

## D008 — Own hotspot or USB tethering; Wi-Fi Aware/Direct deferred
2026-09-23 · Linux laptops lack practical NAN support; venue Wi-Fi isolates clients. USB tethering is the low-jitter default and charges the phone. Status: accepted, measure both in T004.
