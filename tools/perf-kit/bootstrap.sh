#!/bin/bash
# Sets up this Mac for the benchmarks: Command Line Tools, git, the repository, rustup, the pinned
# toolchain, Java 25 and the release build. Safe to run again; every step checks before it installs.
# Usage: bootstrap.sh [--check | --build-only]
#   --check       report what is missing, install nothing, build nothing
#   --build-only  only run the release build (used by run.sh)
set -u

REPO_URL="https://github.com/andreypfau/mcrs"
CLONE_DIR="$HOME/mcrs-perf/mcrs"
MIN_DISK_GB=40
CLT_MARKER="/tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress"
PROXY_HINT="check the internet connection; behind a proxy run: export HTTPS_PROXY=http://host:port, then rerun"

mode=full
case "${1:-}" in
    --check) mode=check ;;
    --build-only) mode=build-only ;;
    "") ;;
    *) printf 'unknown argument: %s\n' "$1"; exit 2 ;;
esac

names=()
statuses=()
hints=()
# step NAME STATUS [HINT]
step() {
    names+=("$1")
    statuses+=("$2")
    hints+=("${3:-}")
    printf '[%s] %s%s\n' "$2" "$1" "${3:+ - $3}"
}

summary() {
    local i failed=0
    printf '\n%-14s %-8s %s\n' STEP STATUS HINT
    for i in "${!names[@]}"; do
        printf '%-14s %-8s %s\n' "${names[$i]}" "${statuses[$i]}" "${hints[$i]}"
        case "${statuses[$i]}" in FAILED | MISSING) failed=1 ;; esac
    done
    exit $failed
}

load_cargo_env() {
    local env_file="${CARGO_HOME:-$HOME/.cargo}/env"
    # shellcheck disable=SC1090
    [ -f "$env_file" ] && . "$env_file"
    return 0
}

kit_repo() {
    local root
    root="$(cd "$(dirname "$0")/../.." 2>/dev/null && pwd)"
    if [ -f "$root/Cargo.toml" ] && [ -f "$root/rust-toolchain.toml" ]; then printf '%s\n' "$root"; fi
}

build() {
    local repo="$1"
    msg "Building the release binaries in $repo (the first build takes 10-40 minutes)" "Собираем release-бинарники в $repo (первая сборка занимает 10-40 минут)"
    if (cd "$repo" && cargo build --release -p mcrs -p mcrs_minecraft_client -p mcrs_mob_bench -p mcrs_perf_kit); then
        step build OK
    else
        step build FAILED "$(msg "see the cargo output above; rerun bootstrap.sh to check the toolchain" "причина в выводе cargo выше; чтобы проверить тулчейн, запустите bootstrap.sh еще раз")"
    fi
}

if [ "$mode" = build-only ]; then
    load_cargo_env
    # shellcheck source=SCRIPTDIR/common.sh
    . "$(dirname "$0")/common.sh"
    repo="$(kit_repo)"
    [ -n "$repo" ] || { step build FAILED "$(msg "run bootstrap.sh from inside the mcrs checkout" "запустите bootstrap.sh из клона mcrs")"; summary; }
    build "$repo"
    summary
fi

# network
offline=""
for host in static.rust-lang.org piston-data.mojang.com api.adoptium.net; do
    curl -sS -o /dev/null -m 15 -I "https://$host" 2>/dev/null || offline="$offline $host"
done
if [ -z "$offline" ]; then step network OK; else step network FAILED "cannot reach$offline; $PROXY_HINT"; fi

# Xcode Command Line Tools
if xcode-select -p >/dev/null 2>&1; then
    step "xcode clt" OK
elif [ "$mode" = check ]; then
    step "xcode clt" MISSING "run bootstrap.sh without --check"
else
    printf 'Installing the Xcode Command Line Tools; macOS asks for your password.\n'
    touch "$CLT_MARKER"
    label="$(softwareupdate -l 2>/dev/null | awk -F'Label: ' '/Label: Command Line Tools/ { print $2 }' | tail -1)"
    [ -n "$label" ] && sudo softwareupdate -i "$label" --verbose
    rm -f "$CLT_MARKER"
    if ! xcode-select -p >/dev/null 2>&1; then
        printf 'A dialog opens: click Install and wait. The script continues when it is done.\n'
        xcode-select --install >/dev/null 2>&1
        waited=0
        while ! xcode-select -p >/dev/null 2>&1 && [ $waited -lt 1800 ]; do
            sleep 10
            waited=$((waited + 10))
        done
    fi
    if xcode-select -p >/dev/null 2>&1; then step "xcode clt" FIXED; else step "xcode clt" FAILED "run: xcode-select --install"; fi
