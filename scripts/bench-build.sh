#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

out=target/bench-build
runs=5
target_dir=$root/target/bench-build/target
label=baseline
while getopts o:r:t:l: opt; do
    case $opt in
        o) out=$OPTARG ;;
        r) runs=$OPTARG ;;
        t) target_dir=$OPTARG ;;
        l) label=$OPTARG ;;
        *) exit 2 ;;
    esac
done
shift $((OPTIND - 1))

leaf=${BENCH_LEAF:-mcrs_minecraft_painting_variant}
core=${BENCH_CORE:-mcrs_minecraft_core}
test_pkg=${BENCH_TEST_PKG:-mcrs_minecraft_block}
test_filter=${BENCH_TEST_FILTER:-}
lib_of() { echo "crates/$1/src/lib.rs"; }

read -r -a extra <<< "${BENCH_CARGO_ARGS:-}"
extra_q=
((${#extra[@]})) && extra_q=$(printf '%q ' "${extra[@]}")

probes=("$(lib_of "$leaf")" "$(lib_of "$core")" "$(lib_of "$test_pkg")")
if ! git diff --quiet -- "${probes[@]}"; then
    echo "commit or set aside the changes to ${probes[*]} first" >&2
    exit 1
fi

export CARGO_TARGET_DIR=$target_dir
mkdir -p "$out"
out=$(cd "$out" && pwd)
rm -f "$out/$label"@*.json

edited=()
restore() {
    for f in ${edited[@]+"${edited[@]}"}; do git checkout --quiet -- "$f"; done
}
trap restore EXIT

edit_cmd() {
    local file=$1 vis=$2
    echo "git checkout --quiet -- $file && printf '\n#[allow(dead_code)]\n${vis}fn bench_probe() -> u64 { %s }\n' \$(date +%s%N) >> $file"
}

cargo_cmd() { echo "cargo $1 --locked --quiet $extra_q ${*:2}"; }

run() {
    local name=$1 n=$2; shift 2
    echo "== $label/$name" >&2
    hyperfine --style basic --runs "$n" --export-json "$out/$label@$name.json" "$@" >&2
}

scenario() {
    edited=("${probes[@]}")
    case $1 in
        clean-check)
            run "$1" "${BENCH_CLEAN_RUNS:-3}" --prepare "rm -rf '$target_dir'" \
                "$(cargo_cmd check --workspace --all-targets)" ;;
        noop-check)
            run "$1" "$runs" --warmup 1 "$(cargo_cmd check --workspace --all-targets)" ;;
        leaf-edit-check)
            run "$1" "$runs" --warmup 1 --prepare "$(edit_cmd "$(lib_of "$leaf")" "")" \
                "$(cargo_cmd check -p "$leaf" --all-targets)" ;;
        core-private-edit-check)
            run "$1" "$runs" --warmup 1 --prepare "$(edit_cmd "$(lib_of "$core")" "")" \
                "$(cargo_cmd check --workspace --all-targets)" ;;
        core-pub-edit-check)
            run "$1" "$runs" --warmup 1 --prepare "$(edit_cmd "$(lib_of "$core")" "pub ")" \
                "$(cargo_cmd check --workspace --all-targets)" ;;
        core-pub-edit-clippy)
            run "$1" "$runs" --warmup 1 --prepare "$(edit_cmd "$(lib_of "$core")" "pub ")" \
                "$(cargo_cmd clippy --workspace --all-targets)" ;;
        leaf-edit-test)
            run "$1" "$runs" --warmup 1 --prepare "$(edit_cmd "$(lib_of "$test_pkg")" "")" \
                "cargo nextest run --locked $extra_q -p $test_pkg --no-tests=pass --status-level none --final-status-level none $test_filter" ;;
        core-pub-edit-crate-test-build)
            run "$1" "$runs" --warmup 1 --prepare "$(edit_cmd "$(lib_of "$core")" "pub ")" \
                "cargo nextest run --locked $extra_q -p $test_pkg --no-run" ;;
        core-pub-edit-test-build)
            run "$1" "${BENCH_TEST_BUILD_RUNS:-3}" --warmup 1 --prepare "$(edit_cmd "$(lib_of "$core")" "pub ")" \
                "cargo nextest run --locked $extra_q --workspace --no-run" ;;
        *)
            echo "unknown scenario $1" >&2
            exit 2 ;;
    esac
    restore
}

if [[ $# -eq 0 ]]; then
    set -- noop-check leaf-edit-check core-private-edit-check core-pub-edit-check core-pub-edit-clippy leaf-edit-test
fi
for s in "$@"; do scenario "$s"; done

for f in "$out/$label"@*.json; do
    jq -c --arg label "$label" --arg scenario "$(basename "${f##*@}" .json)" '.results[] | .summary as $s | {
        label: $label, scenario: $scenario, command, runs: $s.time_wall_clock.count,
        wall_median: $s.time_wall_clock.median, wall_stddev: $s.time_wall_clock.stddev,
        cpu_median: $s.time_cpu.median, cpu_stddev: $s.time_cpu.stddev, peak_rss_mb: ($s.memory_peak_resident.max / 1048576 | round)}' "$f"
done > "$out/$label.jsonl"
jq -r '[.label, .scenario, "\(.wall_median * 100 | round / 100)s", "±\(.wall_stddev * 100 | round / 100)", "cpu \(.cpu_median * 10 | round / 10)s ±\(.cpu_stddev * 10 | round / 10)", "\(.peak_rss_mb)MB"] | @tsv' "$out/$label.jsonl"
