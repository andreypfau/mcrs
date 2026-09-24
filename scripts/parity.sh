#!/usr/bin/env bash
# Captures every scene with a base commit and with the working tree, each run on
# a fresh copy of one saved world, and writes a report of the pixels that differ.
set -euo pipefail
LC_ALL=C

root=$(cd "$(dirname "$0")/.." && pwd)
if [ $# -lt 2 ] || [ $# -gt 3 ]; then
    echo "usage: $0 <base-rev> <world-dir> [<scenes-file>]" >&2
    exit 1
fi
base=$(git -C "$root" rev-parse --verify --quiet "$1^{commit}") || {
    echo "unknown revision: $1" >&2
    exit 1
}
if [ ! -f "$2/level.dat" ]; then
    echo "not a world folder, no level.dat in: $2" >&2
    exit 1
fi
world=$(cd "$2" && pwd)
scenes_file=${3:-$root/scripts/parity-scenes.txt}

name_re='^[a-z0-9_]+$'
token_re='^MCRS_[A-Z0-9_]+=[A-Za-z0-9_.,+-]*$'
names=()
settings=()
seen=" "
while IFS= read -r line || [ -n "$line" ]; do
    words=()
    read -r -a words <<< "$line"
    if [ ${#words[@]} -eq 0 ]; then
        continue
    fi
    name=${words[0]}
    case $name in \#*) continue ;; esac
    if ! [[ $name =~ $name_re ]]; then
        echo "scene $name: rejected name" >&2
        exit 1
    fi
    case $seen in *" $name "*)
        echo "scene $name: listed twice" >&2
        exit 1
        ;;
    esac
    seen="$seen$name "
    for token in "${words[@]:1}"; do
        if ! [[ $token =~ $token_re ]] || [[ $token == MCRS_CAPTURE=* ]]; then
            echo "scene $name: rejected token $token" >&2
            exit 1
        fi
    done
    names+=("$name")
    settings+=("${words[*]:1}")
done < "$scenes_file"
if [ ${#names[@]} -eq 0 ]; then
    echo "no scenes in $scenes_file" >&2
    exit 1
fi

if ! python3 -c 'import PIL, numpy' 2>/dev/null; then
    echo "the report needs Pillow and NumPy: python3 -m pip install pillow numpy" >&2
    exit 1
fi
if ! git -C "$root" grep -q '"CAPTURE"' "$base" -- crates/mcrs_minecraft_client/src; then
    echo "$1 predates the client's capture mode and cannot be a base" >&2
    exit 1
fi

out=$root/target/parity/$(date +%Y%m%d-%H%M%S)
mkdir -p "$root/target/parity"
mkdir "$out"
tmp=$(mktemp -d)
cleanup() {
    if [ -d "$tmp/base-src" ]; then
        git -C "$root" worktree remove --force "$tmp/base-src" > /dev/null 2>&1 || true
    fi
    rm -rf "$tmp"
}
trap cleanup EXIT

echo "building head from the working tree" >&2
CARGO_TARGET_DIR=$root/target cargo build --release -p mcrs_minecraft_client \
    --manifest-path "$root/Cargo.toml"
cp "$root/target/release/mcrs_minecraft_client" "$tmp/head"

echo "building base $base" >&2
git -C "$root" worktree add --detach --quiet "$tmp/base-src" "$base"
CARGO_TARGET_DIR=$root/target/parity-base cargo build --release -p mcrs_minecraft_client \
    --manifest-path "$tmp/base-src/Cargo.toml"
cp "$root/target/parity-base/release/mcrs_minecraft_client" "$tmp/base"

run() {
    local side=$1 scene=$2 knobs=()
    read -r -a knobs <<< "$3"
    rm -rf "$tmp/run"
    mkdir -p "$tmp/run" "$out/$scene"
    cp -R "$world" "$tmp/run/world"
    echo "scene $scene: $side" >&2
    if ! (cd "$tmp/run" && env MCRS_RESOLUTION=1280x720 MCRS_VIEW=10 MCRS_HOT=1 \
        ${knobs[@]+"${knobs[@]}"} MCRS_CAPTURE="$out/$scene/$side.png" \
        "$tmp/$side" "$tmp/run/world") > "$out/$scene/$side.log" 2>&1; then
        echo "scene $scene: the $side build failed, see $out/$scene/$side.log" >&2
    fi
}

for i in "${!names[@]}"; do
    echo "${names[$i]}" >> "$out/scenes.txt"
    run base "${names[$i]}" "${settings[$i]}"
    run head "${names[$i]}" "${settings[$i]}"
done

status=0
python3 "$root/scripts/parity_report.py" "$out" \
    "$(git -C "$root" log -1 --format='%h %s' "$base")" \
    "$(git -C "$root" describe --always --dirty)" || status=$?
echo "report: $out/report.md"
exit "$status"
