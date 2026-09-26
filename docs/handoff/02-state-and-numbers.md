# State, measurements and how to run it

Everything here was executed. Nothing is projected unless it says so.

## Every measurement we have

### On Yuvaraj's laptop (Intel i3-7020U, 2 cores, 7.7 GB RAM, Ubuntu 24.04), 25 September

| What | Result |
|---|---|
| Laptop alone, Qwen3-0.6B Q8_0 | **23.0 to 24.9 tok/s** decode, 37 tok/s prompt, ready in 2 to 4 s |
| Split with the helper on the **same machine** (no network at all) | **19.6 tok/s**, helper holding 504 MB |
| Split laptop plus phone over **home Wi-Fi** | **4.9 tok/s**, phone holding 499 MB |
| Split over **USB tethering**, 15 of 28 layers on the phone | **8.0, 8.5, 8.3 tok/s** |
| Split over tethering, layer sweep 6 / 10 / 15 / 20 | 11.1–12.2, 11.7–11.9, 11.1–12.2, 12.2–12.4 tok/s |
| **Phone alone as host** (realme RMX5110) | **20.3 tok/s** decode, **38.3 tok/s** prompt |

**The protocol itself costs about 15%** (23.0 to 19.6 with zero network). Everything else is the
link.

### Link quality, measured

| Link | Round trip avg | Worst | Split speed |
|---|---|---|---|
| USB tethering | **0.64 to 0.77 ms** | 2.4 ms | 8 to 12 tok/s |
| Home Wi-Fi | 4.9 to 11.3 ms | 56 to 153 ms | 3.5 to 6.7 tok/s |
| adb debugging cable relay (earlier build) | 4 to 13 ms | — | 1.2 to 1.4 tok/s |
| Shared or venue Wi-Fi (earlier build) | 65 to 154 ms | — | refused by policy |

**The cable is not just faster, it is steady.** Wi-Fi bounced between 3.5 and 6.7 tok/s on identical
work because the link itself bounces.

### Phone thread and core placement, over Wi-Fi, 15 layers on the phone

| Configuration | tok/s | tok/s |
|---|---|---|
| Default threads, all cores | 6.6 | 6.0 |
| 4 threads, all cores | 3.9 | 3.0 |
| **4 threads, big cores only** | **7.7** | **8.2** |
| **2 threads, big cores only** | **8.1** | **7.9** |

Pinning to the big cores is worth about 30%; spreading threads across fast and slow cores is
actively harmful, because every step waits for the slowest thread. The app gets this for free by
running in the foreground, which earns it the big cores; our raw test through `adb shell` did not.

### Memory, measured on the realme (7.6 GB total)

- Fresh after a restart: **3.03 GB available**. Under load with apps open: **0.48 GB**.
- With the proportional reserve (15%, so 1.14 GB), it offers about **1.9 GB** for layers. Under the
  old flat 1.5 GB reserve it offered 0.6 GB and the planner refused it.

### From the earlier build, for reference

Phone as host 28.8 tok/s on a POCO F5; split 7.1 to 8.9 tok/s over home Wi-Fi; laptop alone
Qwen3-8B at 2.9 to 3.6 tok/s; image chat with Qwen2.5-VL-3B at 150 s per screenshot; speech to text
with whisper base.en at 1.5 s per 11 s clip; image generation 18 minutes per image and text to
speech 42 s, both excluded as too slow.

## Bugs found and fixed on 25 September

1. **`--fit off` passed to an engine that does not have it.** llama.cpp exits with status 0 on an
   unknown flag, which reads as a crash with no message. Fixed: the service asks the binary what it
   supports and drops what it does not, logging the drop.
2. **USB pairing never forwarded the host port.** When the planner made the phone the host, the
   phone downloaded the model, started the engine and served correctly while the laptop waited on
   `127.0.0.1:8081` with nothing forwarded, failing minutes later as "device disconnected". Fixed.
3. **Flat memory reserve.** See above. Fixed with a proportional rule and tests.
4. **Chip gate required `i8mm`.** ARMv8.2 phones (Cortex-A78, Dimensity 7400) do not have it and
   were refused. Fixed: the gate requires `dotprod`, and a matching build decides the rest.
5. **Two silent failures in the Android build script.** `set -e` plus a false test killed the script
   with no output: the ninja probe, and a command substitution inside the build-dir name. Both had
   only ever run on a machine with ninja installed.

## Setting up from scratch on a new machine

Tested end to end on 25 September; expect 40 minutes plus downloads.

```bash
# 1. free disk. builds plus a model need 10 GB or so
df -h /

# 2. the engine
git clone --depth 1 https://github.com/ggml-org/llama.cpp
cd llama.cpp
cmake -B build -DCMAKE_BUILD_TYPE=Release -DGGML_RPC=ON -DLLAMA_CURL=OFF
cmake --build build -j$(nproc)        # about 20 min on a dual core
# produces build/bin/{llama-server, ggml-rpc-server, llama-bench}

# 3. a model
mkdir -p ~/models && cd ~/models
curl -L -O https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf

# 4. the engine for phones (needs Android NDK r28+)
ANDROID_NDK_HOME=~/Android/ndk/28.2.13676358 LLAMA_SRC=~/llama.cpp \
  bash scripts/android-build-llama.sh
# for a chip WITHOUT i8mm (Dimensity 7400 and similar), build with
#   -DCMAKE_C_FLAGS="-march=armv8.2-a+dotprod" and the same for CXX
bash scripts/android-sync-natives.sh    # copies them into the app as lib*.so

# 5. the service
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
cargo build --manifest-path desktop/Cargo.toml       # about 2 min

# 6. the app
cd android && ANDROID_HOME=~/Android ./gradlew assembleDebug    # 8 min first time
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

**Run it:**

```bash
MESHAI_API_TOKEN=demo ./desktop/target/debug/meshd \
  --models ~/models --llama-bin ~/llama.cpp/build/bin \
  serve --lan --api-token demo
# panel: http://localhost:8080/admin/?token=demo
```

**Ask it what it would do, without running anything:**

```bash
./desktop/target/debug/meshd --models ~/models --llama-bin ~/llama.cpp/build/bin \
  plan Qwen3-0.6B-Q8_0.gguf
```

## Traps, learned the hard way

- **Build everything once with the network on, then verify it builds offline.** Gradle failed
  offline because the Kotlin plugin had never been downloaded. At a venue that is a lost hour.
- **`/health` answers 503 while the model is still loading.** Check for 200, not for a response.
- **Qwen3 writes its reasoning first.** Without `--reasoning off` a short token limit produces an
  empty answer.
- **On Linux, adb needs a udev rule** or the phone shows "no permissions":
  `SUBSYSTEM=="usb", ATTR{idVendor}=="<vendor>", MODE="0666", GROUP="plugdev"` in
  `/etc/udev/rules.d/`, then reload and replug. realme and OPPO are `22d9`. **The loaner will have a
  different vendor id, so do this at check-in.**
- **realme and OPPO also need** "Install via USB" on and "Verify apps over USB" off, or installs
  fail with `INSTALL_FAILED_VERIFICATION_FAILURE`.
- **Enabling USB debugging can switch off USB tethering**, and switching USB mode can break adb.
  Pair first, then change the link.
- **Do not trigger pairing while a run is in progress.** It relaunches the app and kills the run.
- **Phone memory collapses under pressure.** The realme went from 3.03 GB available to 0.48 GB and
  restarted itself. Close apps and keep the app in front.
- **RAM expansion is not memory.** That 12 GB figure on a 8 GB phone is storage used as overflow,
  far too slow for weights. Count physical free memory only.
