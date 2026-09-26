# Open problems and the roadmap

Condensed from `docs/remote-mesh-research.md`, which has the full citations.

## What is left to build in the product

**Missing and promised.** The background job queue on deck slides 5 and 6 does not exist. It is the
honest answer to slow tokens: start a job, keep working, come back to the result.

**Missing and valuable.** Voice input on the phone (whisper.cpp measured 1.5 s for an 11 s clip); a
small CLI so the local endpoint is obviously real; `--mmproj` so the phone can host a vision model;
a benchmark on join so the planner can split by speed rather than memory alone; a ten-minute
sustained run to characterise throttling; encryption on the control channel.

**Measured and excluded.** Image generation at 18 minutes per image and text to speech at 42
seconds. Both too slow on this hardware. Say so rather than promising them.

## Running the mesh across the internet

This is the roadmap item that sounds easy and is not.

**The obstacle is not NAT, it is physics.** Decoding is strictly sequential: one token, one round
trip. Home links are 1 to 10 ms; between cities 30 to 80; across continents 150 to 250. Petals, the
only peer-reviewed system that has actually done this at scale, measured **0.83 tokens per second**
on a real 14-server deployment across two continents, and 1.57 at 100 ms in simulation. The closest
published match to our own architecture measured 8.7 to 9.3 tok/s for a 7B model at 80 ms.

**Read that honestly: over the internet we would be slower than we already are over a USB cable.**

**The one technique that changes the ceiling is speculative decoding.** A small model drafts several
tokens and the big model verifies them in one pass. Published acceptance rates of 0.71 to 0.82 imply
about 3.6 tokens per round trip, which lifts a 100 ms link's ceiling from 10 tokens per second to
roughly 36. prima.cpp measured 26 tok/s on a 32B model with speculation on a home cluster, proving
it composes with a llama.cpp-style layer split. The catch: the optimal number of drafted tokens
grows only logarithmically with delay, and the one system that tried our exact setup achieved 1.2 to
1.3 tokens per round trip, not 3.6, because it used weak n-gram guessing rather than a real draft
model. **Closing that gap on a phone that cannot hold a second model is a genuine open problem.**

**Two findings that constrain what we may claim:**
1. **Activations are close to plaintext.** A USENIX Security 2025 paper reconstructs input text from
   intermediate activations: 88 F1 at an early layer, still 56 at the final one, and a 4,112-token
   medical prompt recovered at 95% semantic similarity. Depth is not a defence. So "we only send
   activations" is fine between your own devices and indefensible to a stranger's device.
2. **No published result shows WAN splitting beating not splitting** when not splitting was an
   option. Our case rests entirely on the model fitting nowhere else, which is where the pitch
   already puts it.

**The claim that survives:** near mode, on a link you own, splits layers. Far mode never splits
layers; it sends whole jobs over an encrypted tunnel. The system decides which mode a link deserves,
from measurements it already takes. **That decision is the contribution**, and nobody ships it.

## Selling spare compute

Three problems sit under it, and none is the payments.

**Verification.** How do you know a stranger's phone did the work honestly? Exact recomputation
fails, because the same model gives different low bits on different chips. Cryptographic proof is
10³ to 10⁴ times too slow, and a 2026 paper shows such a proof can pass while the node runs
degenerate cheap weights. What does work today is statistical: canary queries with known-good
activations, fingerprints of the output distribution, spot checks. Several 2025 to 2026 schemes
report near-perfect detection at single-digit-percent overhead, but none has been tested on a
heterogeneous, quantised phone mesh, where hardware noise is largest. That gap is ours to fill if we
want it.

**Privacy in the other direction.** The lender's device sees the borrower's work. See the inversion
result above. Trusted execution costs 7 to 30% and needs hardware volunteer phones do not have.

**The economics may not work.** Cloud inference is already cheap per token. The defensible market is
not "cheaper than the cloud", it is "hardware you already trust, doing work you are not allowed to
send away". A subscription for managing your own or a company's fleet is more defensible than a
marketplace of strangers.

## Where the genuinely novel work is

If this continues past the weekend, the contribution is a **cost model that chooses the execution
strategy from measurements**: single device, layer split, speculative split, or whole-job offload,
decided per request from measured latency, memory, thermal state and acceptance rate. Everything
underneath it (transport, placement, honesty checks) has existing answers. That decision layer does
not.
