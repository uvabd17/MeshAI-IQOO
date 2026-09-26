# MeshAI handoff bundle

Written 26 September 2026, the morning of the Hyderabad City Battle. Everything a person or
another machine needs to pick this up cold. Each file stands alone; read them in order if you know
nothing.

| File | What it holds |
|---|---|
| `01-what-this-is.md` | The product, the fundamentals behind it, and the architecture |
| `02-state-and-numbers.md` | What is built, every measurement taken, and how to run it |
| `03-event-day.md` | Schedule, rules, judging, venue, what to bring |
| `04-red-light-playbook.md` | **What to do during the 10.5 hours the laptop is closed** |
| `05-pitch-and-defence.md` | The story, the claims that hold, and answers to hard questions |
| `06-open-problems.md` | The roadmap, what the research says, what is genuinely unsolved |

## The one-paragraph version

MeshAI pools the free memory of a laptop and one or more Android phones over a private link, so a
model too big for either device can run across both, and exposes it as one local OpenAI-compatible
endpoint so existing tools work unchanged. A planner decides where each layer goes and refuses
devices that would make things worse. Everything runs on hardware the user already owns; nothing
leaves their devices.

## The three numbers that matter

- **Phone alone runs a model at 20.3 tokens per second** (realme RMX5110, Dimensity 7400, 8 GB),
  against 23 to 25 on an i3 laptop, and it reads prompts *faster* than the laptop. The phone is not
  a helper, it is a peer.
- **A split across laptop and phone works**: 4.9 tok/s over Wi-Fi, **8 to 12 over a USB tethering
  link** measured at 0.77 ms round trip.
- **Splitting costs about 15% before any network exists**, measured with the helper on the same
  machine. Every network cost stacks on top of that.

## Today's hardware, confirmed on the day

**Two 16 GB iQOO 15 loaner phones** (Snapdragon 8 Elite Gen 5, which has `i8mm`, so they run the
standard arm64 build rather than the reduced one). With everything closed each should offer roughly
8 to 9 GB for layers after the proportional reserve.

| Setup | Pooled memory | What fits |
|---|---|---|
| Laptop + one phone | ~11 to 12 GB | gpt-oss-20b (12.1 GB), tight |
| Laptop + two phones | ~20 GB | Qwen3-Coder-30B-A3B (18.6 GB), the deck's headline |
| **Two phones, no laptop** | ~16 GB | **gpt-oss-20b on phones alone** |

That last row is the demo nobody else will have: a 20-billion-parameter model running on two phones
with the laptop only coordinating. It is the purest form of "phone-first" and the rubric rewards
exactly that.

**The binding constraint is the download, not the memory.** gpt-oss-20b is 12.1 GB and the 30B is
18.6 GB. Get them onto a laptop disk before relying on venue internet, and check free space first.

## The rules we are building under

Code must be written during the event window, so the older branch is not used at the venue.
Open-source libraries are fine with attribution. Fresh repository, first commit after 10:00
Saturday, frequent timestamped commits. Third-party components to declare: llama.cpp and ggml
(MIT), the model weights (Apache 2.0), Android libraries, vivo Office Kit.

## Where everything lives

- This repository, branch `mesh-v3`, cut from `native-mesh-v2`.
- `docs/MESHAI.md` is the long-standing source of truth from the earlier build: design, decisions
  D001 to D035, risks K01 to K40, every benchmark with conditions.
- `docs/walkthrough-25sep.md` is a step-by-step record of building the whole thing from scratch on
  a second laptop, with the real commands and outputs.
- `docs/office-kit.md`, `docs/event-day.md`, `docs/feature-ideas.md`,
  `docs/remote-mesh-research.md` are the research from 25 September.
- `~/IQOO/research/` holds the hackathon intel and the technical evidence file.
- `~/IQOO/v2/` holds the submitted deck and the specification PDFs.
