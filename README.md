# MeshAI

Pool the memory and compute of the devices you already own — an Android phone and a laptop — into one private, offline AI machine that can run open models neither device can run alone. Any OpenAI-compatible tool points at `localhost:8080/v1` and never knows how many devices are behind it.

Team Maynards · iQOO Hackathon 2026 (Developer Tools) · and beyond.

## Status

**Design + audit stage on branch `native-mesh`.** Nothing user-facing is built yet. Read, in order:

1. `docs/ARCHITECTURE-v3-native-mesh.md` — **current plan**: what was verified upstream on 23 Sep 2026, what changed, the native architecture, the phased plan and gates.
2. `docs/MeshAI_Architecture_Team_Notes.pdf` — the plain-English mental model.
3. `docs/MeshAI_Technical_Spec_v2.pdf`, `docs/MeshAI_Pitch_Deck_v3.pdf` — hackathon submission docs (positioning and claims discipline still apply).
4. `docs/*_v1.*` — archived first drafts.

## Layout

```
android/   Kotlin + Jetpack Compose worker app (minSdk 30, arm64-v8a). Embeds ggml RPC in-process.
desktop/   meshd — Rust daemon for Linux (primary) and Windows: pairing, planner, scheduler, llama-server supervisor, /v1 proxy, serves the admin web.
admin/     Admin web SPA (devices, models, plans & jobs, analytics), embedded into meshd.
proto/     mesh.proto — the control-plane schema shared by every node.
scripts/   android-build-llama.sh (NDK cross-compile, verified), phase0-split-test.sh (first laptop<->phone split + measurements).
docs/      design documents and, from Phase 0 on, docs/benchmarks.md with every measured number.
third_party/llama.cpp   (not committed) upstream checkout used by the scripts.
```

## Phase 0 — do this first

```bash
scripts/android-build-llama.sh                    # arm64 ggml-rpc-server / llama-server / llama-bench
scripts/phase0-split-test.sh path/to/small.gguf   # physical arm64 phone on USB debugging + same private link
```

Gate: a model that does not fit one device runs split, output is correct, decode ≥ 3 tok/s. Everything after that is in the architecture doc.




