# Research log (one entry per question; format in .claude/skills/research-protocol/references/template.md)

## R001 — Which inference engine can serve as MeshAI's base?            (2026-09-23)
**QUESTION** Can an existing engine give native Android + RPC split + Adreno GPU + gpt-oss/Qwen3-MoE support without forking?
**CANDIDATES** A llama.cpp upstream (preferred before research) · B prima.cpp · C Qualcomm GenieX · D exo / distributed-llama
**OFFICIAL EVIDENCE** [T1] llama.cpp RPC README: proof-of-concept, "never run on an open network", protocol v7, `-c` cache, `--device`, RDMA w/ TCP fallback — https://github.com/ggml-org/llama.cpp/blob/master/tools/rpc/README.md · [T2] `ggml_backend_rpc_start_server(endpoint, cache_dir, n_threads, n_devices, devices[])` — https://github.com/ggml-org/llama.cpp/blob/master/ggml/include/ggml-rpc.h · [T1] OpenCL backend lists Adreno 840, supports MXFP4 — https://github.com/ggml-org/llama.cpp/blob/master/docs/backend/OPENCL.md · [T1] GenieX BSD-3, Android AAR, 8 Elite Gen 5, OpenAI server, no multi-device — https://github.com/qualcomm/GenieX · [T2] prima.cpp: MIT, ZMQ, Termux-only Android, CUDA-only, no Windows, Llama/Qwen2.5/QwQ/DeepSeek only — https://github.com/OpenCPIL/prima.cpp
**REAL-WORLD EVIDENCE** [T3] #11957 rpc-server on Android picks CPU over OpenCL — closed not-planned, bug-unconfirmed — https://github.com/ggml-org/llama.cpp/issues/11957 · [T3] #22850 RPC 28–55% loss on 2.5 GbE, causes: metadata re-serialisation, no hash <10 MB, synchronous — https://github.com/ggml-org/llama.cpp/issues/22850 · [T3] #25876 Hexagon HMX garbled (whisper), closed stale — https://github.com/ggml-org/llama.cpp/issues/25876
**BENCHMARKS** [T4] prima.cpp (ICLR 2026): 8B/14B no gain; 30B 72 vs 202 ms/tok; 70B 674 ms/tok, 4 home devices 3–7 ms — https://arxiv.org/abs/2504.08791 · [T5] 8 Elite OpenCL: prefill 3.7–24× vs CPU, decode −17% on 7B — https://github.com/ggml-org/llama.cpp/discussions/23736
**KNOWN ISSUES** Vulkan hangs on load on 8 Elite era devices (unverified for 840) · Hexagon ~3.5 GB/session, experimental.
**UNKNOWN** Does an in-app RPC server with explicit OpenCL device actually run GPU kernels on Adreno 840? → T005. Real decode t/s laptop↔phone → T004.
**DECISION** A · **CONFIDENCE** High
**WHY NOT B?** stale fork, no Windows/gpt-oss, Termux · **WHY NOT C?** no splitting; keep as Tier-S backend · **WHY NOT D?** no Android.
**CONSEQUENCE** D001, D002, D006.

## R002 — Can an unprivileged Android app get the big cores and keep running?            (2026-09-23)
**QUESTION** Can the worker pin ggml threads to big cores and run for hours without Android killing or throttling it?
**CANDIDATES** A top-app + sched_setaffinity + connectedDevice FGS (preferred) · B dataSync FGS · C root / vendor hooks
**OFFICIAL EVIDENCE** [T1] cpusets: top-app gets all cores, background restricted to little cores — https://source.android.com/docs/core/power/performance · [T1] dataSync/mediaProcessing 6 h/24 h then `onTimeout` (Android 15+); Android 16 quotas on FGS-started jobs — https://developer.android.com/develop/background-work/services/fgs/timeout · [T1] 16 KB pages required since Nov 2025; NDK r28+ default — https://developer.android.com/guide/practices/page-sizes · [T1] NNAPI deprecated in Android 15; migrate to LiteRT / vendor runtimes — https://developer.android.com/ndk/guides/neuralnetworks/migration-guide
**REAL-WORLD EVIDENCE** [T5] LWN on Android EAS/cpusets — https://lwn.net/Articles/706374/ · [T5] NDK list: affinity via `syscall(__NR_sched_setaffinity, gettid(), …)` — https://groups.google.com/g/android-ndk/c/PKpldjrx8c8
**UNKNOWN** Real kill threshold per tier → T006. Whether `connectedDevice` stays un-capped on Android 16 devices → T020.
**DECISION** A · **CONFIDENCE** Medium-High
**WHY NOT B?** 6 h cap · **WHY NOT C?** not a consumer product.
**CONSEQUENCE** D004, D005; android-native skill.

## R003 — Link: hotspot vs USB tethering vs Wi-Fi Aware/Direct            (2026-09-23)
**QUESTION** Which link gives low, stable RTT between an Android phone and a Linux laptop without venue infrastructure?
**CANDIDATES** A own hotspot · B USB tethering · C Wi-Fi Aware (NAN) · D Wi-Fi Direct
**OFFICIAL EVIDENCE** [T1] Wi-Fi Aware overview (Android side; NDP simplified on 12+) — https://developer.android.com/develop/connectivity/wifi/wifi-aware
**REAL-WORLD EVIDENCE** [T4] home Wi-Fi 3–7 ms (prima.cpp, TPI-LLM); busy office p99 ~250 ms (Sui et al., MobiSys 2016) — cited in spec v2.
**UNKNOWN** RTT p50/p95 and jitter for A vs B on our hardware → T004.
**DECISION** A + B (measure both; B default when phone is on the desk) · **CONFIDENCE** Medium
**WHY NOT C/D?** no practical Linux-laptop NAN/P2P stack; keep for phone↔phone in Phase 3.
**CONSEQUENCE** D008.
