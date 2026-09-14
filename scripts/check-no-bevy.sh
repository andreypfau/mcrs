#!/usr/bin/env bash
# The crates below must build with their `bevy` feature off and pull in no Bevy
# crate. `bevy_math` is exempt: without its default features it is glam.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
crates=(
    mcrs_minecraft_nbt
    mcrs_minecraft_random
    mcrs_voxel_math
    mcrs_voxel_storage
    mcrs_minecraft_core
    mcrs_minecraft_protocol
    mcrs_minecraft_anvil
    mcrs_minecraft_worldgen
    mcrs_minecraft_decoration
    mcrs_minecraft_light
    mcrs_minecraft_network
)

status=0
for crate in "${crates[@]}"; do
    leaked=$(cargo tree --manifest-path "$root/Cargo.toml" -p "$crate" --no-default-features \
        -e normal --prefix none --format '{p}' |
        awk '$1 ~ /^bevy_/ && $1 != "bevy_math" { print $1 }' | sort -u | tr '\n' ' ')
    if [ -n "$leaked" ]; then
        echo "$crate pulls in Bevy: $leaked" >&2
        status=1
    fi
done

packages=()
for crate in "${crates[@]}"; do packages+=(-p "$crate"); done
cargo check --manifest-path "$root/Cargo.toml" --no-default-features "${packages[@]}"

exit $status
