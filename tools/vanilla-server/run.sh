#!/usr/bin/env bash
# Provisions the vanilla dedicated server of the corpus version under target/vanilla-server,
# verified against the package descriptor pinned in release.json, with online mode off on
# loopback and the compression threshold of VANILLA_COMPRESSION (default -1, compression off;
# 0 and up compress packets of that size and more). `serve` runs it until interrupted; `attempt`
# (the default) runs one headless join of the MCRS client against it. Exit: 0 joined, 1 not
# joined, 2 provisioning failed or a bad argument, 3 the Minecraft EULA is not accepted in this
# environment.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
release="$root/crates/mcrs_minecraft_client_jar/src/release.json"
port="${VANILLA_PORT:-25566}"
compression="${VANILLA_COMPRESSION:--1}"

sha1_of() {
    shasum -a 1 "$1" | cut -d' ' -f1
}

check_file() {
    local file=$1 sha1=$2 size=${3:-}
    if [ "$(sha1_of "$file")" != "$sha1" ]; then
        echo "$file fails its SHA-1: expected $sha1, got $(sha1_of "$file")" >&2
        rm -f "$file"
        return 1
    fi
    if [ -n "$size" ] && [ "$(wc -c <"$file" | tr -d ' ')" != "$size" ]; then
        echo "$file fails its size: expected $size, got $(wc -c <"$file" | tr -d ' ')" >&2
        rm -f "$file"
        return 1
    fi
}

fetch_verified() {
    local file=$1 url=$2 sha1=$3 size=${4:-}
    if [ ! -f "$file" ]; then
        curl -fsSL --retry 3 -o "$file" "$url" || { rm -f "$file"; echo "could not fetch $url" >&2; return 1; }
    fi
    check_file "$file" "$sha1" "$size"
}

# The server reads the threshold with Integer.parseInt and silently falls back to 256 on a
# value outside the int range.
valid_compression() {
    [[ $1 == -1 ]] && return 0
    [[ $1 =~ ^0*([0-9]{1,10})$ ]] && ((10#${BASH_REMATCH[1]} <= 2147483647))
}

main() {
    local mode=${1:-attempt}
    case "$mode" in
        attempt | serve) ;;
        *) echo "usage: $0 [attempt|serve]" >&2; exit 2 ;;
    esac

    if [ "${MINECRAFT_EULA:-}" != true ]; then
        echo "The Minecraft EULA is not accepted in this environment." >&2
        echo "Read https://aka.ms/MinecraftEULA; setting MINECRAFT_EULA=true states the acceptance." >&2
        exit 3
    fi

    if ! valid_compression "$compression"; then
        echo "VANILLA_COMPRESSION must be -1 or a whole number from 0 to 2147483647, got '$compression'" >&2
        exit 2
    fi

    local id package_url package_sha1
    id=$(jq -r '.id' "$release")
    package_url=$(jq -r '.json.url' "$release")
    package_sha1=$(jq -r '.json.sha1' "$release")

    if nc -z 127.0.0.1 "$port" 2>/dev/null; then
        echo "port $port is already in use" >&2
        exit 2
    fi

    local work="$root/target/vanilla-server/$id"
    mkdir -p "$work"

    fetch_verified "$work/$id.json" "$package_url" "$package_sha1" || exit 2
    local server_url server_sha1 server_size
    server_url=$(jq -r '.downloads.server.url' "$work/$id.json")
    server_sha1=$(jq -r '.downloads.server.sha1' "$work/$id.json")
    server_size=$(jq -r '.downloads.server.size' "$work/$id.json")
    fetch_verified "$work/server.jar" "$server_url" "$server_sha1" "$server_size" || exit 2

    echo "eula=true" >"$work/eula.txt"
    cat >"$work/server.properties" <<EOF
online-mode=false
network-compression-threshold=$compression
server-ip=127.0.0.1
server-port=$port
level-seed=42
gamemode=creative
force-gamemode=true
view-distance=6
simulation-distance=6
spawn-protection=0
enforce-secure-profile=false
enable-rcon=false
enable-query=false
EOF

    local log="$work/server.log"
    (cd "$work" && exec java -Xmx2G -jar server.jar nogui >"$log" 2>&1 </dev/null) &
    server=$!
    trap 'kill "$server" 2>/dev/null && wait "$server" 2>/dev/null || true' EXIT
    trap 'exit 130' INT TERM

    local up=
    for _ in $(seq 1 300); do
        if nc -z 127.0.0.1 "$port" 2>/dev/null; then up=1; break; fi
        kill -0 "$server" 2>/dev/null || { echo "server exited early, see $log" >&2; exit 2; }
        sleep 1
    done
    [ -n "$up" ] || { echo "server never opened port $port, see $log" >&2; exit 2; }

    if [ "$mode" = serve ]; then
        echo "vanilla $id listening on 127.0.0.1:$port, compression threshold $compression; interrupt to stop"
        wait "$server"
        return
    fi

    if (cd "$root" && MCRS_VANILLA_SERVER="127.0.0.1:$port" \
        cargo test -p mcrs --test client_joins_vanilla_server -- --ignored --nocapture); then
        echo "verdict: joined"
    else
        echo "verdict: not joined (server log: $log)"
        return 1
    fi
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    main "$@"
fi
