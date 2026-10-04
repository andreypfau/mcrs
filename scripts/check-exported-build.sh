#!/usr/bin/env bash
# A tree exported from git, outside any repository, must build the network crate and its
# identity tests must pass there with the commit hash placeholder.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
tree=${1:-HEAD}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

if git -C "$tmp" rev-parse --git-dir > /dev/null 2>&1; then
    echo "$tmp is inside a repository; the export would not be a tree without one" >&2
    exit 1
fi

git -C "$root" archive "$tree" | tar -x -C "$tmp"
if [ -e "$tmp/.git" ]; then
    echo "the export holds a .git entry" >&2
    exit 1
fi

log=$tmp/test.log
(cd "$tmp" && CARGO_TARGET_DIR="$root/target/exported" \
    cargo test -p mcrs_minecraft_network --locked --lib identity) 2>&1 | tee "$log"

if ! grep -Eq '^test result: ok\. [1-9][0-9]* passed' "$log"; then
    echo "the exported tree ran no passing identity test" >&2
    exit 1
fi
