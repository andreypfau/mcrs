#!/usr/bin/env bash
# Runs the server benchmark matrix: each server, bot count and behaviour on a fresh world, one
# server at a time. Settings come from the environment; see README.md.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
# shellcheck source=../vanilla-server/run.sh
source "$root/tools/vanilla-server/run.sh"

servers=${SERVERS:-mcrs vanilla}
bot_counts=${BOTS:-1 10 25 50}
behaviours=${BEHAVIOURS:-walk explore idle mixed}
warmup=${WARMUP:-60}
measure=${MEASURE:-120}
explore_speed=${EXPLORE_SPEED:-0.8}
target=${CARGO_TARGET_DIR:-$root/target}
out=${OUT:-$target/server-bench}
bench=$target/release/mcrs_minecraft_server_bench
mcrs=$target/release/mcrs
java=${JAVA:-java}
# `taskset -c <cpus>` where taskset exists; empty runs unpinned.
server_pin=${SERVER_CPUS:+taskset -c $SERVER_CPUS}
bots_pin=${BOT_CPUS:+taskset -c $BOT_CPUS}

[ "${MINECRAFT_EULA:-}" = true ] || { echo "set MINECRAFT_EULA=true after reading https://aka.ms/MinecraftEULA" >&2; exit 3; }

release="$root/crates/mcrs_minecraft_client_jar/src/release.json"
id=$(jq -r '.id' "$release")
jars="$target/vanilla-server/$id"
mkdir -p "$jars" "$out"
fetch_verified "$jars/$id.json" "$(jq -r '.json.url' "$release")" "$(jq -r '.json.sha1' "$release")"
fetch_verified "$jars/server.jar" "$(jq -r '.downloads.server.url' "$jars/$id.json")" \
    "$(jq -r '.downloads.server.sha1' "$jars/$id.json")" "$(jq -r '.downloads.server.size' "$jars/$id.json")"

wait_for_load() {
    [ -r /proc/loadavg ] || return 0
    while [ "$(cut -d. -f1 /proc/loadavg)" -ge 40 ]; do
        echo "load $(cut -d' ' -f1-3 /proc/loadavg), waiting" | tee -a "$out/load.txt"
        sleep 60
    done
}

load() {
    [ -r /proc/loadavg ] && cut -d' ' -f1-3 /proc/loadavg || uptime | sed 's/.*averages*: //'
}

for behaviour in $behaviours; do
    for bots in $bot_counts; do
        for server in $servers; do
            name="$server-$behaviour-$bots"
            world="$target/server-bench-world/$name"
            rm -rf "$world"
            mkdir -p "$world"
            if [ "$server" = vanilla ]; then
                echo "eula=true" >"$world/eula.txt"
                cat >"$world/server.properties" <<EOF
online-mode=false
server-port=25565
level-seed=12345
view-distance=10
simulation-distance=10
gamemode=creative
force-gamemode=true
difficulty=easy
spawn-protection=0
max-players=200
sync-chunk-writes=true
enforce-secure-profile=false
network-compression-threshold=-1
white-list=false
EOF
                command=("$java" -Xms4g -Xmx4g -jar "$jars/server.jar" nogui)
            else
                command=(env MCRS_WORLD_SEED=12345 MCRS_COMPRESSION_THRESHOLD=-1 MCRS_DEFAULT_GAMEMODE=creative MCRS_LAN_ANNOUNCE=off
                    BEVY_ASSET_ROOT="$root"
                    RUST_LOG=mcrs_minecraft_server=info,mcrs_minecraft_engine=info,mcrs_minecraft_engine::muxer::column_views=info,mcrs_minecraft_engine_world::entity::player::chunk_view=info,mcrs_minecraft_network=info,mcrs_tick=trace
                    "$mcrs")
            fi
            wait_for_load
            echo "[$(date -u +%T)] $name start, load $(load)" | tee -a "$out/load.txt"
            # shellcheck disable=SC2086
            (cd "$world" && $bots_pin "$bench" "$server" "$bots" "$behaviour" "$out/$name.json" \
                --warmup "$warmup" --measure "$measure" --explore-speed "$explore_speed" \
                -- $server_pin "${command[@]}" >"$out/$name.stdout" 2>&1) ||
                echo "$name failed, see $out/$name.stdout" | tee -a "$out/load.txt"
            echo "[$(date -u +%T)] $name end, load $(load)" | tee -a "$out/load.txt"
            rm -rf "$world"
        done
    done
done
