#!/usr/bin/env bash
# Stop: if source files changed since the last successful verify, run scripts/verify --quick.
# Exit 2 blocks the stop (only honoured when stop_hook_active is true). Loop guard: max 2 blocks per session.
input=$(cat)
active=$(jq -r '.stop_hook_active // false' <<<"$input")
sid=$(jq -r '.session_id // "nosession"' <<<"$input")
root="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$root" || exit 0
mkdir -p state/.cache
# Fingerprint of tracked+untracked source under watched dirs (content hash of the diff + untracked list).
fp=$( { git diff HEAD -- android desktop admin proto scripts CLAUDE.md .claude 2>/dev/null; git ls-files --others --exclude-standard -- android desktop admin proto scripts .claude 2>/dev/null | sort; } | sha256sum | cut -c1-16)
last=$(cat state/.cache/last-verify 2>/dev/null || echo none)
[[ "$fp" == "$last" ]] && exit 0                     # nothing changed since last green verify
guard="state/.cache/stop-blocks-$sid"
n=$(cat "$guard" 2>/dev/null || echo 0)
if scripts/verify --quick >state/.cache/verify.log 2>&1; then
  echo "$fp" >state/.cache/last-verify
  exit 0
fi
if [[ "$active" == "true" && "$n" -lt 2 ]]; then
  echo $((n+1)) >"$guard"
  { echo "scripts/verify --quick FAILED — fix before finishing (attempt $((n+1))/2). Tail:"; tail -15 state/.cache/verify.log; } >&2
  exit 2
fi
# Cannot block (or guard exhausted): still tell the user.
jq -n --arg m "verify --quick is failing; see state/.cache/verify.log" '{systemMessage:$m}'
exit 0
