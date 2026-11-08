#!/usr/bin/env bash
# Writes summary.csv in the given folder: one row a phase of every run report in it.
set -euo pipefail
dir=${1:?usage: summary.sh <folder of run reports>}
{
    echo "run,server,client,threads,phase,skipped,pigs_before,pigs_after,mspt_samples,mspt_mean,mspt_p50,mspt_p95,mspt_p99,mspt_worst,console_tps,console_ms_per_tick,watcher_set_time_p50,watcher_set_time_p95,watcher_set_time_p99,sprint_wall_ms,sprint_server_ms,motion_console_pigs,motion_console_moved,motion_console_path_bps,motion_watcher_pigs,motion_watcher_moved,motion_watcher_path_bps,watcher_moves_per_s,busy_cores,server_cores,harness_cores,other_cores,rss_mib_mean,load_1m_max,contended"
    for file in "$dir"/*.json; do
        jq -r --arg run "$(basename "$file" .json)" '
            . as $r | .phases[] | [
                $run, $r.server, $r.client, ($r.threads // ""), .phase, (.skipped // ""),
                .pigs_before, .pigs_after,
                .server_mspt.samples, .server_mspt.mean, .server_mspt.p50, .server_mspt.p95,
                .server_mspt.p99, .server_mspt.worst,
                .external.console_game_time.tps, .external.console_game_time.mean_ms_per_tick,
                .external.watcher_set_time.p50_ms, .external.watcher_set_time.p95_ms,
                .external.watcher_set_time.p99_ms,
                .external.sprint.wall_ms_per_tick, .external.sprint.server_ms_per_tick,
                .motion.console.pigs, .motion.console.moved_fraction,
                .motion.console.mean_path_blocks_per_second,
                .motion.watcher.pigs, .motion.watcher.moved_fraction,
                .motion.watcher.mean_path_blocks_per_second, .motion.watcher_move_packets_per_second,
                .cpu.busy_cores, .cpu.server_cores, .cpu.harness_cores, .cpu.other_cores,
                .server_rss_mib_mean, .load_average_1m_max, .contended
            ] | map(if . == null then "" else . end) | @csv' "$file"
    done
} >"$dir/summary.csv"
echo "$dir/summary.csv"
