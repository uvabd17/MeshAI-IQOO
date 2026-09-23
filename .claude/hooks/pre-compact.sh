#!/usr/bin/env bash
# PreCompact: persist a marker so the post-compaction context knows to re-read state from disk.
input=$(cat)
trig=$(jq -r '.trigger // "auto"' <<<"$input")
root="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$root" || exit 0
[[ -f state/progress.md ]] || exit 0
printf '\n- %s — context compacted (%s). Next context: re-read state/tasks.json, state/blockers.md, docs/DECISIONS.md before acting. Uncommitted files: %s\n' \
  "$(date +%Y-%m-%dT%H:%M)" "$trig" "$(git status --short 2>/dev/null | wc -l)" >> state/progress.md
exit 0
