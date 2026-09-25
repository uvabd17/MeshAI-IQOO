#!/usr/bin/env bash
# Measure a laptop+phone layer split over whatever link is up, with no app involved.
#
# What it does, in order:
#   1. reads the phone's CPU features and free memory, and refuses to continue
#      if the binaries were built for instructions this chip does not have
#   2. pushes the llama.cpp helper and its libraries to the phone
#   3. starts the helper ON the phone, bound to the phone's own address
#   4. starts llama-server on the laptop pointing at it
#   5. asks one question and reports tokens per second, next to the laptop-only number
#
# Usage:
#   scripts/phone-split-test.sh <model.gguf> [layers_on_phone] [phone_ip]
#
# phone_ip defaults to the USB tethering gateway. With no tethering, pass the
# phone's Wi-Fi address, or use `adb reverse` instead (slower: adb relays).
set -uo pipefail

MODEL="${1:?usage: phone-split-test.sh <model.gguf> [layers_on_phone] [phone_ip]}"
NGL="${2:-15}"
BUILD="${PHONE_BUILD:-$HOME/llama.cpp/build-android-dotprod}"   # must match the phone's ISA
HOST_BIN="${HOST_BIN:-$HOME/llama.cpp/build/bin}"
REMOTE=/data/local/tmp/mesh
PORT=50052

say() { printf '\n\033[1m== %s\033[0m\n' "$*"; }

say "phone"
adb wait-for-device
MODEL_NAME=$(adb shell getprop ro.product.model | tr -d '\r')
SOC=$(adb shell getprop ro.soc.model | tr -d '\r')
FEATURES=$(adb shell "grep -m1 Features /proc/cpuinfo" | tr -d '\r')
AVAIL_KB=$(adb shell "grep MemAvailable /proc/meminfo" | awk '{print $2}' | tr -d '\r')
echo "model     : $MODEL_NAME ($SOC)"
echo "features  : $FEATURES"
printf 'free mem  : %.2f GB\n' "$(echo "$AVAIL_KB/1048576" | bc -l)"

case "$BUILD" in
  *i8mm*|*arm64-v8a) NEED=i8mm ;;
  *) NEED=asimddp ;;
esac
if ! grep -q "$NEED" <<<"$FEATURES"; then
  echo "STOP: this build needs '$NEED' and the chip does not report it. It would crash."
  echo "      Build a variant without it, or use a phone that has it."
  exit 1
fi

say "link"
PHONE_IP="${3:-$(ip route | awk '/dev enx/ && /default/ {print $3; exit}')}"
if [ -z "$PHONE_IP" ]; then echo "no tethering interface found; pass the phone IP"; exit 1; fi
echo "phone address: $PHONE_IP"
ping -c 40 -i 0.05 -q "$PHONE_IP" 2>&1 | tail -2

say "pushing the engine to the phone"
adb shell "mkdir -p $REMOTE"
adb push "$BUILD/bin/ggml-rpc-server" "$REMOTE/" >/dev/null
adb push "$BUILD/bin/"*.so "$REMOTE/" >/dev/null
adb shell "chmod 755 $REMOTE/ggml-rpc-server"
echo "pushed $(adb shell "ls $REMOTE | wc -l" | tr -d '\r') files"

say "starting the helper on the phone"
adb shell "pkill -f ggml-rpc-server" 2>/dev/null
adb shell "cd $REMOTE && LD_LIBRARY_PATH=$REMOTE nohup ./ggml-rpc-server -H 0.0.0.0 -p $PORT > rpc.log 2>&1 &" &
sleep 4
adb shell "cat $REMOTE/rpc.log" | head -8

say "starting the engine on the laptop"
pkill -x llama-server 2>/dev/null; sleep 1
"$HOST_BIN/llama-server" -m "$MODEL" -c 2048 --host 127.0.0.1 --port 8090 \
  --rpc "$PHONE_IP:$PORT" -ngl "$NGL" --reasoning off > /tmp/phone-split.log 2>&1 &
for i in $(seq 1 120); do
  [ "$(curl -s -o /dev/null -w '%{http_code}' -m 2 http://127.0.0.1:8090/health)" = "200" ] && break
  pgrep -x llama-server >/dev/null || { echo "engine died:"; tail -20 /tmp/phone-split.log; exit 1; }
  sleep 1
done
echo "ready in ${i}s with $NGL of the layers on the phone"

say "asking one question"
curl -s http://127.0.0.1:8090/v1/chat/completions -H 'content-type: application/json' \
  -d '{"messages":[{"role":"user","content":"Name three Indian cities."}],"max_tokens":60}' |
python3 -c "
import json,sys
d=json.load(sys.stdin); t=d.get('timings',{})
print(d['choices'][0]['message']['content'].strip()[:200])
print()
print(f\"split over the link : {t.get('predicted_per_second',0):.1f} tok/s\")
print(f\"prompt reading      : {t.get('prompt_per_second',0):.1f} tok/s\")
"

say "what the phone actually held"
adb shell "ps -A -o RSS,NAME | grep ggml-rpc-server" | awk '{printf "phone helper memory: %.0f MB\n", $1/1024}'

say "cleanup"
adb shell "pkill -f ggml-rpc-server" 2>/dev/null
pkill -x llama-server 2>/dev/null
echo "done"
