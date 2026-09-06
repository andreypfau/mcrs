#!/usr/bin/env bash
# Times how long a fresh join takes to mesh every column of the view.
# usage: bench-columns.sh <tag> [seconds]
set -euo pipefail
tag=${1:?tag}
secs=${2:-45}
world="${MCRS_BENCH_WORLD:-$HOME/Library/Application Support/minecraft/saves/two}"
out="target-tracy/bench/$tag.log"
mkdir -p "$(dirname "$out")"
MCRS_CENSUS=${MCRS_CENSUS:-1} MCRS_RESOLUTION=${MCRS_RESOLUTION:-1280x720} MCRS_FULLSCREEN=0 \
  ./target/release/mcrs_minecraft_client "$world" > "$out" 2>&1 &
pid=$!
sleep "$secs"
kill $pid 2>/dev/null || true
sleep 2
kill -9 $pid 2>/dev/null || true
awk -v tag="$tag" '
  match($0, /census t=[0-9.]+s .*/) {
    line = substr($0, RSTART, RLENGTH)
    split(line, f, " ")
    t = substr(f[2], 3); sub(/s$/, "", t)
    mesh = 0
    for (i = 1; i <= length(f); i++) if (f[i] ~ /^mesh=[0-9]+$/) { mesh = substr(f[i], 6) }
    if (mesh > peak) { peak = mesh; peak_t = t }
    if (mesh >= peak && done == "") { }
    last = line
  }
  END {
    printf "%s: peak meshed=%d first reached at t=%ss\n", tag, peak, peak_t
    print "  final: " last
  }
' "$out"
