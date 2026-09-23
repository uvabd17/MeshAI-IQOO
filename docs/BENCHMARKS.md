# Benchmarks — measured only. A number without conditions is not a result.

Columns: date · setup · device(s) · model (quant, file GB) · ctx · threads · backend · link · **prompt t/s** · **decode t/s** · TTFT · notes (the exact pp/tg sizes are in the command column) · command · llama.cpp commit

| date | setup | devices | model | ctx | thr | backend | link | prompt t/s | decode t/s | TTFT | notes | cmd | commit |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2026-09-24 | laptop alone | IdeaPad 3 i5-1035G1 (18 GB, busy: ~4 GB free) | Qwen3-0.6B Q8_0 (0.64 GB) | bench | 4 | CPU | local | 39.4 ± 11.2 | 6.7 ± 4.6 | — | machine loaded (cargo/gradle running); high variance | `llama-bench -m Qwen3-0.6B-Q8_0.gguf -p 128 -n 32 -r 2` | 66fba63 |
| 2026-09-24 | split: laptop host + 2 local RPC workers (sim phones) | same laptop, workers `ggml-rpc-server -t 2` on 127.0.0.1:50081/50082 | Qwen3-0.6B Q8_0 | bench | 4 | CPU+RPC, `-ngl 24 -ts 0.5,0.5` | loopback | 70.9 ± 6.4 / 57.5 ± 6.4 | 12.4 ± 3.1 / 9.6 ± 0.7 | — | proves the RPC layer-split path end to end; **two rpc-servers sharing `-c` cache on one host crash `llama_context::synchronize` — run sims without `-c`** | `llama-bench --rpc 127.0.0.1:50081,127.0.0.1:50082 -ngl 24 -ts 0.5,0.5 -p 128 -n 32 -r 2` | 66fba63 |

| 2026-09-24 | split via meshd: laptop host (capped 0.35 GB) + 2 local RPC workers | IdeaPad 3 (loaded), workers `-t 2` | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 24 --tensor-split 0.32,0.38` | loopback | 25.7 (server) / 39.5 | 4.0–4.8 | 0.58–2.9 s | end-to-end through `/v1` proxy; model ready in 4.6–13 s; thinking off → 19-token direct answer | admin Chat / `curl /v1/chat/completions` | 66fba63 |

| 2026-09-24 | split via meshd (rewritten): laptop host (capped 0.35 GB) + 2 local RPC workers | IdeaPad 3, machine quiet; workers `-t 2` | Qwen3-0.6B Q8_0 | 2048 | 4 | CPU+RPC `-ngl 25 --override-tensor output\.weight=CPU --tensor-split 0.44,0.56` | loopback | 113.4 (server, 22 tok prompt) | 22.0 (server) / 19.2–21.5 (client rows) | 186–223 ms | placement verified against `load_tensors: layer N assigned to device`: CPU 0–3, RPC0 4–14, RPC1 15–27 (+ head pinned to CPU) = the plan exactly | admin Chat / `curl /v1/chat/completions` stream, max_tokens 9–26 | 66fba63 |

## Sustained (T006)
| date | device | model | minute | thermal headroom | status | current mA | tg128 | notes |
|---|---|---|---|---|---|---|---|---|

## Memory kill threshold (T006)
| date | device (RAM) | model | ctx at last success | MemAvailable before kill | headroom chosen |
|---|---|---|---|---|---|
