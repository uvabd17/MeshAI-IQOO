# Build walkthrough, 25 September 2026

Written as it happened, on Yuvaraj's laptop (Intel i3-7020U, 2 cores, 7.7 GB RAM, Ubuntu 24.04), for someone who has not done any of this before. Every command is the real one that ran, and every number is the real output.

Branch: `mesh-v3`, cut from `native-mesh-v2`.

---

## Step 0. Making room

The laptop had 4.4 GB free out of 98 GB. The model files alone need more than that, so the first job was finding space that could safely go.

```
$ du -sh ~/.cache/* | sort -rh | head -6
2.0G  ~/.cache/BraveSoftware
1.6G  ~/.cache/google-chrome-headless
1.3G  ~/.cache/ms-playwright
1.3G  ~/.cache/google-chrome
863M  ~/.cache/hyperframes
230M  ~/.cache/pip
```

A **cache** is a copy of something kept nearby to save fetching it again. Deleting one costs nothing but a slower first load. Three browser caches plus the npm download cache were removed:

```
$ df -h /          # before
/dev/sda4  98G  89G  4.4G  96% /
$ rm -rf ~/.cache/BraveSoftware ~/.cache/google-chrome ~/.cache/google-chrome-headless
$ npm cache clean --force
$ df -h /          # after
/dev/sda4  98G  79G   15G  85% /
```

Kept on purpose: `~/.gradle` (6.9 GB), which holds the Android build packages we will need if the venue has no internet, and `~/Android/ndk` (2.2 GB), which builds the phone version of llama.cpp. Also untouched: `~/.claude`, which holds conversation history and is not a cache.

**Lesson for the event:** check free disk before anything else. On the other laptop the root disk filled up mid-project and build folders had to be moved.

---

## Step 1. Getting the engine

MeshAI does not do the AI maths itself. **llama.cpp** does, an open-source C++ program that runs models on ordinary hardware. We compile it and build our product around it.

```
$ git clone --depth 1 https://github.com/ggml-org/llama.cpp
$ cd llama.cpp
$ git log -1 --format='commit %h %ad' --date=short
commit b248f4a 2026-09-25
```

`--depth 1` fetches only the latest snapshot instead of the whole history, which is faster and smaller.

Then configuring the build:

```
$ cmake -B build -DCMAKE_BUILD_TYPE=Release -DGGML_RPC=ON -DLLAMA_CURL=OFF
```

What those switches mean:

| Switch | Why |
|---|---|
| `-B build` | put the generated files in a folder called `build` |
| `-DCMAKE_BUILD_TYPE=Release` | optimise for speed, not for debugging |
| `-DGGML_RPC=ON` | **the important one.** RPC is what lets one machine use another machine's memory and compute. Without it there is no MeshAI |
| `-DLLAMA_CURL=OFF` | do not link the network download library we do not need |

Compiling took about twenty minutes on this laptop and produced:

```
$ ls ~/llama.cpp/build/bin | grep -E 'llama-server|ggml-rpc-server|llama-bench'
ggml-rpc-server      the helper: holds some layers, computes when asked
llama-server         the host: loads a model, answers over HTTP
llama-bench          a speed measuring tool
```

Note the helper is called `ggml-rpc-server` in the current version. Older notes say `rpc-server`; that name is gone.

---

## Step 2. Getting a model

```
$ curl -L -o ~/models/Qwen3-0.6B-Q8_0.gguf \
  https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf
```

640 MB, about three minutes at 4 MB/s.

Reading the name:
- **Qwen3** is the model family, made by Alibaba, weights published openly.
- **0.6B** means 600 million parameters. Parameters are the numbers inside the model that were learned during training.
- **Q8_0** means each parameter is stored in 8 bits instead of 16, halving the size for almost no loss of quality. This is called quantisation.
- **GGUF** is the file format llama.cpp reads: one file containing a header that describes the model plus all the numbers.

The smallest useful model was chosen deliberately: it fits anywhere, including a phone, so every mechanism can be proven quickly. Bigger models change the numbers, not the mechanism.

---

## Step 3. Reading a model file without loading it

The first piece of our own code: `tools/inspect/gguf.py`.

To decide where layers should run, we need to know how big each layer is. Loading a 12 GB model to find out would be absurd, so we read only the header.

```
$ python3 tools/inspect/gguf.py ~/models/Qwen3-0.6B-Q8_0.gguf
Qwen3-0.6B-Q8_0.gguf: qwen3, 28 layers, weights 0.59 GB, fixed parts 0.15 GB, cache 112 KB per token
  trained context: 40960, hidden size: 1024
  needs at 4k context: 1.03 GB
  per layer: 16 to 16 MB
```

What each number means:

