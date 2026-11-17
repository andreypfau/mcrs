#!/usr/bin/env bash
set -euo pipefail

days=${1:-14}
if [[ ! $days =~ ^[0-9]+$ ]]; then
    echo "usage: $0 [days]" >&2
    exit 2
fi
build_dir=$(cargo metadata --no-deps --format-version 1 | jq -r .build_directory)
[[ -d $build_dir ]] || exit 0

before=$(du -sk "$build_dir" | cut -f1)

find "$build_dir" -type d -path '*/build/*/*/fingerprint' -prune -print |
    while read -r fingerprint; do
        if [[ -z $(find "$fingerprint" -mtime "-$days" -print -quit) ]]; then
            rm -rf "$(dirname "$fingerprint")"
        fi
    done

find "$build_dir" -mindepth 2 -maxdepth 4 -type d -path '*/incremental/*' ! -path '*/incremental/*/*' |
    while read -r cache; do
        if [[ -z $(find "$cache" -mtime "-$days" -print -quit) ]]; then
            rm -rf "$cache"
        fi
    done

find "$build_dir" -mindepth 3 -type d -empty -delete

after=$(du -sk "$build_dir" | cut -f1)
echo "pruned $(((before - after) / 1024)) MiB from $build_dir, $((after / 1024)) MiB left"
