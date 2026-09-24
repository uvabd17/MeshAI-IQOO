#!/usr/bin/env bash
# Usage: scripts/split-sim.sh [model.gguf] [laptop_cap_gb]   — starts meshd if needed; needs third_party/llama.cpp/build-host and the model in the models dir.
# T009: local split simulation — laptop host + 2 real ggml-rpc-server workers on localhost, Qwen3-0.6B.
# Forces a split by capping the laptop's usable memory below the model size.
set -u
cd /home/prajwal/Documents/GitHub/MeshAI-IQOO
MODEL="${1:-Qwen3-0.6B-Q8_0.gguf}"
CAP_GB="${2:-0.35}"
api() { curl -s -m 20 -H "x-mesh-token: ${MESH_TOKEN:-}" -H 'content-type: application/json' "$@"; }

if ! pgrep -x meshd >/dev/null; then
  echo "starting meshd"
  RUST_LOG=meshd=info setsid nohup desktop/target/debug/meshd serve > state/.cache/meshd.log 2>&1 < /dev/null &
  for i in $(seq 1 30); do curl -s -m 1 -H "x-mesh-token: ${MESH_TOKEN:-}" localhost:8080/api/state >/dev/null && break; sleep 0.5; done
fi
echo "== state =="; api localhost:8080/api/state | jq -c '{mesh_id, models: [.models[] | .file], run: .run.status}'

echo "== A. laptop-alone plan for $MODEL =="
api -X POST localhost:8080/api/plan -d "{\"model\":\"$MODEL\",\"n_ctx\":2048}" | jq -r '.plan.summary, (.plan.placements[] | "  \(.name): \(.reason)")'

echo "== cap laptop to ${CAP_GB} GB and add 2 simulated phones with live RPC workers =="
api -X POST localhost:8080/api/devices/local/limit -d "{\"usable_gb\":$CAP_GB}" | jq -c .
api -X POST localhost:8080/api/sim/workers -d '{"n":2,"usable_gb":0.5,"spawn":true}' | jq -c .
sleep 1.5
echo "== B. split plan =="
api -X POST localhost:8080/api/plan -d "{\"model\":\"$MODEL\",\"n_ctx\":2048}" | jq -r '.plan.summary, (.plan.placements[] | "  \(.name) [\(.role)] layers \(.layer_start)-\(.layer_end): \(.reason)"), "  args: \(.args.program) \(.args.args | join(" "))"'

echo "== run =="
curl -s -m 120 -H "x-mesh-token: ${MESH_TOKEN:-}" -H "content-type: application/json" -X POST localhost:8080/api/run -d "{\"model\":\"$MODEL\",\"n_ctx\":2048}" | jq -c "{ok, error, mode: .plan.mode}"
t0=$(date +%s.%N)
for i in $(seq 1 240); do
  s=$(api localhost:8080/api/state | jq -r .run.status)
  [[ "$s" == "ready" || "$s" == "error" ]] && break
  sleep 0.5
done
t1=$(date +%s.%N)
echo "status=$s after $(echo "$t1 - $t0" | bc | cut -c1-5)s"
api localhost:8080/api/state | jq -r '.run.log_tail[-12:][]' | cut -c1-160
[[ "$s" != "ready" ]] && { echo "NOT READY — aborting"; exit 1; }

echo "== chat (streaming) x3 =="
for q in "Say hello in exactly five words." "Explain in two sentences why phones throttle when hot." "Write a haiku about a laptop and a phone working together."; do
  curl -s -N -m 180 localhost:8080/v1/chat/completions -H "x-mesh-token: ${MESH_TOKEN:-}" -H 'content-type: application/json' \
    -d "{\"model\":\"$MODEL\",\"stream\":true,\"max_tokens\":120,\"messages\":[{\"role\":\"user\",\"content\":\"$q\"}]}" \
    | grep -o '"content":"[^"]*"' | sed 's/"content":"//;s/"$//' | tr -d '\n' | head -c 300; echo
done
echo "== runs recorded =="
api localhost:8080/api/runs | jq -r '.[] | "\(.model) \(.mode) devices=\(.devices) ttft=\(.ttft_ms)ms tokens=\(.tokens_out) tps=\(.tps|.*10|round/10) total=\(.total_ms)ms"' | tail -5

echo "== worker processes =="; pgrep -af ggml-rpc-server | cut -c1-120
echo "== host log tail =="; api localhost:8080/api/state | jq -r '.run.log_tail[-6:][]' | cut -c1-160
echo "== stop =="; api -X POST localhost:8080/api/stop | jq -c .
echo "DONE"
