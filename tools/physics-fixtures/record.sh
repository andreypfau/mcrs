#!/usr/bin/env bash
# Records the entity physics experiments of docs/spec/entity-physics.md, section 15, from the
# vanilla dedicated server of the corpus version. The server runs headless; record.py drives it
# through its console with /summon, /tick step and /data get, and writes one JSON fixture per
# experiment. Needs MINECRAFT_EULA=true, as tools/vanilla-server/run.sh does.
set -euo pipefail

fixtures_here=$(cd "$(dirname "$0")" && pwd)
fixtures_root=$(cd "$fixtures_here/../.." && pwd)
# shellcheck source=../vanilla-server/run.sh
source "$fixtures_root/tools/vanilla-server/run.sh"

if [ "${MINECRAFT_EULA:-}" != true ]; then
    echo "The Minecraft EULA is not accepted in this environment." >&2
    echo "Read https://aka.ms/MinecraftEULA; setting MINECRAFT_EULA=true states the acceptance." >&2
    exit 3
fi

release="$fixtures_root/crates/mcrs_minecraft_client_jar/src/release.json"
id=$(jq -r '.id' "$release")
work="$fixtures_root/target/vanilla-server/$id-physics"
mkdir -p "$work"
fetch_verified "$work/$id.json" "$(jq -r '.json.url' "$release")" "$(jq -r '.json.sha1' "$release")" || exit 2
fetch_verified "$work/server.jar" \
    "$(jq -r '.downloads.server.url' "$work/$id.json")" \
    "$(jq -r '.downloads.server.sha1' "$work/$id.json")" \
    "$(jq -r '.downloads.server.size' "$work/$id.json")" || exit 2

echo "eula=true" >"$work/eula.txt"
cat >"$work/server.properties" <<EOF
online-mode=false
server-ip=127.0.0.1
server-port=${PHYSICS_PORT:-25567}
level-type=minecraft\:flat
generate-structures=false
spawn-protection=0
view-distance=4
simulation-distance=4
enable-rcon=false
enable-query=false
EOF

out="${1:-$fixtures_root/crates/mcrs_minecraft_collision_table/tests/fixtures/physics}"
mkdir -p "$out"
python3 -I "$fixtures_here/record.py" --work "$work" --out "$out" --version "$id"
