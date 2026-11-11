#!/usr/bin/env bash
# Records the trace of one scenario against the vanilla dedicated server of the corpus version.
# Usage:
#   MINECRAFT_EULA=true tools/mcrs_minecraft_probe/record.sh bans
set -euo pipefail

probe_here=$(cd "$(dirname "$0")" && pwd)
probe_root=$(cd "$probe_here/../.." && pwd)

if [ "${MINECRAFT_EULA:-}" != true ]; then
    echo "The Minecraft EULA is not accepted in this environment." >&2
    echo "Read https://aka.ms/MinecraftEULA; setting MINECRAFT_EULA=true states the acceptance." >&2
    exit 3
fi

scenario="$probe_here/scenarios/${1:-}.json"
if [ ! -f "$scenario" ]; then
    echo "usage: $0 SCENARIO, one of: $(cd "$probe_here/scenarios" && ls | sed 's/\.json$//' | tr '\n' ' ')" >&2
    exit 2
fi

jar=$("$probe_root/tools/vanilla-server/run.sh" fetch)
cd "$probe_root"
cargo run -p mcrs_minecraft_probe -- "$jar" "$scenario" "$probe_here/traces/$1.json"
