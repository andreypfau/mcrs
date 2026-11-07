#!/usr/bin/env bash
# Records the client ticks of one experiment of docs/spec/entity-physics.md, section 15, with a
# vanilla client behind a proxy to the vanilla server of the corpus version. Usage:
#   MINECRAFT_EULA=true [DELAY_TRACE=FILE] tools/mcrs_minecraft_client_recorder/record.sh e9 [OUT_DIRECTORY]
# Without OUT_DIRECTORY, the recordings of e* go to crates/mcrs_minecraft_physics/tests/fixtures/player
# and the others to crates/mcrs_minecraft_move_predict/tests/fixtures/conformance. DELAY_TRACE holds
# the round trip samples of the delay between the client and the proxy. Relative paths start at the
# root of the repository. AUTOPLAY=true starts the vanilla client of tools/vanilla-client, whose
# input agent plays the `input` lines of the scenario in place of a human; MAX_FPS then sets its
# frame rate. CHEAT names a module that the proxy emulates on the serverbound movement.
set -euo pipefail

recorder_here=$(cd "$(dirname "$0")" && pwd)
recorder_root=$(cd "$recorder_here/../.." && pwd)
# shellcheck source=../vanilla-server/run.sh
source "$recorder_root/tools/vanilla-server/run.sh"

if [ "${MINECRAFT_EULA:-}" != true ]; then
    echo "The Minecraft EULA is not accepted in this environment." >&2
    echo "Read https://aka.ms/MinecraftEULA; setting MINECRAFT_EULA=true states the acceptance." >&2
    exit 3
fi

scenario="$recorder_here/scenarios/${1:-}.txt"
if [ ! -f "$scenario" ]; then
    echo "usage: $0 EXPERIMENT, one of: $(cd "$recorder_here/scenarios" && ls | sed 's/\.txt$//' | tr '\n' ' ')" >&2
    exit 2
fi

release="$recorder_root/crates/mcrs_minecraft_client_jar/src/release.json"
id=$(jq -r '.id' "$release")
work="$recorder_root/target/vanilla-server/$id-recorder"
mkdir -p "$work"
fetch_verified "$work/$id.json" "$(jq -r '.json.url' "$release")" "$(jq -r '.json.sha1' "$release")" || exit 2
fetch_verified "$work/server.jar" \
    "$(jq -r '.downloads.server.url' "$work/$id.json")" \
    "$(jq -r '.downloads.server.sha1' "$work/$id.json")" \
    "$(jq -r '.downloads.server.size' "$work/$id.json")" || exit 2

case "$1" in
    e*) out="$recorder_root/crates/mcrs_minecraft_physics/tests/fixtures/player" ;;
    *) out="$recorder_root/crates/mcrs_minecraft_move_predict/tests/fixtures/conformance" ;;
esac

cd "$recorder_root"
if [ "${AUTOPLAY:-}" = true ]; then
    export INPUT_FILE="$work/input.txt"
    : >"$INPUT_FILE"
    (
        until nc -z 127.0.0.1 25565 2>/dev/null; do sleep 1; done
        exec tools/vanilla-client/run.sh localhost:25565 "$INPUT_FILE" ${MAX_FPS:+"$MAX_FPS"}
    ) >"$work/client.log" 2>&1 &
    client=$!
    trap 'kill "$client" 2>/dev/null || true' EXIT
fi
cargo run --release -p mcrs_minecraft_client_recorder -- \
    "$scenario" "$work" "${2:-$out}" ${DELAY_TRACE:+"$DELAY_TRACE"}
