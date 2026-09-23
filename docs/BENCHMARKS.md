# Benchmarks — measured only. A number without conditions is not a result.

Columns: date · setup · device(s) · model (quant, file GB) · ctx · threads · backend · link (RTT p50/p95 ms) · pp512 t/s · tg128 t/s · TTFT s · notes · command · llama.cpp commit

| date | setup | devices | model | ctx | thr | backend | link | pp512 | tg128 | TTFT | notes | cmd | commit |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 2026-09-24 | laptop alone | IdeaPad 3 i5-1035G1 (18 GB, busy: ~4 GB free) | Qwen3-0.6B Q8_0 (0.64 GB) | bench | 4 | CPU | local | 39.4 ± 11.2 | 6.7 ± 4.6 | — | machine loaded (cargo/gradle running); high variance | `llama-bench -m Qwen3-0.6B-Q8_0.gguf -p 128 -n 32 -r 2` | 66fba63 |
| 2026-09-24 | split: laptop host + 2 local RPC workers (sim phones) | same laptop, workers `ggml-rpc-server -t 2` on 127.0.0.1:50081/50082 | Qwen3-0.6B Q8_0 | bench | 4 | CPU+RPC, `-ngl 24 -ts 0.5,0.5` | loopback | 70.9 ± 6.4 / 57.5 ± 6.4 | 12.4 ± 3.1 / 9.6 ± 0.7 | — | proves the RPC layer-split path end to end; **two rpc-servers sharing `-c` cache on one host crash `llama_context::synchronize` — run sims without `-c`** | `llama-bench --rpc 127.0.0.1:50081,127.0.0.1:50082 -ngl 24 -ts 0.5,0.5 -p 128 -n 32 -r 2` | 66fba63 |

## Sustained (T006)
| date | device | model | minute | thermal headroom | status | current mA | tg128 | notes |
|---|---|---|---|---|---|---|---|---|

## Memory kill threshold (T006)
| date | device (RAM) | model | ctx at last success | MemAvailable before kill | headroom chosen |
|---|---|---|---|---|---|
