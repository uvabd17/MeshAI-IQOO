# What MeshAI is, from first principles

Written for someone who has not worked with language models before. Skip to "The architecture" if
you have.

## The fundamentals, briefly

A language model is a large pile of numbers called **parameters** or **weights**, organised into
**layers**. Text is split into **tokens**, roughly three quarters of a word each. To produce one
token the machine runs the token through layer 1, then 2, and so on, and each layer hands the next
a small array of numbers called the **hidden state**, about 4 KB for a mid-size model.

Two phases matter. **Prefill** reads your prompt, processes many tokens at once, and wants
bandwidth. **Decode** writes the answer one token at a time, each waiting for the last, and wants
low latency. Speed is measured in tokens per second, and the wait before the first word is **time
to first token**.

To avoid redoing work, the model keeps notes on everything it has seen: the **KV cache**. It grows
with conversation length and costs real memory, about 112 KB per token for a small model, which is
why a long chat can make a model that fit stop fitting.

Weights are usually stored at 16 bits each. **Quantisation** shrinks them to about 4 or 8 bits with
little quality loss. The file format llama.cpp reads is **GGUF**: one file with a header describing
the model, followed by the numbers.

**The fact the whole project rests on:** producing a token means *reading* the active weights out of
memory, so speed is roughly memory bandwidth divided by bytes read per token. This is why a phone
can beat a laptop. A budget laptop with one memory stick manages about 25 GB/s; the Snapdragon 8
Elite Gen 5 does roughly 85 GB/s.

**Dense versus mixture of experts.** A dense model uses every parameter for every token. A
**mixture of experts (MoE)** model has many small expert sub-networks and uses a few per token:
gpt-oss-20b holds 21 billion parameters but uses about 3.6 billion per token. MoE models are heavy
to store and light to run, which is exactly the shape that suits pooling memory across devices.

## The problem

New open models need roughly 12 GB. A budget laptop has about 4 GB free; an 8 GB phone about 2 to 3
GB. Neither can load the model. A GPU upgrade often costs more than the laptop, and much
professional code and data may not be sent to a cloud service at all.

## What we built

Two apps and a page.

**The laptop service (`meshd`, Rust).** Six jobs: pairs phones, collects their telemetry, plans
where layers go, starts and stops llama.cpp with the right flags, proxies one local API, and serves
the panel.

**The phone app (Kotlin, Jetpack Compose).** Joins by QR or USB, reports free memory, temperature,
battery, core speeds and link latency every two seconds, then runs llama.cpp in one of two roles:
**host** (it loads the whole model and answers) or **helper** (it holds some layers and computes
them on request). It keeps a foreground service and the screen awake, because Android otherwise
kills background work and slows the radio.

**The panel.** Four steps: Devices, Models, Prepare, Use. It holds no logic. It shows what the
planner decided and why, and lets a human override one thing: a memory cap per device.

## How one request flows

1. The phone scans a QR code (or is handed pairing details over USB) and joins. It gets a permanent
   per-device secret, compared in constant time on every reconnect.
2. The phone reports its numbers every two seconds over a plain TCP connection carrying
   length-prefixed protobuf messages.
3. You pick a model. The planner reads the GGUF header for per-layer sizes, compares against each
   device's free memory, and chooses: run on one device, split the layers, or refuse with a reason.
4. The supervisor starts `llama-server` with `--rpc phone:port` and `-ngl`, waits for each phone to
   *report* that it is listening, then waits for a real 200 from `/health`.
5. Your request goes to `localhost:8080/v1/chat/completions`. The proxy forwards it to whichever
   engine is running. For each token the hidden state crosses to the phone and the result comes
   back.
6. If a phone gets hot, loses battery or disappears, the plan is revised.

## The rules the planner follows, and why

- **Never split a model that fits on one device.** Research agrees: prima.cpp measured no gain from
  splitting 8B and 14B models, and its own scheduler puts everything on the strongest device.
- **Refuse a device whose round trip exceeds 60 ms.** Every token waits for one round trip. Shared
  Wi-Fi measured 65 to 154 ms; a cable measured 0.77 ms.
- **Refuse a device that is too hot or nearly flat.** Phone throughput falls in steps as it
  throttles.
- **Keep each device's layers contiguous**, because every boundary costs a network crossing per
  token, and keep the output head on the host, because moving it would mean sending a score for
  every word in the vocabulary each time.
- **Reserve memory in proportion to the device**, 15% of total, floored at 0.8 GB and capped at
  2.5 GB. A flat reserve made 6 to 8 GB phones look useless.

## What we reuse, and must credit

llama.cpp and ggml (MIT) do all the inference: `llama-server` hosts, `ggml-rpc-server` helps,
`llama-bench` measures. The models are open weights (Apache 2.0). Android libraries and Jetpack
Compose. vivo Office Kit is a user-facing bridge with no API, so nothing in our code calls it.

Our own work is the layer above: pairing, profiling, planning, supervision, the single local API,
and the two interfaces.
