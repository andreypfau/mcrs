#!/bin/sh
# UserPromptSubmit hook: a session above 150k tokens of context writes a hand-off prompt.
# Reads the transcript backward from its end in bounded chunks; it can hold tens of megabytes.
py=python3
python3 -c '' >/dev/null 2>&1 || py='py -3'
exec $py -c '
import json, os, sys
LIMIT = 150000
path = json.load(sys.stdin).get("transcript_path")
if not path:
    sys.exit(0)

def lines_backward(f, chunk=1 << 20):
    position = f.seek(0, os.SEEK_END)
    carry = b""
    while position > 0:
        start = max(0, position - chunk)
        f.seek(start)
        parts = (f.read(position - start) + carry).split(b"\n")
        position = start
        carry = parts[0]
        yield from reversed(parts[1:])
    yield carry

def last_context(path):
    with open(path, "rb") as f:
        for line in lines_backward(f):
            if b"\"usage\"" not in line or b"\"assistant\"" not in line:
                continue
            try:
                entry = json.loads(line)
            except ValueError:
                continue
            if entry.get("type") != "assistant" or entry.get("isSidechain"):
                continue
            usage = entry.get("message", {}).get("usage")
            tokens = sum(usage.get(k) or 0 for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")) if usage else 0
            if tokens:
                return tokens
    return 0

try:
    tokens = last_context(path)
except OSError:
    sys.exit(0)
if tokens > LIMIT:
    json.dump({"hookSpecificOutput": {"hookEventName": "UserPromptSubmit", "additionalContext": "Context is above 150k tokens: load the skill handoff and write the prompt for a new session."}}, sys.stdout)
'
