#!/usr/bin/env bash
# Builds the server, starts it on 25565, and runs a Fabric client that drives the
# drag and double-click scenarios against it. Exit code is the verdict.
# UAT_SCENARIO=join only joins and counts the registry packets; UAT_KNOWN_PACKS=off makes
# the server offer no known pack, so the client decodes every registry entry.
set -euo pipefail

verdict() {
    local found
    [ -f "$1" ] || { echo "no server log at $1" >&2; return 2; }
    found=$(grep -F 'inbound packet: ' "$1" | sed 's/^.*inbound packet: /inbound packet: /' | sort || true)
    [ -z "$found" ] && return 0
    echo "the server logged packet decode warnings:" >&2
    printf '%s\n' "$found" >&2
    return 1
}

if [ "${1:-}" = verdict ]; then
    verdict "${2:?usage: run.sh verdict <log file>}"
    exit $?
fi

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
port=25565
log="$here/build/server.log"
report="$here/build/uat-report.json"
mkdir -p "$here/build"
scenario="${UAT_SCENARIO:-inventory}"
known_packs="${UAT_KNOWN_PACKS:-on}"
echo "scenario=$scenario known_packs=$known_packs"

if nc -z 127.0.0.1 "$port" 2>/dev/null; then
    echo "port $port is already in use" >&2
    exit 2
fi

(cd "$root" && cargo build --bin mcrs)

BEVY_ASSET_ROOT="$root" MCRS_KNOWN_PACKS="$known_packs" MCRS_DEFAULT_GAMEMODE=creative MCRS_NO_LIGHTING=1 "$root/target/debug/mcrs" >"$log" 2>&1 &
server=$!
trap 'kill $server 2>/dev/null || true' EXIT

for _ in $(seq 1 120); do
    nc -z 127.0.0.1 "$port" 2>/dev/null && break
    kill -0 "$server" 2>/dev/null || { echo "server exited early, see $log" >&2; exit 2; }
    sleep 1
done

rm -f "$report"
mkdir -p "$here/run"
{ grep -v '^onboardAccessibility:' "$here/run/options.txt" 2>/dev/null || true; echo 'onboardAccessibility:false'; } > "$here/run/options.txt.new"
mv "$here/run/options.txt.new" "$here/run/options.txt"
(cd "$here" && ./gradlew runClient --console=plain -q -PuatAddress="127.0.0.1:$port" -PuatReport="$report" -PuatScenario="$scenario" ${UAT_DUMP_PACKET:+-PuatDumpPacket="$UAT_DUMP_PACKET"}) &
client=$!
trap 'kill $server $client 2>/dev/null || true; pkill -f inventory-uat 2>/dev/null || true' EXIT
for _ in $(seq 1 "${UAT_DEADLINE_SECONDS:-600}"); do
    kill -0 "$client" 2>/dev/null || break
    sleep 1
done
status=0
if kill -0 "$client" 2>/dev/null; then
    echo "client still running after the deadline; killing it" >&2
    kill "$client" 2>/dev/null || true
    pkill -f inventory-uat 2>/dev/null || true
    status=124
else
    wait "$client" || status=$?
fi

echo
if [ -f "$report" ]; then
    cat "$report"
else
    echo "no report written; the client never reached the scenarios (see gradle output and $log)" >&2
    status=${status:-1}
    [ "$status" -eq 0 ] && status=1
fi
verdict "$log" || { [ "$status" -ne 0 ] || status=3; }
exit "$status"
