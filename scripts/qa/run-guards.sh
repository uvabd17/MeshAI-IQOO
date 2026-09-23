#!/usr/bin/env bash
# End-to-end laptop check against a running meshd (localhost, no token): split simulation, stop race,
# 416 header, refused requests that must leave the live run alone, forget-mid-run, browser QA.
# Usage: scripts/qa/run-guards.sh            (logs go to $OUT, default ./state/.cache/qa)
# Needs: third_party/llama.cpp/build-host, the test model in the catalog dir, node + the browser-automation
# skill (set BROWSER_MJS to its browser.mjs; the browser step is skipped when it is missing).
set -u
cd "$(dirname "$0")/../.."
OUT="${OUT:-state/.cache/qa}"; mkdir -p "$OUT"
MODEL="${MODEL:-Qwen3-0.6B-Q8_0.gguf}"
BROWSER_MJS="${BROWSER_MJS:-$HOME/.claude/skills/browser-automation/browser.mjs}"
api() { curl -s -m 20 -H 'content-type: application/json' "$@"; }
status() { api localhost:8080/api/state | jq -r .run.status; }
wait_settled() { for i in $(seq 1 120); do s=$(status); [[ "$s" == "ready" || "$s" == "error" || "$s" == "idle" ]] && break; sleep 0.5; done; echo "$s"; }
fail=0; check() { if [[ "$1" == "$2" ]]; then echo "  ok   $3"; else echo "  FAIL $3 (got '$1', want '$2')"; fail=1; fi; }

echo "== split-sim =="; scripts/split-sim.sh "$MODEL" > "$OUT/split-sim.log" 2>&1; check "$?" 0 "split-sim exit"; grep -E "status=|tps=" "$OUT/split-sim.log" | tail -3
echo "== stop-race =="; scripts/qa/stop-race.sh
echo "== 416 carries Content-Range */len =="
LEN=$(curl -sI -m 10 "localhost:8080/api/models/file/$MODEL" | tr -d '\r' | awk 'tolower($1)=="content-length:"{print $2}')
HDR=$(curl -s -o /dev/null -D - -H "Range: bytes=$LEN-" "localhost:8080/api/models/file/$MODEL" | tr -d '\r')
check "$(echo "$HDR" | grep -ci "^content-range: bytes \*/$LEN")" 1 "416 content-range bytes */$LEN"
echo "== refused requests must leave the live run alone =="
api -X POST localhost:8080/api/run -d "{\"model\":\"$MODEL\",\"n_ctx\":2048}" >/dev/null; check "$(wait_settled)" ready "baseline run ready"
SIM=$(api localhost:8080/api/state | jq -r '[.devices[] | select(.is_local|not) | .id][0]')
code=$(curl -s -o "$OUT/refused.json" -w '%{http_code}' -m 20 -H 'content-type: application/json' -X POST localhost:8080/api/run -d "{\"model\":\"$MODEL\",\"n_ctx\":2048,\"host\":\"$SIM\"}")
check "$code" 422 "phone host without --lan → 422"; check "$(status)" ready "run still ready after the --lan refusal"
code=$(curl -s -o "$OUT/refused2.json" -w '%{http_code}' -m 20 -H 'content-type: application/json' -X POST localhost:8080/api/run -d "{\"model\":\"$MODEL\",\"n_ctx\":4000000}")
check "$code" 422 "n_ctx beyond n_ctx_train → 422 ($(jq -r .error "$OUT/refused2.json" | cut -c1-60))"; check "$(status)" ready "run still ready after the n_ctx refusal"
code=$(curl -s -o /dev/null -w '%{http_code}' -m 20 -H 'content-type: application/json' -X POST localhost:8080/api/run -d '{"model":"no-such.gguf","n_ctx":2048}')
check "$code" 422 "unknown model → 422"; check "$(status)" ready "run still ready after the unknown-model refusal"
BIG="${BIG_MODEL:-Qwen3-8B-Q4_K_M.gguf}"
if api localhost:8080/api/state | jq -e --arg m "$BIG" '.models[] | select(.file==$m)' >/dev/null; then
  code=$(curl -s -o "$OUT/refused3.json" -w '%{http_code}' -m 20 -H 'content-type: application/json' -X POST localhost:8080/api/run -d "{\"model\":\"$BIG\",\"n_ctx\":2048}")
  check "$code" 422 "shortfall no stop could cure ($BIG on the capped pool) → 422"
  check "$(grep -c 'previous run stopped' "$OUT/refused3.json")" 0 "…refused WITHOUT stopping the live run"
  check "$(status)" ready "run still ready after the shortfall refusal"
