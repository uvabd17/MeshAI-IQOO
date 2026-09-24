# MeshAI — the one document

*Single source of truth for design, decisions, risks, measurements, status and how to run. Everything below was either executed on this laptop and the POCO F5 on 24 Sep 2026 or is marked as estimate / not executed. Generated from the repo state at 2026-09-24 15:42 IST; the machine-readable ledgers stay in `state/` (tasks, tests, progress, blockers).*

Branch: `native-mesh-v2` on GitHub (uvabd17/MeshAI-IQOO). Older PDFs/decks are in `docs/archive/` and are superseded by this file.

## 1. What MeshAI is

Your laptop and your phone pool their memory over a private link (USB cable, USB tethering or a hotspot) and serve **one local, OpenAI-compatible AI endpoint** — no cloud, no account. A model that fits one device runs there alone ("don't split if it fits"); a model that fits neither is cut into layers and the phone computes its share for the laptop.

Three pieces: `meshd` (Rust service on the laptop: planner, llama.cpp supervisor, control plane for phones, admin web panel, `/v1` API), the Android app (Kotlin; pairs, reports specs, runs llama.cpp as helper or host), and the admin panel (three plain pages: Devices, Run, Chat).

## 2. Status in one screen (24 Sep 2026)

| Area | State | Evidence |
|---|---|---|
| Laptop service `meshd` | built, reviewed 11 rounds, 36 Rust tests, clippy clean, Windows type-check clean | `scripts/verify --quick` = 26 ok; `scripts/qa/run-guards.sh` = 32 checks ALL OK |
| Android app | built, 19 JVM tests, lint clean; **executed on the POCO F5** (pair, host mode, helper mode, reconnect, dynamic ports) | logcat + admin screenshots, BENCHMARKS |
| Admin panel | three pages + top status bar with per-device checklist, download/loading indicators, model cards, chat with image/file attachments | headless-browser runs, screenshots |
| Phone as host | **works**: 28.8 tok/s on Qwen3-0.6B, model pushed from the laptop in ~130 s | measured |
| Laptop+phone layer split | **works and is correct**; 1.2–1.4 tok/s over the USB-debugging cable (adb relays every RPC round trip) | measured; gate (≥3 tok/s) needs USB tethering/hotspot, not yet measured |
| Image chat | **works** on the laptop (Qwen2.5-VL-3B + projector described a real screenshot) | measured, 150 s per image on this CPU |
| Windows | code is cross-platform, scripts written, **not executed on Windows** | `cargo check --target x86_64-pc-windows-gnu` passed before the TLS fix; since `rustls-tls` (ring) it needs `mingw-w64` on the Linux box, which is not installed, so the check is skipped (verify says so) |
| Oracle mirror | scripts ready, **not deployed** (needs approval) | deploy/oracle |

Tasks: 12 done, 5 active, 6 pending (section 9).

## 3. How to run it

Linux laptop:
```bash
cargo build --manifest-path desktop/Cargo.toml          # Rust stable; protobuf is compiled by prost at build time
MESHAI_API_TOKEN=<any secret> desktop/target/debug/meshd serve --lan   # models in /mnt/storage/meshai/models by default (--models to change)
# open http://localhost:8080/admin/?token=<secret>
```
Phone: install `android/app/build/outputs/apk/debug/app-debug.apk` (`gradle -p android assembleDebug`). Plug it in with USB debugging on, press **Pair over USB** on the Devices page, tap **Join** on the phone. From then on the app reconnects by itself. Or show the QR and scan it in the app (works on hotspot / Wi-Fi / tethering; the QR carries every laptop address).

Then Run page → pick a model → Run; Chat page → talk, attach an image (vision model) or a text/CSV/JSON/code file.

Windows: section 13. Checks: `scripts/verify` (Rust + Android), `scripts/qa/run-guards.sh` (live behaviour against a running meshd).

## 4. What was measured (headline numbers)

| Scenario | Result |
|---|---|
| Phone hosts Qwen3-0.6B (Wi-Fi, app-driven) | 28.8 tok/s decode, 251 ms first token, ready in 131 s incl. 640 MB copy |
| Laptop alone, Qwen3-8B Q4 | asked "ask me one question": answer in 4.8 s |
| Laptop host + phone helper (layers 0–12 / 13–27), USB via adb | ready in 12–20 s; correct multi-turn interview; 0.65–1.4 tok/s |
| Laptop + 2 simulated helpers, Qwen3-0.6B | 22–24 tok/s client, ~200 ms first token |
| Image chat, Qwen2.5-VL-3B on the laptop | correct description of a 1280×1200 screenshot; 150 s end to end |
| Wi-Fi (shared home router) | RTT p95 65–154 ms → planner refuses the phone as helper (policy 60 ms) |
| USB via adb | RTT p50 2–5 ms, p95 4–10 ms |

Full rows with conditions: section 12.

## 5. Biggest models on a good day (estimates, not measured)

Laptop 20 GB (≈11 GB usable idle, 2 GB headroom); phone 7.4 GB (≈1–2.5 GB usable, 1.5 GB headroom).
- Fits the laptop alone: Qwen3-14B Q4 (~9 GB), gpt-oss-20b MXFP4 (12.1 GB, tight). These stay on the laptop by design.
- Needs the split: Qwen3-30B-A3B Q4 (18.6 GB) with a quiet phone and small context; Q3 (~14.7 GB) safer. Speed hinges on the link (adb: ~1 tok/s; tethering/hotspot: unmeasured).
- Images: yes with Qwen2.5-VL / Gemma-3 vision GGUFs + projector (llama.cpp mtmd). Image *generation* is a different engine, not supported.
- Agentic / tool calls: pass through `/v1` (llama-server chat templates); Qwen3-Coder-30B-A3B, gpt-oss-20b, Qwen3-14B fit best.

## 6. What is not done / needs the user