fi

# git ships with the Command Line Tools
if xcode-select -p >/dev/null 2>&1 && git --version >/dev/null 2>&1; then
    step git OK
else
    step git FAILED "install the Xcode Command Line Tools first"
fi

# repository
repo="$(kit_repo)"
if [ -n "$repo" ]; then
    step repo OK "$repo"
elif [ -f "$CLONE_DIR/Cargo.toml" ]; then
    repo="$CLONE_DIR"
    step repo OK "$repo"
elif [ "$mode" = check ]; then
    step repo MISSING "run bootstrap.sh without --check to clone into $CLONE_DIR"
else
    mkdir -p "$(dirname "$CLONE_DIR")"
    if git clone "$REPO_URL" "$CLONE_DIR"; then
        repo="$CLONE_DIR"
        step repo FIXED "$repo"
    else
        step repo FAILED "git clone $REPO_URL failed; $PROXY_HINT"
    fi
fi
[ -n "$repo" ] || summary
# shellcheck source=SCRIPTDIR/common.sh
. "$repo/tools/perf-kit/common.sh"

# disk
free="$(free_gb "$repo")"
if [ "${free:-0}" -ge $MIN_DISK_GB ]; then
    step disk OK "${free} GB free"
else
    step disk FAILED "${free:-?} GB free on the disk of $repo, need $MIN_DISK_GB GB; free space and rerun"
    summary
fi

# rustup
load_cargo_env
if command -v rustup >/dev/null 2>&1; then
    step rustup OK
elif [ "$mode" = check ]; then
    step rustup MISSING "run bootstrap.sh without --check"
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal
    load_cargo_env
    if command -v rustup >/dev/null 2>&1; then step rustup FIXED; else step rustup FAILED "rustup download failed; $PROXY_HINT"; fi
fi

# the toolchain pinned by rust-toolchain.toml
channel="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$repo/rust-toolchain.toml")"
if ! command -v rustup >/dev/null 2>&1; then
    step toolchain FAILED "install rustup first"
elif rustup toolchain list 2>/dev/null | grep -q "^$channel"; then
    step toolchain OK "$channel"
elif [ "$mode" = check ]; then
    step toolchain MISSING "$channel; run bootstrap.sh without --check (about 1.5 GB)"
else
    printf 'Installing %s with clippy, rustfmt and the wasm targets (about 1.5 GB)\n' "$channel"
    (cd "$repo" && { rustup toolchain install || rustup show; })
    if rustup toolchain list 2>/dev/null | grep -q "^$channel"; then step toolchain FIXED "$channel"; else step toolchain FAILED "$PROXY_HINT"; fi
fi

# Java 25, from the system or a kit-local Temurin
if java="$(find_java)"; then
    step java OK "$java"
elif [ "$mode" = check ]; then
    step java MISSING "Java $JAVA_MIN or newer; run bootstrap.sh without --check"
else
    case "$(uname -m)" in arm64) arch=aarch64 ;; *) arch=x64 ;; esac
    url="https://api.adoptium.net/v3/binary/latest/$JAVA_MIN/ga/mac/$arch/jdk/hotspot/normal/eclipse"
    printf 'Downloading Temurin %s into %s/jdk\n' "$JAVA_MIN" "$KIT_HOME"
    mkdir -p "$KIT_HOME/jdk"
    if curl -fL --retry 2 -o "$KIT_HOME/jdk.tar.gz" "$url" && tar -xzf "$KIT_HOME/jdk.tar.gz" -C "$KIT_HOME/jdk"; then
        rm -f "$KIT_HOME/jdk.tar.gz"
    fi
    if java="$(find_java)"; then step java FIXED "$java"; else step java FAILED "download from $url failed; $PROXY_HINT"; fi
fi

# release build
if [ "$mode" = check ]; then
    if [ -x "$repo/target/release/perf-kit" ]; then step build OK "binaries present"; else step build MISSING "run bootstrap.sh without --check"; fi
elif command -v cargo >/dev/null 2>&1; then
    build "$repo"
else
    step build FAILED "install rustup and the toolchain first"
fi

summary
