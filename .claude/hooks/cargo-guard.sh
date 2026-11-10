#!/bin/sh
# PreToolUse hook for Bash: `cargo test` ignores .config/nextest.toml and runs test binaries one
# after another; most crates set `doctest = false`, so nextest covers every test this repository runs.
exec python3 -c '
import json, re, shlex, sys
cmd = json.load(sys.stdin).get("tool_input", {}).get("command", "")
for part in re.split(r"&&|\|\||[;|&\n()]", cmd):
    try:
        words = shlex.split(part)
    except ValueError:
        words = part.split()
    while words and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", words[0]):
        words.pop(0)
    while words and words[0] in ("time", "nice", "env", "sh", "bash", "-c"):
        words.pop(0)
    if words and words[0] in ("sh", "bash"):
        continue
    if len(words) >= 2 and words[0] == "cargo":
        rest = [w for w in words[1:] if not w.startswith("+")]
        if rest and rest[0] in ("test", "t"):
            sys.stderr.write("Use cargo nextest run -p <crate>; cargo test is not used in this repository.\n")
            sys.exit(2)
'
