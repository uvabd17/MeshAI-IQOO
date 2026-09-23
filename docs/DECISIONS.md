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

## D009 — Any device can be the model host; the coordinator stays on the laptop
2026-09-24 · The user wants input/output on a chosen device (phone or laptop) with every other device as pure compute. **Decision:** three roles. *Coordinator* (`meshd`, laptop): pairing, planning, admin panel, telemetry. *Host* (any paired device): runs `llama-server` with `--rpc <workers>`, owns the session, serves `/v1`; `meshd` proxies `localhost:8080/v1` to it. *Worker*: runs `ggml-rpc-server`. Selecting a host in the admin panel flips the others to worker. Both binaries ship for arm64 and x86-64. **Rejected:** host fixed to laptop (simpler, but not what the product promises). Status: accepted.

## D010 — Android v1 runs llama.cpp as child processes from the app's native-lib dir; in-process JNI embed is Phase 2
2026-09-24 · Executables packaged as `jniLibs/<abi>/lib*.so` are extracted with exec permission and can be spawned from `applicationInfo.nativeLibraryDir`; the child inherits the app's cpuset and UID sandbox. This gives host mode (`llama-server`) and worker mode (`ggml-rpc-server`) on the phone immediately, with no Termux and no terminal, and reuses the verified arm64 build. **Cost:** per-process memory accounting (LMK may kill the child first; the FGS keeps the parent) and process-spawn latency. **Rejected for v1:** `ggml_backend_rpc_start_server` via JNI (better, but blocks host mode and costs a week). Status: accepted; revisit after Phase 0 numbers.

## D011 — Model catalog lives on /mnt/storage (HDD); warm before demo
2026-09-24 · NVMe has 22 GB free; the HDD has 72 GB at ~80 MB/s. `MESHAI_MODELS=/mnt/storage/meshai/models`. First load of an 18.6 GB model ≈ 3 min; the admin panel shows a preload state and offers "pin to fast disk". Status: accepted.

## D012 — Demo model set
2026-09-24 · Cloud reference: pluggable OpenAI-compatible provider (default Claude Sonnet 5). Laptop baseline: Qwen3-8B Q4_K_M (5.0 GB, 36 layers). Budget-laptop story: gpt-oss-20b MXFP4 (12.1 GB, 24 layers) with laptop capped at 6 GB. Mesh headline: Qwen3-Coder-30B-A3B Q4_K_M (18.6 GB, 48 layers). Test: Qwen3-0.6B Q8_0. Status: accepted.

## D013 — Pairing v1: one-time QR token → per-device secret; Noise XX deferred (supersedes the transport part of D002 for v1)
2026-09-24 · The reviewer found reconnects accepted a self-reported device id alone and the pairing token was served by `/api/state`. **Decision:** the coordinator answers a valid token with a random 32-byte device secret (`Paired`), persists it in `state/paired.json`, and every reconnect must present it (constant-time compare). The offer/token is returned only by `POST /api/pair/offer` to the local admin and never appears in `/api/state` or the mirror. Control-plane *encryption* stays a Phase-2 item; v1 relies on the private hotspot/USB link. Status: accepted.

## D014 — Cloud mirror is an explicit, opt-in exception to "no telemetry off-device"
2026-09-24 · The user asked for an internet-ready demo. **Decision:** `meshd mirror` exists only as a read-only admin (GET `/admin`, `/api/state`, `/api/runs`; POST `/api/relay/state`); the coordinator pushes a *stripped* snapshot (hashed device ids, no addresses, no paths, no process args, no token) only when `--push-to` is set; TLS is required before any real-network use; nothing else (models, weights, prompts, RPC) leaves the mesh. Off by default. Status: accepted, pending user sign-off for the live deployment (T050).

## D015 — Android floor raised to dotprod **+ i8mm** (amends D004)
2026-09-24 · The shipped arm64 build is compiled with `+i8mm` (`smmla` present in `libggml-cpu.so`); a dotprod-only phone would SIGILL. Rather than ship a slower dotprod-only build for the hackathon, the tier gate now requires both. Cortex-A78/X1 (2021+) and newer qualify; A76-class cores do not. Revisit with `GGML_CPU_ALL_VARIANTS` runtime dispatch in Phase 2. Status: accepted.

## D016 — Headroom constants until measured
2026-09-24 · Laptop reserves 2 GB; the phone reports 1.5 GB (`Profiler.HEADROOM`) and the coordinator uses whatever the device reports. Both are guesses to be replaced by the kill-threshold measurement (T006). llama.cpp compute/RPC buffers (~25–30 MiB per device at 2k ctx on the 0.6B) are not yet modelled. Status: provisional.

