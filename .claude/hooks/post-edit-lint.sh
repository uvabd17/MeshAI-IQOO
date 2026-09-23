#!/usr/bin/env bash
# PostToolUse(Edit|Write): syntax/format check the edited file with whatever tool is installed.
# Exit 2 surfaces stderr to Claude as a warning (tool already ran; cannot block).
input=$(cat)
f=$(jq -r '.tool_input.file_path // .tool_response.filePath // empty' <<<"$input")
[[ -z "$f" || ! -f "$f" ]] && exit 0
have() { command -v "$1" >/dev/null 2>&1; }
fail() { echo "lint: $f — $1" >&2; exit 2; }
case "$f" in
  *.sh)   bash -n "$f" 2>&1 || fail "bash -n failed"
          have shellcheck && { shellcheck -S warning "$f" 2>&1 || fail "shellcheck warnings"; } ;;
  *.py)   python3 -m py_compile "$f" 2>&1 || fail "py_compile failed" ;;
  *.json) jq empty "$f" 2>&1 || fail "invalid JSON" ;;
  *.rs)   have rustfmt && { rustfmt --check --edition 2021 "$f" >/dev/null 2>&1 || fail "rustfmt --check failed (run cargo fmt)"; } ;;
  *.kt|*.kts) have ktlint && { ktlint "$f" 2>&1 || fail "ktlint failed"; } ;;
  *.proto) have protoc && { protoc --proto_path="$(dirname "$f")" --descriptor_set_out=/dev/null "$f" 2>&1 || fail "protoc failed"; } ;;
  *.md)   grep -nE '^\s*\|.*\|\s*$' "$f" >/dev/null; : ;;  # no-op placeholder; markdown has no linter here
esac
exit 0
