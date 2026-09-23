#!/usr/bin/env bash
# PreToolUse(Bash): deny destructive commands. Exit 2 + permissionDecision=deny blocks the call.
# Heredoc bodies are ignored (so writing a file that *mentions* `rm -rf` is fine) unless the
# heredoc is fed to a shell/interpreter (bash/sh/zsh/dash/eval/python/sudo), which would execute it.
input=$(cat)
raw=$(jq -r '.tool_input.command // empty' <<<"$input")
[[ -z "$raw" ]] && exit 0

cmd=$(python3 - "$raw" <<'PY'
import re, sys
src = sys.argv[1]
out, i, lines = [], 0, src.split("\n")
interp = re.compile(r'(^|[|;&(]\s*)(sudo\s+)?(bash|sh|zsh|dash|ksh|eval|python[0-9.]*|perl|ruby|node)\b[^<]*<<')
while i < len(lines):
    line = lines[i]
    m = re.search(r'<<-?\s*([\'"]?)([A-Za-z_][A-Za-z0-9_]*)\1', line)
    if m and not interp.search(line):
        out.append(line)                      # keep the command line, drop the body
        term = m.group(2); i += 1
        while i < len(lines) and lines[i].strip() != term:
            i += 1
        i += 1                                # skip terminator
        continue
    out.append(line); i += 1
print("\n".join(out))
PY
)

deny() {
  jq -n --arg r "$1" '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",permissionDecisionReason:$r}}'
  echo "blocked by .claude/hooks/pre-bash-safety.sh: $1" >&2
  exit 2
}
S='(^|[;&|[:space:]])'   # command-position anchor

# rm -rf: allowed only inside the session scratchpad / /tmp/claude-*
if grep -Eq "${S}rm[[:space:]]+(-[a-zA-Z]*r[a-zA-Z]*f|-[a-zA-Z]*f[a-zA-Z]*r)[[:space:]]" <<<"$cmd"; then
  grep -Eq 'rm[[:space:]]+-[a-zA-Z]+[[:space:]]+"?(/tmp/claude-|\$\{?SCRATCH)' <<<"$cmd" || deny "rm -rf outside the scratchpad"
fi
grep -Eq "${S}git[[:space:]]+push[[:space:]].*(--force|-f([[:space:]]|$)|[[:space:]]\+[a-zA-Z])" <<<"$cmd" && deny "git push --force"
grep -Eq "${S}git[[:space:]]+reset[[:space:]]+--hard" <<<"$cmd" && deny "git reset --hard"
grep -Eq "${S}git[[:space:]]+clean[[:space:]]+-[a-zA-Z]*[fd]" <<<"$cmd" && deny "git clean"
grep -Eq "${S}git[[:space:]]+checkout[[:space:]]+--[[:space:]]+\." <<<"$cmd" && deny "git checkout -- . (discards work)"
grep -Eq "${S}git[[:space:]]+branch[[:space:]]+-D" <<<"$cmd" && deny "git branch -D"
grep -Eq "${S}adb[[:space:]]+(-s[[:space:]]+[^[:space:]]+[[:space:]]+)?shell[[:space:]]+.*(${S}rm[[:space:]]+-|reboot|pm[[:space:]]+uninstall|wipe|recovery|factory)" <<<"$cmd" && deny "destructive adb shell command"
grep -Eq "${S}adb[[:space:]]+(-s[[:space:]]+[^[:space:]]+[[:space:]]+)?(reboot|sideload|uninstall)" <<<"$cmd" && deny "destructive adb command"
grep -Eq "${S}(mkfs|dd[[:space:]]+if=|shred|wipefs)" <<<"$cmd" && deny "disk-destructive command"
grep -Eq '>[[:space:]]*/dev/(sd|nvme|mmcblk)' <<<"$cmd" && deny "write to block device"
grep -Eq "${S}docker[[:space:]]+(system[[:space:]]+prune|volume[[:space:]]+prune|rm[[:space:]]+-f)" <<<"$cmd" && deny "docker prune/rm -f"
grep -Eq "${S}chmod[[:space:]]+-R[[:space:]]+[0-7]*7[0-7]*[[:space:]]+/([[:space:]]|$)" <<<"$cmd" && deny "chmod -R on /"
exit 0