- Split speed over a native link (USB tethering enabled from the phone's Settings, or a hotspot) — decides the go/no-go gate T007.
- Phone hosting a vision model (the app does not pass `--mmproj` yet).
- Windows: never executed on a Windows machine. The Linux cross type-check now needs `apt install mingw-w64` (ring, pulled in by the TLS fix); until then `scripts/verify` skips it and says why.
- Oracle mirror go-live (approval), hackathon date and iQOO 15 memory size, cloud API key for the comparison bar.
- Keep the phone unlocked with the app open during runs (a lock screen dropped the link once); avoid switching USB mode from adb (it kills adb).

## 7. Tests and checks (state/tests.json)

| Id | Check | Status | Evidence |
|---|---|---|---|
| V001 | verify --quick | PASS | 24 ok, 0 failed (04:54 IST, round-11 batch) |
| V002 | arm64 build | PASS | ELF aarch64, Android 30, NDK r28c, LOAD Align 0x4000 |
| V003 | phase0 split | BLOCKED | no physical arm64 phone attached |
| V004 | cargo test (meshcore + meshd) | PASS | 38 passed (15 meshcore incl. D027 host-capability tests + 23 meshd) 11:12 IST; clippy clean; cargo check x86_64-pc-windows-gnu clean (in verify) |
| V005 | local split simulation | PASS | PASS 04:52 IST (round-11 build, quiet machine) via scripts/qa/run-guards.sh: ready, 3 chats up to 22.1 tok/s client, stop ok |
| V006 | browser-driven admin QA | PASS | PASS 04:53 IST (round-11 build): all views; chat 201 ms TTFT / 22.7 tok/s / 34 tok; 0 console errors, 0 failed requests |
| V007 | gradle lint + assembleDebug | PASS | PASS gradle-final.log 11:16 IST (redesigned two-tab app): lint + testDebugUnitTest (19) + assembleDebug; SoC fields guarded for API 31 |
| V008 | request guards | PASS | 415 / 200 / 403 / 206 bytes N-M/len / 'set --api-token' / 401 then 200 |
| V009 | stop responsiveness | PASS | PASS 04:52 IST (round-11 build): stop at 0.05/0.3/1.0/2.5 s into bring-up, 114–122 ms, 0 orphans |
| V010 | control-plane sessions (tokio, fake phone) | PASS | 11 control tests pass (26.3 s wall), test-r7.log 04:01 IST: pre-auth pool test now also proves a second source address (127.0.0.2) gets in while 127.0.0.1 is saturated; re-send/stop test is a sequential regression guard |
| V011 | Android JVM unit tests | PASS | tests=19 failures=0 (HostArgsTest 4, ProcessGateTest 8, StopReportTest 3, ProcStatusTest 4 incl. pid from Process.toString), gradle-r11.log 04:52 IST |
| V012 | run/forget guards on a live run | PASS | PASS 04:53 IST (round-11 build, run-guards.sh ALL OK, 32 checks): refusals (--lan, n_ctx, unknown model, 8B shortfall) 422 with the run still ready and the plan_id read BEFORE the request unchanged and non-null; replacement path with the laptop MEASURED: immed |

## 8. Tasks done (state/tasks.json)

- **T001** Cross-compile llama.cpp (ggml-rpc-server, llama-server, llama-bench) for arm64 with NDK r28 — commit 897dd3c; ELF aarch64 Android 30, LOAD Align 0x4000; scripts/android-build-llama.sh
- **T002** Identify a physical arm64 phone for Phase 0 (model, SoC, RAM, Android version) — POCO F5 (SM7475, 8 GB, Android 15, dotprod+i8mm, OpenCL present) seen on adb 2026-09-24 00:40; intermittently disconnected since
- **T008** Foundations: rustup, host llama.cpp build (RPC), Gradle 8.14, model storage on /mnt/storage, Qwen3-0.6B + Qwen3-8B downloaded — rustup stable; llama.cpp build-host (RPC) + build-android-{arm64-v8a,x86_64}; Gradle 8.14.3; /mnt/storage/meshai/models; Qwen3-0.6B + Qwen3-8B downloading
- **T009** Local split simulation: laptop llama-server + 2 local ggml-rpc-server workers on Qwen3-0.6B/8B; record t/s (stand-in until a phone is attached) — 2026-09-24: laptop host + 2 local RPC workers, Qwen3-0.6B: llama-bench pp 57-71 / tg 9.6-12.4 t/s; via meshd+/v1 chat: TTFT 2.9 s, 4.8 tok/s (loaded machine). docs/BENCHMARKS.md
- **T011** meshd: `pair` (QR in terminal), `serve` (control plane listener), `plan <gguf>` (GGUF metadata → placement with reasons) — meshd serve/pair/plan/worker; planner API; QR offer
- **T020** Android app skeleton: Compose dashboard, connectedDevice FGS, wake+wifi locks, profiler (RAM/thermal/battery/cpuset) — EXECUTED on the POCO F5: Compose dashboard (neobrutalist, two tabs), connectedDevice FGS, locks, profiler (RAM/thermal/battery/cpuset/core loads) — screenshots in the session scratchpad
- **T021** Android: embed ggml RPC server in-process with explicit devices[]; start/stop tied to plan lifecycle — EXECUTED on the POCO F5 11:26 IST: plan pushed → app spawned ggml-rpc-server bound to the link address (127.0.0.1 via adb reverse) → worker-listening report → laptop llama-server connected → correct answers; kill on link
- **T022** Android: QR/secret pairing + control client (framed protobuf-lite; UniFFI dropped for v1, D018); Telemetry stream to meshd — EXECUTED on the POCO F5: QR/intent pairing with confirmation card, per-device secret stored, telemetry every 2 s (avail, thermal, battery, RTT, core loads, held_bytes) visible in the admin; reconnect/forget/re-pair exerc
- **T023** Android: host mode (llama-server child process) and worker mode (ggml-rpc-server); role set by plan; x86_64 ABI for emulator testing — EXECUTED on the POCO F5: host mode (model fetched from meshd, llama-server on :8081, 28.8 tok/s, 10:39 IST) and worker mode (11:26 IST) both ran
- **T030** Admin web pages Devices / Models / Plans&Jobs served by meshd at /admin — admin at /admin; localhost-only by default; token-aware; offer never in state
- **T031** Admin web: monochrome design system (dark/light), provider marks, model catalog with HF download + progress, plan preview with layer map, chat with timing (TTFT, tok/s) — browser QA on hardened meshd (19f2e4f): plan preview, Run, ready 2.0 s, layer map, chat 198 ms TTFT / 21.3 tok/s, analytics, QR via POST offer, catalog; 0 console errors
- **T060** Independent review round 2 after the C1–C3/H1–H7 fixes (reviewer agent) — rounds 1–11 acted on; round 11 = PASS (4 LOWs, all fixed: plan_id read before the request, phone pid fallback via Process.toString + one-time warning + D024/K19 caveat, credit compared with /proc directly, D024 premise m

## 9. Tasks active / pending

- **T004** Laptop↔phone layer split over RPC (hotspot, then USB tethering): TTFT, pp/tg t/s, RTT p50/p95 — EXECUTED 11:26 IST over USB via adb port-forward: split correct (laptop 0–12, phone 13–27), ready 18.6 s, decode 0.65–1.36 tok/s, TTFT 1.5–1.8 s, RTT p50 5 ms; Wi-Fi attempt refused (RTT p95 154 ms). Still to do: USB tet
- **T010** meshcore crate skeleton: prost build of proto/mesh.proto, framing, Noise XX pairing handshake, unit tests — meshcore: gguf, planner, framing, pairing (token→per-device secret, D013; Noise deferred), 10 tests — held at ACTIVE until the Phase-0 gate (T007) passes on hardware (review round 2)
- **T012** meshd: llama-server supervisor (--rpc from admitted workers), /v1 proxy on :8080, restart on replan — supervisor rewritten after review: workers-first ordering, plan-id-keyed worker-ready, TCP check, child watcher for the whole run, teardown on failure, serialized run/stop; local split verified 22 tok/s — held at ACTIVE 
- **T013** meshd: host-role selection — any paired device runs llama-server, others rpc-server; /v1 proxied to the host — host-role selection implemented; phone-host path (model fetch, llama-server on phone) untested — needs a real phone
- **T032** Driven test: open admin in headless browser, download model, run chat on laptop-only and on simulated mesh, record timings to BENCHMARKS.md — laptop-only and simulated-mesh chats measured (docs/BENCHMARKS.md); cloud reference needs an API key; real-phone rows pending
- **T003** Host build of llama.cpp with GGML_RPC=ON; phone-alone and laptop-alone llama-bench on a 1–3B GGUF
- **T005** OpenCL build exposing Adreno as RPC device; prefill vs decode vs CPU
- **T006** 10-min sustained run (thermal headroom, drain) and memory kill-threshold per tier
- **T007** GATE: split correct and ≥3 tok/s decode on a model that fits neither device → go/no-go for Phase 1 scope — NOT passed yet: 0.65–1.36 tok/s on the adb-relayed transport (< 3 tok/s). Split correctness proven. Decision pending a measurement over a native link (tethering/hotspot)
- **T040** Headline demo: Qwen3-Coder-30B-A3B across laptop + iQOO 15 (or +2 phones), internet off, coding CLI on localhost:8080/v1
- **T050** Oracle (ap-hyderabad-1, Always Free ARM): admin panel + telemetry relay + public dashboard; go-live only after explicit user approval


---

## 10. Design of record (architecture, 23 Sep 2026 audit; sections superseded by later decisions are marked in section 11)

**Date:** 23 September 2026 · **Branch:** `native-mesh` · **Supersedes:** Technical Spec v2 / Pitch Deck v3 on architecture only (positioning and claims discipline in v2 still stand).

This document records what was verified against upstream state on 23 Sep 2026, what in the v2 plan is wrong or stale, and the architecture for the three deliverables the team now wants: a native Android worker (universal Android, not iQOO-only), a native Linux/Windows desktop node, and an admin web console. Every claim marked **[verified]** was checked today at the linked source; **[assumed]** is our own reasoning and must be measured in Phase 0.

---

#### 0. Decisions in one screen

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

#### 1. What the audit found (upstream state on 23 Sep 2026)

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

#### 2. Target architecture

##### 2.1 Two planes, three deliverables

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

##### 2.2 Why Rust for `meshcore` and not C++ or Kotlin Multiplatform

| Option | For | Against |
|---|---|---|
| **Rust `meshcore`** (recommended) | One codebase for Linux/Windows/Android; memory-safe network + parsing code (this is the attack surface); `tokio` + `snow` (Noise) + `prost` (protobuf) + `gguf` crates exist; `cargo-ndk` + UniFFI give Kotlin bindings with no hand-written JNI; static single-binary desktop. | Team has no Rust yet. Learning cost lands in the first two weeks. |
| C++ everywhere | Same language as ggml; no FFI at all. | Networking, pairing and crypto in C++ under time pressure is where bugs live; cross-platform build glue is heavier; no memory safety. |
| Kotlin Multiplatform + Compose Desktop | Team knows Kotlin; one UI toolkit for phone and desktop. | Desktop becomes a JVM app, which is not what "native desktop" means; JNI to llama.cpp on desktop is avoidable pain; no static binary. Fine for UI, wrong for the core. |
| Python/FastAPI coordinator (v2 option A) | Fastest first demo. | Not native, not shippable as a single binary, and every line has to be rewritten for the product. Use only if Phase 0 proves the whole idea dead. |

Rule of thumb that falls out: **Kotlin owns pixels, Rust owns decisions, C/C++ owns tensors.**

##### 2.3 Universal Android: capability floor and tiers

| Tier | Hardware | Roles allowed | Compute path |
|---|---|---|---|
| **S** | Snapdragon 8 Elite / 8 Elite Gen 5 (iQOO 15, etc.), 12–16 GB | layer-split holder, whole-model-alone, parallel job | CPU (dotprod+i8mm), OpenCL Adreno 830/840 for prefill, **GenieX Hexagon** for whole-model-alone |
| **A** | SD 8 Gen 3 / 8s Gen 3 / Dimensity 9300+ / Tensor G4+, ≥12 GB | layer-split holder, parallel job | CPU; OpenCL on Adreno 7xx; Mali/Immortalis via CPU only in v1 |
| **B** | any arm64 with ARMv8.2 `dotprod`, ≥8 GB, Android 11+ | parallel job with a model that fits; small share of a split if RTT is good | CPU only |
| **Unsupported** | <8 GB, no `dotprod`, 32-bit, Android ≤10 | — | app installs, shows "this phone can watch but not work" |

Detection at first launch: `Build.SUPPORTED_ABIS`, `/proc/cpuinfo` features (`asimddp`, `i8mm`, `sve`), `ActivityManager.getMemoryInfo().totalMem`, presence of `libOpenCL.so` + `clGetPlatformIDs` success, `Build.SOC_MODEL` (API 31) for the GenieX allow-list, and a 10-second `llama-bench`-equivalent on a 0.5 B model. The result is the device's **profile card** in the control plane and in the admin web.

##### 2.4 Phone compute path, in order

1. **CPU** — `n_threads` = number of big+mid cores (SD 8 Elite Gen 5: 2 prime + 6 performance → start at 6, measure 8). Build flags `-march=armv8.2-a+dotprod+i8mm`, `GGML_OPENMP=OFF` (pthreads), Q4_0 / Q4_K_M / MXFP4.
2. **OpenCL (Adreno)** — expose as a second RPC device for **prefill-heavy** work. Measure decode; expect parity-or-worse vs CPU on 8 Elite class. Scheduler chooses per-phase only if llama.cpp lets us (v1: choose per-model, not per-phase).
3. **GenieX / Hexagon** — Tier S only, whole-model-alone mode only (GenieX has no multi-device story). The phone then *is* the model server and the desktop proxies `/v1` to it. This is the "phone is the stronger device" demo.

##### 2.5 Link

Own hotspot (phone or laptop) or USB tethering. Measure both in Phase 0. USB tethering is the low-jitter path and should be the default when the phone is on the desk anyway — it also charges the phone, which the scheduler prefers.

---

#### 3. Android worker — native detail

##### 3.1 Cores, threads, scheduling

- The app must be **top-app** (dashboard Activity visible, `FLAG_KEEP_SCREEN_ON`) to be in the all-cores cpuset. Read `/proc/self/status → Cpus_allowed_list` at start and after every lifecycle change; show it on the dashboard (judges love it).
- Rank cores by `/sys/devices/system/cpu/cpuN/cpufreq/cpuinfo_max_freq` and `/sys/devices/system/cpu/cpuN/cpu_capacity`. Pin ggml worker threads with `syscall(__NR_sched_setaffinity, gettid(), …)` to the big set; leave one big core unpinned for the network/UI threads. Expose this as a policy knob (`cores: auto | big-only | all`).
- Foreground service: `FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE` (permission prerequisite satisfied by `CHANGE_WIFI_STATE`, which we need for the Wi-Fi lock). Fallback `SPECIAL_USE` with the manifest property justification. Never `dataSync` (6 h cap).
- `PARTIAL_WAKE_LOCK` while a plan is active; `WifiManager.WIFI_MODE_FULL_LOW_LATENCY` lock (effective only in foreground — another reason the dashboard stays up); `MulticastLock` only if we add mDNS later.

##### 3.2 Memory

- `ActivityManager.getMemoryInfo()` → `availMem`, `threshold`, `lowMemory`; `onTrimMemory` levels → report `TRIM_MEMORY_RUNNING_LOW` and above to the scheduler as a hard "shrink me" signal.
- Model files: app-specific external dir (`getExternalFilesDir`), or a user-picked file via SAF → keep the fd and hand llama.cpp `/proc/self/fd/<n>` as the path (mmap works through it). This is how an Office-Kit-transferred file in `Downloads` gets used without `MANAGE_EXTERNAL_STORAGE`.
- Headroom rule from the team notes (1–2 GB) is the starting point; the profiler measures the real kill threshold on each tier in Phase 0 test 6.
- `mlock` is not available at these sizes on Android (`RLIMIT_MEMLOCK`); rely on mmap + headroom + `-c` tensor cache in the RPC server.

##### 3.3 Thermal and battery

- `PowerManager.getThermalHeadroom(forecastSeconds)` (API 30) — 0.0…1.0, where 1.0 = severe throttling. Scheduler policy: headroom > 0.85 → shed 25 % of layers; > 0.95 → pause. `addThermalStatusListener` for the coarse status shown on the dashboard.
- `BatteryManager`: `isCharging()`, `BATTERY_PROPERTY_CAPACITY`, `BATTERY_PROPERTY_CURRENT_NOW` (live drain in mA for the analytics page). Policy: prefer charging phones; user-set floor (default 30 %); refuse new plans below floor, finish current job then leave.
- All of this is **reported, not decided** on the phone. Decisions are in `meshcore` on the desktop so the admin web can show *why*.

##### 3.4 Pairing and security

- Desktop shows a QR: `{mesh_id, host, control_port, noise_static_pubkey, one_time_token}`. Phone scans, opens the control TCP stream, performs a Noise `XX` handshake (`snow` crate on both sides via `meshcore`), proves the one-time token, and is then in the allow-list.
- The RPC port on the phone is opened **only after** pairing, bound to the interface of the paired link, and closed when the plan ends. This is the mitigation for "RPC is insecure on open networks". (RPC traffic itself stays plaintext in v1; encrypting it means owning the data plane — Phase 3.)

---

#### 4. Desktop node (`meshd`)

- Single Rust binary. Subcommands: `meshd serve` (daemon: control plane, admin web, `/v1` proxy), `meshd pair` (print QR in terminal), `meshd plan <model.gguf>` (dry-run the scheduler and print the placement with reasons), `meshd bench`.
- Owns the llama.cpp process: spawns `llama-server --rpc <phone1>,<phone2> --tensor-split … --model … --port 8081`, proxies `localhost:8080/v1` → it, and restarts it on replan. Reads GGUF metadata itself (`gguf` crate) for the planner: bytes per layer, KV bytes per token per layer, so the plan can be shown *before* the model is loaded.
- Laptop backend: CPU (AVX-512 on this i5) and Vulkan on the Intel Iris Plus — measure; Vulkan on Intel iGPU is often a wash for decode.
- Linux first (this machine, Ubuntu 25.10, kernel 6.17). Windows: same crate, llama.cpp built with the MSVC preset; show it running, do not tune it.
- Memory honesty on the demo laptop: run the headline on Qwen3-Coder-30B-A3B (18.6 GB, does not fit 18 GB) and run the "budget laptop" story under `systemd-run --user -p MemoryMax=6G --scope`.

#### 5. Admin web (`admin/`)

Served by `meshd` at `/admin`; pure SPA talking to `meshd`'s JSON API (the same one the CLI uses). Pages:

1. **Devices** — every paired node's profile card, tier, live telemetry (RAM, headroom, battery, RTT, tok/s), cpuset, and per-device policy (allowed roles, battery floor, thermal ceiling, core policy, schedule windows).
2. **Models** — catalog of GGUFs on each node (with hashes), per-model settings (context, quant, backend preference), "where would this run?" dry-run that shows the planner's placement and the reasons it rejected any device.
3. **Plans & jobs** — active plan with the layer map, job queue (write tests / review branch / document module / read logs), progress, results.
4. **Analytics** — per-job tok/s, TTFT, energy (mAh from `CURRENT_NOW` integrated), thermal curve, RTT histogram; the benchmark table the pitch needs, generated from real runs.

No accounts, no cloud. Auth = it is only reachable on localhost and the paired link.

#### 6. Control plane protocol (v0)

`proto/mesh.proto` in this branch. Messages: `Hello`, `DeviceProfile`, `Telemetry`, `Plan` (+ `Placement` with a `reason` per device, including rejections), `JobSubmit`/`JobProgress`/`JobResult`, `Heartbeat`, `Bye`. Framing: 4-byte length prefix, Noise-encrypted payload, one long-lived TCP stream per device, desktop is the server. Versioned from day one.

#### 7. Scheduler v1 (unchanged from team notes §04, now with inputs named)

Inputs: per-device `free_bytes − headroom`, bench score per backend, RTT p50/p95 to desktop, thermal headroom, charging + level, failure count; per-model bytes per layer + KV bytes per token × requested context.
Rules, in order: (1) fits on one device → run there, fastest device wins, no network. (2) Else split by free memory across the fewest devices, contiguous blocks, laptop last. (3) Reject a device whose RTT p95 would cost more per token than its memory saves (prima.cpp's rule, made explicit). (4) Many independent jobs → one job per device with a model that fits it. Every decision carries a human-readable `reason` that the dashboard and admin web display.

---

#### 8. Phased plan with gates

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

#### 9. Risk register (delta from v2)

| Risk | New information | Action |
|---|---|---|
| Loaner iQOO 15 is 12 GB | Confirmed such a SKU exists at ₹76,999 | Second phone is core, not stretch. |
| Demo laptop too strong | 18 GB here | Headline on 30B-A3B; cgroup-cap for the 8 GB story; say so. |
| GPU decode slower than CPU | Community data on 8 Elite | Treat OpenCL as prefill accelerator; measure before promising. |
| FGS time cap kills long jobs | 6 h/24 h on `dataSync` | `connectedDevice` type; verify on Android 15/16 in Phase 1. |
| Play 16 KB rule | Nov 2025 | NDK 28 default; audit prebuilt `.so`s. |
| Rust ramp-up | Team is new to it | `meshcore` v0 is small (protocol + planner); pair on it; keep Kotlin for everything visible. |
| Prebuilt emulators are x86_64 | Both attached emulators are `sdk_gphone64_x86_64` | Useless for perf; UI only. Phase 0 needs a physical phone. |

#### 10. Verified today on this machine

- Toolchain: NDK 25.1 / 27.0 / **28.2**, platforms android-31…37, CMake 3.31 (host) + 3.22 (SDK), JDK 21, adb 1.0.41, Python 3.13, Node 20, Docker 29. Missing: Rust, Go, Ninja, Gradle CLI (Android Studio wrapper will provide it).
- llama.cpp upstream (commit `66fba63`, 23 Sep 2026, ggml 0.25.0) cross-compiled for `arm64-v8a` / `android-30` / `dotprod+i8mm` / `GGML_RPC=ON` with NDK 28.2 — **success**. Artifacts: `ggml-rpc-server` (1.3 MB), `llama-server`, `llama-bench` + `libggml-{base,cpu,rpc}.so`, `libllama.so`, `libllama-common.so`, `libllama-server-impl.so`, `libmtmd.so`. `file` reports *ELF 64-bit ARM aarch64, for Android 30, built by NDK r28c*; `llvm-readelf -l` shows every LOAD segment aligned to **0x4000** (16 KB rule satisfied). The upstream CMake target is `ggml-rpc-server`, not `rpc-server` — `scripts/android-build-llama.sh` has the working invocation. Unstripped, debug-info binaries are large (`libllama-common.so` ≈ 80 MB); strip before packaging into the APK. Not yet run on a phone: both attached devices are x86_64 emulators.

#### 11. Open decisions for the team

1. Which physical Android phone is available **this week** for Phase 0, and its RAM/SoC?
2. Hackathon date and whether the iQOO 15 loaner is 12 GB or 16 GB — this decides the headline model.
3. Rust for `meshcore` (recommended) vs C++ — decide before Phase 1 day 1.
4. Admin web framework: keep it dependency-light (Preact/Svelte, no build server at runtime) so `meshd` stays a single binary.
5. Whether Windows is demoed at the hackathon at all or only listed as "same binary, built".

#### 12. Sources checked (23 Sep 2026)

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

---

## 11. Decisions (D001–D031, newest wins)

Format: **D### — title** · date · context · decision · alternatives rejected · consequences · status.

#### D001 — Inference engine is upstream llama.cpp / ggml
2026-09-23 · Only engine with native Android build, RPC backend, OpenCL-Adreno (incl. 840, MXFP4), Hexagon, gpt-oss and Qwen3-MoE support in one tree. **Rejected:** prima.cpp (ZMQ, Termux-only, CUDA-only, stale fork, no gpt-oss), GenieX as base (Snapdragon-only, no multi-device), own engine (pointless). **Consequences:** track upstream master; `third_party/llama.cpp` pinned by commit in scripts. Status: accepted.

#### D002 — Two planes: ggml RPC data plane (borrowed), MeshAI control plane (owned)
2026-09-23 · RPC v7 is proof-of-concept and insecure on open networks, but re-implementing tensor transport is out of scope for v1; the product value is in pairing, profiling, planning, scheduling, observability. **Decision:** tensors over ggml RPC embedded in-process on the phone with explicit `devices[]`; everything else over `proto/mesh.proto` on a Noise-encrypted TCP stream. RPC port opens only after pairing, bound to the paired link. **Rejected:** own transport in v1 (Phase 3 candidate to fix #22850 overhead). Status: accepted.

#### D003 — Shared native core in Rust (`meshcore`), Kotlin shell on Android, `meshd` daemon on desktop
2026-09-23 · One implementation of protocol/planner/scheduler for Linux, Windows and Android; memory-safe networking; single static desktop binary. **Rejected:** C++ everywhere (network/crypto risk under time pressure), Kotlin Multiplatform (JVM desktop is not native), Python/FastAPI (not shippable). **Consequences:** team ramps on Rust; cargo-ndk + UniFFI for the Android binding. Status: proposed — confirm before T010 (see state/blockers.md).

#### D004 — Android floor: arm64-v8a, minSdk 30, ≥8 GB, `dotprod`; tiers S/A/B
2026-09-23 · `getThermalHeadroom` is API 30; layer-split of 20B-class models needs ≥8 GB with headroom; ggml CPU path needs dotprod for acceptable speed. Tier S = 8 Elite / 8 Elite Gen 5 (GenieX NPU), A = 8 Gen 3 / D9300+ / Tensor G4+, B = any arm64 ≥8 GB with dotprod (CPU, parallel-job role). Status: accepted.

#### D005 — Foreground service type `connectedDevice`; dashboard stays visible during a plan
2026-09-23 · `dataSync`/`mediaProcessing` are capped at 6 h/24 h on Android 15+; top-app cpuset is required for big cores; Wi-Fi low-latency lock is foreground-only. `specialUse` is the fallback. Status: accepted, verify on Android 15/16 device in T020.

#### D006 — Phone compute order: CPU → OpenCL (prefill) → GenieX/Hexagon (Tier S, whole-model-alone only)
2026-09-23 · Community data on 8 Elite: OpenCL prefill 3.7–24× faster, decode ~17% slower than CPU on 7B; Hexagon experimental (~3.5 GB/session); NNAPI deprecated. Vulkan not planned on phone (historic hangs on 8 Elite). Status: accepted pending T005 measurement.

#### D007 — Headline demo model is Qwen3-Coder-30B-A3B; gpt-oss-20b runs under a 6 GB cgroup for the "budget laptop" story
2026-09-23 · Dev laptop has 18 GB; gpt-oss-20b (12.1 GB) fits on it alone, which would make the "neither device can hold it" demo false. 30B-A3B (18.6 GB) genuinely needs the phone. Status: accepted; revisit if the loaner iQOO 15 is 12 GB (then +2 phones).

#### D008 — Own hotspot or USB tethering; Wi-Fi Aware/Direct deferred
2026-09-23 · Linux laptops lack practical NAN support; venue Wi-Fi isolates clients. USB tethering is the low-jitter default and charges the phone. Status: accepted, measure both in T004.

#### D009 — Any device can be the model host; the coordinator stays on the laptop
2026-09-24 · The user wants input/output on a chosen device (phone or laptop) with every other device as pure compute. **Decision:** three roles. *Coordinator* (`meshd`, laptop): pairing, planning, admin panel, telemetry. *Host* (any paired device): runs `llama-server` with `--rpc <workers>`, owns the session, serves `/v1`; `meshd` proxies `localhost:8080/v1` to it. *Worker*: runs `ggml-rpc-server`. Selecting a host in the admin panel flips the others to worker. Both binaries ship for arm64 and x86-64. **Rejected:** host fixed to laptop (simpler, but not what the product promises). Status: accepted.

#### D010 — Android v1 runs llama.cpp as child processes from the app's native-lib dir; in-process JNI embed is Phase 2
2026-09-24 · Executables packaged as `jniLibs/<abi>/lib*.so` are extracted with exec permission and can be spawned from `applicationInfo.nativeLibraryDir`; the child inherits the app's cpuset and UID sandbox. This gives host mode (`llama-server`) and worker mode (`ggml-rpc-server`) on the phone immediately, with no Termux and no terminal, and reuses the verified arm64 build. **Cost:** per-process memory accounting (LMK may kill the child first; the FGS keeps the parent) and process-spawn latency. **Rejected for v1:** `ggml_backend_rpc_start_server` via JNI (better, but blocks host mode and costs a week). Status: accepted; revisit after Phase 0 numbers.

#### D011 — Model catalog lives on /mnt/storage (HDD); warm before demo
2026-09-24 · NVMe has 22 GB free; the HDD has 72 GB at ~80 MB/s. `MESHAI_MODELS=/mnt/storage/meshai/models`. First load of an 18.6 GB model ≈ 3 min; the admin panel shows a preload state and offers "pin to fast disk". Status: accepted.

#### D012 — Demo model set
2026-09-24 · Cloud reference: pluggable OpenAI-compatible provider (default Claude Sonnet 5). Laptop baseline: Qwen3-8B Q4_K_M (5.0 GB, 36 layers). Budget-laptop story: gpt-oss-20b MXFP4 (12.1 GB, 24 layers) with laptop capped at 6 GB. Mesh headline: Qwen3-Coder-30B-A3B Q4_K_M (18.6 GB, 48 layers). Test: Qwen3-0.6B Q8_0. Status: accepted.

#### D013 — Pairing v1: one-time QR token → per-device secret; Noise XX deferred (supersedes the transport part of D002 for v1)
2026-09-24 · The reviewer found reconnects accepted a self-reported device id alone and the pairing token was served by `/api/state`. **Decision:** the coordinator answers a valid token with a random 32-byte device secret (`Paired`), persists it in `state/paired.json`, and every reconnect must present it (constant-time compare). The offer/token is returned only by `POST /api/pair/offer` to the local admin and never appears in `/api/state` or the mirror. Control-plane *encryption* stays a Phase-2 item; v1 relies on the private hotspot/USB link. Status: accepted.

#### D014 — Cloud mirror is an explicit, opt-in exception to "no telemetry off-device"
2026-09-24 · The user asked for an internet-ready demo. **Decision:** `meshd mirror` exists only as a read-only admin (GET `/admin`, `/api/state`, `/api/runs`; POST `/api/relay/state`); the coordinator pushes a *stripped* snapshot (hashed device ids, no addresses, no paths, no process args, no token) only when `--push-to` is set; TLS is required before any real-network use; nothing else (models, weights, prompts, RPC) leaves the mesh. Off by default. Status: accepted, pending user sign-off for the live deployment (T050).

#### D015 — Android floor raised to dotprod **+ i8mm** (amends D004)
2026-09-24 · The shipped arm64 build is compiled with `+i8mm` (`smmla` present in `libggml-cpu.so`); a dotprod-only phone would SIGILL. Rather than ship a slower dotprod-only build for the hackathon, the tier gate now requires both. Cortex-A78/X1 (2021+) and newer qualify; A76-class cores do not. Revisit with `GGML_CPU_ALL_VARIANTS` runtime dispatch in Phase 2. Status: accepted.

#### D016 — Headroom constants until measured
2026-09-24 · Laptop reserves 2 GB; the phone reports 1.5 GB (`Profiler.HEADROOM`) and the coordinator uses whatever the device reports. Both are guesses to be replaced by the kill-threshold measurement (T006). llama.cpp compute/RPC buffers (~25–30 MiB per device at 2k ctx on the 0.6B) are not yet modelled. Status: provisional.

#### D017 — Thinking off by default for the demo
2026-09-24 · Qwen3 spends the whole token budget in reasoning unless told otherwise. `llama-server --reasoning off` on every host (laptop and phone). A per-request toggle is a Phase-2 admin control. Status: accepted.

#### D018 — Android v1 without UniFFI/meshcore.so (amends D003)
2026-09-24 · The v1 phone app re-implements framing and plan handling in Kotlin (~700 lines) instead of binding a Rust `meshcore.so`; the wire format is the shared `proto/mesh.proto`, so both sides stay in sync through the schema, and the llama.cpp argument rule (ngl+1, head pinned) is duplicated in `LlamaRunner.startHost` with a comment pointing at `supervisor.rs`. UniFFI binding returns when the planner needs to run on a phone host. Status: accepted.

#### D019 — x86_64 Android build is test-only
2026-09-24 · Built without AVX2/FMA/F16C so it runs on the emulator's CPU; never shipped to users (the floor is arm64). Status: accepted.

#### D020 — The laptop's RPC worker binds to its end of the host phone's control link
2026-09-24 · Round-2 review: `local_ip()` is the default-route interface, which on venue Wi-Fi is not the paired link. **Decision:** every control connection records the coordinator's local socket address; the laptop worker (phone-host mode) binds to the address of the host phone's link, and the plan sent to each device carries the laptop address *as that device sees it*. `meshd worker` (manual CLI) still uses the default-route address and prints it. Status: accepted.

#### D021 — `--lan` exposure rules
2026-09-24 · With `--lan`, every API route requires the token except the admin static files, `/api/catalog` and `GET /api/models/file/*` (weights are not secret and a phone host must fetch them). Even without a token, non-GET requests must carry `application/json` (defeats cross-site form posts) and, on loopback, the Host header must be a loopback name (defeats DNS rebinding). Status: accepted.

#### D022 — Heartbeat on a fixed interval (fixes a round-2 CRITICAL)
2026-09-24 · The coordinator's heartbeat lived in a `select!` sleep branch that was re-created on every loop iteration, so it never fired while telemetry streamed every 2 s; combined with the phone's read timeout every real link would drop every ~30 s. **Decision:** `tokio::time::interval(10 s)` created once per connection; phone read timeout 45 s. Not yet executed on a device (no phone on USB) — T021 stays open until a 5-minute link test is logged. Status: accepted, verification pending.

#### D023 — Greedy contiguous fill, host first (amends ARCHITECTURE §7 rule 2)
2026-09-24 · Proportional targets produced false `DoesNotFit` on tight pools (round-2 N7). **Decision:** the host keeps embeddings/output plus as many leading layers as it can hold, then each further device (largest usable memory first) takes as many consecutive layers as it can hold; devices keep being added while layers remain. The "laptop last" phrasing in §7 is superseded: the host is the laptop unless the user picks a phone, because input/output happen there (D009); loading the host fully first is accepted because it holds the head and KV for the earliest layers anyway. Status: accepted.

#### D024 — A phone can be the host only when meshd runs with `--lan --api-token`; a replacement run is planned ONCE, before the stop, crediting only the memory the live run's own processes hold
2026-09-24 · The phone host fetches the model from `/api/models/file/…`, which is loopback-only by default, so `POST /api/run` answers 422 for a remote host unless `--lan` is set. **Replacement runs (rounds 4–10):** planning after the stop saw stale memory (phones report every 2 s; round 8); planning before the stop saw memory the old run still held (round 5); crediting the plan's *estimate* credited memory not yet held (round 9: 5.32 GB credited on a laptop whose MemAvailable had not moved during load); crediting the *observed drop* in MemAvailable since the run started credited memory other processes took (round 10: with a 3 GB Python allocation alive, the whole 868 MB placement was credited although stopping the run would free nothing extra — measured by the round-10 audit on this laptop). **Decision:** `api_run` does cheap checks (model exists, `0 < n_ctx ≤ n_ctx_train`, host known/online, remote host needs `--lan`), then makes exactly one plan against `credited_caps()`. A device is credited only when the run is `ready`, the device is online with *measured* memory (no override) sampled after `ready_ms`, and the credit is `min(placement bytes, held_bytes)`, where `held_bytes` is the anonymous RSS (RssAnon + RssShmem from `/proc/<pid>/status`) of the run's own llama.cpp child processes on that device — the laptop measures its llama-server / ggml-rpc-server children in `refresh_local_profile`, a phone measures its child (`/proc/<child>/status`; the pid comes from UNIXProcess's private field or, if the hidden-API policy refuses, from `Process.toString()`) and reports `Telemetry.held_bytes` — implemented and JVM-unit-tested, NOT yet executed on a device; if the read fails the phone reports 0 and earns no credit, which is the safe direction (the replacement request is refused and the live run stays up). Anonymous RSS is what a stop returns to MemAvailable; file-backed mmap'd weights live in page cache, are already counted as available, and were never subtracted from `avail_bytes` — so they need no credit. Everything else (loading run, stale sample, override, offline, rejected placement, memory taken by unrelated processes) earns nothing. If the plan fails the request is refused and the live run is untouched; if it succeeds: stop → start that plan, no re-plan. `/api/plan` previews with the same capacities and both return `credited: [[device, bytes]]`. Remaining limits (K19): a phone's `held_bytes` is self-reported; RSS is sampled up to 2 s (phone) / one refresh (laptop) before the request; the laptop's `avail_bytes` used for planning is taken before the stop, which is exactly why the held memory is credited back. Status: accepted; unit-tested (`credit_tests`: parser, loading earns nothing, cap at placement, unrelated pressure never credited, stale/override/offline/rejected, planner end to end); executed by `scripts/qa/run-guards.sh` (no credit while loading; after ready 0 < credit ≤ placement and equal to the llama-server's RssAnon read straight from /proc; and the premise itself — a stop returns the credited anonymous RSS to MemAvailable — is measured on every run: ctx-8192 laptop-alone run, credit vs MemAvailable released by the stop, must agree within 30 %; see BENCHMARKS for the numbers).

#### D025 — A device's failure report ends the run, but only for the plan it names
2026-09-24 · Phones report process exits and refused plans as `JobResult{ok:false}`. Without a plan id a stale "download cancelled" from a superseded run could kill the next run (round-4 #1). **Decision:** `JobResult.plan_id` is mandatory; meshd honours a report only if the device holds a plan and the id equals the current run's plan id; the phone never reports a cancelled job (cancellation is a real `CancellationException`). Same rule for a device that disconnects or is forgotten mid-run: the run ends with `error`. Status: accepted, tested (`failure_report_ends_only_the_run_it_belongs_to`).

#### D026 — Locking discipline in meshd: `run_lock` → `proc_lock`, never the reverse
2026-09-24 · `run_lock` serialises the *initiation* of start/stop/forget (held by API handlers and device cleanup). `proc_lock` is held across every "check generation → spawn/register/kill a process, push a plan, or change run status" so a stop can never interleave with a registration. Lock order is always run_lock then proc_lock; no std guard is held across an await. Background tasks carry the generation they were started for and use `fail_if_current`. On the phone the mirror is: one plan actor owns `planJob`; `LlamaRunner` owns the process under one lock with `armed`/`procGen` so a racing start is refused and a stale exit is never reported. Status: accepted.

#### D027 — Only host-capable devices can be the host; a requested host is honoured or refused, never swapped
2026-09-24 · With the laptop capped, the planner made a *simulated* phone (a bare ggml-rpc-server) the single-device host and the run waited forever for a llama-server that never existed. **Decision:** `DeviceCap.can_host` (false for `DeviceKind::Sim`; true for laptops and phones — a phone runs llama-server as host, D009). Single-device and split host selection only consider host-capable devices (`PlanError::NoHost` if none). A `prefer_host` that is offline, rejected (RTT, thermal, battery, tier, memory) or compute-only returns `PlanError::HostNotEligible{reason}` instead of silently choosing another host. Tests: `a_compute_only_worker_is_never_the_host`, `a_requested_host_is_honoured_or_refused_never_swapped`. Status: accepted.

#### D028 — Desktop is one cross-platform binary; Windows via sysinfo, no Linux-only calls in meshd
2026-09-24 · The user asked for full Windows compatibility of the desktop app. **Decision:** every platform call in `meshd` goes through `sysinfo` (available memory, hostname, process alive/terminate, process RSS) or std; `/proc` is used only under `cfg(target_os = "linux")` for the RssAnon credit, with the sysinfo resident set elsewhere; `kill(1)` is gone. `cargo check --target x86_64-pc-windows-gnu` is part of `scripts/verify` when the target is installed; `scripts/build-windows.sh` cross-compiles with mingw; `scripts/windows/setup.ps1` fetches llama.cpp Windows binaries and `run-meshd.ps1` opens the firewall and starts meshd. Nothing has executed on Windows yet (see docs/WINDOWS.md "Not yet done"). Status: accepted, unexecuted on Windows.

#### D029 — Pairing must survive restarts and bad links: secret beats token, one QR carries every address, USB pairs with one click
2026-09-24 · Three real-hardware failures in one afternoon: (1) after a meshd restart the paired phone was absent from the device table, so "forget everything" skipped it and every new token was refused as "already paired"; (2) the app erased its stored secret on *any* Bye, so a used-up token deadlocked re-pairing; (3) the QR named only one laptop address, useless when the phone was on a different link. **Decisions:** paired devices are listed (offline) after a restart; in Hello a valid `device_secret` is accepted as a reconnect even when a token is also present, and the phone always offers its stored secret; the phone clears the secret only on a definitive "not paired / wrong secret / forgotten" reply and remembers the last payload so it reconnects by itself on launch; the offer payload carries `hosts` (every non-loopback IPv4 of the laptop, USB-tether → ethernet → Wi-Fi) and the phone dials them in turn; `GET /api/usb` lists phones on USB debugging and `POST /api/usb/pair` sets up adb reverse/forward and hands the app the payload (the phone still shows the confirmation card, M9). Status: accepted; meshd side executed (offline listing, USB detection, one-click offer delivered); the phone-side reconnect path is built and installed but its confirmation is pending the user unlocking the phone.

#### D030 — The desktop panel is three plain pages; everything else is behind "Advanced"
2026-09-24 · The user asked for a design anyone can read. **Decision:** the top bar always states the current task (planning / sending the model X % / loading / ready / answering). Page 1 *Devices*: connect (USB one click, or QR) and both devices side by side with specs, the permissions each granted (the phone reports them in `DeviceProfile.permissions`), and live compute. Page 2 *Run*: model cards with a fit verdict, Run/Stop, "who holds what" with the stacked layer bar, live feed. Page 3 *Chat*: available compute at the top, ten one-click prompts, attachments (images → `image_url` parts for vision models with a projector, text/CSV/JSON/code → inlined context), and who answered. Raw logs, llama.cpp arguments, downloads, simulation and analytics sit under an Advanced toggle. The catalog gains Qwen2.5-VL-3B + its projector; `llama_args` adds `--mmproj` when the projector is present (laptop host only for now). Status: accepted; executed in the headless browser (all three pages, live chat 23.8 tok/s on the sim split).

#### D031 — The phone's RPC port is dynamic: try a spread of ports, report the one that bound, forward the whole set over the cable
2026-09-24 · Mid-session the POCO F5's kernel added 50048–50061 to `ip_local_reserved_ports`, so the worker's `bind` on 50052 failed ("Failed to create server socket") although it had worked an hour earlier; netcat and the RPC server failed on every port in that range and succeeded on 50062+. **Decision:** the app tries the planned port and then 50062, 50070, 50080, 50100, 50200, 51000, waiting up to 6 s for its own connect probe; it reports `listening:host:port`; meshd adopts the reported port (`Device.rpc_port`) and rebuilds the llama-server arguments *after* the worker-ready wait and before the reachability check; `POST /api/usb/pair` forwards the whole set through adb. Status: accepted; executed (worker came up on 50062, split ready in 20.2 s).

---

## 12. Benchmarks (every number here was run; conditions included)

Columns: date · setup · device(s) · model (quant, file GB) · ctx · threads · backend · link · **prompt t/s** · **decode t/s** · TTFT · notes (the exact pp/tg sizes are in the command column) · command · llama.cpp commit

| date | setup | devices | model | ctx | thr | backend | link | prompt t/s | decode t/s | TTFT | notes | cmd | commit |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2026-09-24 | laptop alone | IdeaPad 3 i5-1035G1 (18 GB, busy: ~4 GB free) | Qwen3-0.6B Q8_0 (0.64 GB) | bench | 4 | CPU | local | 39.4 ± 11.2 | 6.7 ± 4.6 | — | machine loaded (cargo/gradle running); high variance | `llama-bench -m Qwen3-0.6B-Q8_0.gguf -p 128 -n 32 -r 2` | 66fba63 |
| 2026-09-24 | split: laptop host + 2 local RPC workers (sim phones) | same laptop, workers `ggml-rpc-server -t 2` on 127.0.0.1:50081/50082 | Qwen3-0.6B Q8_0 | bench | 4 | CPU+RPC, `-ngl 24 -ts 0.5,0.5` | loopback | 70.9 ± 6.4 / 57.5 ± 6.4 | 12.4 ± 3.1 / 9.6 ± 0.7 | — | proves the RPC layer-split path end to end; **two rpc-servers sharing `-c` cache on one host crash `llama_context::synchronize` — run sims without `-c`** | `llama-bench --rpc 127.0.0.1:50081,127.0.0.1:50082 -ngl 24 -ts 0.5,0.5 -p 128 -n 32 -r 2` | 66fba63 |
| 2026-09-24 | split via meshd: laptop host (capped 0.35 GB) + 2 local RPC workers | IdeaPad 3 (loaded), workers `-t 2` | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 24 --tensor-split 0.32,0.38` | loopback | 25.7 (server) / 39.5 | 4.0–4.8 | 0.58–2.9 s | end-to-end through `/v1` proxy; model ready in 4.6–13 s; thinking off → 19-token direct answer | admin Chat / `curl /v1/chat/completions` | 66fba63 |
| 2026-09-24 | split via meshd (rewritten): laptop host (capped 0.35 GB) + 2 local RPC workers | IdeaPad 3, machine quiet; workers `-t 2` | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 25 --override-tensor output\.weight=CPU --tensor-split 0.44,0.56` | loopback | 113.4 (server, 22 tok prompt) | 22.0 (server) / 19.2–21.5 (client rows) | 186–223 ms | placement verified against `load_tensors: layer N assigned to device`: CPU 0–3, RPC0 4–14, RPC1 15–27 (+ head pinned to CPU) = the plan exactly | admin Chat / `curl /v1/chat/completions` stream, max_tokens 9–26 | 66fba63 |
| 2026-09-24 | split via meshd (round-2 build): laptop host (capped 0.35 GB) + 2 local RPC workers | IdeaPad 3, gradle+cargo running concurrently | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 25 -ot output\.weight=CPU --tensor-split 0.44,0.56` | loopback | 91.7 (server) | 10.8 (server) | — | same placement; machine loaded → lower than the quiet-machine row above | split-sim.sh | 66fba63 |
| 2026-09-24 | split via meshd, browser-driven chat (rounds 1–2) | IdeaPad 3 | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC as above | loopback | — | 21.3 / 21.6 (client) | 198 ms | admin Chat, 22 tokens, direct answer (thinking off) | admin-qa.mjs | 66fba63 |
| 2026-09-24 | split via meshd (round-3 build, proc_lock): laptop host 7 layers + sim workers 19 + 2 | IdeaPad 3, gradle idle | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 22 -ot output\.weight=CPU --tensor-split 0.863636,0.136364` | loopback | 121.1 (server) | 22.5 (server) / 23.6 (client) | 228 ms | browser chat, 24 tokens; ready in 2.0 s; `--tensor-split` added to this row afterwards from the run's recorded args (same plan shape as the row below) | admin-qa.mjs, split-sim.sh | 66fba63 |
| 2026-09-24 | split via meshd (round-4 build): laptop host 7 layers + sim workers 19 + 2 | IdeaPad 3, idle | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 22 -ot output\.weight=CPU --tensor-split 0.863636,0.136364` | loopback | — | 22.6 (client) | 198 ms | browser chat, 34 tokens; ready in 2.0 s; split-sim chats 21.1–24.6 tok/s client | scripts/qa/admin-qa.mjs, scripts/split-sim.sh | 66fba63 |
| 2026-09-24 | split via meshd (round-6 build, 03:50 IST): laptop host 7 layers + sim workers 19 + 2 | IdeaPad 3, idle | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 22 -ot output\.weight=CPU --tensor-split 0.863636,0.136364` | loopback | — | 22.6 (client) | 225 ms | browser chat, 33 tokens; split-sim chats 19.7–21.7 tok/s client, TTFT 162–197 ms | scripts/qa/run-guards.sh | 66fba63 |
| 2026-09-24 | split via meshd (round-7 build, 04:02 IST): laptop host 7 layers + sim workers 19 + 2 | IdeaPad 3, idle | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 22 -ot output\.weight=CPU --tensor-split 0.863636,0.136364` | loopback | — | 23.7 (client) | 207 ms | browser chat, 19 tokens; split-sim chats up to 22.7 tok/s client, TTFT 161–181 ms | scripts/qa/run-guards.sh | 66fba63 |
| 2026-09-24 | split via meshd (round-8 build, 04:16 IST): laptop host 7 layers + sim workers 19 + 2 | IdeaPad 3, idle | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 22 -ot output\.weight=CPU --tensor-split 0.863636,0.136364` | loopback | — | 22.7 (client) | 197 ms | browser chat, 33 tokens; split-sim chats up to 22.3 tok/s client, TTFT 161–190 ms | scripts/qa/run-guards.sh | 66fba63 |
| 2026-09-24 | split via meshd (round-9 build, 04:31 IST): laptop host 7 layers + sim workers 19 + 2 | IdeaPad 3, idle | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 22 -ot output\.weight=CPU --tensor-split 0.863636,0.136364` | loopback | — | 23.3 (client) | 193 ms | browser chat, 19 tokens; split-sim chats up to 21.7 tok/s client, TTFT 166–207 ms; laptop MemAvailable drop 3 s after a 0.6B laptop-alone run reached ready: 0 B (mmap) | scripts/qa/run-guards.sh | 66fba63 |
| 2026-09-24 | split via meshd (round-10 build, 04:43 IST) — machine LOADED (Gradle compiling concurrently) | IdeaPad 3, loaded | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 22 -ot output\.weight=CPU --tensor-split 0.863636,0.136364` | loopback | — | 9.2 (client) | 453 ms | browser chat, 16 tokens; split-sim chats 2.4–8.7 tok/s client; laptop-alone 0.6B run at ready: llama-server anonymous RSS 293 MB (placement estimate 868 MB) | scripts/qa/run-guards.sh | 66fba63 |
| 2026-09-24 | split via meshd (round-11 build, 04:53 IST) | IdeaPad 3, idle | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 22 -ot output\.weight=CPU --tensor-split 0.863636,0.136364` | loopback | — | 22.7 (client) | 201 ms | browser chat, 34 tokens; split-sim chats up to 22.1 tok/s client, TTFT 169–190 ms | scripts/qa/run-guards.sh | 66fba63 |
| 2026-09-24 | D024 premise: laptop-alone run, credit vs memory a stop releases (04:53 IST) | IdeaPad 3, idle | Qwen3-0.6B Q8_0 | 8192 | 4 | CPU, mmap (default) | — | — | — | — | llama-server RssAnon+RssShmem = credit 997,859,328 B; MemAvailable released by /api/stop = 1,133,116,416 B (avg of 3 samples before/after, 2 s settle); at ctx 2048 the credit was 293,187,584 B = /proc RssAnon exactly (placement estimate 868 MB) | scripts/qa/run-guards.sh | 66fba63 |
| 2026-09-24 | **FIRST RUN ON THE REAL PHONE** — phone-hosted via the app (meshd pushed the plan, phone fetched the model over Wi-Fi in ~130 s and ran llama-server itself; laptop proxied /v1), 10:39 IST | POCO F5 (SM7475 Snapdragon 7+ Gen 2, 8 GB, Android 15) as HOST; IdeaPad 3 as coordinator | Qwen3-0.6B Q8_0 | 2048 | 4 (app default) | CPU (arm64 dotprod+i8mm) | Wi-Fi 192.168.21.0/24 (shared home router) | — | 19.4 (3 tok, warm-up) / **28.8** (48 tok) client via meshd proxy | 1120 ms (first) / 251 ms | ready 131.1 s incl. 640 MB download; model cached on the phone afterwards | app + meshd | 66fba63 |
| 2026-09-24 | link quality, same Wi-Fi: ping laptop→phone 20 pkts | POCO F5 ↔ IdeaPad 3 | — | — | — | — | Wi-Fi shared | — | — | — | laptop→phone 9.5/56.1/99.6 ms min/avg/max, 0 % loss; phone→laptop 80 % loss, avg 211 ms (phone Wi-Fi power save); meshd control-link RTT p50 4.9 ms at pairing, later 20.8 ms p50 / 65.7 ms p95 → planner rejected the phone as a worker (RTT p95 > 60 ms policy) | ping, /api/state | — |
| 2026-09-24 | **FIRST REAL LAPTOP↔PHONE LAYER SPLIT via the app**, 11:26 IST: laptop host layers 0–12 + head (capped 0.5 GB), POCO F5 worker layers 13–27 (ggml-rpc-server spawned by the app, 4 threads, bound to the link address) | IdeaPad 3 host + POCO F5 (SM7475, 8 GB, Android 15; ~2.1 GB free, WhatsApp in use) | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 16 -ot output\.weight=CPU` | **USB via adb port-forward** (adb reverse 7070/8080, adb forward 50052; control RTT p50 5.2 ms / p95 9.6 ms) | 15.8–16.2 (server) | **0.65–1.36** (server: 1547 ms and 735 ms per token) / 0.6–1.3 (client) | 1514–1776 ms | ready in 18.6 s; 3 correct answers (7/34/16 tokens, 12–54 s each). Gate T007 (≥ 3 tok/s) NOT met on this transport: every RPC round trip is relayed by adbd over USB; needs re-measuring over USB tethering / hotspot (K20, K21) | app + meshd, adb | 66fba63 |
| 2026-09-24 | same session, shared Wi-Fi at the time of the split attempt (11:21 IST): control RTT p50 90.3 ms / p95 153.9 ms → planner refused the phone (policy 60 ms p95) | POCO F5 ↔ IdeaPad 3 | — | — | — | — | Wi-Fi shared | — | — | — | refusal is by design; USB/hotspot required | /api/state | — |
| 2026-09-24 | **image chat** — Qwen2.5-VL-3B Q4_K_M + f16 projector (`--mmproj`), laptop alone, 14:17 IST | IdeaPad 3 (uncapped, ~7.8 GB usable at the time) | Qwen2.5-VL-3B Q4_K_M (1.93 GB) + mmproj (1.34 GB) | 4096 | 4 | CPU | — | — (2154 prompt tokens incl. the image) | — | — | ready in 48.8 s; asked to describe a 1280×1200 screenshot of the panel: correct (named the app, the USB phone, the QR code, the simulated-phone controls); 64 tokens in 150.1s s end to end | /v1 with image_url (base64) | 66fba63 |
| 2026-09-24 | laptop+phone split, second session (15:12 IST): laptop host 0–12 (capped 0.5 GB) + POCO F5 worker 13–27 on port 50062 (50052 had become kernel-reserved) | IdeaPad 3 + POCO F5 | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC | USB via adb (RTT p50 2.5 ms) | — | 1.2–1.4 (client) | 1037–1448 ms | ready 20.2 s; 3-turn interview: the model asked a question, took the answer, followed up twice (27/34/16 tokens, 20/29/13 s) | /v1 via python | 66fba63 |

#### Sustained (T006)
| date | device | model | minute | thermal headroom | status | current mA | tg128 | notes |
|---|---|---|---|---|---|---|---|---|

#### Memory kill threshold (T006)
| date | device (RAM) | model | ctx at last success | MemAvailable before kill | headroom chosen |
|---|---|---|---|---|---|

---

## 13. Windows

`meshd` is one Rust binary (`meshd.exe`) that serves the admin panel, the control plane for
phones and the OpenAI-compatible `/v1` endpoint. Everything platform-specific goes through
`sysinfo` (memory, process control, hostname) and `local-ip-address`, so the Linux and Windows
builds are the same code. The parts that differ are listed at the end.

#### What you need on the Windows machine

| Item | Why | Where |
|---|---|---|
| Rust (stable, MSVC or GNU toolchain) | build `meshd.exe` | https://rustup.rs |
| protoc — **not needed**: `protoc-bin-vendored` ships one | control-plane protobufs | (crate) |
| llama.cpp Windows binaries built with `GGML_RPC=ON`: `llama-server.exe`, `ggml-rpc-server.exe`, `llama-bench.exe` and their DLLs | host / worker / bench processes | `scripts/windows/setup.ps1` downloads the pinned release, or build from `third_party/llama.cpp` with CMake |
| GGUF models | the catalog | `scripts/windows/setup.ps1 -Models` or the admin's Models page |

#### Build and run

```powershell
### from the repo root, in PowerShell
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

#### Windows Firewall and links

- `meshd --lan` listens on `0.0.0.0:8080` (token required) and `0.0.0.0:7070` (control plane,
  paired devices only). `run-meshd.ps1` adds inbound rules for both; remove them with `-Remove`.
- Private link options, same as Linux: the laptop's mobile hotspot (Settings → Network → Mobile
  hotspot), the phone's hotspot, or USB tethering (the phone appears as an "RNDIS" adapter).
- The RPC data plane (`:50052` on the phone) is dialled *from* the laptop, so no extra inbound rule.

#### Platform differences (all handled in code)

| Concern | Linux | Windows |
|---|---|---|
| free memory | `sysinfo` `available_memory()` | same |
| what a stop frees (D024 credit) | `RssAnon+RssShmem` from `/proc/<pid>/status` | `sysinfo` process memory (private working set) |
| stop a child | SIGTERM, then SIGKILL after 3 s | `TerminateProcess` (no gentler signal exists); llama-server writes nothing on exit so this is safe |
| is a pid alive | `sysinfo` | same |
| pairing secrets file mode | `0600` | NTFS ACL of the user profile (`state\paired.json` is under the user's directory) |
| binaries | `llama-server`, `ggml-rpc-server` | same names; Rust appends `.exe` |
| open the admin | `xdg-open` (by the user) | `Start-Process` in `run-meshd.ps1` |

#### Not yet done on Windows

- Nothing has been *executed* on a Windows machine in this repo yet: the port is `cargo check`
  clean for `x86_64-pc-windows-gnu` and the platform calls are covered by `sysinfo`, but the
  Definition of Done requires a run. Do that first on a Windows laptop: `scripts/verify` there,
  then `scripts/qa/run-guards.sh` under Git Bash.
- A Windows service wrapper (NSSM or `sc.exe`) is optional; `run-meshd.ps1` is a foreground run.

---

## 14. Demo tasks

MeshAI's promise is one sentence: **your laptop and your phone pool their memory over a private
link and serve one local, OpenAI-compatible AI endpoint — no cloud, no account.** The compute we
have (measured 24 Sep 2026, see BENCHMARKS.md) decides which tasks are convincing.

| Device | Memory for models (measured, with headroom kept) | What it can do alone |
|---|---|---|
| IdeaPad 3 (i5-1035G1, 20 GB) | ~10–12 GB idle, 3–5 GB while building | Qwen3-14B Q4 alone; gpt-oss-20b MXFP4 (12.1 GB) when quiet |
| POCO F5 (SM7475, 8 GB) | 1.0–1.6 GB with the user's apps open, ~2.5 GB after clearing apps | Qwen3-0.6B at 28.8 tok/s as host; 1.7B plausible |

#### Task 1 — "My phone is the assistant" (works today, fast)
The phone is the **host**: the laptop stores the models and pushes one to the phone; the phone
runs it and answers; the laptop's Chat page (and any OpenAI client on `localhost:8080/v1`) talks
to it. Measured: Qwen3-0.6B, 28.8 tok/s, 251 ms first token, model copied in ~130 s once.
*Shows:* pairing, the model transfer with a live progress bar, the phone's cores lighting up, and
a private assistant that keeps working when the laptop is busy.

#### Task 2 — "Bigger brain on the laptop, phone as the remote" (works today)
The laptop hosts Qwen3-8B/14B (fits alone — the planner will *refuse* to split, by design); the
phone stays paired as a client and a helper for later. Chat: summaries, code, translations,
data-file analysis (upload a CSV/JSON/text on the Chat page). With Qwen2.5-VL downloaded, image
questions too (screenshots, photos, charts).

#### Task 3 — "A model neither device can hold" (the headline; needs a real link)
Laptop capped or genuinely short of memory + phone helper: Qwen3-30B-A3B (18.6 GB Q4, 14.7 GB Q3)
split by layers. **Executed** with Qwen3-0.6B as the stand-in (laptop 0–12, phone 13–27): correct
answers, ready in 18.6 s — but 0.65–1.4 tok/s over the adb-relayed USB transport. The gate for
this task is **≥ 3 tok/s**, which needs USB tethering (turned on from the phone's Settings) or a
hotspot; the planner already refuses links worse than 60 ms p95. Until that is measured, Task 3 is
demonstrated as *correct*, not as *fast*.

#### What the three pages show
1. **Devices** — connect (USB one click or QR), then both devices side by side: chip, cores, memory,
   system, capability, the permissions each granted, and live compute (cores at work, memory split,
   heat, battery, link).
2. **Run** — pick a model (each card says "fits the laptop alone" / "needs laptop + phone" / "too
   big"), press Run, watch *who holds what* (stacked layer bar, per-device cards) and the live feed.
3. **Chat** — the available compute at the top (model, memory, laptop, phone, last speed), ten
   one-click things to try, attachments (images for vision models, text/CSV/JSON/code files to
   analyse), and *who answered*.
The top bar always says what is happening right now (planning, sending the model, loading,
ready, answering). "Advanced" reveals the raw logs, llama.cpp arguments, downloads and analytics.

---

## 15. Risks (K01–K22)

| ID | Risk | Likelihood | Impact | Signal | Mitigation | Owner task |
|---|---|---|---|---|---|---|
| K01 | Split decode < 3 tok/s on real hardware | Med | High | T004 | Product = single-device-per-job + parallel jobs + GenieX Tier S; control plane still the product | T007 |
| K02 | Loaner iQOO 15 is the 12 GB SKU | Med | High | blockers | 2nd phone core; smaller quant | T040 |
| K03 | Demo laptop (18 GB) fits the model alone | Certain | Med | — | Headline 30B-A3B; cgroup 6 GB for budget story, say so on slide | T040 |
| K04 | OpenCL decode slower than CPU on Adreno 840 | High | Low | T005 | Treat GPU as prefill accelerator only | T005 |
| K05 | In-app RPC server doesn't use the GPU device | Med | Med | T005 | Explicit `devices[]`; CPU fallback is fine | T021 |
| K06 | FGS killed / time-capped on Android 16 | Low | High | T020 | `connectedDevice`, dashboard visible, `specialUse` fallback | T020 |
| K07 | Android LMK kills worker under memory pressure | Med | High | T006 | Measured headroom per tier; `onTrimMemory` → shrink plan | T006 |
| K08 | Thermal throttling collapses t/s mid-demo | Med | Med | T006 | Headroom-based shedding; charging; cap phone share | T006 |
| K09 | Rust ramp-up slows Phase 1 | Med | Med | velocity | `meshcore` v0 small (proto+planner); Kotlin for all UI | T010 |
| K10 | Prebuilt `.so` (OpenCL loader, GenieX) not 16 KB aligned | Low | High | readelf | Check every `.so` in verify (full) | T021 |
| K11 | RPC port reachable before pairing | Low | High | review | Open only after Noise handshake, bind to paired iface; reviewer gate | T021 |
| K12 | Venue Wi-Fi isolates clients | High | High | demo day | Own hotspot + USB tethering | T004 |
| K13 | Android phantom-process killer ends the child llama.cpp process (D010) | Med | High | T006 | Top-app + FGS; measure 10 min screen on/off; fall back to JNI embed (D010 Phase 2) | T006 |
| K14 | i8mm build SIGILLs on dotprod-only phones | Med | High | tier gate | D015: gate requires i8mm; runtime-dispatch build in Phase 2 | T020 |
| K15 | API exposed on LAN/venue Wi-Fi | Low | High | review | Localhost by default; `--lan` needs a token; file names validated; no CORS (C1) | T012 |
| K16 | Mirror leaks mesh details to the internet | Low | Med | review | Stripped snapshot, read-only router, TLS before use (D014) | T050 |
| K17 | On shared Wi-Fi the phone's RPC port and its `:8081` llama-server are reachable by any device on that network (bound to the link address, no auth) | Med | High | design | Demo on hotspot or USB tethering only (ARCHITECTURE §2); Phase 2: RPC over the paired channel or a per-link firewall rule | T004 |
| K18 | Pre-auth control sockets from one LAN peer starve reconnects (16-slot pool) | Low | Med | test | Per-IP cap of 4 pre-auth sockets (control.rs `PreauthPool`, tested); Hello deadline 5 s; frame cap 4 KiB | T022 |
| K19 | Replacement-plan credit vs reality: the credit is the anonymous RSS of the run's own processes (D024). Remaining error sources: a phone's held_bytes is self-reported and the phone path is not yet executed on a device (a failed /proc read reports 0 → no credit, safe); RSS is sampled up to 2 s before the request; RSS can dip during a stop/swap; swapped/zram anon, hugetlbfs and GPU buffers are outside RssAnon (under-credit, safe). Earlier rules were measured wrong on this laptop by the audits: the plan estimate credited 5.32 GB with no MemAvailable movement (round 9); the observed-drop rule credited the full placement while an unrelated 3 GB allocation was alive (round 10). An over-committed mmap load does not fail cleanly on Linux — it pages or is OOM-killed | Low | High | measured | credit = min(placement, held RSS), only when ready and sampled after ready_ms; loading earns nothing (run-guards); Phase 2: phone RSS attested by the worker-ready handshake, `--no-mmap` option | T012 |
| K20 | Switching the phone's USB mode to RNDIS from adb (`svc usb setFunctions rndis`) dropped adb *and* the tether link on the POCO F5 (Android 15); the phone then needs USB debugging re-enabled by hand | High | Med | observed 24 Sep | Turn USB tethering on from the phone's Settings (keeps adb), or use a hotspot; never switch USB functions from adb | T004 |
| K21 | Shared home Wi-Fi: phone→laptop pings lost 80 % (Wi-Fi power save) and control RTT p95 reached 65.7 ms, so the planner refuses the phone as a worker (policy 60 ms) | High | High | measured | Demo on the laptop hotspot / phone hotspot / USB tethering as the design says (§2); the app's low-latency Wi-Fi lock helps only while joined | T004 |
| K22 | Android reserves port ranges at runtime (`ip_local_reserved_ports` grew to include 50048–50061 on the POCO F5 during the day) — a fixed worker port stops binding without warning | High | High | measured | D031: dynamic port with fallbacks reported to the laptop; the cable pairing forwards the set | T021 |

---

## 16. Research notes (upstream facts checked)

#### R001 — Which inference engine can serve as MeshAI's base?            (2026-09-23)
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

#### R002 — Can an unprivileged Android app get the big cores and keep running?            (2026-09-23)
**QUESTION** Can the worker pin ggml threads to big cores and run for hours without Android killing or throttling it?
**CANDIDATES** A top-app + sched_setaffinity + connectedDevice FGS (preferred) · B dataSync FGS · C root / vendor hooks
**OFFICIAL EVIDENCE** [T1] cpusets: top-app gets all cores, background restricted to little cores — https://source.android.com/docs/core/power/performance · [T1] dataSync/mediaProcessing 6 h/24 h then `onTimeout` (Android 15+); Android 16 quotas on FGS-started jobs — https://developer.android.com/develop/background-work/services/fgs/timeout · [T1] 16 KB pages required since Nov 2025; NDK r28+ default — https://developer.android.com/guide/practices/page-sizes · [T1] NNAPI deprecated in Android 15; migrate to LiteRT / vendor runtimes — https://developer.android.com/ndk/guides/neuralnetworks/migration-guide
**REAL-WORLD EVIDENCE** [T5] LWN on Android EAS/cpusets — https://lwn.net/Articles/706374/ · [T5] NDK list: affinity via `syscall(__NR_sched_setaffinity, gettid(), …)` — https://groups.google.com/g/android-ndk/c/PKpldjrx8c8
**UNKNOWN** Real kill threshold per tier → T006. Whether `connectedDevice` stays un-capped on Android 16 devices → T020.
**DECISION** A · **CONFIDENCE** Medium-High
**WHY NOT B?** 6 h cap · **WHY NOT C?** not a consumer product.
**CONSEQUENCE** D004, D005; android-native skill.

#### R003 — Link: hotspot vs USB tethering vs Wi-Fi Aware/Direct            (2026-09-23)
**QUESTION** Which link gives low, stable RTT between an Android phone and a Linux laptop without venue infrastructure?
**CANDIDATES** A own hotspot · B USB tethering · C Wi-Fi Aware (NAN) · D Wi-Fi Direct
**OFFICIAL EVIDENCE** [T1] Wi-Fi Aware overview (Android side; NDP simplified on 12+) — https://developer.android.com/develop/connectivity/wifi/wifi-aware
**REAL-WORLD EVIDENCE** [T4] home Wi-Fi 3–7 ms (prima.cpp, TPI-LLM); busy office p99 ~250 ms (Sui et al., MobiSys 2016) — cited in spec v2.
**UNKNOWN** RTT p50/p95 and jitter for A vs B on our hardware → T004.
**DECISION** A + B (measure both; B default when phone is on the desk) · **CONFIDENCE** Medium
**WHY NOT C/D?** no practical Linux-laptop NAN/P2P stack; keep for phone↔phone in Phase 3.
**CONSEQUENCE** D008.

#### R004 — Will Android kill the llama.cpp child process (D010)?            (2026-09-24)
**QUESTION** Does Android 12+ terminate an app's forked native process even while the app holds a foreground service?
**CANDIDATES** A child process from `nativeLibraryDir` (D010, current) · B in-process `ggml_backend_rpc_start_server` via JNI (D010 Phase 2)
**OFFICIAL EVIDENCE** [T1] Android 12 introduced the *phantom process* limit: apps may keep at most 32 phantom (forked) processes system-wide and the OS kills phantom processes that use excessive CPU while the app is in the background — https://developer.android.com/about/versions/12/behavior-changes-12 (recalled from memory, **not re-fetched today**: Medium confidence)
**REAL-WORLD EVIDENCE** [T7] Termux users report "signal 9" kills of long-running shells on Android 12+ unless `settings put global settings_enable_monitor_phantom_procs false` (needs adb) — Termux issue tracker (recalled, Medium)
**UNKNOWN** Whether a top-app + `connectedDevice` FGS keeps the child out of the "excessive CPU in background" rule for a 10-minute sustained run on the POCO F5 (Android 15) with the screen on and off → **T006** must include this.
**DECISION** keep A for the hackathon, treat B as the fix if T006 shows kills · **CONFIDENCE** Low until measured
**CONSEQUENCE** RISKS K13; the dashboard must stay top-app during a plan (already required for cpusets).
