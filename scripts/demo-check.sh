#!/usr/bin/env bash
# Demo preflight: is this laptop + phone ready to run MeshAI on stage right now?
# Checks the running meshd, the models on disk, free memory, the phone's link, battery and thermal state,
# stray processes, and (unless --no-smoke) runs a real one-question smoke test on the laptop and reports the time.
# Usage: MESH_TOKEN=<token> scripts/demo-check.sh [--no-smoke] [--phone] [--model <file.gguf>]
# Exit code: 0 = all PASS (warnings allowed), 1 = at least one FAIL.
set -u
API=${MESH_API:-http://localhost:8080}
TOKEN=${MESH_TOKEN:-}
SMOKE=1; NEED_PHONE=0; MODEL=${DEMO_MODEL:-Qwen3-0.6B-Q8_0.gguf}
while [ $# -gt 0 ]; do case "$1" in --no-smoke) SMOKE=0;; --phone) NEED_PHONE=1;; --model) MODEL=$2; shift;; *) echo "unknown arg $1"; exit 2;; esac; shift; done
fails=0; warns=0
pass() { printf '  \033[32mPASS\033[0m %s\n' "$1"; }
warn() { printf '  \033[33mWARN\033[0m %s\n' "$1"; warns=$((warns+1)); }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$1"; fails=$((fails+1)); }
api() { curl -s -m "${2:-5}" -H "x-mesh-token: $TOKEN" -H 'content-type: application/json' "$API$1" "${@:3}"; }
gb() { awk -v b="$1" 'BEGIN{printf "%.1f GB", b/1e9}'; }

echo "== MeshAI demo check · $(date '+%Y-%m-%d %H:%M')"

echo "-- laptop"
if ! ST=$(api /api/state 3) || [ -z "$ST" ]; then fail "meshd is not answering at $API (start: MESHAI_API_TOKEN=… desktop/target/debug/meshd serve --lan)"; echo "== FAIL"; exit 1; fi
if echo "$ST" | grep -q '"mirrored":true'; then fail "this is a mirror, not the coordinator"; fi
if [ -z "$TOKEN" ]; then warn "MESH_TOKEN is empty: mutating calls and /v1 will be refused if meshd was started with a token"; fi
avail_kb=$(awk '/MemAvailable/{print $2}' /proc/meminfo); avail=$((avail_kb*1024))
lap_usable=$(echo "$ST" | jq -r '.devices[] | select(.is_local==true) | .usable_bytes // 0')
if [ "$avail" -ge 6000000000 ]; then pass "free memory $(gb $avail) (planner offers $(gb $lap_usable))"; else fail "only $(gb $avail) free (planner offers $(gb $lap_usable)) — close heavy apps"; fi
gd=$(pgrep -fc 'GradleDaemon|kotlin-compiler-embeddable|kotlin-build-tools' || true)
[ "${gd:-0}" -gt 0 ] && warn "$gd idle Gradle/Kotlin build daemons are running — they held 10.7 GB once; stop them (gradle --stop / kill)" || pass "no build daemons holding memory"
stray=$(pgrep -fc 'llama-server|ggml-rpc-server|llama-bench' || true)
run_status=$(echo "$ST" | jq -r '.run.status')
if [ "$run_status" = "idle" ] && [ "${stray:-0}" -gt 0 ]; then warn "$stray llama processes running while meshd is idle — stray? (pkill -x llama-server / ggml-rpc-server)"; else pass "processes: run status '$run_status', $stray llama processes"; fi
tempc=$(cat /sys/class/thermal/thermal_zone*/temp 2>/dev/null | sort -n | tail -1); [ -n "${tempc:-}" ] && { t=$((tempc/1000)); [ "$t" -lt 80 ] && pass "CPU temperature ${t} °C" || warn "CPU temperature ${t} °C — hot; let it cool before a long run"; }
if command -v adb >/dev/null; then
  emu=$(adb devices | awk 'NR>1 && /^emulator-/{c++} END{print c+0}'); [ "$emu" -gt 0 ] && warn "$emu Android emulator(s) attached — never used by MeshAI, but they eat RAM and confuse adb; close them" || true
fi

echo "-- models"
for m in "$MODEL" Qwen3-8B-Q4_K_M.gguf Qwen2.5-VL-3B-Instruct-Q4_K_M.gguf; do
  if echo "$ST" | jq -e --arg f "$m" '.models[] | select(.file==$f)' >/dev/null; then pass "$m on disk"; else warn "$m missing — the download is slow tonight (Hugging Face ~1 MB/s); fetch models before the demo"; fi
done
[ "$MODEL" != "Qwen3-0.6B-Q8_0.gguf" ] || true
if echo "$ST" | jq -e '.models[] | select(.file=="Qwen2.5-VL-3B-Instruct-Q4_K_M.gguf")' >/dev/null; then
  [ -f "$(dirname "$0")/../third_party/llama.cpp/build-host/bin/llama-server" ] && true
  ls "${MESHAI_MODELS:-/mnt/storage/meshai/models}"/mmproj-Qwen2.5-VL-3B* >/dev/null 2>&1 && pass "vision projector on disk" || warn "vision projector (mmproj-Qwen2.5-VL-3B…) missing — image chat will not work"
fi

echo "-- phone"
PH=$(echo "$ST" | jq -c '[.devices[] | select(.kind=="Phone")] | .[0] // empty')
if [ -z "$PH" ]; then
  if [ "$NEED_PHONE" = 1 ]; then fail "no phone is paired"; else warn "no phone paired (fine for laptop-only tasks)"; fi
else
  name=$(echo "$PH" | jq -r .name); online=$(echo "$PH" | jq -r .online); addr=$(echo "$PH" | jq -r '.addr // ""')
  usable=$(echo "$PH" | jq -r '.usable_bytes // 0'); batt=$(echo "$PH" | jq -r '.telemetry.battery_pct // 0 | floor'); chg=$(echo "$PH" | jq -r '.telemetry.charging // false')
  rtt=$(echo "$PH" | jq -r '.telemetry.rtt_ms_p95 // 0 | . * 10 | round / 10'); therm=$(echo "$PH" | jq -r '.telemetry.thermal_status // 0')
  if [ "$online" = "true" ]; then pass "$name online"; else fail "$name is paired but offline — open the app, keep the screen on, replug the cable"; fi
  if [ "$online" != "true" ]; then
    if command -v adb >/dev/null && adb devices | grep -qE '^[0-9a-f]+\s+device'; then warn "phone is on USB: press 'Pair over USB' in the panel to bring it back"; else warn "no phone on adb either — plug the cable in with USB debugging on, then pair"; fi
  else
  case "$addr" in 127.0.0.1) link="USB cable (adb relay: split runs at ~1 tok/s; use USB tethering for speed)";; 192.168.*|10.*|172.*) link="Wi-Fi/tethering/hotspot ($addr)";; *) link="unknown ($addr)";; esac
  echo "       link: $link"
  if awk -v r="$rtt" 'BEGIN{exit !(r>0 && r<=60)}'; then pass "link RTT p95 ${rtt} ms (policy ≤ 60)"; else fail "link RTT p95 ${rtt} ms — the planner will refuse the phone as a helper (shared Wi-Fi? use a cable, tethering or a hotspot)"; fi
  if [ "$usable" -ge 1800000000 ]; then pass "phone room for models $(gb $usable)"; elif [ "$usable" -ge 900000000 ]; then warn "phone room for models only $(gb $usable) — close apps on the phone (need ≈1.9 GB for a 13-layer share of an 8B model)"; else fail "phone room for models $(gb $usable) — close apps on the phone; the planner cannot use it"; fi
  if [ "$chg" = "true" ] || [ "$batt" -ge 40 ]; then pass "battery ${batt} % (charging: $chg)"; elif [ "$batt" -ge 20 ]; then warn "battery ${batt} % and not charging — plug it in"; else fail "battery ${batt} % — plug it in now"; fi
  [ "$therm" -le 1 ] && pass "thermal status $therm" || warn "thermal status $therm — the phone is throttling; let it cool"
  if command -v adb >/dev/null && adb devices | grep -qE '^[0-9a-f]+\s+device'; then pass "USB debugging attached (one-click pairing available)"; else warn "no phone on adb — QR/Wi-Fi pairing only"; fi
  fi
