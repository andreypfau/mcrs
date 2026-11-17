#!/bin/sh
py=python3
python3 -c '' >/dev/null 2>&1 || py='py -3'
exec $py -c '
import json, sys
tool_input = json.load(sys.stdin).get("tool_input", {})
kind = tool_input.get("subagent_type")
roles = "implementer for a code edit, reviewer for a review, Explore for a search, fork to keep this context"

def decide(decision, reason, updated=None):
    out = {"hookEventName": "PreToolUse", "permissionDecision": decision, "permissionDecisionReason": reason}
    if updated is not None:
        out["updatedInput"] = updated
    json.dump({"hookSpecificOutput": out}, sys.stdout)
    sys.exit(0)

if not kind:
    decide("deny", "Set subagent_type: " + roles + ". See the skill orchestrate.")
if kind in ("Explore", "log-reader") and tool_input.get("model") != "haiku":
    decide("allow", kind + " runs on haiku.", dict(tool_input, model="haiku"))
if kind == "general-purpose" and not tool_input.get("model"):
    decide("deny", "Use an mcrs role instead of general-purpose: " + roles + ". A general-purpose call needs an explicit model.")
'
