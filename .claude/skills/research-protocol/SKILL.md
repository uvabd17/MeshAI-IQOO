---
name: research-protocol
description: How MeshAI answers a technology question with sourced evidence and records it in docs/MESHAI.md (§16 research). Use before any decision that depends on upstream behaviour (llama.cpp, Android, Rust crates, hardware) — "is X supported", "does Y work on Android", "which crate", "what does issue #N say".
argument-hint: "[question]"
---

# Research protocol

Goal: remove *expensive* uncertainty with evidence, not with searching until something agrees with us.

## Procedure
1. **Write the question** as a testable claim: "Can X satisfy requirement Y under constraint Z?"
2. **List candidates** before searching (A, B, C). Include the one you already prefer and say so.
3. **Collect evidence per tier**, highest first, and label each finding with its tier:
   1 official docs · 2 source repo · 3 issues / PRs / release notes · 4 standards / papers · 5 maintainer discussion · 6 real benchmarks · 7 forums (Reddit/HN) · 8 blogs / video.
   Tiers 7–8 are for "works in docs, breaks in practice" signals — never the sole basis for a decision.
4. **Record what is UNKNOWN.** If it can only be settled by measurement, say which measurement and add it to `state/tasks.json`.
5. **Decide, with confidence** (High / Medium / Low) and **why not** each rejected candidate.
6. **Write the entry** to `docs/MESHAI.md (§16 research)` using [references/template.md](references/template.md). One entry per question, ID `R###`, dated, every claim with a URL and the date checked.
7. If the answer changes a design choice, draft the `docs/MESHAI.md (§11 decisions)` entry too.

## Rules
- Fetch primary sources; do not cite a summary of a doc when the doc is one request away.
- For GitHub issues: state open/closed, labels (`bug-unconfirmed` matters), and last activity date.
- Separate *verified today* from *recalled from memory*. Recalled facts get Medium confidence at best.
- Numbers from other people's benchmarks are quoted with their conditions or not at all.

$ARGUMENTS
