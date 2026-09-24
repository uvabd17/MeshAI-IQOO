#!/usr/bin/env bash
# Phase 0 step 2 — first laptop <-> phone layer split over llama.cpp RPC.
# Pushes the arm64 binaries + a small GGUF to a physical phone via adb, starts the RPC
# worker there, and runs llama-server / llama-bench on the laptop against it.
#
# Prereqs: scripts/android-build-llama.sh done; a *physical* arm64 phone with USB debugging;
#          laptop llama.cpp built natively with -DGGML_RPC=ON (build-host/); phone and laptop on the
#          same private link (phone hotspot, laptop hotspot, or USB tethering).
#
# Usage: scripts/phase0-split-test.sh <model.gguf> [phone_ip] [threads_on_phone]
set -euo pipefail

MODEL="${1:?path to a small GGUF (1-3B) on the laptop}"
PHONE_IP="${2:-}"
THREADS="${3:-6}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LLAMA_SRC="${LLAMA_SRC:-$ROOT/third_party/llama.cpp}"
ANDROID_BIN="$LLAMA_SRC/build-android-arm64-v8a/bin"
HOST_BIN="${HOST_BIN:-$LLAMA_SRC/build-host/bin}"
PORT=50052
REMOTE=/data/local/tmp/meshai

abi=$(adb shell getprop ro.product.cpu.abi | tr -d '\r')
[[ "$abi" == "arm64-v8a" ]] || { echo "phone ABI is '$abi' — need a physical arm64 phone, not an x86_64 emulator"; exit 1; }

echo "== phone profile =="
adb shell "getprop ro.product.model; getprop ro.soc.model; getprop ro.build.version.release; \
           grep -m1 -oE 'asimddp|i8mm|sve' /proc/cpuinfo | sort -u | tr '\n' ' '; echo; \
           grep MemTotal /proc/meminfo; grep MemAvailable /proc/meminfo; \
           cat /sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq 2>/dev/null | tr '\n' ' '; echo"

echo "== push binaries + model =="
adb shell mkdir -p $REMOTE
adb push "$ANDROID_BIN/ggml-rpc-server" "$ANDROID_BIN/llama-bench" "$ANDROID_BIN"/*.so $REMOTE/ 2>/dev/null || true
adb push "$MODEL" $REMOTE/model.gguf
adb shell chmod +x $REMOTE/ggml-rpc-server $REMOTE/llama-bench

if [[ -z "$PHONE_IP" ]]; then
  PHONE_IP=$(adb shell ip -4 addr show 2>/dev/null | grep -oE 'inet (192|10|172)\.[0-9.]+' | head -1 | awk '{print $2}')
  echo "detected phone IP: ${PHONE_IP:-<none — pass it as arg 2>}"
fi

echo "== A. phone alone (llama-bench on the phone, CPU, $THREADS threads) =="
adb shell "cd $REMOTE && LD_LIBRARY_PATH=$REMOTE ./llama-bench -m model.gguf -t $THREADS -p 512 -n 128" || true

echo "== start RPC worker on the phone (port $PORT, tensor cache on) =="
adb shell "cd $REMOTE && LD_LIBRARY_PATH=$REMOTE nohup ./ggml-rpc-server -H 0.0.0.0 -p $PORT -t $THREADS -c > rpc.log 2>&1 &"
sleep 2
adb shell "tail -5 $REMOTE/rpc.log"

echo "== RTT laptop -> phone =="
ping -c 20 -i 0.2 "$PHONE_IP" | tail -2

echo "== B. laptop alone =="
"$HOST_BIN/llama-bench" -m "$MODEL" -p 512 -n 128 || true

echo "== C. split: laptop + phone over RPC (llama-bench with --rpc) =="
"$HOST_BIN/llama-bench" -m "$MODEL" -p 512 -n 128 --rpc "$PHONE_IP:$PORT" || true

echo
echo "Record TTFT / prompt tok/s / decode tok/s for A, B, C plus RTT in docs/MESHAI.md (§12 benchmarks)."
echo "Stop the worker with:  adb shell pkill ggml-rpc-server"
