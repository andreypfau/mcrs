#!/bin/sh
py=python3
python3 -c '' >/dev/null 2>&1 || py='py -3'
exec $py -c '
import json, re, shlex, shutil, subprocess, sys, time
cmd = json.load(sys.stdin).get("tool_input", {}).get("command", "")
if not re.search(r"\bpr\b", cmd) or not re.search(r"\bmerge\b", cmd):
    sys.exit(0)
VALUE_FLAGS = ("-b", "--body", "-F", "--body-file", "-t", "--subject", "--match-head-commit", "-A", "--author-email")
WRAPPERS = ("time", "nice", "env", "command")
PLAIN = "Run gh pr merge <N> as a plain command in this repository; the hook checks pr-review ready <N> first."

def deny(reason):
    json.dump({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", "permissionDecisionReason": reason}}, sys.stdout)
    sys.exit(0)

def segments(cmd):
    lexer = shlex.shlex(cmd, posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    segment = []
    for token in lexer:
        if token and all(c in "();<>|&" for c in token):
            yield segment
            segment = []
        else:
            segment.append(token)
    yield segment

def merge_targets(cmd):
    for words in segments(cmd):
        while words and (re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", words[0]) or words[0] in WRAPPERS):
            words = words[1:]
        if len(words) >= 3 and words[0] in ("sh", "bash") and words[1] == "-c":
            yield from merge_targets(words[2])
            continue
        if "pr" not in words or "merge" not in words:
            continue
        if words[0] != "gh":
            deny(PLAIN)
        positional = []
        args = iter(words[1:])
        for word in args:
            if word in ("--repo", "--hostname") or word.startswith(("--repo=", "--hostname=")) or re.match(r"-[A-Za-z]*R", word):
                deny(PLAIN)
            if word in VALUE_FLAGS:
                next(args, None)
            elif not word.startswith("-"):
                positional.append(word)
        if positional[:2] != ["pr", "merge"] or "--disable-auto" in words:
            continue
        target = positional[2] if len(positional) > 2 else ""
        match = re.fullmatch(r"#?(\d+)", target)
        if not match:
            deny(PLAIN)
        yield match.group(1)

if shutil.which("pr-review") is None:
    sys.exit(0)
try:
    targets = list(merge_targets(cmd))
except ValueError:
    deny(PLAIN)
deadline = time.monotonic() + 4
for number in targets:
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        deny("pr-review ready " + number + " timed out. Run it, then merge.")
    try:
        result = subprocess.run(["pr-review", "ready", number], capture_output=True, text=True, timeout=remaining)
    except subprocess.TimeoutExpired:
        deny("pr-review ready " + number + " timed out. Run it, then merge.")
    if result.returncode != 0:
        deny("pr-review ready " + number + " failed: " + (result.stdout + result.stderr).strip()[-400:])
'
