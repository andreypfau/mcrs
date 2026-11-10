#!/bin/bash
# The one entry point: records the machine, prepares it, builds, runs the benchmarks and zips the results.
# Usage: run.sh [--full] [--label NAME]
set -u
# shellcheck source=SCRIPTDIR/common.sh
. "$(dirname "$0")/common.sh"

clean_label() { printf '%s' "$1" | tr -cs 'A-Za-z0-9_-' '-' | sed 's/^-*//; s/-*$//'; }

profile=quick
label=""
while [ $# -gt 0 ]; do
    case "$1" in
        --full) profile=full; shift ;;
        --label) label="$2"; shift 2 ;;
        *) say "unknown argument: $1"; exit 2 ;;
    esac
done

repo="$(repo_of_kit)"
[ -n "$repo" ] || { say "run.sh must stay inside the mcrs checkout; run bootstrap.sh first."; exit 1; }

say "mcrs benchmark kit, profile: $profile"
if [ "$profile" = full ]; then say "Expected time: about 3-5 hours, depending on the machine."; else say "Expected time: about 45-90 minutes, depending on the machine."; fi
say "While it runs, do not touch the computer. The game window must stay visible, on top and not minimised."
say ""

label="$(clean_label "$label")"
while [ -z "$label" ]; do
    printf 'Machine label (for example m2-air or ryzen-3060): ' >&2
    read -r label || exit 1
    label="$(clean_label "$label")"
done

say "The mob benchmark runs the Minecraft server, which needs the Minecraft EULA: https://aka.ms/MinecraftEULA"
if ask "Do you accept the Minecraft EULA?"; then
    sections=(--accept-eula)
else
    say "Running without the mob section."
    sections=(--only "client,server,worldgen")
fi

name="$label-macos-$(date -u +%Y%m%dT%H%M%SZ)"
results="$KIT_DIR/results"
dir="$results/$name"
mkdir -p "$dir"

caffeinate -dims -w $$ &
awake=$!
# shellcheck disable=SC2329 # called by the EXIT trap
cleanup() { kill "$awake" 2>/dev/null; }
trap cleanup EXIT
trap 'say ""; say "Interrupted; partial results stay in $dir"; exit 130' INT TERM

"$KIT_DIR/sysinfo.sh" --label "$label" --out "$dir/system.json" || exit 1
"$KIT_DIR/prepare.sh" --out "$dir/hygiene.json" || exit 1
"$KIT_DIR/bootstrap.sh" --build-only || { say "The build failed; run $KIT_DIR/bootstrap.sh and try again."; exit 1; }

java="$(find_java)" || { say "Java $JAVA_MIN not found; run $KIT_DIR/bootstrap.sh first."; exit 1; }

# The client sizes a --window in physical pixels (scale factor 1), so the visible area of the main
# display, menu bar and dock excluded, is compared in pixels.
visible="$(osascript -l JavaScript -e 'ObjC.import("AppKit"); const s = $.NSScreen.screens.objectAtIndex(0); const f = s.visibleFrame; const k = s.backingScaleFactor; Math.floor(f.size.width * k) + " " + Math.floor(f.size.height * k)')"
window=""
for candidate in 1920x1080 1600x900 1280x720 960x540; do
    if [ "${candidate%x*}" -le $((${visible% *} - 40)) ] && [ "${candidate#*x}" -le $((${visible#* } - 40)) ]; then
        window="$candidate"
        break
    fi
done
window="${window:-960x540}"

heap=$(($(sysctl -n hw.memsize) / 1073741824 / 2))
[ "$heap" -gt 16 ] && heap=16
say "Window $window, Java heap ${heap} GB, results in $dir"

"$repo/target/release/perf-kit" run --out "$dir" --profile "$profile" --repo "$repo" \
    --window "$window" --heap "${heap}g" --java "$java" "${sections[@]}" 2>&1 | tee "$dir/run.log"
status=${PIPESTATUS[0]}
find "$dir" -type f -exec env LC_ALL=C sed -i '' "s|$HOME|~|g" {} +

"$KIT_DIR/prepare.sh" --check-only --out "$dir/hygiene-after.json"

(cd "$results" && ditto -c -k --keepParent "$name" "$name.zip") || { say "Could not create the zip; send the folder $dir instead."; exit 1; }
say ""
if [ "$status" -ne 0 ]; then say "Some sections failed (exit $status); the zip still holds everything that was measured."; fi
say "Done. Send this file: $results/$name.zip"
say "It holds system.json, hygiene.json, hygiene-after.json, report.json, raw/ and run.log."
exit "$status"
