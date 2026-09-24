---
name: phase0-bench
description: Run the Phase 0 hardware-truth measurements (phone alone, laptop alone, laptop+phone split over llama.cpp RPC; RTT; thermal over 10 min) and record them in docs/MESHAI.md (§12 benchmarks) with full conditions. Use when a physical arm64 phone is attached and a small GGUF is available.
argument-hint: "[model.gguf] [phone_ip]"
allowed-tools: Bash(scripts/*), Bash(adb *), Bash(ping *)
---

# Phase 0 benchmark procedure

Precondition check first — stop and report BLOCKED if any fails:
- `adb devices -l` shows a **physical** device with `ro.product.cpu.abi = arm64-v8a` (emulators are x86_64 and useless here).
- `third_party/llama.cpp/build-android-arm64-v8a/bin/ggml-rpc-server` exists (else run `scripts/android-build-llama.sh`).
- A host build with `-DGGML_RPC=ON` exists at `third_party/llama.cpp/build-host/bin` (else build it: `cmake -S third_party/llama.cpp -B third_party/llama.cpp/build-host -DGGML_RPC=ON && cmake --build … --target llama-bench llama-server -j`).
- Phone and laptop are on the same private link (hotspot or USB tethering); note which.

Then run `scripts/phase0-split-test.sh $0 $1` and capture:

| Step | What | Record |
|---|---|---|
| A | phone alone (`llama-bench` on device, CPU, N threads) | pp512 t/s, tg128 t/s, threads, MemAvailable |
| B | laptop alone (`llama-bench`) | pp512, tg128 |
| C | split (`llama-bench --rpc phone:50052`) | pp512, tg128, RTT p50/p95 from `ping -c 20` |
| C' | split with the OpenCL build exposing the GPU device | same; note prefill vs decode separately |
| D | 10-minute sustained (`llama-bench -r 20` or looped) | thermal headroom curve (`dumpsys thermalservice`), `dumpsys battery` current, t/s at minute 1/5/10 |
| E | kill threshold | grow `-c` context until Android kills the worker; record last surviving ctx and MemAvailable |

Write every row into `docs/MESHAI.md (§12 benchmarks)` with: date, phone model + SoC + RAM, Android version, laptop, llama.cpp commit, model + quant + file size, context, threads, backend, link type, and the raw command. A number without conditions is not a result.

Finally: update `state/tasks.json` (T003–T006) and `state/progress.md`, and state whether the **gate** passed: split works, output correct, decode ≥ 3 tok/s on a model that does not fit one device.
