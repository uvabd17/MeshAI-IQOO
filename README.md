# MeshAI

Pool the memory and compute of the devices you already own — an Android phone and a laptop — into one private, offline AI machine that can run open models neither device can run alone. Any OpenAI-compatible tool points at `localhost:8080/v1` and never knows how many devices are behind it.

Team Maynards · iQOO Hackathon 2026 (Developer Tools).

## Read this first

**`docs/MESHAI.md`** is the single source of truth: what is built, what was executed on real hardware (with every measured number), the design, all decisions and risks, how to run it on Linux / Windows / the phone, and what is still open. Older decks and PDFs are in `docs/archive/` and are superseded by it.

Machine-readable state lives in `state/` (`progress.md`, `tasks.json`, `tests.json`, `blockers.md`).

## Status (25 Sep 2026, branch `native-mesh-v2`, paused)

Read **section 0 of `docs/MESHAI.md`** first: what works on real hardware, what is built but not yet run on a phone, the open items, and how to pick the work up.

- Works on hardware: USB/QR pairing, phone as host (28.8 tok/s on a 0.6B model), laptop+phone layer split (1.2–1.4 tok/s over the USB-debugging cable, 7–9 tok/s over home Wi-Fi), image chat and speech-to-text on the laptop.
- Built, tested on the laptop, waiting for a phone session: the guided four-step panel (Devices → Models → Prepare → Use), USB care, cache-aware resume on the phone.
- Open: the ≥3 tok/s gate on a model that fits neither device, four reviewer fixes listed in section 0, Windows on a real machine, the universal adapter (designed, not built).

## Layout

```
android/   Kotlin + Jetpack Compose app (minSdk 30, arm64-v8a): pairs, reports specs, runs llama.cpp as helper or host.
desktop/   meshd — Rust service for Linux (primary) and Windows: pairing, planner, llama-server supervisor, /v1 proxy, control plane, serves the admin panel.
admin/     Admin web panel (Devices · Run · Chat), embedded into meshd.
proto/     mesh.proto — the control-plane schema shared by every node.
scripts/   verify, android-build-llama.sh, phase0-split-test.sh, split-sim.sh, qa/ (behaviour checks), windows/ (setup + run).
docs/      MESHAI.md (the one document) and archive/.
third_party/llama.cpp   (not committed) upstream checkout used by the scripts.
```

## Run it

```bash
cargo build --manifest-path desktop/Cargo.toml
MESHAI_API_TOKEN=secret desktop/target/debug/meshd serve --lan      # then open http://localhost:8080/admin/?token=secret
gradle -p android assembleDebug && adb install -r android/app/build/outputs/apk/debug/app-debug.apk
scripts/verify --quick                                              # syntax + schema + unit tests
```

Plug the phone in with USB debugging on, press **Pair over USB** on the Devices page, tap **Join** on the phone, pick a model on the Run page, talk on the Chat page.