- **28 layers.** A model is a stack of processing blocks. Data goes through block 1, then 2, and so on. Layers are the unit we can split across devices.
- **weights 0.59 GB.** The layers themselves.
- **fixed parts 0.15 GB.** The embedding table (which turns words into numbers) and the output head (which turns numbers back into word probabilities). These stay on the host and never move, because moving the head would mean sending the score for every word in the vocabulary across the network for every token.
- **cache 112 KB per token.** As the model reads a conversation it keeps notes so it does not redo work. Those notes are the KV cache. At 4,000 tokens of conversation this model needs about 0.45 GB of notes on top of its weights. That is why the total at 4k context is 1.03 GB, not 0.74.
- **16 MB per layer.** The unit of currency when splitting: a phone with 320 MB free can hold 20 of these.

The technique matters: layer sizes are derived from the distance between tensors in the file, not from understanding the compression format. So a new format next month needs no code change.

---

## Step 4. Deciding where a model runs

Second piece of our own code: `tools/inspect/planner.py`. Given a model and a list of devices, it returns one of three answers, each with a reason.

Four scenarios, real output:

```
--- laptop alone
    runs on this laptop alone, all 28 layers
    fits on this laptop with 3.22 GB to spare

--- laptop plus a fast phone
    runs on phone alone, all 28 layers
    refused this laptop: not needed: the model already fits on phone,
    another device would only add waiting

--- laptop plus a phone on bad wifi
    runs on this laptop alone, all 28 layers
    refused phone on venue wifi: link too slow (120 ms round trip,
    limit 60 ms): every token would wait for it

--- laptop 0.5GB + phone 0.6GB          (memory squeezed on purpose)
    split across 2 devices: phone layers 0 to 18, laptop layers 19 to 27
```

The second case is the interesting one. Nobody told it to prefer the phone. It measured, found the phone faster, and left the laptop out with an explanation. That is the product in one line.

The third case is the safety rule. Every token has to make one round trip to a helper device. At 120 ms per round trip, a hundred-token answer spends twelve seconds just waiting. So the planner refuses. Measured numbers behind the 60 ms limit: shared Wi-Fi 65 to 154 ms, enterprise Wi-Fi 250 ms at the 99th percentile (Sui et al., MobiSys 2016), USB cable 2 to 5 ms.

The fourth case cuts the model in two contiguous runs. Contiguous matters: every boundary between devices costs one network crossing per token, so you want as few as possible.

---

## Step 5. The first real measurement

Starting the engine by hand, to see what it does:

```
$ ~/llama.cpp/build/bin/llama-server \
    -m ~/models/Qwen3-0.6B-Q8_0.gguf \
    -c 2048 --host 127.0.0.1 --port 8081
```

- `-m` the model file
- `-c 2048` how many tokens of conversation it can hold
- `--host 127.0.0.1` listen only on this machine. llama.cpp's own documentation warns never to expose it on an open network
- `--port 8081` where it listens

**A trap found immediately.** Asking whether it is ready:

```
$ curl -s http://127.0.0.1:8081/health
{"error":{"message":"Loading model","type":"unavailable_error","code":503}}
```

It answers *while still loading*. So "did it respond" is the wrong question; the right one is "did it answer 200". Checking wrongly here is how a demo shows an empty screen at the worst moment.

Once ready, the first question:

```
$ curl -s http://127.0.0.1:8081/v1/chat/completions \
  -H 'content-type: application/json' \
  -d '{"messages":[{"role":"user","content":"In one sentence: what is a hidden state in a transformer?"}],"max_tokens":80}'
```

Result: ready in 4 seconds, **22.8 tokens per second** generated, 37 tokens per second reading the prompt.

**A second trap, and a worse one.** The answer came back empty, despite using all 80 tokens. Qwen3 is a "thinking" model: it writes its reasoning first and the answer afterwards. With an 80 token limit it spent the budget thinking and never got to the reply. On stage that looks like a broken product. Fix: allow more tokens, or switch thinking off.

---

## Step 6. Building the laptop service

Rust was not installed, so first:

```
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
$ cargo build --manifest-path desktop/Cargo.toml
   Compiling ... 199 crates ...
    Finished `dev` profile in 2m 15s
```

Running it, pointed at our own model folder and our own llama.cpp build:

```
$ MESHAI_API_TOKEN=demo ./desktop/target/debug/meshd \
    --models ~/models --llama-bin ~/llama.cpp/build/bin \
    serve --lan --api-token demo
```

Asking it what it would do with the model, before running anything:

```
$ ./desktop/target/debug/meshd --models ~/models --llama-bin ~/llama.cpp/build/bin \
    plan Qwen3-0.6B-Q8_0.gguf

Qwen3 0.6B Instruct runs entirely on this laptop (1.4 GB of 4.0 GB usable,
incl. 300 MB compute reserve)
  Host: fits on one device: needs 1.1 GB (weights 0.6 GB + KV 0.5 GB @ 4096 ctx)
        + 300 MB compute reserve = 1.4 GB, has 4.0 GB usable — no network in the path
```

