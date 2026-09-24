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
| K19 | Replacement-plan credit vs reality: the credit is the anonymous RSS of the run's own processes (D024). Remaining error sources: a phone's held_bytes is self-reported and the phone path is not yet executed on a device (a failed /proc read reports 0 → no credit, safe); RSS is sampled up to 2 s before the request; RSS can dip during a stop/swap; swapped/zram anon, hugetlbfs and GPU buffers are outside RssAnon (under-credit, safe). Earlier rules were measured wrong on this laptop by the audits: the plan estimate credited 5.32 GB with no MemAvailable movement (round 9); the observed-drop rule credited the full placement while an unrelated 3 GB allocation was alive (round 10). An over-committed mmap load does not fail cleanly on Linux — it pages or is OOM-killed | Low | High | measured | credit = min(placement, held RSS), only when ready and sampled after ready_ms; loading earns nothing (run-guards); Phase 2: phone RSS attested by the worker-ready handshake, `--no-mmap` option | T012 |
| K20 | Switching the phone's USB mode to RNDIS from adb (`svc usb setFunctions rndis`) dropped adb *and* the tether link on the POCO F5 (Android 15); the phone then needs USB debugging re-enabled by hand | High | Med | observed 24 Sep | Turn USB tethering on from the phone's Settings (keeps adb), or use a hotspot; never switch USB functions from adb | T004 |
| K21 | Shared home Wi-Fi: phone→laptop pings lost 80 % (Wi-Fi power save) and control RTT p95 reached 65.7 ms, so the planner refuses the phone as a worker (policy 60 ms) | High | High | measured | Demo on the laptop hotspot / phone hotspot / USB tethering as the design says (§2); the app's low-latency Wi-Fi lock helps only while joined | T004 |

