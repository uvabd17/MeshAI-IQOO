# MeshAI v3 — Native Mesh: Audit and Forward Plan

**Date:** 23 September 2026 · **Branch:** `native-mesh` · **Supersedes:** Technical Spec v2 / Pitch Deck v3 on architecture only (positioning and claims discipline in v2 still stand).

This document records what was verified against upstream state on 23 Sep 2026, what in the v2 plan is wrong or stale, and the architecture for the three deliverables the team now wants: a native Android worker (universal Android, not iQOO-only), a native Linux/Windows desktop node, and an admin web console. Every claim marked **[verified]** was checked today at the linked source; **[assumed]** is our own reasoning and must be measured in Phase 0.

---

## 0. Decisions in one screen

| Decision | Choice | Why |
|---|---|---|
| Inference engine | **llama.cpp / ggml, upstream master** (not prima.cpp, not GenieX as the base) | Only engine with native Android build, RPC, OpenCL-Adreno, Hexagon, MXFP4 (gpt-oss) and Qwen3-MoE in one tree. prima.cpp is a stale fork on ZMQ/Termux; GenieX is Snapdragon-only. |
| Data plane (tensors between devices) | **ggml RPC v7, embedded in-process** via `ggml_backend_rpc_start_server(endpoint, cache_dir, n_threads, n_devices, devices[])` | Already exists, we pass our own `devices[]` so the OpenCL-fallback bug (#11957) does not apply. We do **not** write our own tensor transport in v1. |
| Control plane (join, profile, plan, health, jobs) | **Ours. Protobuf messages, length-prefixed over one TCP stream, Noise-encrypted after QR pairing** | This is where "native code talks to native code seamlessly" lives. Separate from the data plane so it can be secured and evolved independently. |
| Shared native core | **Rust crate `meshcore`** → Linux/Windows binary `meshd`, Android `.so` via `cargo-ndk` + UniFFI Kotlin bindings | One implementation of profiling model, planner, scheduler, control protocol, pairing. Memory-safe networking. Team keeps Kotlin for the phone UI. |
| Android shell | **Kotlin + Jetpack Compose, minSdk 30, arm64-v8a only** | Thermal headroom API is 30+. 32-bit and x86 Android are out of scope for a compute worker. |
| Desktop app | **`meshd` daemon + admin web served on `localhost`; optional Tauri window** | One binary for Linux and Windows. The "native desktop app" and the "admin web" are the same UI; Tauri gives it a native window when wanted. |
| Admin web | **SPA served by `meshd`** (`/admin`), same API the CLI uses | Model catalog, per-device policy, plan explanation, analytics. No cloud. |
| Phone compute path | **CPU first → OpenCL (Adreno) for prefill → GenieX/Hexagon as Tier-S bonus** | See §2.4. |
| Link | **Own hotspot or USB tethering** | Wi-Fi Aware / Wi-Fi Direct have no practical Linux-laptop side. |

---

## 1. What the audit found (upstream state on 23 Sep 2026)

| v2 plan assumed | What is true now | Consequence |
|---|---|---|
| llama.cpp RPC is "proof of concept, insecure" | **[verified]** Still says *"Never run the RPC server on an open network"*. Protocol v7.0.0, max 16 servers, `-c` tensor cache, `--device` selection, RDMA on Linux/mac with TCP fallback. No auth, no encryption. [tools/rpc/README] | Our control plane must own pairing + encryption; RPC port only on the private link, only after pairing. |
| Issue #11957: Android RPC worker falls back to CPU instead of OpenCL | **[verified]** Closed *not planned*, unconfirmed. But the CLI bug is about `rpc-server` auto-picking a backend. The library call takes an explicit `devices[]` array. | Embedding the server in-app and passing the OpenCL device ourselves sidesteps it. Must still be measured (Phase 0, test 3). |
| Issue #22850: RPC loses 28–55% even on wired 2.5 GbE | **[verified]** Open/unconfirmed. Causes named: tensor metadata re-serialised every graph, no hash check under 10 MB, strictly synchronous request/response. | This is protocol overhead, not bandwidth. It is the one place a future MeshAI-owned data plane (or an upstream PR) can win. Not v1 scope. |
| Issue #25876: Hexagon garbled output on SM8850 (iQOO 15 chip) | **[verified]** Closed as stale. It was the **whisper** HMX path, not LLM decode. `--no-hexagon` works around. Hexagon backend README: *experimental*, ~3.5 GB virtual address space per NPU session, auto-remaps larger weights, Q4_0/Q8_0, examples up to gpt-oss-20b. Built via Snapdragon toolchain Docker containers. | NPU is not a v1 dependency (unchanged). But there is now a cleaner NPU path — see GenieX below. |
| OpenCL backend supports Adreno 840 | **[verified]** Adreno 750/810/830/**840**/X1-85/X2-90 listed. Quants: Q4_0, Q4_K, Q5_K, Q6_K, Q8_0, **MXFP4**, IQ4_NL. Needs NDK 26.3+, CMake 3.29+, Ninja. Known: flash-attn not always faster; old A6xx drivers fail. Community numbers on 8 Elite: prefill 3.7–24× faster than CPU, **decode ~17% slower than CPU on 7B** because of per-token CPU↔GPU sync. | GPU is a **prefill** win, not a decode win. Scheduler should treat OpenCL as "use for prompt processing" and measure decode separately. MXFP4 support means gpt-oss-20b layers can sit on the phone GPU. |
| Vulkan as GPU alternative | Reports from 8 Elite era: *hangs on load*. No 2026 confirmation it is fixed on Adreno 840. | Do not plan on Vulkan for the phone. Vulkan is fine for the **laptop's** Intel iGPU. |
| NPU access via NNAPI | **[verified] NNAPI is deprecated from Android 15.** Android 17 requires `FEATURE_NEURAL_PROCESSING_UNIT` in the manifest for direct NPU access. Vendors moved to their own runtimes: Qualcomm QNN/AI Engine Direct, MediaTek NeuroPilot, Samsung ENN. | There is no universal NPU path. Universal Android = CPU + (Adreno OpenCL where present). NPU is per-vendor. |
| — (not in v2) | **[verified] Qualcomm GenieX** — BSD-3, open source, v0.3.14 (Jul 2026). Runs any GGUF on Snapdragon **NPU/GPU/CPU** by dispatching to llama.cpp kernels (CPU/GPU/Hexagon HTP) or QAIRT (NPU-only bundles). Android delivery is a Gradle artifact `com.qualcomm.qti:geniex-android`. Ships an OpenAI-compatible server at `127.0.0.1:18181/v1`. Supports 8 Elite and **8 Elite Gen 5**. No multi-device features. | This is the sanctioned Hexagon path for the iQOO 15 and the Tier-S "phone runs the whole model alone on NPU" mode. It does **not** do splitting, so it is a *backend inside our worker*, not a replacement for MeshAI. |
| prima.cpp as a possible base | **[verified]** ICLR 2026. MIT. **ZeroMQ** transport, **Termux-only** on Android, **CUDA-only** GPU, no Windows, models: Llama/Qwen2.5/QwQ/DeepSeek (no gpt-oss, no Qwen3-MoE, no MXFP4), untracked llama.cpp fork, 9 commits. Scheduler inputs: compute, disk speed, memory, OS. Devices too slow get *"No layer is assigned to me, exit"*. | Not a base. It is the **research baseline**: its numbers (8B/14B no gain; 30B 72 vs 202 ms/tok; 70B 674 ms/tok) and its "skip the weak device" rule are exactly what our scheduler must match or explain. |
| exo, distributed-llama | exo: Mac GPU, Linux CPU-only, no Android. distributed-llama: tensor-parallel, Linux/mac/Win, no Android. | No direct competitor ships an Android phone + laptop product. The gap in v2 is still real. |
| Foreground service keeps the worker alive | **[verified]** `dataSync` and `mediaProcessing` FGS types are capped at **6 h per 24 h** (Android 15+), then `onTimeout()`; Android 16 also applies job quotas to jobs started from FGS. `specialUse` needs a declared justification. | Use `FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE` (not time-capped; we hold `CHANGE_WIFI_STATE` which satisfies its prerequisite) with `specialUse` as fallback. Keeping the dashboard Activity visible resets everything anyway. |
| "Direct access to cores" | **[verified]** Android puts the visible app in the `top-app` cpuset (all cores), other foreground in `foreground`, and background in `background` (little cores only, "pack" policy). `sched_setaffinity` works from the NDK (`syscall(__NR_sched_setaffinity, gettid(), …)`) **within** the current cpuset. | You get the big cores by being the top app, then pin ggml worker threads to them. You cannot escape the cpuset from an unprivileged app. Design §3.1. |
| Play requirement for native code | **[verified]** Since 1 Nov 2025 native apps targeting Android 15+ must be 16 KB page-aligned. NDK r28+ aligns by default. | We have NDK 28.2. Any prebuilt `.so` (GenieX, OpenCL loader) must also be 16 KB-aligned — check before shipping. |
| iQOO 15 is the 16 GB phone | **[verified]** India variants: **12 GB/256 GB (₹76,999)** and 16 GB/512 GB (₹83,999). SD 8 Elite Gen 5, LPDDR5X, 7,000 mAh. Launched 26 Nov 2025. | The v2 memory table only closes with the 16 GB unit. With a 12 GB loaner, gpt-oss-20b needs the second phone. Plan for it. |
| "8 GB budget laptop" | **This dev laptop is an IdeaPad 3, i5-1035G1 (4C/8T, AVX-512), 18 GB RAM, Intel Iris Plus iGPU.** | gpt-oss-20b (12.1 GB) **fits on this laptop alone**. The "neither device can hold it" demo is false on this hardware unless we (a) headline Qwen3-Coder-30B-A3B (18.6 GB) which genuinely needs the phone, or (b) cap the laptop process with a cgroup (`systemd-run --user -p MemoryMax=6G --scope …`) and say so on the slide. Do (a) for the headline and (b) for the "budget laptop" story. |
| Wi-Fi Aware / Wi-Fi Direct later | Android side is fine (API 26+, NDP simplified on 12+). Linux laptop side needs driver + `wpa_supplicant` NAN support that most laptop Wi-Fi does not have. | Stays "later". Own hotspot + USB tethering remain the product answer, and USB tethering is the one to measure for lowest jitter. |

**Net result:** the v2 thesis survives. Three things change: (1) the Hexagon story is now GenieX, (2) the memory-demo must be re-based on the real laptop, (3) the control plane is where the native engineering goes — the tensor transport is borrowed.

---

## 2. Target architecture

### 2.1 Two planes, three deliverables

```
                 ┌──────────────────────────── control plane (ours, encrypted) ───────────────────────────┐
                 │                                                                                         │
   ┌─────────────┴───────────────┐                                                   ┌─────────────────────┴─────────────┐
   │  DESKTOP NODE  (Linux/Win)  │                                                   │  ANDROID WORKER  (universal arm64) │
   │  meshd  (Rust, one binary)  │                                                   │  Kotlin/Compose shell              │
   │  ├ meshcore: pairing, planner│      ggml RPC v7  (data plane, borrowed)          │  ├ meshcore.so (UniFFI)            │
   │  │  scheduler, control proto │◄──── hidden state ~4 KB/token/boundary ──────────►│  ├ rpc worker thread:              │
   │  ├ llama-server --rpc a,b   │                                                   │  │   ggml_backend_rpc_start_server │
   │  │   /v1/chat/completions   │                                                   │  │   devices[] = {CPU | OpenCL}    │
   │  ├ admin web  /admin        │                                                   │  ├ profiler: RAM, thermal headroom,│
   │  ├ job queue                │                                                   │  │   battery, RTT, bench-on-join   │
   │  └ QR pairing               │                                                   │  ├ FGS connectedDevice + wakelock  │
   └─────────────┬───────────────┘                                                   │  └ dashboard, QR scanner          │
                 │                                                                   └───────────────────────────────────┘
      coding tool / CLI / IDE  ──HTTP──►  localhost:8080/v1      (never knows how many devices are behind it)
      browser  ──────────────────HTTP──►  localhost:8080/admin   (model catalog, policies, plan explainer, analytics)
```

- **Data plane** = ggml RPC. `llama-server` on the desktop is the client; each phone runs an RPC server *inside the app process*. We never touch tensors.
- **Control plane** = `meshcore`. One protobuf schema (`proto/mesh.proto`), one Rust implementation compiled for every node. Carries: pairing, device profile, live telemetry, the plan, job submission and progress, health/heartbeat, and the *reason* for every scheduling decision (the admin web shows it).
- **Deliverable 1 (Android)**: shell in Kotlin, brains in `meshcore.so`, muscle in `libllama.so` (+ `libggml-opencl.so`, optional GenieX AAR).
- **Deliverable 2 (Desktop)**: `meshd` static binary for Linux (primary) and Windows (same code, `llama.cpp` built with MSVC/clang-cl). Optional Tauri wrapper for a windowed app.
- **Deliverable 3 (Admin web)**: SPA in `admin/`, embedded into `meshd` at build time, served on localhost (and optionally on the mesh link so a phone browser can open it).

### 2.2 Why Rust for `meshcore` and not C++ or Kotlin Multiplatform

| Option | For | Against |
|---|---|---|
| **Rust `meshcore`** (recommended) | One codebase for Linux/Windows/Android; memory-safe network + parsing code (this is the attack surface); `tokio` + `snow` (Noise) + `prost` (protobuf) + `gguf` crates exist; `cargo-ndk` + UniFFI give Kotlin bindings with no hand-written JNI; static single-binary desktop. | Team has no Rust yet. Learning cost lands in the first two weeks. |
| C++ everywhere | Same language as ggml; no FFI at all. | Networking, pairing and crypto in C++ under time pressure is where bugs live; cross-platform build glue is heavier; no memory safety. |
| Kotlin Multiplatform + Compose Desktop | Team knows Kotlin; one UI toolkit for phone and desktop. | Desktop becomes a JVM app, which is not what "native desktop" means; JNI to llama.cpp on desktop is avoidable pain; no static binary. Fine for UI, wrong for the core. |
| Python/FastAPI coordinator (v2 option A) | Fastest first demo. | Not native, not shippable as a single binary, and every line has to be rewritten for the product. Use only if Phase 0 proves the whole idea dead. |

Rule of thumb that falls out: **Kotlin owns pixels, Rust owns decisions, C/C++ owns tensors.**

### 2.3 Universal Android: capability floor and tiers

| Tier | Hardware | Roles allowed | Compute path |
|---|---|---|---|
| **S** | Snapdragon 8 Elite / 8 Elite Gen 5 (iQOO 15, etc.), 12–16 GB | layer-split holder, whole-model-alone, parallel job | CPU (dotprod+i8mm), OpenCL Adreno 830/840 for prefill, **GenieX Hexagon** for whole-model-alone |
| **A** | SD 8 Gen 3 / 8s Gen 3 / Dimensity 9300+ / Tensor G4+, ≥12 GB | layer-split holder, parallel job | CPU; OpenCL on Adreno 7xx; Mali/Immortalis via CPU only in v1 |
| **B** | any arm64 with ARMv8.2 `dotprod`, ≥8 GB, Android 11+ | parallel job with a model that fits; small share of a split if RTT is good | CPU only |
| **Unsupported** | <8 GB, no `dotprod`, 32-bit, Android ≤10 | — | app installs, shows "this phone can watch but not work" |

Detection at first launch: `Build.SUPPORTED_ABIS`, `/proc/cpuinfo` features (`asimddp`, `i8mm`, `sve`), `ActivityManager.getMemoryInfo().totalMem`, presence of `libOpenCL.so` + `clGetPlatformIDs` success, `Build.SOC_MODEL` (API 31) for the GenieX allow-list, and a 10-second `llama-bench`-equivalent on a 0.5 B model. The result is the device's **profile card** in the control plane and in the admin web.

### 2.4 Phone compute path, in order

1. **CPU** — `n_threads` = number of big+mid cores (SD 8 Elite Gen 5: 2 prime + 6 performance → start at 6, measure 8). Build flags `-march=armv8.2-a+dotprod+i8mm`, `GGML_OPENMP=OFF` (pthreads), Q4_0 / Q4_K_M / MXFP4.
2. **OpenCL (Adreno)** — expose as a second RPC device for **prefill-heavy** work. Measure decode; expect parity-or-worse vs CPU on 8 Elite class. Scheduler chooses per-phase only if llama.cpp lets us (v1: choose per-model, not per-phase).
3. **GenieX / Hexagon** — Tier S only, whole-model-alone mode only (GenieX has no multi-device story). The phone then *is* the model server and the desktop proxies `/v1` to it. This is the "phone is the stronger device" demo.

### 2.5 Link

Own hotspot (phone or laptop) or USB tethering. Measure both in Phase 0. USB tethering is the low-jitter path and should be the default when the phone is on the desk anyway — it also charges the phone, which the scheduler prefers.

---

## 3. Android worker — native detail

### 3.1 Cores, threads, scheduling

- The app must be **top-app** (dashboard Activity visible, `FLAG_KEEP_SCREEN_ON`) to be in the all-cores cpuset. Read `/proc/self/status → Cpus_allowed_list` at start and after every lifecycle change; show it on the dashboard (judges love it).
- Rank cores by `/sys/devices/system/cpu/cpuN/cpufreq/cpuinfo_max_freq` and `/sys/devices/system/cpu/cpuN/cpu_capacity`. Pin ggml worker threads with `syscall(__NR_sched_setaffinity, gettid(), …)` to the big set; leave one big core unpinned for the network/UI threads. Expose this as a policy knob (`cores: auto | big-only | all`).
- Foreground service: `FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE` (permission prerequisite satisfied by `CHANGE_WIFI_STATE`, which we need for the Wi-Fi lock). Fallback `SPECIAL_USE` with the manifest property justification. Never `dataSync` (6 h cap).
- `PARTIAL_WAKE_LOCK` while a plan is active; `WifiManager.WIFI_MODE_FULL_LOW_LATENCY` lock (effective only in foreground — another reason the dashboard stays up); `MulticastLock` only if we add mDNS later.

### 3.2 Memory

- `ActivityManager.getMemoryInfo()` → `availMem`, `threshold`, `lowMemory`; `onTrimMemory` levels → report `TRIM_MEMORY_RUNNING_LOW` and above to the scheduler as a hard "shrink me" signal.
- Model files: app-specific external dir (`getExternalFilesDir`), or a user-picked file via SAF → keep the fd and hand llama.cpp `/proc/self/fd/<n>` as the path (mmap works through it). This is how an Office-Kit-transferred file in `Downloads` gets used without `MANAGE_EXTERNAL_STORAGE`.
- Headroom rule from the team notes (1–2 GB) is the starting point; the profiler measures the real kill threshold on each tier in Phase 0 test 6.
- `mlock` is not available at these sizes on Android (`RLIMIT_MEMLOCK`); rely on mmap + headroom + `-c` tensor cache in the RPC server.

### 3.3 Thermal and battery

- `PowerManager.getThermalHeadroom(forecastSeconds)` (API 30) — 0.0…1.0, where 1.0 = severe throttling. Scheduler policy: headroom > 0.85 → shed 25 % of layers; > 0.95 → pause. `addThermalStatusListener` for the coarse status shown on the dashboard.
- `BatteryManager`: `isCharging()`, `BATTERY_PROPERTY_CAPACITY`, `BATTERY_PROPERTY_CURRENT_NOW` (live drain in mA for the analytics page). Policy: prefer charging phones; user-set floor (default 30 %); refuse new plans below floor, finish current job then leave.
- All of this is **reported, not decided** on the phone. Decisions are in `meshcore` on the desktop so the admin web can show *why*.

### 3.4 Pairing and security

- Desktop shows a QR: `{mesh_id, host, control_port, noise_static_pubkey, one_time_token}`. Phone scans, opens the control TCP stream, performs a Noise `XX` handshake (`snow` crate on both sides via `meshcore`), proves the one-time token, and is then in the allow-list.
- The RPC port on the phone is opened **only after** pairing, bound to the interface of the paired link, and closed when the plan ends. This is the mitigation for "RPC is insecure on open networks". (RPC traffic itself stays plaintext in v1; encrypting it means owning the data plane — Phase 3.)

---

## 4. Desktop node (`meshd`)

- Single Rust binary. Subcommands: `meshd serve` (daemon: control plane, admin web, `/v1` proxy), `meshd pair` (print QR in terminal), `meshd plan <model.gguf>` (dry-run the scheduler and print the placement with reasons), `meshd bench`.
- Owns the llama.cpp process: spawns `llama-server --rpc <phone1>,<phone2> --tensor-split … --model … --port 8081`, proxies `localhost:8080/v1` → it, and restarts it on replan. Reads GGUF metadata itself (`gguf` crate) for the planner: bytes per layer, KV bytes per token per layer, so the plan can be shown *before* the model is loaded.
- Laptop backend: CPU (AVX-512 on this i5) and Vulkan on the Intel Iris Plus — measure; Vulkan on Intel iGPU is often a wash for decode.
- Linux first (this machine, Ubuntu 25.10, kernel 6.17). Windows: same crate, llama.cpp built with the MSVC preset; show it running, do not tune it.
- Memory honesty on the demo laptop: run the headline on Qwen3-Coder-30B-A3B (18.6 GB, does not fit 18 GB) and run the "budget laptop" story under `systemd-run --user -p MemoryMax=6G --scope`.

## 5. Admin web (`admin/`)

Served by `meshd` at `/admin`; pure SPA talking to `meshd`'s JSON API (the same one the CLI uses). Pages:

1. **Devices** — every paired node's profile card, tier, live telemetry (RAM, headroom, battery, RTT, tok/s), cpuset, and per-device policy (allowed roles, battery floor, thermal ceiling, core policy, schedule windows).
2. **Models** — catalog of GGUFs on each node (with hashes), per-model settings (context, quant, backend preference), "where would this run?" dry-run that shows the planner's placement and the reasons it rejected any device.
3. **Plans & jobs** — active plan with the layer map, job queue (write tests / review branch / document module / read logs), progress, results.
4. **Analytics** — per-job tok/s, TTFT, energy (mAh from `CURRENT_NOW` integrated), thermal curve, RTT histogram; the benchmark table the pitch needs, generated from real runs.

No accounts, no cloud. Auth = it is only reachable on localhost and the paired link.

## 6. Control plane protocol (v0)

`proto/mesh.proto` in this branch. Messages: `Hello`, `DeviceProfile`, `Telemetry`, `Plan` (+ `Placement` with a `reason` per device, including rejections), `JobSubmit`/`JobProgress`/`JobResult`, `Heartbeat`, `Bye`. Framing: 4-byte length prefix, Noise-encrypted payload, one long-lived TCP stream per device, desktop is the server. Versioned from day one.

## 7. Scheduler v1 (unchanged from team notes §04, now with inputs named)

Inputs: per-device `free_bytes − headroom`, bench score per backend, RTT p50/p95 to desktop, thermal headroom, charging + level, failure count; per-model bytes per layer + KV bytes per token × requested context.
Rules, in order: (1) fits on one device → run there, fastest device wins, no network. (2) Else split by free memory across the fewest devices, contiguous blocks, laptop last. (3) Reject a device whose RTT p95 would cost more per token than its memory saves (prima.cpp's rule, made explicit). (4) Many independent jobs → one job per device with a model that fits it. Every decision carries a human-readable `reason` that the dashboard and admin web display.

---

## 8. Phased plan with gates

**Phase 0 — hardware truth (this week, before any app code).** Needs one physical arm64 phone (any Tier A/B phone you own now; the iQOO 15 loaner comes later).
1. `scripts/android-build-llama.sh` — cross-compile `rpc-server`, `llama-server`, `llama-bench` for arm64 with NDK r28 (CPU build; then `--opencl`). *Started today; see §10.*
2. `scripts/phase0-split-test.sh` — push binaries + a 1–3 B GGUF to the phone with `adb`, start `rpc-server` there, run `llama-server --rpc` from the laptop over hotspot, then over USB tethering. Record TTFT, prompt tok/s, decode tok/s, RTT.
3. Repeat with the OpenCL build exposing the GPU device. Record prefill vs decode separately.
4. Phone alone vs laptop alone vs split on the same model.
5. 10-minute sustained run: thermal headroom curve, drain in mA.
6. Find the real kill threshold: grow context until Android kills the process; that sets the headroom constant per tier.
**Gate:** split works, is correct, and decode ≥ 3 tok/s on a model that does not fit one device → proceed. If not, the product is single-device-per-job + parallel jobs + GenieX on Tier S, and the control plane is still the product.

**Phase 1 — hackathon MVP.**
Android: Kotlin app, `meshcore.so`, embedded RPC worker with explicit `devices[]`, profiler, QR pairing, FGS `connectedDevice`, dashboard. Desktop: `meshd serve/pair/plan`, planner from GGUF metadata, `llama-server` supervisor, `/v1` proxy, admin web pages 1–3. Headline demo: Qwen3-Coder-30B-A3B across laptop + iQOO 15 (16 GB) or laptop + 2 phones (12 GB); "budget laptop" story with gpt-oss-20b under a 6 GB cgroup; internet off; a coding CLI pointed at `localhost:8080/v1`.

**Phase 2 — universal Android product.** Device tiers and the capability gate; GenieX Tier-S whole-model mode; parallel jobs; Windows `meshd`; admin analytics; signed model manifests; Play-store-grade FGS and 16 KB compliance check on every `.so`.

**Phase 3 — own the data plane where it pays.** Address #22850 (metadata caching, async pipelining) either as an upstream PR to ggml-rpc or as a MeshAI transport behind the same `ggml_backend` interface; encrypted data plane; MoE expert placement; Wi-Fi Aware on Android-to-Android meshes.

---

## 9. Risk register (delta from v2)

| Risk | New information | Action |
|---|---|---|
| Loaner iQOO 15 is 12 GB | Confirmed such a SKU exists at ₹76,999 | Second phone is core, not stretch. |
| Demo laptop too strong | 18 GB here | Headline on 30B-A3B; cgroup-cap for the 8 GB story; say so. |
| GPU decode slower than CPU | Community data on 8 Elite | Treat OpenCL as prefill accelerator; measure before promising. |
| FGS time cap kills long jobs | 6 h/24 h on `dataSync` | `connectedDevice` type; verify on Android 15/16 in Phase 1. |
| Play 16 KB rule | Nov 2025 | NDK 28 default; audit prebuilt `.so`s. |
| Rust ramp-up | Team is new to it | `meshcore` v0 is small (protocol + planner); pair on it; keep Kotlin for everything visible. |
| Prebuilt emulators are x86_64 | Both attached emulators are `sdk_gphone64_x86_64` | Useless for perf; UI only. Phase 0 needs a physical phone. |

## 10. Verified today on this machine

- Toolchain: NDK 25.1 / 27.0 / **28.2**, platforms android-31…37, CMake 3.31 (host) + 3.22 (SDK), JDK 21, adb 1.0.41, Python 3.13, Node 20, Docker 29. Missing: Rust, Go, Ninja, Gradle CLI (Android Studio wrapper will provide it).
- llama.cpp upstream (commit `66fba63`, 23 Sep 2026, ggml 0.25.0) cross-compiled for `arm64-v8a` / `android-30` / `dotprod+i8mm` / `GGML_RPC=ON` with NDK 28.2 — **success**. Artifacts: `ggml-rpc-server` (1.3 MB), `llama-server`, `llama-bench` + `libggml-{base,cpu,rpc}.so`, `libllama.so`, `libllama-common.so`, `libllama-server-impl.so`, `libmtmd.so`. `file` reports *ELF 64-bit ARM aarch64, for Android 30, built by NDK r28c*; `llvm-readelf -l` shows every LOAD segment aligned to **0x4000** (16 KB rule satisfied). The upstream CMake target is `ggml-rpc-server`, not `rpc-server` — `scripts/android-build-llama.sh` has the working invocation. Unstripped, debug-info binaries are large (`libllama-common.so` ≈ 80 MB); strip before packaging into the APK. Not yet run on a phone: both attached devices are x86_64 emulators.

## 11. Open decisions for the team

1. Which physical Android phone is available **this week** for Phase 0, and its RAM/SoC?
2. Hackathon date and whether the iQOO 15 loaner is 12 GB or 16 GB — this decides the headline model.
3. Rust for `meshcore` (recommended) vs C++ — decide before Phase 1 day 1.
4. Admin web framework: keep it dependency-light (Preact/Svelte, no build server at runtime) so `meshd` stays a single binary.
5. Whether Windows is demoed at the hackathon at all or only listed as "same binary, built".

## 12. Sources checked (23 Sep 2026)

- llama.cpp RPC README — https://github.com/ggml-org/llama.cpp/blob/master/tools/rpc/README.md
- ggml-rpc.h (`ggml_backend_rpc_start_server` signature, protocol v7) — https://github.com/ggml-org/llama.cpp/blob/master/ggml/include/ggml-rpc.h
- OpenCL backend (Adreno 840, MXFP4) — https://github.com/ggml-org/llama.cpp/blob/master/docs/backend/OPENCL.md
- Hexagon backend — https://github.com/ggml-org/llama.cpp/blob/master/docs/backend/snapdragon/README.md
- Android build doc — https://github.com/ggml-org/llama.cpp/blob/master/docs/android.md
- Issues #11957, #22850, #25876 — https://github.com/ggml-org/llama.cpp/issues/11957 · /22850 · /25876
- Adreno OpenCL numbers on 8 Elite — https://github.com/ggml-org/llama.cpp/discussions/23736 · https://www.qualcomm.com/developer/blog/2024/11/introducing-new-opn-cl-gpu-backend-llama-cpp-for-qualcomm-adreno-gpu
- Qualcomm GenieX — https://github.com/qualcomm/GenieX · https://aihub.qualcomm.com/geniex
- NNAPI deprecation / migration — https://developer.android.com/ndk/guides/neuralnetworks/migration-guide
- Foreground service timeouts — https://developer.android.com/develop/background-work/services/fgs/timeout · https://developer.android.com/develop/background-work/services/fgs/changes
- Android cpusets / scheduling — https://source.android.com/docs/core/power/performance · https://lwn.net/Articles/706374/
- 16 KB page sizes — https://developer.android.com/guide/practices/page-sizes
- Wi-Fi Aware — https://developer.android.com/develop/connectivity/wifi/wifi-aware
- prima.cpp (ICLR 2026) — https://github.com/OpenCPIL/prima.cpp · https://arxiv.org/abs/2504.08791
- exo — https://github.com/exo-explore/exo · distributed-llama — https://github.com/b4rtaz/distributed-llama
- iQOO 15 India variants — https://www.91mobiles.com/iqoo-15-price-in-india · https://shop.iqoo.com/in/product/2067
