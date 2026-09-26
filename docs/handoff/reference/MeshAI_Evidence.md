# MeshAI: Technical Evidence (checked 22 Sep 2026)

Every source was opened and checked. Numbers marked [derived] are our own arithmetic, not measurements.

## 1. Distributed inference on edge devices

| System | Source | Setup | Result |
|---|---|---|---|
| LinguaLinked | arXiv 2312.00388 (2023) | 4 Android phones, BLOOM up to 3B | 1.11 to 1.61x over their own baseline distributed placement, not over a single device. About 3.6 s per token. |
| EdgeShard | arXiv 2405.14371 (2024) | Jetson Orin cluster plus RTX 3090 | Llama2 7B: 75.9 ms/token vs 140.3 ms/token on one device. 70B: 3.1 s/token. |
| Galaxy | arXiv 2405.17245, INFOCOM 2024 | Jetson Nano | 1.3 to 2.5x, but only for small single-pass models. |
| Jupiter | arXiv 2504.08242, INFOCOM 2025 | Edge devices | Up to 26x, coming from speculative/outline decoding, not plain layer split. |
| TPI-LLM | arXiv 2410.00531 (2024) | 4 laptops on home Wi-Fi (3 to 7 ms, 320 to 610 Mbps) | 70B at 26.1 s/token with 3.1 GB per device. Pipeline parallelism leaves devices idle. |
| prima.cpp | arXiv 2504.08791 (2025/26) | 4 home devices on Wi-Fi, including Android phones under Termux | ms/token, llama.cpp vs prima.cpp: 8B 15 vs 15, 14B 20 vs 20, 30B 202 vs 72, 70B 10120 vs 674. For small models its scheduler puts all layers on the single strongest device. |
| Petals | arXiv 2209.01188, NeurIPS 2023 follow-up | Consumer GPUs over the internet | Up to 10x faster than offloading to disk. |

**What this means:**
- **Layer splitting does not speed up decode** when one device can already hold the model.
- **The wins come from** avoiding swap or out-of-memory failures, from speculative or tensor parallel methods, or from batch throughput.

## 2. Phones: speed, heat and RAM

**Speed and heat**
- MELTing Point (arXiv 2403.12844, MobiCom 2024): on-device LLM decode is memory bound.
  - Throughput drops in steps under DVFS throttling.
  - Sustained power stays under 8.5 W on a Galaxy S23.
- Qualcomm IWOCL 2025 slides (Snapdragon 8 Elite Gen 1, Adreno 830, llama.cpp OpenCL, prefill/decode tok/s):
  - Llama 2 7B: 155/15
  - Llama 3 8B: 114/11
  - Gemma 2 2B: 314/21
- llama.cpp discussion #23736 (8 Elite tablet, Qwen2.5 Coder 7B Q4_K_M): GPU decode 6.6 tok/s vs CPU decode 7.95 tok/s. So the GPU is not automatically faster.
- There are no published llama.cpp numbers for the Snapdragon 8 Elite Gen 5 / Adreno 840. **We must measure.**
- GSMArena review of the iQOO 15: CPU and GPU both drop below 50% under sustained stress.

**Usable RAM**
- PowerInfer-2 (arXiv 2406.06282): a 16 GB phone had 11 GB available.
- **Realistic budget for the iQOO 15** [derived]:
  - 16 GB variant: about 9 to 11 GB
  - 12 GB variant: about 6 to 8 GB

**Android constraints**
- The low memory killer kills apps by priority.
- Low-latency Wi-Fi mode works only with the screen on and the app in the foreground.
- The Android 12+ phantom process killer affects Termux-style child processes.

## 3. llama.cpp RPC and backends
- **RPC README:** "proof of concept… fragile and insecure. Never run the RPC server on an open network."
  - It splits weights in proportion to free memory.
  - The `-c` flag caches tensors locally, so weights are not re-sent.
- **Issue #22850:** RPC on 2.5 GbE lost 28 to 55% of decode speed vs local, due to protocol and sync overhead rather than bandwidth.
- **Discussion #9136:** only a few KB of hidden state crosses per boundary per token.
- **Issue #11957:** rpc-server on Android fell back to CPU instead of OpenCL. The newer `--device` flag must be tested.
- **OpenCL docs:** Adreno 840 (8 Elite Gen 5) is listed as supported. Q4_0 is recommended.
- **Hexagon NPU backend:** experimental. There is a 3.5 GB per-session limit, and issue #25876 reports garbled output on SM8850 (v81). **Treat it as roadmap only.**
- **No published phone plus PC RPC tokens/s measurement exists.** Ours would be among the first.

## 4. Memory bandwidth
- **PC:** DDR4-3200 gives 25.6 GB/s per channel. Budget laptops often run single channel. Dual channel is 51.2 GB/s theoretical.
- **Phone:** the Snapdragon 8 Elite Gen 5 uses LPDDR5X at up to 5300 MHz, about 85 GB/s theoretical (Qualcomm brief) [derived].
- **Decode speed is roughly memory bandwidth divided by bytes read per token.** Sources:
  - Pope et al., arXiv 2211.05102
  - Yuan et al., arXiv 2402.16363
  - MELTing Point
- So on most budget setups **the phone is the faster AI device**, and the PC is the weak link.

## 5. Networks
- Enterprise Wi-Fi latency is long tailed: p90 about 20 ms, p99 about 250 ms (Sui et al., MobiSys 2016).
- Home Wi-Fi: 3 to 7 ms (prima.cpp, TPI-LLM).
- Guest and BYOD Wi-Fi commonly enables client isolation, which blocks device-to-device traffic (Cisco Meraki docs).
- **Plan:** bring our own link. Use the phone hotspot, with USB tethering as a fallback, and measure it.

## 6. India context
- IDC 2025: 152 million smartphones shipped vs 15.9 million traditional PCs (7.3 million consumer).
- Counterpoint 2025: phones above ₹30k were 22% of shipments. That is about 33 million, around 4x all consumer PCs [derived].
- NSO CAMS 2023 via Data For India: over 95% of households own a mobile phone, only about 9% own a computer.
- iQOO 15 launch price: ₹76,999 (12/256) and ₹83,999 (16/512).

## 7. Consequences for MeshAI claims
- **Do not claim** a split makes a model faster than a single device that can hold it.
- **Do not claim:**
  - 70B at interactive speed
  - works on any Wi-Fi
  - NPU acceleration
  - background operation
  - security of raw RPC
- **The claim that holds:** MeshAI runs models that do not fit on either device alone, where the PC would otherwise swap or fail. When a model fits on one device, MeshAI runs it there.
- **MoE models are the best fit.** For example Qwen3 30B A3B: about 30B total parameters but only about 3B active per token. They are heavy on memory but light on bytes read per token, so pooling memory helps a lot and the per-token cost of splitting stays small [derived, must measure].
