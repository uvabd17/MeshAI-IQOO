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
