# What we actually do with this compute — the demo, in three tasks

MeshAI's promise is one sentence: **your laptop and your phone pool their memory over a private
link and serve one local, OpenAI-compatible AI endpoint — no cloud, no account.** The compute we
have (measured 24 Sep 2026, see BENCHMARKS.md) decides which tasks are convincing.

| Device | Memory for models (measured, with headroom kept) | What it can do alone |
|---|---|---|
| IdeaPad 3 (i5-1035G1, 20 GB) | ~10–12 GB idle, 3–5 GB while building | Qwen3-14B Q4 alone; gpt-oss-20b MXFP4 (12.1 GB) when quiet |
| POCO F5 (SM7475, 8 GB) | 1.0–1.6 GB with the user's apps open, ~2.5 GB after clearing apps | Qwen3-0.6B at 28.8 tok/s as host; 1.7B plausible |

## Task 1 — "My phone is the assistant" (works today, fast)
The phone is the **host**: the laptop stores the models and pushes one to the phone; the phone
runs it and answers; the laptop's Chat page (and any OpenAI client on `localhost:8080/v1`) talks
to it. Measured: Qwen3-0.6B, 28.8 tok/s, 251 ms first token, model copied in ~130 s once.
*Shows:* pairing, the model transfer with a live progress bar, the phone's cores lighting up, and
a private assistant that keeps working when the laptop is busy.

## Task 2 — "Bigger brain on the laptop, phone as the remote" (works today)
The laptop hosts Qwen3-8B/14B (fits alone — the planner will *refuse* to split, by design); the
phone stays paired as a client and a helper for later. Chat: summaries, code, translations,
data-file analysis (upload a CSV/JSON/text on the Chat page). With Qwen2.5-VL downloaded, image
questions too (screenshots, photos, charts).

## Task 3 — "A model neither device can hold" (the headline; needs a real link)
Laptop capped or genuinely short of memory + phone helper: Qwen3-30B-A3B (18.6 GB Q4, 14.7 GB Q3)
split by layers. **Executed** with Qwen3-0.6B as the stand-in (laptop 0–12, phone 13–27): correct
answers, ready in 18.6 s — but 0.65–1.4 tok/s over the adb-relayed USB transport. The gate for
this task is **≥ 3 tok/s**, which needs USB tethering (turned on from the phone's Settings) or a
hotspot; the planner already refuses links worse than 60 ms p95. Until that is measured, Task 3 is
demonstrated as *correct*, not as *fast*.

## What the three pages show
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
