#!/usr/bin/env bash
# The crates below must build with their `bevy` feature off and pull in no Bevy
# crate. `bevy_math` is exempt: without its default features it is glam.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
crates=(
    mcrs_minecraft_nbt
    mcrs_minecraft_random
    mcrs_minecraft_core
    mcrs_minecraft_entity
    mcrs_minecraft_item
    mcrs_minecraft_chunk
    mcrs_minecraft_registry
    mcrs_minecraft_keys
    mcrs_minecraft_text
    mcrs_minecraft_profile
    mcrs_minecraft_block
    mcrs_minecraft_block_predicate
    mcrs_minecraft_particle
    mcrs_minecraft_sound
    mcrs_minecraft_value_provider
    mcrs_minecraft_biome
    mcrs_minecraft_biome_file
    mcrs_minecraft_dimension
    mcrs_minecraft_enchantment
    mcrs_minecraft_predicate
    mcrs_minecraft_loot
    mcrs_minecraft_game_rule
    mcrs_minecraft_registry_catalog
    mcrs_minecraft_protocol
    mcrs_minecraft_anvil
    mcrs_minecraft_worldgen_testing
    mcrs_minecraft_worldgen_noise
    mcrs_minecraft_worldgen_density
    mcrs_minecraft_worldgen_surface
    mcrs_minecraft_worldgen_feature
    mcrs_minecraft_worldgen_feature_place
    mcrs_minecraft_worldgen_carver
    mcrs_minecraft_worldgen_structure
    mcrs_minecraft_worldgen
    mcrs_minecraft_world
    mcrs_minecraft_dimension_environment
    mcrs_minecraft_light
    mcrs_minecraft_light_color
    mcrs_minecraft_network
    mcrs_minecraft_mesh
    mcrs_minecraft_client_jar
)

status=0
for crate in "${crates[@]}"; do
    leaked=$(cargo tree --locked --manifest-path "$root/Cargo.toml" -p "$crate" --no-default-features \
        -e normal --prefix none --format '{p}' |
        awk '$1 ~ /^bevy_/ && $1 != "bevy_math" { print $1 }' | sort -u | tr '\n' ' ')
    if [ -n "$leaked" ]; then
        echo "$crate pulls in Bevy: $leaked" >&2
        status=1
    fi
done

packages=()
for crate in "${crates[@]}"; do packages+=(-p "$crate"); done
cargo check --locked --manifest-path "$root/Cargo.toml" --no-default-features "${packages[@]}"

exit $status