## D017 — Thinking off by default for the demo
2026-09-24 · Qwen3 spends the whole token budget in reasoning unless told otherwise. `llama-server --reasoning off` on every host (laptop and phone). A per-request toggle is a Phase-2 admin control. Status: accepted.

## D018 — Android v1 without UniFFI/meshcore.so (amends D003)
2026-09-24 · The v1 phone app re-implements framing and plan handling in Kotlin (~700 lines) instead of binding a Rust `meshcore.so`; the wire format is the shared `proto/mesh.proto`, so both sides stay in sync through the schema, and the llama.cpp argument rule (ngl+1, head pinned) is duplicated in `LlamaRunner.startHost` with a comment pointing at `supervisor.rs`. UniFFI binding returns when the planner needs to run on a phone host. Status: accepted.

## D019 — x86_64 Android build is test-only
2026-09-24 · Built without AVX2/FMA/F16C so it runs on the emulator's CPU; never shipped to users (the floor is arm64). Status: accepted.

## D020 — The laptop's RPC worker binds to its end of the host phone's control link
2026-09-24 · Round-2 review: `local_ip()` is the default-route interface, which on venue Wi-Fi is not the paired link. **Decision:** every control connection records the coordinator's local socket address; the laptop worker (phone-host mode) binds to the address of the host phone's link, and the plan sent to each device carries the laptop address *as that device sees it*. `meshd worker` (manual CLI) still uses the default-route address and prints it. Status: accepted.

## D021 — `--lan` exposure rules
2026-09-24 · With `--lan`, every API route requires the token except the admin static files, `/api/catalog` and `GET /api/models/file/*` (weights are not secret and a phone host must fetch them). Even without a token, non-GET requests must carry `application/json` (defeats cross-site form posts) and, on loopback, the Host header must be a loopback name (defeats DNS rebinding). Status: accepted.

## D022 — Heartbeat on a fixed interval (fixes a round-2 CRITICAL)
2026-09-24 · The coordinator's heartbeat lived in a `select!` sleep branch that was re-created on every loop iteration, so it never fired while telemetry streamed every 2 s; combined with the phone's read timeout every real link would drop every ~30 s. **Decision:** `tokio::time::interval(10 s)` created once per connection; phone read timeout 45 s. Not yet executed on a device (no phone on USB) — T021 stays open until a 5-minute link test is logged. Status: accepted, verification pending.

## D023 — Greedy contiguous fill, host first (amends ARCHITECTURE §7 rule 2)
2026-09-24 · Proportional targets produced false `DoesNotFit` on tight pools (round-2 N7). **Decision:** the host keeps embeddings/output plus as many leading layers as it can hold, then each further device (largest usable memory first) takes as many consecutive layers as it can hold; devices keep being added while layers remain. The "laptop last" phrasing in §7 is superseded: the host is the laptop unless the user picks a phone, because input/output happen there (D009); loading the host fully first is accepted because it holds the head and KV for the earliest layers anyway. Status: accepted.

## D024 — A phone can be the host only when meshd runs with `--lan --api-token`
2026-09-24 · The phone host fetches the model from `/api/models/file/…`, which is loopback-only by default. **Decision:** `POST /api/run` answers 422 for a remote host unless `--lan` is set, and validates before it stops the current run (a refused request leaves the running mesh alone). Status: accepted.

## D025 — A device's failure report ends the run, but only for the plan it names
2026-09-24 · Phones report process exits and refused plans as `JobResult{ok:false}`. Without a plan id a stale "download cancelled" from a superseded run could kill the next run (round-4 #1). **Decision:** `JobResult.plan_id` is mandatory; meshd honours a report only if the device holds a plan and the id equals the current run's plan id; the phone never reports a cancelled job (cancellation is a real `CancellationException`). Same rule for a device that disconnects or is forgotten mid-run: the run ends with `error`. Status: accepted, tested (`failure_report_ends_only_the_run_it_belongs_to`).

## D026 — Locking discipline in meshd: `run_lock` → `proc_lock`, never the reverse
2026-09-24 · `run_lock` serialises the *initiation* of start/stop/forget (held by API handlers and device cleanup). `proc_lock` is held across every "check generation → spawn/register/kill a process, push a plan, or change run status" so a stop can never interleave with a registration. Lock order is always run_lock then proc_lock; no std guard is held across an await. Background tasks carry the generation they were started for and use `fail_if_current`. On the phone the mirror is: one plan actor owns `planJob`; `LlamaRunner` owns the process under one lock with `armed`/`procGen` so a racing start is refused and a stale exit is never reported. Status: accepted.