fi

if [ "$SMOKE" = 1 ]; then
  echo "-- smoke test: $MODEL on the laptop alone, one question"
  if [ "$run_status" != "idle" ]; then warn "a run is active ('$run_status'); skipping the smoke test so it is not interrupted"; else
    LOCAL=$(echo "$ST" | jq -r '.devices[] | select(.is_local==true) | .id')
    t0=$(date +%s%N)
    R=$(api /api/run 120 -X POST -d "{\"model\":\"$MODEL\",\"n_ctx\":2048,\"host\":\"$LOCAL\"}")
    if ! echo "$R" | jq -e '.ok==true' >/dev/null 2>&1; then fail "run refused: $(echo "$R" | cut -c1-160)"; else
      ok=0; for i in $(seq 1 120); do s=$(api /api/state 3 | jq -r '.run.status'); [ "$s" = "ready" ] && { ok=1; break; }; [ "$s" = "error" ] && break; sleep 1; done
      t1=$(date +%s%N); load=$(( (t1-t0)/1000000 ))
      if [ "$ok" = 1 ]; then pass "model ready in $((load/1000)).$(( (load%1000)/100 )) s"
        C=$(api /v1/chat/completions 120 -X POST -d '{"model":"mesh","messages":[{"role":"user","content":"Ask me one short question about my day."}],"max_tokens":40,"temperature":0}')
        t2=$(date +%s%N); ans=$(echo "$C" | jq -r '.choices[0].message.content // empty' | tr '\n' ' ' | cut -c1-120)
        ntok=$(echo "$C" | jq -r '.usage.completion_tokens // 0'); ms=$(( (t2-t1)/1000000 ))
        if [ -n "$ans" ]; then tps=$(awk -v n="$ntok" -v ms="$ms" 'BEGIN{if(ms>0)printf "%.1f", n*1000/ms; else print "?"}'); pass "answered in $((ms/1000)).$(( (ms%1000)/100 )) s (${ntok} tokens, ≈${tps} tok/s incl. prompt): \"$ans\""; else fail "no answer from /v1: $(echo "$C" | cut -c1-160)"; fi
      else fail "model did not become ready in 120 s (status '$s')"; fi
      api /api/stop 30 -X POST -d '{}' >/dev/null; sleep 2
      left=$(pgrep -fc 'llama-server|ggml-rpc-server' || true); [ "${left:-0}" = 0 ] && pass "stopped cleanly, no llama process left" || warn "$left llama process(es) still alive after stop"
    fi
  fi
fi

echo "== $( [ $fails = 0 ] && echo READY || echo NOT READY ): $fails fail, $warns warn"
[ $fails = 0 ]