else echo "  skip shortfall guard ($BIG not in catalog)"; fi
check "$(pgrep -x llama-server | wc -l)" 1 "exactly one llama-server alive"
echo "== replacement runs with a MEASURED laptop (D024: credit only what is demonstrably held) =="
api -X POST localhost:8080/api/stop >/dev/null
api -X POST localhost:8080/api/devices/local/limit -d '{"usable_gb":null}' >/dev/null   # uncap: laptop memory is measured again
api -X POST localhost:8080/api/run -d "{\"model\":\"$MODEL\",\"n_ctx\":2048}" >/dev/null
RESP=$(api -X POST localhost:8080/api/run -d "{\"model\":\"$MODEL\",\"n_ctx\":2048}")   # immediately: the first run is still starting/loading
check "$(echo "$RESP" | jq -r '.ok')" true "immediate re-run accepted (fits uncredited)"
check "$(echo "$RESP" | jq -r '.credited | length')" 0 "a run that is not ready earns NO credit"
check "$(wait_settled)" ready "laptop-alone run ready"; check "$(api localhost:8080/api/state | jq -r '.plan.mode')" Single "fits on the laptop alone (no split)"
HELD=$(api localhost:8080/api/state | jq -r '.plan.placements[] | select(.device_id=="local") | .bytes')
sleep 3   # let a post-ready laptop memory sample land
RESP=$(api -X POST localhost:8080/api/run -d "{\"model\":\"$MODEL\",\"n_ctx\":4096}")
check "$(echo "$RESP" | jq -r '.ok')" true "replacement request accepted"
CRED=$(echo "$RESP" | jq -r '[.credited[] | select(.[0]=="local") | .[1]] | .[0] // 0')
echo "  info credit for local = $CRED B (observed drop, capped at the placement's $HELD B)"
check "$(( CRED <= HELD ))" 1 "credit never exceeds the placement bytes"
check "$(wait_settled)" ready "replacement run ready (ctx 4096)"
if [[ "$CRED" -gt 0 ]]; then check "$(api localhost:8080/api/state | jq -r '.run.log_tail | map(select(test("plan credits"))) | length')" 1 "run log names the credit"; fi
api -X POST localhost:8080/api/stop >/dev/null
api -X POST localhost:8080/api/devices/local/limit -d '{"usable_gb":0.35}' >/dev/null   # re-cap for the remaining checks
api -X POST localhost:8080/api/run -d "{\"model\":\"$MODEL\",\"n_ctx\":2048}" >/dev/null; check "$(wait_settled)" ready "split run ready again"
echo "== forget a member mid-run → run ends with an error, device gone =="
api -X DELETE "localhost:8080/api/devices/$SIM" >/dev/null; sleep 1
check "$(status)" error "run status error"; check "$(api localhost:8080/api/state | jq -r '.run.error' | grep -c forgotten)" 1 "error names the forgotten device"
check "$(api localhost:8080/api/state | jq -r "[.devices[] | select(.id==\"$SIM\")] | length")" 0 "device removed"
api -X POST localhost:8080/api/stop >/dev/null
echo "== browser QA (fresh sims) =="
if [[ -f "$BROWSER_MJS" ]]; then
  api -X POST localhost:8080/api/sim/workers -d '{"n":2,"usable_gb":0.5,"spawn":true}' >/dev/null; sleep 1.5
  timeout 300 node "$BROWSER_MJS" http://localhost:8080/admin/ --script scripts/qa/admin-qa.mjs > "$OUT/admin-qa.json" 2>&1; check "$?" 0 "browser QA exit"
  check "$(grep -c '"steps"' "$OUT/admin-qa.json")" 1 "browser QA produced its step report"
  check "$(grep -cE '"error": "|SCRIPT ERROR' "$OUT/admin-qa.json")" 0 "browser QA reported no error"
  grep -E "console errors|requests failed" "$OUT/admin-qa.json"
  grep -oE '"chat_stats": \{[^}]*\}' "$OUT/admin-qa.json" | head -1
else echo "  skip browser QA ($BROWSER_MJS missing)"; fi
api -X POST localhost:8080/api/stop >/dev/null
echo "== run-guards: $([[ $fail == 0 ]] && echo ALL OK || echo FAILURES) =="; exit $fail
