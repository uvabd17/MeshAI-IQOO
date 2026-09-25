#!/usr/bin/env bash
# T071 — is the per-token link cost (alpha) a fixed number per link, or does it depend on how many layers the
# phone holds and on the context size?  Runs real laptop+phone splits of a small model with the phone holding
# L layers for several L, three chats each, and records measured tok/s and first-word time per run.
#
# How the phone gets exactly L layers: the laptop's usable memory is capped (POST /api/devices/local/limit)
# so the host can hold only n_layer-L blocks + its fixed tables + compute reserve; the greedy planner then
# gives the remaining L blocks to the phone.  The cap is found by dry-running /api/plan and adjusting.
#
# Usage: MESH_TOKEN=<token> scripts/qa/alpha-experiment.sh [--model Qwen3-0.6B-Q8_0.gguf] [--shares "4 14 24"]
#        [--chats 3] [--ctx "2048"] [--ctx-extra "8192"] [--tokens 64] [--serial e1074ce7] [--no-bulk]
# Needs: meshd running (default :8080), the phone paired and online, the model on disk.  Writes rows to
# state/alpha-experiment.csv and prints a summary.  Does not edit any source or doc.
set -u
API=${MESH_API:-http://localhost:8080}; TOKEN=${MESH_TOKEN:-}
MODEL=Qwen3-0.6B-Q8_0.gguf; SHARES="4 14 24"; CHATS=3; CTX=2048; CTX_EXTRA="8192"; TOKENS=64; SERIAL=${ANDROID_SERIAL:-}; BULK=1
while [ $# -gt 0 ]; do case "$1" in
  --model) MODEL=$2; shift;; --shares) SHARES=$2; shift;; --chats) CHATS=$2; shift;; --ctx) CTX=$2; shift;;
  --ctx-extra) CTX_EXTRA=$2; shift;; --tokens) TOKENS=$2; shift;; --serial) SERIAL=$2; shift;; --no-bulk) BULK=0;;
  *) echo "unknown arg $1"; exit 2;; esac; shift; done
api() { curl -s -m "${2:-10}" -H "x-mesh-token: $TOKEN" -H 'content-type: application/json' "$API$1" "${@:3}"; }
say() { printf '\033[1m%s\033[0m\n' "$*"; }
OUT=state/alpha-experiment.csv
[ -f "$OUT" ] || echo "ts,model,ctx,phone_layers,host_layers,link_addr,rtt_p95_ms,run,prompt_tokens,prompt_tps,ttft_ms,tokens_out,tps,total_ms,ready_s" > "$OUT"

ST=$(api /api/state 5) || { echo "meshd not answering"; exit 1; }
PHONE=$(echo "$ST" | jq -r '[.devices[] | select(.kind=="Phone" and .online==true)][0].id // empty')
[ -n "$PHONE" ] || { echo "no online phone"; exit 1; }
LINK=$(echo "$ST" | jq -r --arg p "$PHONE" '.devices[] | select(.id==$p) | .addr')
NL=$(echo "$ST" | jq -r --arg m "$MODEL" '.models[] | select(.file==$m) | .info.n_layer')
FILE_B=$(echo "$ST" | jq -r --arg m "$MODEL" '.models[] | select(.file==$m) | .info.file_bytes')
[ -n "$NL" ] && [ "$NL" != null ] || { echo "model $MODEL not on disk"; exit 1; }
say "== alpha experiment · $MODEL ($NL layers, $(awk -v b=$FILE_B 'BEGIN{printf "%.2f GB", b/1e9}')) · phone $PHONE via $LINK · $(date '+%Y-%m-%d %H:%M')"

# Per-layer bytes estimate (weights only, tied embeddings assumed for small Qwen3): refined by the dry-run loop.
per_layer_gb=$(awk -v b=$FILE_B -v n=$NL 'BEGIN{printf "%.4f", (b*0.76)/n/1e9}')   # ~76 % of the file is blocks for the 0.6B [estimate]
fixed_gb=$(awk -v b=$FILE_B 'BEGIN{printf "%.4f", b*0.24/1e9 + 0.30}')            # embeddings/output + 300 MB host reserve [estimate]

find_cap() { # $1 = wanted phone layers, $2 = ctx → prints the laptop cap in GB that yields exactly that split, or "" if not found
  local want=$1 ctx=$2 host=$((NL-want)) cap got worker i
  cap=$(awk -v f=$fixed_gb -v p=$per_layer_gb -v h=$host -v c=$ctx -v n=$NL 'BEGIN{printf "%.3f", f + h*(p + c*4096/1e9) + 0.02}')
  for i in $(seq 1 14); do
    api "/api/devices/local/limit" 5 -X POST -d "{\"usable_gb\":$cap}" >/dev/null
    P=$(api /api/plan 20 -X POST -d "{\"model\":\"$MODEL\",\"n_ctx\":$ctx,\"host\":\"local\"}")
    got=$(echo "$P" | jq -r --arg p "$PHONE" '[.plan.placements[]? | select(.device_id==$p and .role=="Worker") | (.layer_end - .layer_start)] | .[0] // 0')
    err=$(echo "$P" | jq -r '.error // empty')
    if [ -n "$err" ]; then cap=$(awk -v c=$cap 'BEGIN{printf "%.3f", c+0.03}'); continue; fi
    if [ "$got" = "$want" ]; then echo "$cap"; return 0; fi
    if [ "$got" -gt "$want" ]; then cap=$(awk -v c=$cap -v d=$per_layer_gb 'BEGIN{printf "%.3f", c+d*0.9}'); else cap=$(awk -v c=$cap -v d=$per_layer_gb 'BEGIN{printf "%.3f", c-d*0.9}'); fi
  done
  echo ""
}

