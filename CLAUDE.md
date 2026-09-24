# MeshAI — project instructions

Pool a phone's and a laptop's memory over a private link to run GGUF models neither can run alone, behind one local OpenAI-compatible endpoint. Team Maynards (Prajwal, Yuvaraj). Design of record: `docs/MESHAI.md (§10 design)`. Read `state/progress.md` before starting work.

## Non-negotiables
- **Claims discipline.** Every number is *measured* (with model, quant, context, backend, link) or marked *target*/*estimate*. Never write a benchmark number you did not run.
- **Don't split if it fits.** The scheduler uses the fewest devices that hold the model. Adding a device must be justified by memory, never assumed to add speed.
- **Two planes.** Tensor traffic is ggml RPC (borrowed, never reimplemented in v1). Control traffic is ours (`proto/mesh.proto`, Rust `meshcore`). The RPC port opens only after pairing and only on the paired link.
- **Native.** Kotlin owns pixels, Rust owns decisions, C/C++ owns tensors. No Python/Flutter coordinator, no JVM desktop core.
- **Android floor.** arm64-v8a, minSdk 30, ≥8 GB RAM, `dotprod` **and `i8mm`** (D015 — the shipped kernels use both). Every shipped `.so` is 16 KB page-aligned.
- **No cloud, no accounts, no telemetry off-device.**

## Build / verify
- `scripts/verify --quick` — syntax + schema + unit tests (what the Stop hook runs). `scripts/verify` — full, includes builds when toolchains exist.
- `scripts/android-build-llama.sh` — NDK r28 cross-compile of llama.cpp (verified 23 Sep 2026).
- `scripts/phase0-split-test.sh <model.gguf>` — first laptop↔phone split; needs a physical arm64 phone.
- Rust (`desktop/`): `cargo fmt --check && cargo clippy -D warnings && cargo test`. Android (`android/`): `./gradlew lint testDebugUnitTest`. Both are wired into `scripts/verify` once they exist.

## Layout
`android/` Kotlin worker · `desktop/` meshd (Rust) · `admin/` web SPA · `proto/` control plane · `scripts/` · `docs/` design, decisions, research, benchmarks · `state/` progress, tasks, tests, blockers · `third_party/llama.cpp` (ignored).

## Definition of done (a task is DONE only when all hold)
1. Code compiles and `scripts/verify --quick` passes.
2. Behaviour was **executed**, not inferred: a test, a script run, an `adb` session, or a screenshot — and the evidence is in the report.
3. Measured numbers, if any, are appended to `docs/MESHAI.md (§12 benchmarks)` with full conditions.
4. Any decision made is in `docs/MESHAI.md (§11 decisions)`; any unknown found is in `docs/MESHAI.md (§16 research)` or `state/blockers.md`.
5. `state/tasks.json` and `state/progress.md` are updated.
6. Independent review by the `reviewer` agent for anything touching pairing, scheduler rules, memory headroom, or the public API.

## Persistent state (survives context loss — keep it current)
`state/progress.md` (what happened, what's next) · `state/tasks.json` (task graph with status) · `state/tests.json` (what verification exists and its last result) · `state/blockers.md` · `docs/MESHAI.md (§11 decisions)` · `docs/MESHAI.md (§16 research)`.

## Delegation
Builder ≠ approver. Use these subagents (`.claude/agents/`):
- `researcher` — any technology/upstream question before deciding. Read-only. Follows `/research-protocol`.
- `architect` — options + tradeoffs for a design decision. Read-only; the decision is recorded by the main thread in `docs/MESHAI.md (§11 decisions)`.
- `engineer` — implementation of one scoped task. Runs verify before reporting.
- `verifier` — independent execution of tests/scripts/device runs; reports evidence, never edits source.
- `reviewer` — completion audit against the Definition of Done. Read-only. PASS/FAIL.
Sequential, small work stays in the main thread. Parallelize only independent workstreams (e.g. Android worker vs `meshd`).

Model routing: STRATEGIC = opus (architect, reviewer, hard debugging); ENGINEERING = sonnet (engineer, verifier, researcher); FAST = haiku (search, triage). Agents set this in frontmatter; update there when models change.

## Development rule
Research enough to remove *expensive* uncertainty → build the smallest vertical slice → measure on real hardware → expand. Before any large piece of work ask: *which assumption, if false, invalidates the most future work?* — and test that first. (Right now: does a laptop↔phone split reach ≥3 tok/s on a model that fits neither device? See `state/tasks.json` T001–T006.)

## Prohibitions
- No `git push --force`, `git reset --hard`, `git clean`, `rm -rf` outside the scratchpad, `adb shell rm`/`reboot`/`uninstall` — the PreToolUse hook blocks these; do not work around it. The hook ignores heredoc bodies, so writing a file that *mentions* these is fine; if it still trips on a legitimate command, use the Write/Edit tools for that file instead of a Bash heredoc.
- Do not commit GGUF files, `third_party/`, build outputs, or `local.properties`.
- Do not claim "should work". Show the run.
