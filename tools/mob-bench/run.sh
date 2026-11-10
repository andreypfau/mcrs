#!/usr/bin/env bash
# Runs the 50,000-pig scene on each server and client mode, one run at a time, each in a fresh
# folder on a free port, and writes one JSON report a run plus summary.csv to OUT.
# See benchmarks/mob-bench/README.md for the method and every setting.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)

servers="${SERVERS:-vanilla horizon mcrs}"
clients="${CLIENTS:-watcher none}"
phases="${PHASES:-idle,roam-still,roam-stir,crowd-still,crowd-stir,stimulus,cost-still,cost-stir}"
pigs="${PIGS:-50000}"
samples="${SAMPLES:-32}"
idle_warmup="${IDLE_WARMUP:-10}"
warmup="${WARMUP:-30}"
measure="${MEASURE:-60}"
probe_interval="${PROBE_INTERVAL:-5}"
summon_rate="${SUMMON_RATE:-5000}"
sprint_ticks="${SPRINT_TICKS:-100}"
stimulus_pigs="${STIMULUS_PIGS:-2000}"
stimulus_period="${STIMULUS_PERIOD:-5}"
stimulus_warmup="${STIMULUS_WARMUP:-20}"
stir_mode="${STIR_MODE:-uuid}"
player_mode="${PLAYER_MODE:-spectator}"
contended_cores="${CONTENDED_CORES:-1.0}"
port="${PORT:-25610}"
java="${JAVA:-java}"
heap="${HEAP:-16g}"
scaling_threads="${SCALING_THREADS:-}"
scaling_phases="${SCALING_PHASES:-crowd-stir}"
scaling_servers="${SCALING_SERVERS:-vanilla}"
out="${OUT:-$root/benchmarks/mob-bench/$(date -u +%Y-%m-%d)}"
work="${WORK:-$root/target/mob-bench}"
horizon_dir="${HORIZON_DIR:-$HOME/src/github.com/horizon}"
horizon_bin="${HORIZON_BIN:-$horizon_dir/target-mobbench/release/horizon-client}"
horizon_bots="${HORIZON_BOTS:-$horizon_dir/target-mobbench/release/horizon-bots}"
horizon_resources="${HORIZON_RESOURCES:-$horizon_dir/.local/horizon-client/26.3}"
mcrs_bin="${MCRS_BIN:-$root/target/release/mcrs}"
bench="${BENCH:-$root/target/release/mob-bench}"

if [ "${MINECRAFT_EULA:-}" != true ]; then
    echo "set MINECRAFT_EULA=true to accept the Minecraft EULA for the vanilla server" >&2
    exit 3
fi
vanilla_jar="${VANILLA_JAR:-$("$root/tools/vanilla-server/run.sh" fetch)}"
mkdir -p "$out" "$work/logs"

# The Horizon watcher is horizon-bots: the harness's own client speaks the protocol of
# 26.4-snapshot-7, and Horizon (26.3) refuses it as incompatible.
client_mode() {
    if [ "$1" = horizon ] && [ "$2" = watcher ]; then echo bot; else echo "$2"; fi
}

run_one() {
    local server=$1 client=$2 name=$3 threads=${4:-} run_phases=${5:-$phases}
    local dir="$work/run/$name"
    rm -rf "$dir"
    mkdir -p "$dir"
    local args=(run --server "$server" --client "$(client_mode "$server" "$client")" --repo "$root"
        --dir "$dir" --out "$out/$name.json" --port "$port" --phases "$run_phases" --pigs "$pigs"
        --samples "$samples" --idle-warmup "$idle_warmup" --warmup "$warmup" --measure "$measure"
        --probe-interval "$probe_interval" --summon-rate "$summon_rate" --sprint-ticks "$sprint_ticks"
        --stimulus-pigs "$stimulus_pigs" --stimulus-period "$stimulus_period" --stimulus-warmup "$stimulus_warmup"
        --stir-mode "$stir_mode" --player-mode "$player_mode"
        --contended-cores "$contended_cores" --java "$java" --heap "$heap")
    case "$server" in
        vanilla) args+=(--binary "$vanilla_jar") ;;
        horizon) args+=(--binary "$horizon_bin" --resources "$horizon_resources" --bot "$horizon_bots") ;;
        mcrs) args+=(--binary "$mcrs_bin") ;;
    esac
    [ -n "$threads" ] && args+=(--threads "$threads")
    echo "[$(date -u +%T)] $name start, load $(sysctl -n vm.loadavg)" | tee -a "$out/runs.txt"
    "$bench" "${args[@]}" >"$work/logs/$name.stdout" 2>&1 ||
        echo "$name failed, see $work/logs/$name.stdout" | tee -a "$out/runs.txt"
    cp "$dir/server.log" "$work/logs/$name.server.log" 2>/dev/null || true
    echo "[$(date -u +%T)] $name end, load $(sysctl -n vm.loadavg)" | tee -a "$out/runs.txt"
    rm -rf "$dir"
}

for server in $servers; do
    for client in $clients; do
        run_one "$server" "$client" "$server-$client"
    done
done

for threads in $scaling_threads; do
    for server in $scaling_servers; do
        run_one "$server" none "$server-none-threads$threads" "$threads" "$scaling_phases"
    done
done

"$here/summary.sh" "$out"
