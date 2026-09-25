# Candidate features

Grouped by what each earns against the judging rubric: end product 30%, novelty 20%,
creative phone use 15% (device telemetry), technical depth 15%, Office Kit 10% (telemetry),
demo 10%.

## Product (end product quality, 30%)
1. **Background job queue** — start a long task, keep working, come back to the result. Promised on the deck, missing in the code. *Medium.*
2. **Small CLI** (`mesh ask "..."`, `mesh review file.py`) — fits the Developer Tools track and proves the local endpoint is real. *Small.*
3. **Point an existing coding tool at `localhost:8080/v1`** — works with tools people already use, no new code. *Tiny.*
4. **"What can I run?"** — given the devices present, list the models that become possible and what would unlock the rest. *Small.*
5. **Model recommendation on join** — suggest the best model for the current device set. *Small.*

## Phone use (15%, measured by telemetry)
6. **Voice input** — record on the phone, transcribe on device with whisper.cpp (measured 1.5 s for an 11 s clip). *Medium.*
7. **Camera document Q&A** — photograph a page or an error, ask about it. Needs `--mmproj` on the phone. *Medium.*
8. **Job-finished notification** — completes the "start it and walk away" story. *Tiny.*
9. **QR pairing with the camera** — already built; keep it visible in the demo. *Done.*

## Depth (15%)
10. **Benchmark each device on join, then split by measured speed** instead of memory alone. *Small.*
11. **Two phones plus laptop** — required for a 30B model anyway. *Medium.*
12. **Speculative decoding** with a small draft model on the phone — several tokens per round trip, the only real way past the one-round-trip-per-token limit. *Large.*
13. **8-bit KV cache on phone workers** — halves conversation memory, so more layers fit. *Small.*
14. **Encrypted control link** (Noise or TLS). *Medium.*
15. **Pause, replan and resume** when a device leaves mid-job. *Medium.*

## Privacy (supports novelty and the pitch)
16. **Airplane mode demo** — switch the internet off on stage and keep answering. *Tiny.*
17. **Consent screen on the phone** — what it will hold, for how long, before agreeing. *Small.*
18. **Traffic panel** — show that nothing leaves the paired devices. *Small.*

## Office Kit (10%, telemetry)
19. **Move the model file with Office Kit file transfer.** *Tiny, manual.*
20. **Mirror the phone dashboard onto the laptop during the demo.** *Tiny, manual.*
21. **Pairing details through the shared clipboard** instead of the QR. *Tiny, manual.*

## Shortlist if time is short
1 (job queue), 6 (voice), 2 and 3 (CLI and existing tool), 16 (airplane mode).

## Roadmap, not for the weekend
- Remote use across networks: send whole jobs over an encrypted overlay, never layers.
- Subscription for managing your own or a company's devices.
- Image generation: measured 18 minutes per image, not viable on this hardware.
