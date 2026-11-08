#!/bin/bash
# Counts the instructions of one tick of the entity pass scene (1 and 16 threads) and of the
# prediction of 10,000 players, under callgrind in a Linux container:
#   tools/callgrind.sh [linux/arm64 | linux/amd64]
# Each scene runs to two tick counts; the difference over the ticks between removes the set-up,
# the warm-up and the exit. Ir of all threads add up. Valgrind on arm64 loses returns, so its
# inclusive Ir per function is wrong (over 100 % of the total); take the function map from amd64.
# The cargo registry and the target directory stay in volumes with the name mcrs-callgrind-*.
set -eu

if [ "${1:-}" != inside ]; then
    platform=${1:-linux/arm64}
    repo=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
    exec docker run --rm --platform "$platform" -v "$repo":/src:ro \
        -v "mcrs-callgrind-registry-${platform#linux/}":/usr/local/cargo/registry \
        -v "mcrs-callgrind-target-${platform#linux/}":/target \
        rust:1-bookworm bash /src/tools/callgrind.sh inside
fi

apt-get update -qq >/dev/null && apt-get install -y -qq valgrind >/dev/null
export CARGO_TARGET_DIR=/target
cd /src
cargo build -q --locked --release -p mcrs_minecraft_engine_physics --example state_hash
predict=$(cargo bench -q --locked -p mcrs_minecraft_move_predict --bench predict --no-run \
    --message-format=json | sed -n 's/.*"executable":"\([^"]*\)".*/\1/p')
out=/target/callgrind
mkdir -p $out

# $1: name, $2 and $3: the two tick counts, then the command with TICKS for the tick count.
count() {
    local name=$1 low=$2 high=$3
    shift 3
    for ticks in "$low" "$high"; do
        valgrind --tool=callgrind --callgrind-out-file="$out/$name.$ticks" \
            "${@/TICKS/$ticks}" >/dev/null 2>&1
        callgrind_annotate --inclusive=yes --threshold=100 "$out/$name.$ticks" \
            | sed -n 's/^ *\([0-9,]*\) .*  \(.*\)$/\1\t\2/p' | tr -d , >"$out/$name.$ticks.txt"
    done
    awk -F'\t' -v ticks=$((high - low)) '
        NR == FNR { low[$2] = $1; next }
        { print int(($1 - low[$2]) / ticks) "\t" $2 }' \
        "$out/$name.$low.txt" "$out/$name.$high.txt" | sort -rn >"$out/$name.tick.txt"
    echo "== $name, ticks $low..$high: $(grep 'PROGRAM TOTALS' "$out/$name.tick.txt" | cut -f1) Ir per tick"
    grep -v 'PROGRAM TOTALS' "$out/$name.tick.txt" | head -25 | cut -c1-200
}

count entity-pass-1 100 120 /target/release/examples/state_hash 1 TICKS
count entity-pass-16 100 120 /target/release/examples/state_hash 16 TICKS
count predict 10 40 "$predict" --ticks TICKS
