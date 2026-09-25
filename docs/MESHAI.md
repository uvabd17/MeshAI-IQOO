# MeshAI — the one document

*Single source of truth for design, decisions, risks, measurements, status and how to run. Everything below was either executed on this laptop and the POCO F5 on 24 Sep 2026 or is marked as estimate / not executed. Generated from the repo state at 2026-09-24 15:42 IST; the machine-readable ledgers stay in `state/` (tasks, tests, progress, blockers).*

Branch: `native-mesh-v2` on GitHub (uvabd17/MeshAI-IQOO). Older PDFs/decks are in `docs/archive/` and are superseded by this file.

## 0. Start here — project status on 25 Sep 2026 (evening, paused)

**What MeshAI is today.** A laptop service (`meshd`, Rust) plus an Android app pair a laptop with phones and serve one local OpenAI-compatible endpoint. A model that fits one device runs there; a model that doesn't is cut into whole layers and spread over devices with llama.cpp RPC. The admin panel walks a user through four locked steps: **Devices → Models → Prepare → Use**.

**Works and was run on real hardware**
- Pairing over USB (one click) or QR; the phone reconnects on its own.
- Phone as host: Qwen3-0.6B at 28.8 tok/s.
- Laptop + phone layer split with correct answers: 1.2–1.4 tok/s over the USB-debugging cable, 7.1–8.9 tok/s over home Wi-Fi (6 of 28 layers on the phone, 25 Sep).
- Laptop alone: Qwen3-8B at 2.9–3.6 tok/s; warm reload 4.6–7.6 s, cold 90 s.
- Image chat (Qwen2.5-VL-3B): 150 s per screenshot. Speech-to-text (whisper.cpp base.en): 1.5 s for 11 s of audio.
- Calculate predicts time per request; on the laptop alone it matched measurement within 3 % (a replay calibrated from the same model).

**Built and tested on the laptop, not yet run on a phone** (task T100 closes these)
- The guided four-step panel, model categories (easy / hard but doable / not possible), advice, readiness checklist, recovery banners, "Start again" after a restart.
- USB care on pairing (background allowed, doze exemption, stay awake while plugged).
- Phone app: cached-layer reporting, reuse of a model already on the phone, keep-alive card, single control connection.

**Known open items**
- **The speed gate (T007)** is not met: we have not yet split a model that fits *neither* device at ≥ 3 tok/s. Needs USB tethering or a hotspot (T004) and a bigger model.
- Two reviewer fixes were in progress when work stopped (25 Sep): (a) new run rows from **simulated** phones are still labelled "cable" — past rows were relabelled by hand, the code fix in `state.rs::current_link_kind` / `calc.rs::link_input` is not done; (b) `POST /api/calculate` does not validate the model name (use it only on a trusted link); (c) USB care writes `stay_on_while_plugged_in 7` then `svc power stayon usb` overwrites it with 2; (d) the phone's `ControlClient.connect()` can still start two connections on a double tap (needs a mutex). All listed with file names in the reviewer audit under §20 and in `state/tasks.json` T097/T098/T100.
- Image generation (SD 1.5: 18 min per image) and speech out (Qwen3-TTS: 42 s, 8 GB) are too slow here — not demo items. Video: not feasible on this hardware.
- Universal adapter (speech, images, route table): designed as D034, not built (T081–T091).
- Windows never run on a Windows machine. Oracle mirror not deployed (needs approval).

**How to pick it up**
1. `cargo build --manifest-path desktop/Cargo.toml` then `MESHAI_API_TOKEN=secret desktop/target/debug/meshd serve --lan`; open `http://localhost:8080/admin/?token=secret`.
2. Build the app: `gradle -p android assembleDebug` (on the original dev laptop use the installed Gradle 8.14.3 wrapper distribution); install it; plug the phone in with USB debugging; press *Pair over USB*.
3. `MESH_TOKEN=secret scripts/demo-check.sh --phone` tells you in one screen whether the laptop and phone are ready.
4. Next work, in order: T100 (phone verification) → finish the four reviewer fixes above → T004/T007 over USB tethering → T078 split predictions → D034 slices.
5. On the original dev laptop the root disk filled up; `desktop/target` and `third_party` are symlinks into `/mnt/storage/meshai/build` (both git-ignored).

Where things are: design §10, decisions §11 (D001–D035), every measured number §12, demo runbook §18, universal-adapter design §19, product-flow audit §20, tasks `state/tasks.json`, day-by-day log `state/progress.md`, screenshots `docs/screenshots/`.

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
| Laptop+phone layer split | **works and is correct**; 1.2–1.4 tok/s over the USB-debugging cable (adb relays every RPC round trip); **7.1–8.9 tok/s over home Wi-Fi on 25 Sep** (RTT p95 4.9 ms that day; 6 of 28 layers on the phone, forced split) | measured; ≥3 tok/s reached over Wi-Fi on a *forced* split of a model that fits the laptop alone — the T007 gate (a model that fits neither device) is still open; link stability (phone went offline on the next run) is the open risk; tethering still unmeasured |
| Image chat | **works** on the laptop (Qwen2.5-VL-3B + projector described a real screenshot) | measured, 150 s per image on this CPU |
| Windows | code is cross-platform, scripts written, **not executed on Windows** | `cargo check --target x86_64-pc-windows-gnu` passed before the TLS fix; since `rustls-tls` (ring) it needs `mingw-w64` on the Linux box, which is not installed, so the check is skipped (verify says so) |
| Oracle mirror | scripts ready, **not deployed** (needs approval) | deploy/oracle |

Tasks: 12 done, 5 active, 15 pending (section 9). **Proposed next step (research + design, not built): section 17 — three model picks, layer rules, a Calculate step that predicts time and compute per request, and a three-step UI.**

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

*Note (25 Sep 2026): wherever this section says a device must hold "weights + KV", read "weights + KV + a compute reserve (estimate), plus the projector on the host" — D033.*

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

## 11. Decisions (D001–D035, newest wins; D032 and D034 are proposed, D033 and D035 are in force)

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

*Amended by D033 (25 Sep 2026): fixed compute reserves are now budgeted as estimates until the load-log measurement replaces them.*

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

#### D033 — Fixed compute reserves per device and the projector on the host, in force now (amends D016; ahead of D032)
2026-09-25 · **Context:** the reviewer's audit of the demo-safety change found the planner admitting devices with zero slack: D016 recorded that llama.cpp compute buffers were not modelled, the vision projector (1.34 GB f16) was never budgeted anywhere, and for tied-embedding models the output head landed on the last worker unbudgeted. A demo must never OOM-kill a phone mid-answer. **Decision:** every device must hold weights + KV **plus a fixed compute reserve: 300 MB for the device running llama-server, 150 MB for a worker** — both **estimates** (basis: the 0.6B run held ≈58 MB of non-KV anonymous memory and llama.cpp reported ≈25–30 MiB compute buffers, so these are deliberately generous), shown in MB and labelled "(estimate)" wherever a user sees them, and included in "needs X" totals and in the does-not-fit error. The host additionally budgets the projector file's size when `--mmproj` will be passed (`Policy::host_extra_bytes`). Eligibility requires one layer + the worker reserve. The replacement-plan credit (D024) may cover the placement's bytes plus its reserve, never more than what is really held. `llama-server` gets `--fit off`, `--no-mmproj-offload` and the pin `^(output|output_norm|token_embd)\.(weight|bias)$=CPU` on both the laptop and the phone host path (D018 sync). **Replaced by:** the load-log measurement in T073 (per-device "compute buffer size" lines, ±5 % placement check) once built. **Status:** in force; the arguments must still be executed once on hardware (T079) before this counts as DONE.

#### D032 — PROPOSED: a three-step flow (Devices → Model → Calculate & Run) on a GGUF layer config and a calibrated cost model (would supersede D030's page layout)
2026-09-24 · **Context:** the product owner asked for an explicit, simpler flow that shows, before running, which device holds which layers, what each stack costs per token, and whether the mesh can run the model at all. Today the panel guesses fit in JavaScript (`file_bytes × 1.15 + 0.2 GB`), the planner has no time model, the single-device choice compares bench tok/s measured on different models, and the "adb RTT" of 2–5 ms is a TCP connect to adbd on the phone's own loopback while the measured adb split spends ≈0.72 s per token on the link. Reading the pinned llama.cpp source showed three placement gaps (projector to RPC device 0 in a split, tied head to the last worker, `--fit` on by default). **Decision (proposed):** (1) meshcore derives a LayerConfig per GGUF (units: token embedding · block i = attention + FFN/experts + its KV · output norm + head · vision encoder + projector; bytes, read bytes and FLOPs per token, KV per token; constraints tagged engine vs policy) served at `GET /api/models/{file}/layers`; (2) `POST /api/calculate` returns the unchanged planner's plan plus a cost model (device ms/token = bytes read ÷ measured bandwidth; prefill = FLOPs ÷ measured compute; link = α + activations ÷ rate, α from a `llama-bench --rpc` probe refined by every split request), verdicts per device and for the mesh, alternatives not chosen, and a provenance tag on every number; (3) `POST /api/run {calc_id}` runs exactly the calculated plan or answers 409; (4) the panel becomes three steps with chat after Run, and phones get a read-only `MeshView` that carries no prompts or answers by schema; (5) `llama-server` gets `--fit off`, `--no-mmproj-offload` and `-ot ^(output|output_norm|token_embd)\.weight$=CPU`, and per-device buffer sizes from the load log are checked against the plan (±5 %). **Rejected:** estimating from datasheets and RTT (the link term is already ≈50× off on adb); loading the plan on every Calculate (minutes per what-if, cannot explain a plan that does not fit); history only (no answer for new combinations); a WebView of the admin API on the phone (needs `--lan` + token on the phone). **Status:** proposed — accept after T071 (α stable within 30 %) and T078 (predicted vs measured within 30 % on the POCO F5). Full design in §17.

