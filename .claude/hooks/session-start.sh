#!/usr/bin/env bash
# SessionStart: stdout is added to Claude's context. Print the persistent state, compactly.
input=$(cat)
src=$(jq -r '.source // "startup"' <<<"$input")
root="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$root" || exit 0
echo "## MeshAI session state (source: $src, $(date +%Y-%m-%dT%H:%M))"
echo "branch: $(git branch --show-current 2>/dev/null) · last: $(git log -1 --format='%h %s' 2>/dev/null)"
dirty=$(git status --short 2>/dev/null | wc -l); echo "uncommitted files: $dirty"
if [[ -f state/tasks.json ]]; then
  echo "### open tasks"
  jq -r '.tasks[] | select(.status != "DONE") | "- \(.id) [\(.status)] \(.title)\(if (.blocked_by // [] | length) > 0 then " (blocked by \(.blocked_by|join(",")))" else "" end)"' state/tasks.json 2>/dev/null | head -12
fi
if [[ -f state/blockers.md ]]; then
  echo "### blockers"; grep -E '^- \[ \]' state/blockers.md | head -6
fi
if [[ -f state/progress.md ]]; then
  echo "### progress (last 8 lines)"; tail -8 state/progress.md
fi
echo "Read CLAUDE.md for the Definition of Done. Verify with: scripts/verify --quick"