It also prints the exact command it would run. One flag there is worth noticing: `--reasoning off`. That is the fix for the empty-answer trap from step 5, already learned and encoded.

Then starting a run and asking a question through the one address:

```
$ curl -X POST -d '{"model":"Qwen3-0.6B-Q8_0.gguf","n_ctx":4096}' .../api/run
$ curl -X POST -d '{"messages":[...]}' .../v1/chat/completions
generated 31 tokens at 23.0 tok/s
```

**Laptop baseline: 23.0 tokens per second.**

---

## Step 7. Building llama.cpp for the phone

A phone has a different processor family (ARM), so the engine must be cross-compiled with the Android NDK.

The build script failed instantly, twice, with no error message. Both were the same class of bug, worth understanding because it cost twenty minutes:

The script begins with `set -e`, meaning "stop if any command fails".

```bash
command -v ninja >/dev/null && GEN=Ninja          # fails on a machine without ninja
BUILD=".../build-android-$ABI$([[ $OPENCL == 1 ]] && echo -opencl)"   # test is false in a normal build
```

A test that returns "no" counts as a failed command, so the script exited silently both times. It had only ever run on a machine where ninja was installed. Both are fixed on this branch.

Then it built:

```
$ ANDROID_NDK_HOME=~/Android/ndk/28.2.13676358 LLAMA_SRC=~/llama.cpp \
    bash scripts/android-build-llama.sh
[219/219] Linking CXX executable bin/llama-server
```

Verifying the result is genuinely a phone binary:

```
$ file bin/ggml-rpc-server
ELF 64-bit LSB pie executable, ARM aarch64
$ llvm-readelf -l bin/ggml-rpc-server | grep LOAD
LOAD align: 0x4000
```

ARM, not Intel, and 16 KB page alignment, which recent Android requires.

Staging them into the app:

```
$ bash scripts/android-sync-natives.sh
arm64-v8a: 13 files, 18M
```

Executables are renamed `lib*.so` on purpose: Android only unpacks files with that shape and gives them permission to run.

Building the app:

```
$ ./gradlew assembleDebug --offline
FAILURE: Plugin [id: 'org.jetbrains.kotlin.android', version: '2.1.20'] was not found
```

**This is the failure we most need to avoid at the venue.** Offline mode could not find a build plugin that was never downloaded on this machine. With no internet on Saturday, that is a lost Green Light block. Building once with the network fixed it permanently:

```
$ ./gradlew assembleDebug
BUILD SUCCESSFUL in 7m 58s
app-debug.apk   67.9 MB

$ ./gradlew assembleDebug --offline      # the proof that matters
BUILD SUCCESSFUL in 15s
```

---

## Step 8. Proving the split without a phone

The phone was not attached yet, so the helper was run on this same laptop. That removes the network entirely and isolates one question: what does splitting cost by itself?

```
$ ggml-rpc-server -H 127.0.0.1 -p 50052 &
Starting RPC server v7.0.0
Devices: CPU: Intel(R) Core(TM) i3-7020U (7840 MiB free)

$ llama-server -m Qwen3-0.6B-Q8_0.gguf -c 2048 --port 8082 \
    --rpc 127.0.0.1:50052 -ngl 15 --reasoning off
```

`-ngl 15` hands fifteen of the twenty-eight layers to the helper.

Result:

| Setup | Speed |
|---|---|
| Laptop alone | 23.0 tok/s |
| Split, helper on the same machine, no network | **19.6 tok/s** |

**A 15% loss with zero network latency.** That is the cost of the split protocol itself: the floor. Every real network cost adds on top. It matches the shape of llama.cpp issue #22850, which measured 28 to 55% loss over a wired link.

And the proof that layers really moved, rather than the number being an illusion:

```
$ ps -o pid,rss -C ggml-rpc-server
504 MB        # the helper is holding model layers in its own memory
$ ps -o pid,rss -C llama-server
570 MB
```

---

## What this session established

1. The whole chain works on a weak laptop: 23.0 tok/s on a 0.6B model.
2. The planner's decisions are real and explainable, including refusing devices.
3. Splitting costs about 15% before any network is involved.
4. The phone binaries build correctly, with the right architecture and alignment.
5. The app builds, and now builds offline, which it could not do an hour ago.
6. Two silent bugs in the build script are fixed.

## Still open

- Nothing has run on a phone yet.
- Office Kit cannot run on Ubuntu, which costs 10% of the score and the Red Light bridge. See `docs/office-kit.md`.
- The gate measurement, a model that fits neither device split over a real link, is still unmeasured.