run_case() { # $1 = phone layers, $2 = ctx
  local want=$1 ctx=$2 cap t0 t1 ready i s row rtt
  cap=$(find_cap "$want" "$ctx")
  if [ -z "$cap" ]; then say "  ! could not find a laptop cap that gives the phone $want layers at ctx $ctx — skipping"; return; fi
  say "-- phone $want layers · ctx $ctx · laptop capped at ${cap} GB"
  t0=$(date +%s%N)
  R=$(api /api/run 180 -X POST -d "{\"model\":\"$MODEL\",\"n_ctx\":$ctx,\"host\":\"local\"}")
  echo "$R" | jq -e '.ok==true' >/dev/null 2>&1 || { say "  ! run refused: $(echo "$R" | cut -c1-200)"; return; }
  ready=0; for i in $(seq 1 240); do s=$(api /api/state 5 | jq -r '.run.status'); [ "$s" = ready ] && { ready=1; break; }; [ "$s" = error ] && break; sleep 1; done
  t1=$(date +%s%N); rs=$(( (t1-t0)/100000000 ))
  [ "$ready" = 1 ] || { say "  ! not ready after 240 s (status $s): $(api /api/state 5 | jq -r '.run.log_tail[-3:] | join(" | ")' | cut -c1-200)"; api /api/stop 30 -X POST -d '{}' >/dev/null; return; }
  hostL=$(api /api/state 5 | jq -r --arg p "$PHONE" '[.plan.placements[]? | select(.role=="Host") | (.layer_end - .layer_start)] | .[0] // "?"')
  say "  ready in $((rs/10)).$((rs%10)) s · host $hostL layers · phone $want layers"
  for i in $(seq 1 "$CHATS"); do
    api /v1/chat/completions 300 -X POST -d "{\"model\":\"mesh\",\"messages\":[{\"role\":\"user\",\"content\":\"Interview me: ask one question about my work, then wait. Question number $i.\"}],\"max_tokens\":$TOKENS,\"temperature\":0}" >/dev/null
    row=$(api /api/runs 10 | jq -c '.[-1]')
    rtt=$(api /api/state 5 | jq -r --arg p "$PHONE" '.devices[] | select(.id==$p) | .telemetry.rtt_ms_p95 // 0 | .*10 | round/10')
    echo "$row" | jq -r --arg ts "$(date +%s)" --arg m "$MODEL" --arg c "$ctx" --arg pl "$want" --arg hl "$hostL" --arg l "$LINK" --arg r "$rtt" --arg i "$i" --arg rs "$rs" \
      '[$ts,$m,$c,$pl,$hl,$l,$r,$i,.prompt_tokens,(.prompt_tps|.*10|round/10),.ttft_ms,.tokens_out,(.tps|.*100|round/100),.total_ms,($rs|tonumber/10)] | @csv' >> "$OUT"
    echo "     chat $i: $(echo "$row" | jq -r '"\(.tps|.*100|round/100) tok/s · first word \(.ttft_ms) ms · \(.tokens_out) tokens · prompt \(.prompt_tokens) @ \(.prompt_tps|.*10|round/10) tok/s"')"
  done
  api /api/stop 60 -X POST -d '{}' >/dev/null
  for i in $(seq 1 30); do [ "$(api /api/state 5 | jq -r '.run.status')" = idle ] && break; sleep 1; done
}

for L in $SHARES; do run_case "$L" "$CTX"; done
mid=$(echo "$SHARES" | awk '{print $((NF+1)/2)}'); mid=${mid:-14}
for c in $CTX_EXTRA; do run_case "$mid" "$c"; done
api "/api/devices/local/limit" 5 -X POST -d '{"usable_gb":null}' >/dev/null   # remove the cap
say "-- laptop cap removed"

if [ "$BULK" = 1 ] && command -v adb >/dev/null; then
  S=${SERIAL:+-s $SERIAL}
  if adb $S get-state >/dev/null 2>&1; then
    f=/tmp/meshai-bulk-200mb.bin; [ -f $f ] || head -c 200000000 /dev/urandom > $f
    say "-- bulk push 200 MB over the cable (file stays at /data/local/tmp/meshai-bulk.bin on the phone; delete it from the phone yourself)"
    t0=$(date +%s%N); adb $S push $f /data/local/tmp/meshai-bulk.bin >/dev/null 2>&1; t1=$(date +%s%N)
    mbps=$(awk -v ns=$((t1-t0)) 'BEGIN{printf "%.1f", 200/(ns/1e9)}'); say "  adb push: ${mbps} MB/s"
    echo "$(date +%s),$MODEL,,,,adb-push,,bulk,,,,,,$mbps MB/s," >> "$OUT"
  else say "-- no phone on adb; bulk test skipped"; fi
fi

say "== rows written to $OUT · summary (tok/s by phone layers, ctx):"
awk -F, 'NR>1 && $8 ~ /^[0-9]+$/ {k=$4" layers @ ctx "$3; s[k]+=$13; n[k]++; if(!($13<mn[k])||n[k]==1)mn[k]=$13} END{for(k in s) printf "  %-22s mean %.2f tok/s (n=%d)\n", k, s[k]/n[k], n[k]}' "$OUT"