#### D034 — PROPOSED: MeshAI becomes a multi-engine adapter through a Bundle Plan (one plan id, several engine processes), compiled-in engine definitions, a unit-based planner and a /v1 route table
2026-09-25 · **Context:** the product owner wants one local OpenAI-compatible endpoint that also serves speech-to-text, text-to-speech, image generation and embeddings with the same pairing, planner, supervisor, credited caps and three-step UI. Today everything assumes one llama.cpp run: one generation with one host and one local worker; the proxy forwards every `/v1/*` path to one endpoint; the API guard answers 415 to the multipart bodies OpenAI clients send for transcription; the catalog, scan and phone fetch accept only `.gguf`; the phone's ProcessGate owns one child. Upstream (read 25 Sep 2026, not executed): sd.cpp places components per device (`--backend te=…,vae=…,diffusion=…` + `--rpc-servers`, RPC merged Jun 2026) but its RPC server must be built from the llama.cpp commit in its `sync-llama.last` with `GGML_MAX_NAME=160`, so our phone's `ggml-rpc-server` (66fba63) is not an sd.cpp worker; `whisper-server` serves multipart with `--inference-path`, exposes `/load`, uses ggml `.bin` models and documents no health endpoint; `llama-server` has no audio endpoints but accepts `input_audio` (Qwen3-ASR among the listed models); `llama-tts` is CLI-only. Measured: laptop 11–12.3 GB usable, phone 0.6–2.0 GB, reloads 2.0 s (0.6B) to 48.8 s (VL-3B). **Decision (proposed):** (1) meshcore holds a compiled-in `Engine` table (id, pinned commit, tasks, routes, modalities, server/worker binaries per platform, `rpc_abi`, unit kind Layers | Components | Whole, lifecycle Resident | PerRequest, readiness probe, reserves marked estimates, one pure argv function per engine — today's `derive_args` becomes the llama one); catalog entries bind file sets to an engine by id only, never argument templates from disk or the API. (2) Every engine yields a `UnitList` (PinnedToHost / MustStayTogether / MaySplitContiguous / MayMove, tagged engine vs policy); `plan_bundle` finds the smallest device set that holds all units — whole units, then components (exhaustive over ≤4 units), then the unchanged D023 greedy layers; a single chat workload yields exactly today's plan. (3) A session is one Bundle Plan (chat + optional voice in, voice out, draw, embeddings) with one `plan_id` and one generation; all processes start and stop together; any exit ends the run (D025); the D024 credit becomes per device min(Σ placement bytes + reserves, Σ held RSS of the run's children). (4) meshd routes `/v1` by path and `model` (catalog id → file name → OpenAI alias → the unique live workload), answers `/v1/models` itself, returns 404 for unknown paths (whisper's `/load`, sd's native API never reachable); multipart only on the audio routes and only with `Authorization` or `x-mesh-token` present (CSRF defence keeping D021's intent); TTS text goes to engines through a file, never argv. (5) Proto grows additively: `Plan.schema/workloads/assignments`, `LaunchSpec` (allow-listed binary + placeholder argv, no shell), `DeviceProfile.engines/max_procs`; devices without `engines` receive only legacy chat placements. (6) The demo ships laptop-local add-ons only, with no proto or Android change; phones stay at one process until a slot-keyed gate exists. **Rejected:** one engine at a time (every voice turn pays 2–3 model loads; revisit only if a warm 8B reload measures ≤ 2 s); independent concurrent runs (multiplies the D024–D026 state machine and the phone cannot host concurrent processes anyway); linking engines into meshd (compute inside the coordinator, no RSS attribution, no phone reuse); JSON engine manifests from disk/API (command-execution surface under `--lan`); unifying all ggml builds on sd.cpp's pin; placing an add-on on the phone "for speed" (unmeasured; forbidden by "don't split if it fits"). **Consequences:** §19; risks K32–K39; tasks T080–T091; reviewer required for the guard exception, `plan_bundle` and `LaunchSpec`. **Status:** proposed — T080 so far (25 Sep): warm Qwen3-8B reload 4.6–7.6 s (> 2 s ✓, option A rejected); whisper base.en RTF 0.13–0.15 alone (✓ pending the co-residency check with the chat model resident); SD 1.5 measured 1084 s per image (≫ 180 s) → draw is out of the demo; Qwen3-TTS via llama-tts measured 42 s / 8 GB for 5 s of speech → TTS only via a small ONNX voice (Piper/Kokoro) or dropped.

#### D035 — A gated four-step flow (Devices → Models → Prepare → Use), USB care on pairing, link kinds on every run row (supersedes D030's page layout; D032's three steps become four)
2026-09-25 · **Context:** the product owner wants a flow a first-time user can follow without reading anything, that opens the next step only when the previous one is satisfied, tells them who hosts and who helps, sorts models into easy / hard-but-doable / not possible with the concrete change that would make one possible, prepares (download or reuse, push, readiness) before use, recovers in plain words, and switches on whatever phone controls we can reach over USB. The reviewer also found link costs being derived from history rows whose link kind was unknown. **Decision:** (1) the panel has four steps with lock reasons; Models, Prepare and Use unlock only after a phone is paired online or the user chooses "this laptop only"; Prepare unlocks after a model is chosen; Use when the run is ready; the server remains the gate (`/v1` needs a live run). (2) Device cards carry a role chip (HOST / HELPER / NOT USABLE + reason) and an engine chip; `/api/state.advice` carries per-device advice (plug in, close apps, keep the app in front, cable is slow → tethering). (3) `POST /api/feasibility` classifies every catalog model and on-disk file from the credited capacities alone (easy = one device with the D033 reserve; hard = only split, only at 2k, only after freeing N MB on device X, or only with one more device; impossible = not even pooled at 2k) with off-disk entries explicitly estimated. (4) *Pair over USB* applies phone care over adb (doze whitelist, background allowed, stay awake while plugged in — a persistent global setting, shown to the user) and never touches USB mode (K20); serials must be in `adb devices`; emulators are refused. (5) The last accepted run is persisted (`state/last_run.json`) and offered as "Start again"; recovery banners map engine and link errors to one sentence and one action. (6) Every `RunRow` records its link kind (local / cable / lan / loopback; simulated devices are loopback, never cable) and the cost model derives per-kind link costs only from rows of that kind; historical rows were backfilled from date, device count and speed. (7) The phone's RPC worker keeps its tensor cache under the app's files dir (`LLAMA_CACHE`), reports its size as a separate JobProgress (`job_id = "cache"`, so the `listening:host:port` note stays parseable) and verifies an existing host model by sha256 when the plan carries one, else by size. **Rejected:** gating in the client only (the server already refuses; the client gate is for guidance, not security); applying care silently (it is shown and listed as persistent). **Consequences:** admin four steps; meshd feasibility/advice/care/last_run; Android cache, verification and keep-alive card; tasks T095–T100; §20 audit. **Status:** in force for the laptop side (executed headless + live curl); the phone side is built and unit-tested but **not yet run on a phone** (T100).

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
| 2026-09-25 12:44 | llama-server args check (T079/V018): Qwen3-0.6B Q8_0 split, laptop host + local ggml-rpc-server (2 threads), `--fit off`, regex head pin, -ngl 15, ctx 2048 | flags accepted; tied head + output_norm pinned to CPU (log 'buffer type overridden to CPU'); layers 0–13 CPU / 14–27 RPC0; model buffers CPU 380.90 MiB, RPC0 223.25 MiB; KV 112 MiB each; compute buffers CPU 28.01 MiB, RPC0 24.01 MiB | executed once, no chat timing |
| 2026-09-25 12:49 | **Laptop + POCO F5 split over home Wi-Fi** (phone 192.168.1.4 ↔ laptop 192.168.1.14, RTT p95 4.9 ms today), Qwen3-0.6B Q8_0, ctx 2048, laptop capped to 1.04 GB so the phone takes layers 22–27 (6 of 28), host = laptop, old app build (pre link-fix), scripts/qa/alpha-experiment.sh | ready in 48.9 s; chat 1: **7.09 tok/s**, first word 598 ms, 24 tokens, prompt 30 tokens @ 50.1 tok/s; chat 2: **8.86 tok/s**, first word 293 ms, 31 tokens, prompt @ 37.5 tok/s | first split ≥ 3 tok/s (T007 speed criterion met over Wi-Fi with a forced split; the model itself fits the laptop alone) |
| 2026-09-25 12:52 | same, phone share 12 layers (laptop capped to 0.89 GB) | **not ready after 240 s**: the phone went offline during bring-up (old app, MIUI, screen state unknown) — run stopped by meshd | link stability, not speed, is the open risk |
| 2026-09-25 12:59 | **Speech-to-text, whisper.cpp** (commit d09f61a, 24 Sep 2026; host build, AVX2/AVX-512, 4 threads), model ggml-base.en.bin (148 MB), clip samples/jfk.wav (11 s), laptop only, panel engineer's cargo build running concurrently | whisper-bench: load 124 ms, encode 1011 ms; whisper-cli: load 130 ms, encode 1070 ms, decode 11 ms, total **1.70 s for 11 s of audio (RTF 0.15)**, peak RSS 291 MB; whisper-server (`--inference-path /v1/audio/transcriptions`): listening after 245 ms, multipart transcription **1.48 s** (RTF 0.13, same on a second request), RSS 243 MB, response `{"text": …}` (OpenAI shape) | T080(b): base.en is demo-viable on the laptop; readiness = listens almost immediately (model loads in ≈0.13 s from page cache), so a warm-up request before 'ready' is cheap |
| 2026-09-25 13:09 | **Qwen3-8B Q4_K_M reload ×3 through meshd** (laptop alone, ctx 4096, HDD, page cache cold for the first load; sd.cpp build running niced in the background), one 52-token interview prompt each, 26 tokens out (temperature 0) | load 1 (cold): **90.0 s** ready; load 2: **7.6 s**; load 3: **4.6 s**. Decode 2.93 / 3.23 / 3.62 tok/s; first word 2.60 / 2.39 / 2.56 s; prompt 20–22 tok/s. Same question each time, identical answer | T080(a): a warm reload is 4.6–7.6 s ≫ 2 s, so per-turn engine swapping (D034 option A) is rejected; the bundle plan (C) stands. Cold start needs the D011 warm-up before a demo |
| 2026-09-25 13:11 | **Co-residency check (inconclusive)**: Qwen3-8B loaded (warm, 4.0 s) while the stable-diffusion.cpp build ran niced in the background; two questions with whisper-server absent, two with it resident and idle (base.en, RSS 187 MB); then one transcription of jfk.wav with the 8B still loaded | decode 2.40 / 2.23 tok/s absent vs 2.20 / 1.64 tok/s resident; transcription 4.53 s (vs 1.48 s alone earlier); MemAvailable 8.6 GB | the concurrent build makes both numbers unreliable (the same 8B decoded at 2.9–3.6 tok/s minutes earlier); repeat on a quiet machine before deciding the D034 co-residency gate |
| 2026-09-25 13:12 | **Qwen3-ASR-0.6B Q8_0 through llama-server** (66fba63, `--mmproj mmproj-Qwen3-ASR-0.6B-Q8_0.gguf --no-mmproj-offload`, ctx 4096, 4 threads, laptop alone, sd.cpp build running niced), OpenAI chat request with an `input_audio` part (jfk.wav, 11 s) | ready in 15.2 s, RSS 1.50 GB; transcription **11.3 s** (RTF ≈1.0; second request 11.1 s), 172 prompt tokens (audio) + 30 out, text correct with a `language English<asr_text>` prefix; llama.cpp warns 'audio input is in experimental stage' | T080(c): works at our pinned commit; 7× slower and 6× the memory of whisper base.en — the stretch option, not the demo pick |
| 2026-09-25 13:45 | **Image generation, stable-diffusion.cpp** (b167b94, 25 Sep 2026; host build, 4 threads), SD 1.5 Q8_0 (1.76 GB GGUF), 512×512, 20 steps, seed 42, laptop alone (TTS download running concurrently) | params in RAM 1.68 GB (text encoders 125 MB, diffusion 1.40 GB); sampling **1020.6 s**, wall **1084 s** (18 min); peak RSS **3.41 GB**; a correct 565 KB PNG | T080(e): far above the 180 s gate — draw is NOT a demo item on this laptop; sd-server has `--backend te=…,vae=…,diffusion=…`, `--rpc-servers`, `--split-mode layer|row`, `--params-backend`, `--max-vram` |
| 2026-09-25 13:45 | **Text-to-speech, llama-tts** (66fba63) with Qwen3-TTS-12Hz-1.7B-Base Q4_K_M + its Q8_0 projector, 18-word sentence, 4 threads, laptop alone | **42.1 s** wall for **5.28 s** of 24 kHz audio (0.32× real time, 6.8–7.4 frames/s), peak RSS **8.06 GB**, 'audio input is in experimental stage' warning | T080(d): not demo-safe (memory and speed); TTS for the demo must come from Piper/Kokoro via sherpa-onnx (not built yet) or be dropped |
| 2026-09-25 13:0x | **Prediction vs measurement, laptop alone** (T078 data points; calibration = the same model's own earlier run on the same device, i.e. a replay, not an independent forecast) | Qwen3-8B, 52-token prompt / 26 out: predicted 3.59 tok/s, TTFT 2.4 s, total 9.3 s (derived) vs measured 3.62 tok/s, 2.56 s, ≈9.5 s; earlier: predicted 2.97 vs measured 3.05 tok/s | within 3 % on a replay; split predictions and the phone are still untested |

---

## 13. Windows

*Open item (25 Sep 2026): `scripts/windows/setup.ps1` pins llama.cpp release b6537; meshd now passes `--fit off`, which that release may not know (the flag exists in the pinned commit 66fba63 used on Linux). Verify or bump the tag before a Windows run.*

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

## 15. Risks (K01–K40)

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
| K23 | Cost-model estimates are wrong: link cost α may vary with layer count or context, phone bandwidth measured on a 0.6B may not carry to 8B-sized blocks, prefill efficiency depends on prompt length | Medium | High | every value shows its source and band; history shown beside the prediction; gates T071/T078; "verify by loading" as fallback |
| K24 | The adb (USB-debugging) link is too slow: every split over the cable is predicted at ≈1 tok/s | High | High | verdict "slow"; step 1 tells the user to turn on USB tethering from the phone's Settings (never from adb, K20) |
| K25 | MoE models: blocks are large (≈380–450 MB), so a 2 GB phone holds 4; "read only k/E of the experts per token" is unverified on CPU with mmap; prefill touches all experts; gpt-oss uses sliding-window layers and MXFP4 | Medium | Medium | measure gpt-oss-20b on the laptop alone first; flag as unverified in the UI |
| K26 | Compute buffers or the tied head over-commit a device (today's planner ignores both) | Medium | High | budget a compute buffer per device; read the real one from the load log; 5 % placement check |
| K27 | Memory changes between Calculate and Run | Medium | Medium | `calc_id` + 409; never swap silently |
| K28 | The phone partner view leaks prompts | Low | High | `MeshView` schema has no text fields beyond labels; privacy test; `Plan` stays the only actionable message |
| K29 | Once RTT is measured with a real echo, the 60 ms policy may reject adb or tethering | Medium | Medium | measure first (T070), then set the policy |
| K30 | Image generation: no CPU benchmark exists for `stable-diffusion.cpp`; FLUX peak memory is unknown; no component fits the POCO F5 | High | Medium | treat as a research spike: measure SD 1.5 on the laptop before promising anything |
| K31 | Turning on USB tethering from the phone's Settings may leave adb at "no permissions" on a Linux host (reported for Xiaomi phones; untested here), breaking one-click pairing mid-demo | Medium | Medium | pair first, then switch the link; udev reload + replug; laptop hotspot as the fallback that never touches USB |
| K32 | sd.cpp RPC ABI mismatch: its worker must be built from sd.cpp's pinned llama.cpp with tensor names of 160 chars; our phone worker (66fba63, 64) would corrupt transfers | High | High | `rpc_abi` matching in the planner; a separate static `libmeshai_rpc_sd.so`; local sd-ABI worker test (T088) |
| K33 | sd.cpp compute buffers unknown and growing with image size → OOM | High | High | planned size cap; read the load log (T080); refuse requests above the planned size |
| K34 | Allowing multipart on the audio routes reopens CSRF | Medium | High | require a non-simple header (`Authorization` or `x-mesh-token`) so a browser form fails the CORS preflight; reviewer |
| K35 | whisper-server readiness unknown → "ready" too early | Medium | Medium | measure in T080; warm-up request before ready |
| K36 | Per-request TTS pays a model load per call | Medium | Medium | measure in T080; resident server only if too slow |
| K37 | Toggling an add-on restarts the chat model (bundle = one generation) | Medium | Low | warm reload measured in T080; diff-based restart later (T091) |
| K38 | Old phone app misreads a multi-placement plan (`firstOrNull` in applyPlan) | Medium | High | legacy gating: devices without `engines` get only their chat placement |
| K39 | Browser audio is webm/opus, which whisper cannot read | High | Low | JS WAV encoder in the panel; documented formats; 415 otherwise |
| K40 | Video generation has no CPU timing anywhere; a clip may take hours on this laptop and no component fits the phone | High | Low | never in the demo; one timing spike (T094) before any claim |

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

#### R005 — Best chat models and the exact llama.cpp RPC placement unit            (2026-09-24)
**Question.** Which chat GGUFs fit laptop+1 / laptop+2 phones, and what exactly does RPC place where? **Evidence.** `config.json` of eight candidates (Qwen3 8B/14B/30B-A3B, Gemma 3 12B/27B, Llama-3.1-8B, Mistral-Small-3.2-24B, gpt-oss-20b, Phi-4) [source]; llama.cpp `docs/multi-gpu.md` ("each GPU holds a contiguous slice of layers; the KV cache for layer l lives on the device that owns layer l"; `split-mode row` deprecated), `tools/rpc/README.md` (`--tensor-split` follows `--device` order), `src/llama-quant.cpp` (output.weight → Q6_K under Q4_K_M), `ggml-rpc.cpp` (graph-level commands, tensor hash cache), issue #13314 (CPU gets the first layers), discussion #11784 (maintainer: "the default behaviour is to split the model by whole layers, so it doesn't slice the experts"), discussion #9136 (ggerganov: "only the hidden state is transferred after each layer, a few kB"). **Answer.** Placement unit = whole contiguous layer (attention + FFN + all experts + its KV); host prefix first, then one range per RPC device in list order — exactly our measured placement; MoE experts cannot be split across devices; per-token traffic = n_embd × 4 B; output head pinned by our own `-ot`. Model table and picks in §17.2. **Unknowns.** Laptop RAM channel count (Intel spec 58 GB/s vs the ≈14 GB/s effective we back out of 3.05 tok/s); phone i8mm prefill GFLOPS (needs a `llama-bench -p 512 -n 0` run); Gemma 3's exact sliding-window layer pattern; iQOO 15 usable memory. **Sources.** github.com/ggml-org/llama.cpp/blob/master/docs/multi-gpu.md · tools/rpc/README.md · issues/13314 · discussions/11784 · discussions/9136 · huggingface.co model cards for the file sizes.

#### R006 — Cost formulas and prior art (exo, prima.cpp, distributed-llama, cake, Petals)            (2026-09-24)
**Formulas** (Kipply "Transformer Inference Arithmetic", EleutherAI "Transformer Math 101"): forward FLOPs per token = 2 × active params; batch-1 decode is memory-bound: t = bytes read ÷ bandwidth; prefill is compute-bound: t = 2·P·params ÷ GFLOPS; KV per token per layer = 4 × n_kv_heads × head_dim bytes (f16); link per hop = RTT-dominated at KB payloads. **Prior art.** exo: topology-aware auto-parallel (pipeline + tensor), macOS/Linux only, `/instance/previews` shows memory deltas pre-run but no throughput prediction. prima.cpp: profiles compute/disk/memory, `-lw` manual layer windows, no pre-run display. distributed-llama: tensor-parallel, power-of-two node counts, no Android. **cake** (Rust/Candle): claims iOS/Android/macOS/Linux/Windows, mDNS discovery, VRAM/compute-weighted layer assignment, streams weights from the master — R001's "no direct Android competitor" needs this caveat (unverified on a device). Petals: throughput-weighted block assignment with rebalancing, no Android. **None shows a predicted time per request before running.** **Sources.** kipp.ly/transformer-inference-arithmetic · blog.eleuther.ai/transformer-math · github.com/exo-explore/exo · github.com/OpenCPIL/prima.cpp · github.com/b4rtaz/distributed-llama · github.com/evilsocket/cake · arxiv.org/abs/2312.08361.

#### R007 — Image generation on this hardware            (2026-09-24)
**Answer.** Only `stable-diffusion.cpp` (ggml, same `ggml-rpc` backend) is viable; its `docs/rpc.md` documents per-component placement ("RPC0 main backend, RPC1/RPC2 text encoders, RPC3 VAE"); the denoiser itself is one placement. Sizes: SD 1.5 q8_0 ≈2.1 GB; SDXL-Turbo Q4_0 ≈5.7 GB with encoders; SD3.5-medium Q4_0 ≈4.1 GB without T5; FLUX.1-schnell Q4_0 DiT 6.77 GB + T5 fp8 4.89 GB (its `flux.md` reports 6.4 GB total for q4_0 — probably peak RSS after freeing the encoder; must be measured). No CPU timing exists in the project's own docs (issue #15 unanswered since 2023). Android: official NDK recipe (API 28+, CPU + OpenCL `SD_OPENCL`), and the `rmatif/Local-Diffusion` app proves arm64 builds run (OpenCL limited to Adreno 7xx). **Sources.** github.com/leejet/stable-diffusion.cpp (docs/rpc.md, docs/flux.md, docs/build.md, docs/quantization_and_gguf.md) · huggingface.co/city96/FLUX.1-schnell-gguf · huggingface.co/calcuis/sd3.5-medium-gguf · github.com/rmatif/Local-Diffusion.

#### R008 — Document and image understanding via llama.cpp mtmd            (2026-09-24)
**Answer.** Qwen2.5-VL-3B (1.93 + 1.34 GB, 36 layers, DocVQA 93.9 / OCRBench 797, image tokens = H×W/784 — reproduces our measured 2,154-token prompt), Granite-Docling-258M (0.13 + 0.18 GB, document→markdown, open looping bug #16678), MiniCPM-V-4.5 (5.03 + ≈1.1 GB, DocVQA 94.7 / OCRBench 89.0, version-pin bug on some builds). Qwen3-VL has an open mtmd accuracy bug (#29251, active 21 Sep 2026). The projector is offloaded to the first GPU-type device by default (`--no-mmproj-offload` to keep it on CPU); `--mmproj-device` exists but RPC targets are unverified. No PDF path in mtmd: extract text when a text layer exists, otherwise rasterise pages. **Sources.** github.com/ggml-org/llama.cpp/blob/master/docs/multimodal.md · tools/server/README.md · issues/29251 · issues/16678 · arxiv.org/pdf/2502.13923 (Qwen2.5-VL report) · huggingface.co/ggml-org/Qwen2.5-VL-3B-Instruct-GGUF · huggingface.co/ggml-org/granite-docling-258M-GGUF · huggingface.co/openbmb/MiniCPM-V-4_5-gguf.

#### R009 — Demo link: USB tethering vs hotspot on HyperOS/Ubuntu            (2026-09-25)
**Answer.** USB tethering remains the best default (wired, no Wi-Fi power save or contention, charges the phone) but no RTT/throughput number exists for this pair yet (T004). New Linux-specific risk: enabling tethering from Settings can leave `adb devices` at "no permissions" on Linux hosts (Arch forum, Xiaomi device), distinct from K20; fix is udev reload + replug, or pair before switching. Android randomises hotspot gateway addresses since ~9–11: read `ip route`, do not assume 192.168.43.1. HyperOS keep-alive: Autostart, battery "No restrictions", lock in Recents, longest screen timeout; all reset by reboots/updates. Possible Android-15 carrier entitlement check can make the tethering toggle silently fail. Two SEO pages with precise but unsourced RTT numbers were found and excluded. **Sources.** developer.android.com/tools/adb (wireless debugging) · source.android.com/docs/core/data/tethering-data · bbs.archlinux.org/viewtopic.php?id=285500 · dontkillmyapp.com/xiaomi · wiki.gentoo.org/wiki/Android_USB_tethering · github.com/Mygod/VPNHotspot/discussions/537.

#### R010 — Open: a pinned tied head may be a second copy in memory            (2026-09-25)
Overriding `token_embd.weight` to CPU selects from the CPU buffer list that includes repack buffers (`llama-model-loader.cpp` ~1240); for tied-embedding models the pinned head may therefore be a separate anonymous copy that `non_layer_bytes` counts once. Small for the 0.6B, large for 262k-vocabulary models; also affects the D024 credit. Check in T070 with a verbose load log. Windows tag b6537 vs `--fit`: see §13.

#### R011 — Speech in and out: STT/TTS engines for this hardware            (2026-09-25)
**Answer.** *STT:* whisper.cpp (ggml, same core as llama.cpp): tiny 75 MB → large-v3-turbo-q5_0 547 MB on disk, 273 MB → 3.9 GB resident; every size fits the laptop alone, so it is never split ("don't split if it fits"); `whisper-cli` has no `--rpc` and the stock `whisper-server` serves `POST /inference` (multipart), **not** `/v1/audio/transcriptions`, so meshd must own that route. No RTF number exists for a Snapdragon 7-series or this i5 (measure with `whisper-bench`). Stretch: audio-input chat models served by `llama-server` through mtmd (Qwen3-ASR 0.6B/1.7B, Ultravox 1B, Qwen2.5-Omni 3B) — the encoder is pinned to the host like a vision projector and the language layers use the existing split; the request shape is `/v1/chat/completions` with an `input_audio` part. llama.cpp's docs mark Qwen2-Audio as poor; avoid. *TTS:* Piper (ONNX, tens of MB per voice, ×4.5–8 real-time on a 2-core ARM cloud CPU in an independent benchmark) is the safe pick; Kokoro-82M (86–326 MB ONNX, ×0.9 real-time on the same weak cores) the better-quality option; llama.cpp's `tools/tts` now documents only Qwen3-TTS (1.04 GB Q4) and Pocket-TTS after a breaking change on 4 Aug 2026 (OuteTTS status unknown) and has no HTTP server; Orpheus needs ≈8 GB; Dia/Zonos/Fish are GPU-only by their own words. Every local `/v1/audio/speech` server found is Python (openedai-speech, speaches, voicebox, Kokoro-FastAPI), so meshd adds the route itself and shells out. On Android, Piper voices run inside sherpa-onnx (AAR/JNI, official arm64 builds); whisper.cpp has an official NDK example. **Recommendation for the demo:** whisper.cpp `base.en` on the laptop behind a meshd-owned `/v1/audio/transcriptions`, Piper on the laptop behind `/v1/audio/speech`; stretch Qwen3-ASR through llama-server. **Sources.** github.com/ggml-org/whisper.cpp (README, models/README.md, examples/server/README.md, examples/cli/README.md, issue #89) · llama.cpp docs/multimodal.md and tools/server/README.md · tools/tts commits (2026-08-04) · huggingface.co/ggml-org/Qwen3-ASR-0.6B-GGUF · github.com/OHF-Voice/piper1-gpl · huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX · github.com/obole-ia/tts-cpu-benchmark · github.com/k2-fsa/sherpa-onnx.

#### R012 — Universal adapters: how LocalAI, llama-swap, Ollama, koboldcpp and others do it            (2026-09-25)
**Answer.** None of LocalAI, llama-swap, Ollama, Jan/Cortex, koboldcpp, LM Studio, ramalama or text-generation-webui places one model across two machines; MeshAI's layer split stays the differentiator. Closest cousins: llama-swap (a proxy that spawns/kills one child process per model by a `cmd` string, `ttl` idle unload, `groups` for concurrent/exclusive sets — no memory accounting at all) for the supervisor shape, and LocalAI's gRPC `backend.proto` (LoadModel/Predict/Embedding/Rerank/GenerateImage/AudioTranscription/TTS/…) for the engine abstraction; LocalAI's model YAML vocabulary `known_usecases` (chat, completion, embeddings, rerank, image, transcript, tts, video) is worth copying. koboldcpp proves the combination llama.cpp + sd.cpp + whisper + TTS behind one OpenAI surface is normal (one process, no isolation). llama-server already serves `/v1/embeddings`, `/rerank`, `/v1/responses`, `/v1/messages` and audio/video *input* parts; it has no `/v1/audio/*` or `/v1/images/*`. stable-diffusion.cpp's server serves OpenAI `/v1/images/generations` and `/edits` (b64_json), A1111 `/sdapi/v1/*` and an async `/sdcpp/v1/*` with video; **its ggml-RPC backend support merged 14 Jun 2026 (PR #1629, after PR #1184)** — the borrowed data plane is ggml-wide, not llama.cpp-only. *Correction to §17.2:* the `docs/rpc.md` pointer could not be found; cite the merged PR and re-derive the current flag names (`--rpc`, `--*-backend-device`) from source. Gaps in our own code: `proxy.rs` forwards every `/v1/*` to whatever is ready (breaks with two engines) and records a benchmark row only for chat routes. Android: whisper.cpp and sd.cpp have official NDK CLI builds (child-process friendly, D010); sherpa-onnx is an AAR/JNI library; Piper on Android = sherpa-onnx. D024's credited-caps replacement logic has no prior-art equivalent. **Sources.** github.com/mudler/LocalAI (model-configuration.md, backend.proto, openai-realtime docs) · github.com/mostlygeek/llama-swap (wiki/Configuration) · ollama.readthedocs.io/en/modelfile · github.com/LostRuins/koboldcpp · github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md · raw.githubusercontent.com/leejet/stable-diffusion.cpp/master/examples/server/api.md · github.com/leejet/stable-diffusion.cpp/pull/1184 and /pull/1629 · raw.githubusercontent.com/leejet/stable-diffusion.cpp/master/docs/build.md · github.com/k2-fsa/sherpa-onnx.

#### R013 — Video generation on this hardware            (2026-09-25)
**Answer.** stable-diffusion.cpp now lists video models (Wan 2.1/2.2, LTX-2.3/2.5, HunyuanVideo 1.5, MiniMax-H3, LingBot) but publishes **no timing at all** (its `performance.md` has none; issue #15 "Benchmark?" is unanswered since 2023). Smallest viable pick: Wan2.1 T2V-1.3B — DiT Q4_K_M 0.98 GB + UMT5-XXL encoder Q4_K_S 3.5 GB + VAE (unquantised, "requires really much VRAM" per its own doc) ≈ 5–6 GB → fits the laptop alone; everything larger (Wan2.2-5B, LTX-2 22B with a Gemma-3-12B encoder, HunyuanVideo with Qwen2.5-VL-7B) exceeds the laptop's budget. The only real timing found is a GPU anecdote (RTX 4060 laptop, ≈400 s per step, hours per clip, possibly misconfigured). CPU-only estimate here: "tens of minutes to several hours" per clip — too wide to promise. No component fits the POCO F5, and moving a VAE off a model that already fits gains nothing. **Verdict:** not a demo item; at most an overnight batch job with Wan2.1 1.3B on the laptop after one timing spike (T094). **Sources.** github.com/leejet/stable-diffusion.cpp (README video section, docs/wan.md, docs/ltx2.md, docs/hunyuan_video.md, docs/performance.md, issues/15) · huggingface.co/samuelchristlie/Wan2.1-T2V-1.3B-GGUF · huggingface.co/city96/umt5-xxl-encoder-gguf · github.com/Wan-Video/Wan2.1/issues/555.

#### R014 — Layer-split state of the art, checked against our pinned llama.cpp (66fba63)            (2026-09-25)
**Already in our tree (grep of third_party/llama.cpp):** async RPC dispatcher + `send_async` (PR #18626, merged 26 Aug 2026), `RPC_CMD_GRAPH_RECOMPUTE` graph caching, the graph_recompute UAF fix (PR #24292, 16 Sep 2026 — commit date implies included; verify), `--spec-draft-device` (draft model on an RPC device), `--cache-type-k/v`, `--slot-save-path` and `/slots` save/restore, `--fit`, `--split-mode tensor` (flag only: **CUDA-only**, explicitly unsupported on RPC and CPU, PR #19378). Not built upstream: disaggregated prefill/decode (ggerganov's roadmap issue #21266, open); CPU-last device order (#13314, closed stale); small-message coalescing (PR #24122, unmerged draft; its +18 % prompt / +1.4 % decode are self-reported on other hardware). **Confirmed working by a maintainer (issue #23982, 22 Jun 2026):** speculative decoding with the draft model on an RPC device: `llama-server … --rpc <phone> --spec-draft-model <gguf> --spec-draft-device RPC0`. **Prior art:** prima.cpp (piped-ring + Halda scheduler, overlaps disk I/O with compute; Termux only), exo (MLX → Apple only), distributed-llama (tensor-parallel, no Android), cake (Candle; Android claim unverified on any device), EdgeShard (DP partition), TPI-LLM (tensor-parallel with a star all-reduce because link *latency*, not bandwidth, is the bottleneck — matches our RTT-bound finding), PipeLLM (sequence slicing fills pipeline bubbles). None of them ships cross-device speculative decoding; llama.cpp does. **Three cheapest wins:** (1) draft model on the phone (Qwen3-0.6B drafting for Qwen3-8B/14B) over tethering — the phone becomes a speed asset for models that fit the laptop alone; expected 1.5–3× from the general literature, unmeasured for this pair (T092); (2) re-run the existing split benchmark: the async/graph-cache work is already in the binary (T004 covers it); (3) `--cache-type-k q8_0 --cache-type-v q8_0 -fa on` on phone workers halves KV bytes per layer — a memory lever that can turn "needs two phones" into "one phone" (T093). **Rejected:** `--split-mode tensor` (no RPC/CPU support), `-nkvo` on RPC workers (conflicting semantics, untested), DIY prefill/decode via slot save (needs full weights on both devices). **Sources.** github.com/ggml-org/llama.cpp pull/18626 · pull/24292 · pull/24122 · pull/19378 · issues/13314 · issues/13083 · issues/21266 · issues/23982 · docs/speculative.md · arxiv.org/abs/2504.08791 · arxiv.org/abs/2410.00531 · arxiv.org/abs/2405.14371 · arxiv.org/abs/2512.16273 · local grep of third_party/llama.cpp at 66fba63.

#### R015 — Open notes from the 25 Sep reviewer audit            (2026-09-25)
The phone's RPC tensor cache moved to `<filesDir>/rpc-cache/rpc` (via `LLAMA_CACHE`); the previous default `<filesDir>/.cache/llama.cpp/rpc` may hold orphaned files and there is no size limit or eviction on either — add a cap and a 'clear cache' control (T100 follow-up). meshd fills `Plan.model_sha` only when the scan or a download recorded a hash; otherwise the phone verifies a reused model by size alone. Whether a re-run reuses cached layers (llama.cpp's hash cache across `ggml-rpc-server` restarts) has not been measured.

---

## 17. Next step (proposed): three model picks, layer rules, the Calculate step and a three-step UI

*Requested by the product owner on the evening of 24 Sep 2026. Sources: two research reports (web + upstream source, every link checked 24 Sep 2026) and one design memo that read the pinned llama.cpp tree (commit 66fba63). Status: **PROPOSED** as D032; nothing in this section is built. Tags: **[M]** measured in this repo, **[E]** estimate, **[E←M]** estimate derived from a named measurement, **[S]** read in llama.cpp source but not executed, **[?]** unknown.*

### 17.1 Measured tonight (24 Sep 2026, 21:30–22:30 IST)

| What | Result |
|---|---|
| Laptop free memory before | 2.6 GB of 19.5 GB — four idle Gradle/Kotlin build daemons held 10.7 GB. The planner refused every model ("host can hold only 0.7 GB") **[M]** |
| Laptop free memory after stopping the daemons | 14.0 GB free; planner offers **12.3 GB** usable **[M]** |
| Phone usable right now | **0.6 GB** = 2.07 GB available − 1.5 GB headroom, with the user's apps open. Battery 32 % **[M]** |
| Qwen3-8B Q4_K_M, planner dry-run with two simulated 12 GB phones | 36 layers, weights 5.0 GB, embeddings+output 0.9 GB, KV 0.6 GB at 4k → **≈116 MB per block, 4 KB KV per token per layer**; host keeps 0–5 + head, worker 6–35, second phone rejected as not needed **[M]** |
| Qwen2.5-VL-3B Q4_K_M, same dry-run | 36 layers, weights 1.9 GB, embeddings+output 0.3 GB → ≈45 MB per block, KV 0.2 GB at 4k; `--mmproj` appended **[M]** |
| Hugging Face CDN from this network | **1.1 MB/s** tonight vs 13 MB/s this afternoon; a 5 GB model would take over an hour **[M]** |
| Phone mirror | apt `scrcpy` 1.25 cannot drive Android 15 (missing `SurfaceControl.createDisplay`); release 3.3.3 works **[M]** |

### 17.2 The three model picks

**Chat.** Architecture numbers from each model's `config.json` [source]; per-block bytes derived from the published Q4_K_M size and llama.cpp's quant rules (±10–15 %, replace with the real GGUF tensor table once T072 exists) **[E]**.

| Model | File (Q4_K_M) | Layers | Bytes per block (resident) | Read per token (MoE: active experts only) | Fits where (laptop 11–12 GB usable) |
|---|---|---|---|---|---|
| Qwen3-8B | 5.03 GB | 36 | ≈116 MB **[M tonight]** | 116 MB | laptop alone — do not split |
| Qwen3-14B | 9.0 GB | 40 | ≈201 MB | 201 MB | laptop alone, marginal |
| Gemma 3 12B | 7.3 GB | 48 | ≈139 MB | 139 MB | laptop alone (sliding-window KV, smaller than the naive figure) |
| Phi-4 14B | 8.9 GB | 40 | ≈207 MB | 207 MB | laptop alone, marginal |
| **gpt-oss-20b** (MoE 32 experts, 4 active, MXFP4) | 12.1 GB | 24 | ≈451 MB | ≈77 MB | **laptop + 1 phone with ≥2 GB free** — the laptop+1 pick |
| **Mistral-Small-3.2-24B** | 14.3 GB | 40 | ≈338 MB | 338 MB | **laptop + 2 phones (≈5 GB pooled)** — the laptop+2 pick |
| Qwen3-30B-A3B (MoE 128 experts, 8 active) | 18.6 GB | 48 | ≈380 MB | ≈35 MB | needs an iQOO-15-class second phone |
| Gemma 3 27B | 16.5 GB | 62 | ≈252 MB | 252 MB | needs an iQOO-15-class second phone |

Picks: **laptop + 1 phone → gpt-oss-20b** (only fits split, cheap to decode because 4 of 32 experts are read per token), runner-up Qwen3-14B (fits alone, the "why bother splitting" foil), third Phi-4. **Laptop + 2 phones → Mistral-Small-3.2-24B**; Qwen3-30B-A3B and Gemma 3 27B only with the bigger loaner phone. Caveats: MoE blocks are big (380–450 MB), so a 2 GB phone holds only 4 of them; on the POCO F5 tonight (0.6 GB usable) none of these split at all, and with apps closed (~2 GB) it holds ≈13 blocks of an 8B-class model. Artificial Analysis (Sep 2026) ranks the newer Qwen3.5 and Gemma 4 families above all of these; they were outside the asked scope and are not researched.

**Image generation.** Engine: `stable-diffusion.cpp` (same ggml core; ggml-RPC backend support merged 14 Jun 2026 in PR #1629 — per-component placement of text encoders, DiT/UNet and VAE on RPC devices; the earlier `docs/rpc.md` pointer could not be verified, see R012). Split granularity is **per component**, never inside the denoiser: a UNet/DiT that does not fit one device cannot run at all.

| Model | All-in size | Steps | Splittable parts | Note |
|---|---|---|---|---|
| SD 1.5 Q8_0 | ≈2.1 GB | 20 | CLIP-L, UNet, VAE | safest; fits the laptop, no reason to split |
| SDXL-Turbo Q4_0 | ≈5.7 GB | 1–4 | CLIP-L, CLIP-G, UNet, VAE | better quality, still laptop alone |
| FLUX.1-schnell Q4_0 + T5 fp8 | ≈11–12 GB files (peak RSS **[?]**, `flux.md` says 6.4 GB for the same quant — must be measured) | 4 | text encoders (5.1 GB) / DiT (6.8 GB) / VAE | the only split-worthy one, but the encoder bundle alone is bigger than any phone here |

CPU time per image: **no `sd.cpp` CPU benchmark exists** (its `performance.md` has none); community PyTorch numbers put SD 1.5 at 1–2 min per 512×512 on an i5 → **[E] 30 s–3 min here, unmeasured**. Android arm64 builds exist (official NDK recipe, CPU + OpenCL for Adreno 7xx; Adreno 725 coverage **[?]**). Verdict: a research spike (measure SD 1.5 on the laptop first), not a demo commitment. No component fits the POCO F5's 0.6–2 GB except SD 1.5's CLIP/VAE.

**Image and document understanding** (llama.cpp `mtmd`, `--mmproj`).

| Model | GGUF + projector | LLM layers | Image tokens | DocVQA / OCRBench | Note |
|---|---|---|---|---|---|
| **Qwen2.5-VL-3B Q4_K_M** | 1.93 + 1.34 GB | 36 | H×W/784 (1280×1200 → 1,959; matches our measured 2,154-token prompt) | 93.9 / 797 | **already runs here: 150 s per screenshot [M]**; best-verified pick |
| **Granite-Docling-258M Q4_K_M** | 0.13 + 0.18 GB | small (Idefics3) | tiled | OmniDocBench (document→markdown/HTML), not VQA | tiny enough for the phone; purpose-built for tables/equations/layout; open looping bug llama.cpp #16678 |
| **MiniCPM-V-4.5 Q4_K_M** | 5.03 + ≈1.1 GB | n/c | n/c | 94.7 / 89.0 | best OCR numbers with a GGUF; must match llama.cpp build to the GGUF's minicpmv version |
| Qwen3-VL | GGUFs exist | — | — | — | **avoid for now**: open mtmd accuracy bug #29251 (active 21 Sep 2026) |
| Gemma 3 4B/12B, InternVL3, Pixtral, LFM2-VL | partial data | — | — | ambiguous or missing | fourth candidates once measured; LFM2-VL is absent from llama.cpp's supported-model list |

PDFs: if the PDF has a text layer, extract text (no model needed); otherwise rasterise pages at ~150 DPI and feed them as images. An A4 page at 150 DPI is ≈2,800 image tokens → **[E] ≥150–200 s per page** with Qwen2.5-VL-3B on this CPU, dominated by prefill. The vision encoder and projector always run in the host process by default; `--mmproj-device` exists upstream but its use with an RPC device is unverified. Only the language layers can move to a phone, exactly like a text model.

### 17.3 Layer rules: what can be combined with what

| Unit | GGUF tensors | Rule | Who imposes it |
|---|---|---|---|
| `embd` | `token_embd.weight` | pinned to the host | engine: "always keep it on the CPU" (`llama-model.cpp:1535`) **[S]** |
| `blk.i` | `blk.i.attn_*`, `blk.i.ffn_*` (dense) or router + expert tensors, **plus layer i's KV cache** | must stay together | attention+FFN: MeshAI policy (each extra cut is one more link crossing per token); all experts of a layer: engine (one tensor per projection); KV with its layer: engine default **[S]** |
| `blk.0 … blk.n-1` | ordered | contiguous ranges, host prefix first, one range per device in `--rpc` order | engine interface (`-ngl` offloads the last N of n_layer+1 entries, `--tensor-split` cuts by cumulative fraction) **[S]**, matches our measured placement CPU 0–3 / RPC0 4–14 / RPC1 15–27 **[M]** |
| `head` | `output_norm` + `output.weight` (tied models: the `token_embd` duplicate) | pinned to the host | policy: moving it would send n_vocab×4 B ≈ 608 KB of logits per token instead of a 16 KB activation |
| `vision` | the mmproj file (`v.*`, `mm.*`) | pinned to the host | policy, enforced by `--no-mmproj-offload` |

Hard engine limits: a tensor lives on exactly one device; experts cannot be spread across devices in layer mode (maintainer answer, discussion #11784); `split-mode row` is deprecated; at most 16 RPC servers; traffic between two workers goes through the host (star topology, `ggml-rpc.cpp:752`) **[S]**; changing placement means a full reload. Per-token link traffic is one activation per boundary, n_embd×4 B (8B model: 16 KB); prefill sends P×16 KB per boundary. The measured adb split is therefore RTT/relay-bound, not bandwidth-bound: ≈0.72 s per token on the link **[E←M]** for a 16 KB payload.

**Three placement gaps in today's `llama-server` arguments [S, verify on hardware = T070]:** (1) RPC devices register as GPU-type and the projector is offloaded to the first GPU device by default, so in a split with a vision model the 1.34 GB projector would land on the phone — add `--no-mmproj-offload`; (2) for tied-embedding models (Qwen3-0.6B) `-ot output\.weight=CPU` matches nothing, so the head goes to the last worker unbudgeted — pin `^(output|output_norm|token_embd)\.weight$=CPU`; (3) `--fit` is on by default and may adjust unset arguments — pass `--fit off` so the calculated plan is the plan that runs.

**Split-speed levers found later (R014, 25 Sep 2026):** the async RPC dispatcher and graph cache are already in our pinned llama.cpp; a draft model on the phone (`--spec-draft-device RPC0`) can speed up models that fit the laptop alone (T092); 8-bit KV on phone workers halves their KV memory (T093).

### 17.4 The Calculate step: compute and time per request

Per device d holding units S_d, with A = n_embd×4 B, P prompt tokens, N answer tokens:

```
memory fit      M_d = Σ weight bytes + n_ctx × Σ KV bytes/token + compute buffer  ≤  usable_d
decode ms/token t_d = max( bytes read per token / BW_d ,  FLOPs per token / G_d )      (CPU: the first term wins)
link per worker h_w = α_w + 2·A / β_w                                                   (α = fixed per-token cost, β = bulk rate)
token time      T   = Σ_d t_d + Σ_w h_w + ~2 ms        →  tok/s = 1000 / T
prefill         TTFT = Σ_d max( P × FLOPs_d / G_d , ⌈P/512⌉ × weights_d / BW_d ) + Σ_w (⌈P/512⌉·α_w + P·A/β_w)
answer          total = TTFT + (N−1) × T
MoE             bytes read per token = dense + experts × k/E  [E]; prefill reads all experts
vision          TTFT += encoder time (calibrated from the measured 142 s TTFT for 2,154 tokens) + prefill of the image tokens
```

Inputs and provenance: unit bytes from the GGUF (exact); usable memory from telemetry minus headroom (credited caps, D024) **[M]**; BW_d and G_d from single-device runs (BW = bytes per token × measured tok/s; G = prefill FLOPs ÷ prompt ms) **[E←M]**; α, β per link from a `llama-bench --rpc` probe, refined by an EWMA of the leftover time on every real split request **[E←M]**; compute buffers from the "compute buffer size" lines llama.cpp prints at load (meshd already captures the host log) **[M after first load]**. Every value carries `{v, lo, hi, src: measured|derived|estimate|unknown, from, n}`; an unknown shows no number, only a Measure button. Bands: measured n≥3 ±15 %, derived ±30 %, estimate ±50 %.

Verdicts. Per device: *not capable* (offline, wrong CPU tier, RTT p95 > 60 ms, throttling, battery < 20 % unplugged, cannot hold one block + its KV); *capable* ("holds the whole model" or "holds up to K of N layers"); *capable only with X* (context ≤ 2k, free N MB, use a cable/tethering/hotspot, plug in); *in the plan / not needed*. For the mesh: "can run on {device} alone" · "can run split across N devices — fast ≥10 / OK 3–10 / slow <3 tok/s" · "cannot run: needs X GB, have Y, short Z" with computed fixes (halve context, smaller quant, add a device).

**Worked example — Qwen3-8B Q4_K_M, 4k context, 500-token question, 200-token answer.** Device inputs: laptop 11.0 GB usable, BW 14.3 GB/s and G 122 GFLOP/s **[E←M: 3.05 tok/s and 8.76 tok/s prompt on this model, runs.jsonl]**; phone 2.0 GB usable (apps closed), BW 18.2 GB/s and G 81 GFLOP/s **[E←M: 28.8 / 92 tok/s on Qwen3-0.6B]**; adb link α ≈ 717 ms per token **[E←M: 1.32 tok/s split minus 38 ms of compute]**, USB-tethering α 2–12 ms **[E, unmeasured]**.

| Case | Stacks | Memory | GFLOP per token | ms per token (compute) | Link ms | tok/s | First word (P=500) | 200-token answer |
|---|---|---|---|---|---|---|---|---|
| **Recommended: laptop alone** | laptop 0–35 + embd + head | 5.9 of 11.0 GB | 15.5 | 334 | — | **3.0** | ≈58 s | ≈2 min |
| Why not split, over the adb cable | laptop 0–22 + embd + head · phone 23–35 | 4.5 / 1.9 GB | 10.4 / 5.1 | 226 / 85 | 720 | **≈1.0** | ≈70 s | ≈4.6 min |
| Same split over USB tethering | same | same | same | 226 / 85 | ≈7 | ≈3.1 | ≈69 s | ≈2.2 min |

Verdicts: laptop capable, host, 36 of 36 layers; POCO F5 capable of up to 13 layers, **not needed**; mesh: *can run on this laptop alone*. Lesson the screen must teach: splitting is for memory, never for speed — even over a fast link the phone's prefill compute is below the laptop's. If the laptop were capped at 4 GB: host keeps 1.16 GB fixed (embd + head + compute buffer) + 21 blocks; the phone would need 15 blocks = 2.14 GB > 2.0 → **cannot run at 4k, short 0.14 GB; capable only with a 2k context** (phone 22–35 = 1.89 GB) → ≈1.0 tok/s over adb or ≈3.2 tok/s over tethering **[E]**. Today's planner ignores compute buffers and would say "fits".

Prior art: exo, prima.cpp, distributed-llama, cake and Petals all partition automatically (memory- or throughput-weighted), and **none shows a predicted time per request before running**; that display is a real differentiator. Correction to R001: `evilsocket/cake` (Rust, Candle) claims Android support with mDNS discovery and compute+memory-weighted layer assignment — unverified on a device, but "no Android competitor" now needs that caveat.

### 17.5 The screens: three steps on the laptop, a view-only mirror on the phone

```
(1) Devices ── (2) Model ── (3) Calculate & Run                      RIGHT NOW: … [Stop]

STEP 1  DEVICES                     STEP 2  MODEL  (using: laptop + POCO F5, 13.0 GB)
 [Pair over USB]  [Show QR]          ┌ Qwen3 8B ─────┐ ┌ gpt-oss-20b ──┐ ┌ Qwen2.5-VL 3B ┐
 ┌ this laptop ──────────┐           │ 5.0 GB · 36 L │ │ 12.1 GB · 24 L│ │ 2.1+1.3 GB    │
 │ ☑ use · room 11.0 GB  │           │ text · tools  │ │ MoE · tools   │ │ text + images │
 │ speed 14 GB/s (m)     │           │ ✓ fits laptop │ │ ✓ laptop+phone│ │ ✓ fits laptop │
 │ ✓ can host, can help  │           └───────────────┘ └───────────────┘ └───────────────┘
 └───────────────────────┘           memory for the conversation: ○2k ●4k ○8k
 ┌ POCO F5 ──────────────┐
 │ ☑ use · room 2.0 GB   │          STEP 3  CALCULATE & RUN   question [500] in, [200] out  [Calculate]
 │ link USB cable 0.72 s │           ✓ CAN RUN on this laptop alone (5.9 of 11.0 GB)
 │   per token (m)       │             first word ≈58 s (e) · 3.0 tok/s (m) · answer ≈2 min (e) · last time 3.05 measured
 │ ✓ can help  ⚠ slow    │           layer 0 ████████████ this laptop 0–35 + in/out ████████████ 35
 │   link: turn on USB   │           device      layers   memory  GFLOP/tok  ms/tok  verdict
 │   tethering           │           laptop      0–35+io  5.9 GB  15.5       334 m   capable · host
 │ [Measure]             │           POCO F5     —        —       —          —       capable (13 layers), not needed
 └───────────────────────┘           ▸ why not the phone too? laptop 0–22 + phone 23–35 → ≈1.0 tok/s: the cable adds 0.72 s/token
                                     [Run this plan]   then CHAT (attachments, last answer: measured vs predicted)
PHONE (view only): steps ✓✓✓ · model · layer bar · "POCO F5 (me): can help 13 layers, not needed" · ≈3.0 tok/s · status. No prompts, no answers.
```

Keep: the RIGHT NOW bar and checklist, USB one-click pairing and QR, device cards (trimmed), model cards, the stacked layer bar, chat with attachments, Stop. Remove or move: the JavaScript fit guess (`file_bytes × 1.15 + 0.2 GB`) → server verdict; separate Run and Chat pages → step 3; "who runs it" and permission lists → a details drawer; the Advanced toggle → one developer drawer (simulated phones, raw args, host log, analytics, URL download). Partner phones receive a `MeshView` message over the control plane that by schema has no prompt, answer or address fields; `Plan` stays the only message that makes a phone act.

API per step: 1 → `/api/state`, `/api/usb`, `/api/usb/pair`, `/api/pair/offer`, `PUT /api/session {devices}`, `POST /api/devices/{id}/measure` (link probe). 2 → `/api/catalog` (+ server-side fit per model), `/api/models/download`, `PUT /api/session {model, n_ctx}`, `GET /api/models/{file}/layers`. 3 → `POST /api/calculate`, `POST /api/run {calc_id}` (409 if a re-plan would differ), `/api/stop`, `/v1/chat/completions`, `/api/runs`.

### 17.6 Build order (each slice is verified on the POCO F5) and gates

| # | Slice | Task | Verified by |
|---|---|---|---|
| S0 | No-code experiment: is the link cost α stable? Qwen3-0.6B, phone 4/14/24 layers × 3 chats, 14 layers at ctx 2k/8k, `adb push` 500 MB for bulk MB/s. Kill criterion: prediction error > 30 % or α drifting > 30 % | T071 (+T070 fact checks) | rows in §12 |
| S1 | `LayerConfig` from the GGUF tensor table meshcore already parses; `meshd layers`; `GET /api/models/{file}/layers` | T072 | block bytes sum to within 1 % of the three files on disk |
| S2 | `--fit off`, `--no-mmproj-offload`, tied-head pin; parse per-device buffer sizes from the load log; 5 % placement check | T073 | phone model buffer shrinks by the head size; projector stays on CPU in a split |
| S3 | Cost model + `/api/calculate` + `/api/session` + `calc_id` | T074 | golden test of the worked example; S0 configurations predicted vs measured |
| S4 | Calibration store `state/calib.json`, heartbeat echo RTT (today's "adb RTT" is a TCP connect to adbd on the phone's own loopback and never crosses the cable), phone bench report, link probe | T075 | probe α within 30 % of S0 |
| S5 | Admin three-step flow | T076 | headless QA + screenshot with the phone paired |
| S6 | Phone partner view (`MeshView`) | T077 | phone screenshot; privacy test |
| S7 | GATE: prediction accuracy ≤ 30 % on tok/s and first-word time (0.6B split × 3 shares, 8B laptop alone, 8B capped split at 2k) | T078 | rows in §12 |
| S8 | Measure α over USB tethering or hotspot → settles the ≥3 tok/s gate with the calculator's own numbers | T004/T007 | rows in §12 |

Accept D032 only after S0 (α stable) and S7 (predictions within 30 %). Risks K23–K29 in §15.

---

## 18. Demo runbook (hackathon day)

*Written 25 Sep 2026 from the link research (R009) and the evening's preflight runs. Everything marked [M] was executed here; [E] is expected but unmeasured; [?] must be tested before the day.*

### 18.1 The night before
1. Charge the phone; free its memory (close apps). The planner needs ≈1.9 GB free on it for a 13-layer share of an 8B model; with apps open it offered 0.6–0.8 GB [M].
2. Models on the laptop disk: Qwen3-0.6B (phone-host demo), Qwen3-8B (laptop demo), Qwen2.5-VL-3B + projector (image chat). Downloads were 1.1 MB/s on the home network at night [M] — do not plan to download at the venue.
3. Laptop memory: stop Gradle/Kotlin daemons and the Android emulator; they held 10.7 GB and made the planner refuse every model [M]. `scripts/demo-check.sh` warns about both.
4. HyperOS keep-alive toggles for the MeshAI app (reset by reboots and OS updates — re-check on the day): Settings → Apps → Manage apps → MeshAI → Autostart **on**; Settings → Battery & performance → Manage apps' battery usage → MeshAI → **No restrictions**; Recents → swipe the MeshAI card down until the padlock shows; Display → screen timeout → longest; keep the phone **unlocked with the app in front** during runs (a lock screen dropped the link once [M]).
5. Run `MESH_TOKEN=… scripts/demo-check.sh --phone` until it prints READY. It executes a real one-question smoke test (last run: model ready in 10.4 s, answer in 1.3 s [M]).

### 18.2 The link (decides whether the split is watchable)
| Link | RTT p95 | Split speed | Status |
|---|---|---|---|
| USB-debugging cable (adb relay) | 4–13 ms [M] | 0.65–1.4 tok/s [M] — the relay, not the RPC protocol, costs ≈0.7 s per token | works today; too slow for the ≥3 tok/s gate |
| Shared/venue Wi-Fi | 65–154 ms [M] | refused by the 60 ms policy | never rely on venue Wi-Fi |
| USB tethering (phone Settings) | [?] expected lowest | [?] | **test first (T004)** |
| Phone hotspot 5 GHz | [?] | [?] | fallback |
| Laptop hotspot | [?] | [?] | fallback that never touches the phone's USB mode |

Order of operations, because switching the link can break adb: **pair first, then switch the link.**
1. Plug in, press *Pair over USB*, tap *Join*. Wait one minute before running anything (the first cable link after pairing was dropped and re-made once, 50 s in [M]; the app fix for this is on disk, not yet verified on the phone).
2. Phone: Settings → Connection & sharing → USB tethering **on** (older builds: Additional settings → Hotspot & tethering). Never switch it from adb (K20).
3. Laptop: `adb devices` must still say `device`. If it says `no permissions` (reported by other Xiaomi users on Linux, R009): `sudo udevadm control --reload-rules && sudo udevadm trigger`, replug once; if still broken, keep tethering for data only and use the QR for any re-pairing.
4. Laptop: `ip addr` shows a new interface (name unknown until tried; historically 192.168.42.x); `ip route` gives the phone's address; `ping` it. The app already tries every laptop address in order (USB tether first), so it moves to the new link by itself.
5. Run `scripts/demo-check.sh --phone` again: the link line must show the tether address and an RTT under 60 ms.
6. If tethering fails: laptop hotspot (GNOME Settings → Wi-Fi → Turn on hotspot), phone joins it, same check.

### 18.3 On stage (three tasks, in this order)
1. **Phone hosts the small model** (Qwen3-0.6B): the laptop pushes the model, the phone answers; 28.8 tok/s measured [M]. Proves the phone runs the engine.
2. **Laptop alone, Qwen3-8B**: ask it to interview you; 3 tok/s, first word in a few seconds for short prompts [M]. Proves "don't split if it fits".
3. **Laptop + phone split**: Qwen3-8B with the laptop capped (Devices → details → limit) so the phone must hold 13 layers; over the cable ≈1 tok/s [M], over tethering [?]. Say out loud that splitting is for memory, never for speed. If the link is the cable, keep the answer short (max 40 tokens).
4. Optional: image chat on the laptop (Qwen2.5-VL-3B, 150 s per screenshot [M]) — start it early or skip if time is short.

### 18.4 If something breaks
- Stop button in the panel, then `scripts/demo-check.sh`. It names the failing item.
- Phone shows *offline*: unlock it, bring the app to the front, replug; it reconnects by itself with the stored secret.
- "host can hold only …": the laptop is out of memory — close apps, or lower the context to 2k.
- Worker "Failed to create server socket": the phone reserved the port; the app now retries other ports (D031) — press Run again.
- Stray processes after a crash: `pkill -x llama-server; pkill -x ggml-rpc-server`, and on the phone force-stop the app.

---

## 19. Proposed: the universal adapter (speech in, speech out, images, embeddings) — design summary

*From the design memo of 25 Sep 2026 (D034, proposed) and research R011/R012. Nothing here is built; every upstream fact was read, not executed.*

### 19.1 Engines and where they can run

| Engine | Task | Unit kind | Can use a phone? | Memory (estimates until measured) | Route meshd serves |
|---|---|---|---|---|---|
| llama.cpp (today) | chat, vision, audio-input chat, embeddings, rerank | layers (blocks), host-pinned embd/head/projector | yes — the existing split | from the GGUF | `/v1/chat/completions`, `/v1/completions`, `/v1/embeddings`, `/v1/rerank` |
| whisper.cpp `whisper-server` | speech → text | whole | later (phone host, one process) | base.en 0.15 GB + 0.2 GB reserve | `/v1/audio/transcriptions`, `/translations` (multipart passthrough via `--inference-path`) |
| TTS: `llama-tts` (Qwen3-TTS / Pocket-TTS) or sherpa-onnx Kokoro/Piper | text → speech | whole, per request (CLI) | later | 0.1–1 GB | `/v1/audio/speech` (meshd runs the CLI, text via a file, returns WAV) |
| stable-diffusion.cpp `sd-server` | text → image | components: text encoder(s) MAY MOVE, drawing network MUST STAY TOGETHER, VAE MAY MOVE | yes with a separate sd-ABI worker (K32) | SD 1.5 Q8 ≈2.1 GB + unknown buffers (K33) | `/v1/images/generations` (b64_json forced, n=1, size capped) |
| Video | text → video | components | no | not a demo item: Wan2.1 1.3B ≈5–6 GB fits the laptop alone but time per clip is unmeasured and could be hours (R013); overnight batch at most | none |

Rules that carry over: "don't split if it fits" applies to a bundle; an add-on moves to a phone only when memory requires it, never for assumed speed. Every reserve is an estimate until a load log replaces it (D033).

### 19.2 The Bundle Plan
- The user picks a chat model plus add-ons (voice in, voice out, draw, embeddings) in step 2. Calculate places all their units together under one plan id; step 3 shows one row per workload ("voice in: whole model → this laptop · 0.15 GB + 0.2 GB reserve (e)", "draw: reads your prompt → phone | drawing network → laptop | pixels → laptop").
- One generation, one stop path: all processes of the bundle start and stop together; any process exit ends the run. Credited caps sum per device.
- Ports on the laptop, loopback only: chat 8081 (unchanged), stt 8082, tts per request, draw 8084, embed 8085; sd-ABI workers on 50352+ with D031 fallbacks.
- `/v1/models` is answered by meshd; unknown `/v1/*` paths return 404 with the route list.

### 19.3 Phone side (v2 only)
`LaunchSpec` = allow-listed binary id + argv with placeholders (`{bind}`, `{port}`, `{file:NAME}`), executed without a shell; a slot-keyed ProcessGate; a static `libmeshai_rpc_sd.so` built from sd.cpp's pinned llama.cpp with `GGML_MAX_NAME=160`, 16 KB aligned. Legacy apps (no `engines` in their profile) keep receiving only their chat placement.

### 19.4 Chat controls
A mic button (browser records, JS encodes 16 kHz mono WAV, posts multipart with the token, transcript lands in the composer for editing), a "speak answers" toggle (posts to `/v1/audio/speech` after the answer, plays the WAV), and `/draw <prompt>` (posts to `/v1/images/generations`, renders inline with its time). Controls appear only when the bundle has that workload.

### 19.5 Order of work (each slice measurable on the laptop; S1–S7 need no phone or proto change)

| # | Task | Slice | Gate |
|---|---|---|---|
| S0 | T080 | Laptop measurements, no code: warm reload of Qwen3-8B ×3; decode tok/s with an idle whisper-server resident vs absent; whisper-server base.en on 10 s/30 s WAVs (load, RSS, RTF, readiness); Qwen3-ASR-0.6B through llama-server at 66fba63; `llama-tts` and sherpa-onnx Kokoro one sentence each; `sd-cli` SD 1.5 q8 512² 20 steps (s/image, peak RSS, module names from `sd-server -h`) | decides A vs C, the STT/TTS engines, whether draw is demoable |
| S1 | T081 | `engine.rs` + catalog v2, behaviour-preserving (llama argv == today's `derive_args`) | 3 supervisor tests + admin QA unchanged |
| S2 | T082 | `plan_bundle` (whole + layers) | 13 planner tests pass through it; add-on reserved before layers; forced-split add-on refused with a reason; `max_procs` |
| S3 | T083 | supervisor bundles (list of children, one generation) | V009 orphan test extended; credit sums pids |
| S4 | T084 | route table, `/v1/models`, multipart guard exception, transcription passthrough, `RunRow.task` | curl multipart WAV → text; 415 without the header; alias `whisper-1`; `/load` → 404; **reviewer** |
| S5 | T085 | admin: add-on toggles, modality chips, mic (JS WAV), bundle rows | headless QA, 0 console errors |
| S6 | T086 | TTS engine (from S0) + `/v1/audio/speech` + speaker toggle | WAV plays; text starting with `-` is spoken |
| S7 | T087 | sd.cpp on the laptop + `/v1/images/generations` + `/draw` | an image and an s/image row in §12 |
| S8 | T088 | sd component placement against a local sd-ABI rpc-server | `--backend te=RPC0` runs; a name-64 worker is refused |
| S9 | T089–T091 | proto schema 1 + `LaunchSpec` on the phone; static sd worker; slot-keyed ProcessGate; diff-based restart | legacy-app test; phone screenshot; **reviewer** |

Cheap first win before any of this: llama-server already serves `/v1/embeddings` and `/rerank`; only the benchmark row for non-chat routes is missing (proxy.rs meters chat only).

---

## 20. Product-flow audit (25 Sep 2026) — the guided flow the product owner asked for vs what exists

Target flow: **Devices** (see who is host / helper / engines, pair first, nothing else opens until paired; after pairing show the available compute and advice to keep the helper charged and idle; over USB, switch on the developer controls we can) → **Models** (every model sorted into *easy* / *hard but doable* / *not possible*, with what would make it possible) → **Prepare** (download or reuse what is already there, push to phones, resume after interruptions, a readiness checklist) → **Use** (chat / audio / images) — with mature error handling (stop in the middle, come back, continue) and no dependence on this laptop or this phone.

| Requirement | State | Evidence / gap |
|---|---|---|
| Pairing persists; the phone reconnects on its own | **done** | stored device secret, auto-join from the last payload, USB one-click + QR; reconnects seen in every session |
| Steps locked until paired; only then Models → Prepare → Use open | **built 25 Sep, QA'd headless** | four gated steps with lock reasons and a 'this laptop only' escape hatch (T095); not yet used with the real phone |
| Who is host, who helps, which engine | **built 25 Sep** | HOST / HELPER / NOT USABLE chips with the reason and an engine chip on every card (T095) |
| Available compute after pairing | **built 25 Sep** | mesh summary (room, best measured speed, link kind and RTT) on the Devices step (T095) |
| Advice to keep the helper charging and free | **built 25 Sep** | `advice[]` in `/api/state` (plug in, close apps, keep the app in front, cable is slow → tethering) rendered as a checklist (T096) |
| Turn on phone developer controls over USB | **built, not run on a phone** | `POST /api/usb/{serial}/care` (doze whitelist, background allowed, stay awake while plugged) is called by *Pair over USB* and reported; verified only against an offline serial so far (T097 → T100) |
| Model categories easy / hard-but-doable / not possible, with what would make it possible | **built 25 Sep** | `POST /api/feasibility` sorts every catalog model and every file on disk with a `needs` list (context, free memory on which device, one more device, download); the panel shows the three groups (T096) |
| Prepare step: download or reuse, push, readiness, resume | **partial** | laptop downloads resume (Range + If-Range on `.part`), phone fetches resume (Range, `*/len`), the phone's RPC worker keeps a tensor cache (`-c`) so a re-run does not re-send layers; and since 25 Sep a Prepare step shows on-disk/download with resume, the phone cache note, a readiness checklist, Calculate and Start (T098). Whether a re-run actually reuses cached layers is **not measured** (T100) |
| Stop in the middle and come back | **partial** | Stop is clean (0 orphans), failure attribution by plan id, dynamic ports; the last accepted run is saved (`state/last_run.json`) and the panel offers 'Start again' after a restart (built 25 Sep); device-side recovery not yet run on the phone (T100) |
| Recovery messages a novice understands | **built 25 Sep** | recovery banners map meshd's error strings, device-offline events and `last_run` to one sentence and one action (T098) |
| Universal (any devices, small to large) | **mostly** | the planner takes any number of devices (simulated phones up to 4 tested); feasibility is computed only from device capacities; catalog notes still mention this laptop; no engine other than llama.cpp; Windows never executed (T099) |
| Layers are the unit; inputs and outputs are all that cross devices | **done** | §17.3 rules, measured placements, per-token activation ≈16 KB |

Measured today for the adapter (§12): SD 1.5 image 18 min, Qwen3-TTS 42 s / 8 GB — neither ships in the demo; whisper base.en 1.5 s per 11 s clip does.

*Reviewer audit of this batch (25 Sep, FAIL → fixes applied the same day): link kinds in the run history were backfilled (cable / lan / loopback) and are now recorded on every row; Calculate and feasibility validate model names and contexts; USB care no longer overwrites the stay-awake setting; the phone's connect path is serialised. Still owed: device verification of the phone half (T100) and the T007 gate model.*
