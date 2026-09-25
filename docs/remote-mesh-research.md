# Running the mesh across the internet: what the research says

Researched 25 September 2026. Every paper below was checked at source. Numbers are quoted with
their setup. Where something could not be verified it says so.

Our own measurements for comparison: 0.77 ms round trip over USB tethering giving 8 to 12 tok/s;
11 ms jittery Wi-Fi giving about 5; and the split protocol alone costing about 15% with no network
at all.

## The headline number

**Petals** (arXiv 2312.08361, NeurIPS 2023) is the only refereed system that has actually run a
large model split across the real internet. Its measurements:

| Setup | Tokens per second, single stream |
|---|---|
| 1 Gbit/s, under 5 ms (LAN-like) | 2.29 (Llama-2 70B) |
| 100 Mbit/s, 100 ms (internet-like) | 1.57 |
| Real deployment, 14 servers across two continents | **0.83** |

So going from a LAN to a 100 ms link costs about 31% of the token rate, and a real
multi-continent deployment of a 176B model lands near **one token per second**.

The closest published match to our exact architecture (arXiv 2602.16760, February 2026, single
author, no venue, so treat with care) measured **8.7 to 9.3 tok/s for a 7B model at 80 ms** using
lookahead speculation, and projects 15 to 19 tok/s at 20 ms.

**Read that honestly: over the internet we would be slower than we already are over a USB cable,
and slower than the laptop alone running a smaller model.**

## The one technique that changes the ceiling

Speculative decoding. A small model drafts several tokens; the big model verifies them in one pass.
When the draft is right you pay one round trip for several words.

- Leviathan et al. (arXiv 2211.17192, ICML 2023) measured acceptance rates of **0.71 to 0.82** for
  good draft and target pairs, giving 2 to 3 times speedup. Their formula at acceptance 0.75 with
  7 drafted tokens yields about **3.6 tokens per round trip** (our arithmetic).
- At 100 ms that lifts the network ceiling from 10 tok/s to roughly 36.
- Self-drafting variants need no second model on the phone: Medusa 2.2 to 3.6x (arXiv 2401.10774),
  EAGLE-3 up to 6.5x (arXiv 2503.01840), Lookahead 1.8 to 4x (arXiv 2402.02057).
- **prima.cpp measured 26 tok/s on a 32B model with speculative decoding on a home cluster**
  (arXiv 2504.08791), which proves it composes with a llama.cpp-style layer split.

The catch, from arXiv 2606.20591 (May 2026): the optimal number of drafted tokens grows only
**logarithmically** with delay, and acceptance falls as you draft more. You cannot buy your way out
of a slow link. The one system that tried our exact setup got **1.2 to 1.3 tokens per round trip**,
not 3.6, because n-gram speculation is much weaker than a real draft model. Closing that gap on a
phone that cannot hold a second model is a genuine open problem.

## Two findings that would sink a careless claim

**1. Activations on the wire are close to plaintext.** Dong et al. (arXiv 2507.16372, USENIX
Security 2025) invert intermediate activations back into the input text: on Llama-3-8B they recover
88 F1 at layer 2 and still 56 F1 at the final layer, and reconstruct a 4,112-token medical prompt at
86.88 F1 with 95.19% semantic similarity. Depth is not a defence. ActInv (CCS 2026, arXiv 2605.23158)
works "even in the presence of common perturbation-based defenses". Measured defences (Shredder,
ASPLOS 2020; DarkneTZ, MobiSys 2020) were evaluated on small vision models, not LLMs. Trusted
execution costs 7 to 30% and needs hardware volunteer phones do not have.

**So "it is only activations, not your text" is not a defensible claim.** On our own private link
this does not matter, because the devices belong to the same person. Across the internet, to a
stranger's device, it matters completely.

**2. Nobody has shown WAN splitting beating not splitting.** Two 2026 papers argue the opposite:
arXiv 2606.25091 shows co-located speculative decoding beats the distributed version at WAN
latencies, and arXiv 2608.14967 finds that at list prices recomputation beats wide-area transfer,
with the gap widening 12 to 19% a year. The case for splitting rests entirely on **the model not
fitting anywhere**, which is exactly our pitch, and we should keep it there.

## What is solved engineering, not research

- **Transport.** WireGuard for the tunnel; it has no NAT traversal of its own. Tailscale-style hole
  punching gets a direct connection "over 90% of the time" by their own estimate, with a relay for
  the rest. QUIC is worth taking for 0-RTT reconnection and connection migration when a phone roams,
  not for head-of-line blocking, which is irrelevant to a serial token chain.
- **Placement.** Helix (ASPLOS 2025, arXiv 2406.01566) solves it as max-flow plus MILP: 3.3x
  throughput, 66% lower prefill latency, but only **24% lower decode latency**, because the serial
  chain is irreducible. HexGen (ICML 2024, arXiv 2311.11514) ran across regions at 40 to 150 ms.
- **Checking a remote node is honest.** Cheap statistical schemes now exist: canary-trap activation
  comparison on exactly a peer-to-peer LLM pipeline (arXiv 2607.19490, AUROC 1.000, no
  recomputation), TOPLOC fingerprints at 258 bytes per 32 tokens (arXiv 2501.16007), DiFR (arXiv
  2511.20621, AUC above 0.999 from two output tokens), SVIP (arXiv 2410.22307, under 0.01 s per
  query). Cryptographic proof is not the answer: zkLLM takes **15 minutes** to prove one forward
  pass of a 13B model (CCS 2024), and Hollow-LLM (IEEE S&P 2026) shows such a proof can pass while
  the node runs degenerate cheap weights.
- **Why exact recomputation cannot be the check:** the same model and prompt give different low bits
  on different chips, so honest nodes disagree (arXiv 2609.25624). Any check must tolerate hardware
  drift while catching tampering.

## Indian network reality

Ookla, July 2026 (via a secondary source): India's multi-server latency is **51.6 ms**, cloud round
trips are 108 to 158 ms, and median 5G **upload is 15.75 Mbps**, only 7.53% of throughput, with
latency degrading 4x under load. Since a phone must upload activations every round trip, the uplink
is the binding constraint, and only one paper (arXiv 2608.04974, August 2026, unrefereed) targets it.

Carrier-grade NAT is ubiquitous on mobile: over 90% of cellular networks (Richter et al., IMC 2016),
so every phone-to-phone link would be CGNAT to CGNAT, the hardest case for hole punching. No
India-specific measurement was found.

## What this means for our roadmap claim

Say this, and it holds up:

> Near mode, on a link you own, splits layers, because the round trip is under a few milliseconds.
> Far mode never splits layers; it sends whole jobs over an encrypted tunnel, because at 30 to 250 ms
> a layer split is slower than not splitting at all. The system decides which mode a link deserves,
> from measurements it already takes.

And the genuinely novel work, if this continues past the weekend, is that decision itself: a cost
model that chooses between single device, layer split, speculative split and whole-job offload from
measured latency, memory and acceptance rate. Nobody ships that.
