#!/bin/sh
# Time to a fully loaded world at the client's default render distance.
# Prints the per-second load curve and the settle time, from the F3 stats line.
set -e
SAVE="${SAVE:-$HOME/Library/Application Support/minecraft/saves/two}"
SECONDS_TO_RUN="${SECONDS_TO_RUN:-150}"
OUT="${OUT:-/tmp/mcrs-chunk-load.log}"

MCRS_STATS=1 ./target/release/mcrs_minecraft_client "$SAVE" >"$OUT" 2>&1 &
pid=$!
sleep "$SECONDS_TO_RUN"
kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true

sed 's/\x1b\[[0-9;]*m//g' "$OUT" | awk '
  /mcrs_client::stats/ {
    if (!match($0, /Sections: [0-9]+\/[0-9]+ in [0-9]+ columns/)) next
    split(substr($0, RSTART + 10, RLENGTH - 10), s, /[\/ ]/)
    done = s[1]; total = s[2]; cols = s[4]
    q = 0; fl = 0
    if (match($0, /Mesh: [0-9]+ queued, [0-9]+ in flight/)) {
      split(substr($0, RSTART + 6, RLENGTH - 6), m, /[, ]+/)
      q = m[1]; fl = m[3]
    }
    printf "%3d s  sections %7d / %7d   columns %6d   mesh queued %7d   in flight %4d\n", n, done, total, cols, q, fl
    if (done == prev_done && total == prev_total && q == 0 && fl == 0 && cols > 1000) {
      if (settled == "") settled = n - 1
    } else settled = ""
    prev_done = done; prev_total = total; last = done; lastcols = cols
    n++
  }
  END {
    print ""
    if (settled != "") printf "SETTLED at %d s  (%d sections in %d columns)\n", settled, last, lastcols
    else printf "NOT SETTLED within %d s\n", n - 1
  }
'
