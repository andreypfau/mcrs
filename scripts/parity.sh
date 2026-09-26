#!/usr/bin/env bash
# Captures every scene with a base commit and with the working tree, or with
# --paths as classic, deferred and the deferred parity mask from the working tree
# alone, each run on a fresh copy of one saved world, and writes a report.
set -euo pipefail
LC_ALL=C

root=$(cd "$(dirname "$0")/.." && pwd)
paths=0
if [ "${1-}" = --paths ]; then
    paths=1
    shift
fi
if [ $# -lt $((2 - paths)) ] || [ $# -gt $((3 - paths)) ]; then
    echo "usage: $0 <base-rev> <world-dir> [<scenes-file>]" >&2
    echo "       $0 --paths <world-dir> [<scenes-file>]" >&2
    exit 1
fi
if [ $paths -eq 0 ]; then
    base=$(git -C "$root" rev-parse --verify --quiet "$1^{commit}") || {
        echo "unknown revision: $1" >&2
        exit 1
    }
    base_rev=$1
    shift
fi
if [ ! -f "$1/level.dat" ]; then
    echo "not a world folder, no level.dat in: $1" >&2
    exit 1
fi
world=$(cd "$1" && pwd)
scenes_file=${2:-$root/scripts/parity-scenes.txt}

token_re='^MCRS_[A-Z0-9_]+=[A-Za-z0-9_.,+-]*$'
dimension_re='^[a-z_]+$'
scene_re='^[a-z0-9_]+$'
uses_scenes=0
names=()
settings=()
while IFS= read -r line || [ -n "$line" ]; do
    words=()
    read -r -a words <<< "$line"
    if [ ${#words[@]} -eq 0 ]; then
        continue
    fi
    name=${words[0]}
    case $name in \#*) continue ;; esac
    for token in "${words[@]:1}"; do
        if ! [[ $token =~ $token_re ]] || [[ $token == MCRS_CAPTURE=* ]] ||
            { [[ $token == MCRS_DIMENSION=* ]] && ! [[ ${token#MCRS_DIMENSION=} =~ $dimension_re ]]; } ||
            { [[ $token == MCRS_SCENE=* ]] && { ! [[ ${token#MCRS_SCENE=} =~ $scene_re ]] ||
                [ ! -f "$root/scripts/scenes/${token#MCRS_SCENE=}.json" ]; }; } ||
            { [ $paths -eq 1 ] && [[ $token == MCRS_RENDER_PATH=* || $token == MCRS_PARITY_MASK=* ]]; }; then
            echo "scene $name: rejected token $token" >&2
            exit 1
        fi
        if [[ $token == MCRS_SCENE=* ]]; then
            uses_scenes=1
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
if [ $paths -eq 0 ] && ! git -C "$root" grep -q '"CAPTURE"' "$base" -- crates/mcrs_minecraft_client/src; then
    echo "$base_rev predates the client's capture mode and cannot be a base" >&2
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
if [ $uses_scenes -eq 1 ]; then
    CARGO_TARGET_DIR=$root/target cargo build --release -p mcrs_minecraft_light_color_bench \
        --features corpus --bin scene --manifest-path "$root/Cargo.toml"
    cp "$root/target/release/scene" "$tmp/scene"
fi

if [ $paths -eq 0 ]; then
    echo "building base $base" >&2
    git -C "$root" worktree add --detach --quiet "$tmp/base-src" "$base"
    CARGO_TARGET_DIR=$root/target/parity-base cargo build --release -p mcrs_minecraft_client \
        --manifest-path "$tmp/base-src/Cargo.toml"
    cp "$root/target/parity-base/release/mcrs_minecraft_client" "$tmp/base"
fi

# The saved player position decides what the server loads before the first teleport, so
# every capture starts where its scene stands rather than where the world was last left.
place_players() {
    python3 - "$1" "$2" <<'EOF'
import glob, gzip, struct, sys
pos = [float(v) for v in sys.argv[2].split(',')]
for path in glob.glob(f'{sys.argv[1]}/players/data/*.dat'):
    data = bytearray(gzip.decompress(open(path, 'rb').read()))
    at = data.find(b'\x09\x00\x03Pos\x06\x00\x00\x00\x03')
    if at < 0:
        sys.exit(f'no Pos list in {path}')
    struct.pack_into('>3d', data, at + 11, *pos)
    open(path, 'wb').write(gzip.compress(bytes(data)))
EOF
}

# The server joins the player in the saved dimension, and a world is saved wherever
# it was last left, so every run writes the scene's dimension.
place_dimension() {
    python3 - "$1" "$2" <<'EOF'
import glob, gzip, struct, sys
key = b'\x08\x00\x09Dimension'
value = f'minecraft:{sys.argv[2]}'.encode()
for path in glob.glob(f'{sys.argv[1]}/players/data/*.dat'):
    data = gzip.decompress(open(path, 'rb').read())
    at = data.find(key)
    if at < 0:
        sys.exit(f'no Dimension string in {path}')
    start = at + len(key)
    end = start + 2 + struct.unpack_from('>H', data, start)[0]
    data = data[:start] + struct.pack('>H', len(value)) + value + data[end:]
    open(path, 'wb').write(gzip.compress(data))
EOF
}

run() {
    local binary=$1 side=$2 scene=$3 knobs=()
    read -r -a knobs <<< "$4"
    shift 4
    rm -rf "$tmp/run"
    mkdir -p "$tmp/run" "$out/$scene"
    cp -R "$world" "$tmp/run/world"
    local dimension=overworld scene_file=
    for knob in ${knobs[@]+"${knobs[@]}"}; do
        case $knob in
            MCRS_POS=*) place_players "$tmp/run/world" "${knob#MCRS_POS=}" ;;
            MCRS_DIMENSION=*) dimension=${knob#MCRS_DIMENSION=} ;;
            MCRS_SCENE=*) scene_file=$root/scripts/scenes/${knob#MCRS_SCENE=}.json ;;
        esac
    done
    place_dimension "$tmp/run/world" "$dimension"
    if [ -n "$scene_file" ] && ! "$tmp/scene" apply "$scene_file" "$world" "$tmp/run/world" \
        > "$out/$scene/$side.scene.log" 2>&1; then
        echo "scene $scene: building the $side world failed, see $out/$scene/$side.scene.log" >&2
        return
    fi
    echo "scene $scene: $side" >&2
    if ! (cd "$tmp/run" && env MCRS_RESOLUTION=1280x720 MCRS_VIEW=10 MCRS_HOT=1 \
        ${knobs[@]+"${knobs[@]}"} "$@" MCRS_CAPTURE="$out/$scene/$side.png" \
        "$tmp/$binary" "$tmp/run/world") > "$out/$scene/$side.log" 2>&1; then
        echo "scene $scene: the $side capture failed, see $out/$scene/$side.log" >&2
    fi
}

status=0
if [ $paths -eq 1 ]; then
    for i in "${!names[@]}"; do
        case " ${settings[$i]} " in
            *" MCRS_SMOOTH_LIGHTING=0 "*)
                echo "scene ${names[$i]}: skipped, flat lighting is Classic-only" >&2
                echo "${names[$i]}" >> "$out/skipped.txt"
                continue
                ;;
        esac
        echo "${names[$i]}" >> "$out/scenes.txt"
        run head classic "${names[$i]}" "${settings[$i]}" MCRS_RENDER_PATH=classic
        run head deferred "${names[$i]}" "${settings[$i]}" MCRS_RENDER_PATH=deferred
        run head mask "${names[$i]}" "${settings[$i]}" MCRS_RENDER_PATH=deferred MCRS_PARITY_MASK=1
    done
    python3 "$root/scripts/parity_report.py" --paths "$out" \
        "$(git -C "$root" describe --always --dirty)" || status=$?
else
    for i in "${!names[@]}"; do
        echo "${names[$i]}" >> "$out/scenes.txt"
        run base base "${names[$i]}" "${settings[$i]}"
        run head head "${names[$i]}" "${settings[$i]}"
    done
    python3 "$root/scripts/parity_report.py" "$out" \
        "$(git -C "$root" log -1 --format='%h %s' "$base")" \
        "$(git -C "$root" describe --always --dirty)" || status=$?
fi
echo "report: $out/report.md"
exit "$status"
