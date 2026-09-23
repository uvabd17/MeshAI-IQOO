#!/usr/bin/env bash
# Usage: scripts/qa/run-guards.sh — end-to-end laptop check on a running meshd: split-sim, stop-race, 416 header,
# refused phone-host request leaves the run alive, forget-mid-run ends the run, browser QA. Logs go to the scratchpad dir set in S.
set -u
cd /home/prajwal/Documents/GitHub/MeshAI-IQOO
S=${S:-/tmp/claude-1000/-home-prajwal-Documents-GitHub-MeshAI-IQOO/f6158629-50fa-4b3f-8dff-a3a9fbc42561/scratchpad}
api() { curl -s -m 20 -H 'content-type: application/json' "$@"; }
echo "== split-sim =="; scripts/split-sim.sh > $S/split-sim-r4.log 2>&1; echo "sim exit $?"; grep -E "status=|tps=" $S/split-sim-r4.log | tail -4
echo "== stop-race =="; scripts/qa/stop-race.sh
echo "== 416 carries Content-Range */len =="
LEN=$(stat -c %s /mnt/storage/meshai/models/Qwen3-0.6B-Q8_0.gguf)
curl -s -o /dev/null -D - -H "Range: bytes=$LEN-" localhost:8080/api/models/file/Qwen3-0.6B-Q8_0.gguf | grep -iE "^HTTP|content-range"
echo "== refused run (phone host without --lan) must leave the current run alone =="
api -X POST localhost:8080/api/run -d '{"model":"Qwen3-0.6B-Q8_0.gguf","n_ctx":2048}' | jq -c '{ok}'
for i in $(seq 1 60); do s=$(api localhost:8080/api/state | jq -r .run.status); [[ "$s" == "ready" || "$s" == "error" ]] && break; sleep 0.5; done; echo "run status: $s"
SIM=$(api localhost:8080/api/state | jq -r '[.devices[] | select(.is_local|not) | .id][0]')
code=$(curl -s -o $S/refused.json -w '%{http_code}' -m 20 -H 'content-type: application/json' -X POST localhost:8080/api/run -d "{\"model\":\"Qwen3-0.6B-Q8_0.gguf\",\"n_ctx\":2048,\"host\":\"$SIM\"}")
echo "phone-host request → HTTP $code: $(jq -r .error $S/refused.json | cut -c1-80)"
echo "run status after refusal: $(api localhost:8080/api/state | jq -r .run.status) (llama-server procs: $(pgrep -x llama-server | wc -l))"
echo "== forget a sim worker mid-run → run ends, worker withdrawn =="
api -X DELETE "localhost:8080/api/devices/$SIM" | jq -c .
sleep 1; api localhost:8080/api/state | jq -c '{status: .run.status, error: .run.error, devices: [.devices[].id] | length}'
echo "== browser QA (fresh sims) =="
api -X POST localhost:8080/api/stop >/dev/null
api -X POST localhost:8080/api/sim/workers -d '{"n":2,"usable_gb":0.5,"spawn":true}' | jq -c . ; sleep 1.5
timeout 300 node /home/prajwal/.claude/skills/browser-automation/browser.mjs http://localhost:8080/admin/ --script scripts/qa/admin-qa.mjs > $S/admin-qa-r4.json 2>&1; echo "qa exit $?"
python3 - <<'PY'
import json,re
t=open('/tmp/claude-1000/-home-prajwal-Documents-GitHub-MeshAI-IQOO/f6158629-50fa-4b3f-8dff-a3a9fbc42561/scratchpad/admin-qa-r4.json').read()
m=re.search(r'\{.*"steps".*\}', t, re.S); d=json.loads(m.group(0)) if m else {}
for s in d.get("steps",[]):
    k,v=list(s.items())[0]; print(f"- {k}: {str(v)[:160]}")
print("error:", d.get("error")); print(t[m.end():][-300:] if m else t[-800:])
PY
api -X POST localhost:8080/api/stop >/dev/null
