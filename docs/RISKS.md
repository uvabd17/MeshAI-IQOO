# Risk register (review at every phase gate)

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
| K19 | Replacement-plan credit vs reality: mmap'd weights partly stay in page cache, so a stop releases less than the placement estimate (measured by the round-9 audit: 3.56 GB released of 5.32 GB placed, Qwen3-8B Q4_K_M on this laptop); an over-committed llama-server load does not fail cleanly — it pages or is OOM-killed | Med | High | measured | D024: credit = min(placement, observed drop), only when ready and sampled after ready_ms; loading runs earn nothing (run-guards); Phase 2: `--no-mmap` option or MemAvailable calibration per device | T012 |
