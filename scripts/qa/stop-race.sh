#!/usr/bin/env bash
# Usage: scripts/qa/stop-race.sh — meshd running with sim workers (scripts/split-sim.sh). Stops a run at several points during bring-up and checks for orphans.
# Round-3 #3: stop while bring-up is between "workers pushed" and "host spawned" must leave no orphan process.
api() { curl -s -m 20 -H 'content-type: application/json' "$@"; }
M=Qwen3-0.6B-Q8_0.gguf
for delay in 0.05 0.3 1.0 2.5; do
  api -X POST localhost:8080/api/run -d "{\"model\":\"$M\",\"n_ctx\":2048}" | jq -c '{ok}' >/dev/null
  sleep $delay
  t0=$(date +%s%N); api -X POST localhost:8080/api/stop >/dev/null; t1=$(date +%s%N)
  sleep 1
  st=$(api localhost:8080/api/state | jq -r '.run.status')
  echo "stop after ${delay}s: took $(( (t1-t0)/1000000 )) ms, status=$st, llama-server procs=$(pgrep -x llama-server | wc -l), sim workers alive=$(pgrep -x ggml-rpc-server | wc -l)"
done
