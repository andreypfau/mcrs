# mcrs_minecraft_server_bench

Loads one dedicated server, mcrs or vanilla, with protocol bots, and writes one JSON report per
run. `run.sh` runs the matrix: each server, bot count and behaviour on a fresh world, one server
at a time.

```sh
cargo build --release -p mcrs -p mcrs_minecraft_server_bench --bins
MINECRAFT_EULA=true tools/mcrs_minecraft_server_bench/run.sh
```

The environment sets the matrix: `SERVERS` (`mcrs vanilla`), `BOTS` (`1 10 25 50`),
`BEHAVIOURS` (`walk explore idle mixed`), `WARMUP` (60 s), `MEASURE` (120 s), `EXPLORE_SPEED`
(0.8 blocks a second), `OUT` (`target/server-bench`), `JAVA` (`java`), and `SERVER_CPUS` and
`BOT_CPUS` (CPU lists for `taskset`; unset runs unpinned). The script downloads the vanilla
server jar of `crates/mcrs_minecraft_client_jar/src/release.json` and checks its SHA-1. It
waits while the 1-minute load average is 40 or more.

## Method

The method follows the equal-work server benchmarks of Horizon (`server-equal-work-2026-10-17`,
`server-mixed-2026-10-18`, `server-four-way-2026-10-19`).

**Same settings.** Seed 12345, view and simulation distance 10, offline, creative, network
compression off, a fresh world for each run. Vanilla also has difficulty easy,
`sync-chunk-writes=true` and a 4 GB heap (`-Xms4g -Xmx4g`). mcrs gets the same seed and game
mode from `MCRS_*` variables; it has no difficulty setting and does not save chunks. Both
servers log at INFO.

**Same game rules.** Vanilla runs with `locator_bar`, `spawn_mobs` and `spawn_monsters` off,
because mcrs has none of these yet, and with `respawn_radius 0`, because mcrs puts every player
on one spawn point.

**Bots.** One joins each second. After the last join, a warm-up, then the measured window. A
bot sends its moves at 20 a second and a `client_tick_end` after each tick, sends
`player_loaded` after the first teleport, answers keep-alives and teleports, and acknowledges
each chunk batch with a desired rate of 64 chunks a tick. It does not move while the column it
stands in has not arrived. All bots first climb 24 blocks above spawn at 5.6 blocks a second.

- `walk`: random points within 48 blocks of spawn, at 5.6 blocks a second.
- `explore`: straight out on a heading per bot, at `EXPLORE_SPEED`. 0.8 blocks a second is slow
  enough that neither server makes a bot wait for terrain.
- `idle`: hovers.
- `mixed`: 60 % walk, 20 % explore and 20 % idle, chosen by bot index.

**Equal-work checks**, over the measured window and summed over the bots: chunks received a
second, first-time chunks a second (columns that no bot received before the window),
chunks resent while a bot held them, pull-backs (teleports after the first), and the ticks a
bot waited for its own column.

**Tick time.** Vanilla: `/tick query` on the console every 5 seconds; each answer covers the
last 100 ticks, printed to 0.1 ms. mcrs has no `/tick query`. Its server loop logs the time of
each tick (the update and the hand-over after it, without the wait for the next tick) at trace
level under the target `mcrs_tick`, and the tool cuts the ticks into windows of 100. In both
cases a window gives its mean and its nearest-rank P50, P95 and P99. The report gives the means
of the windows and the highest window P99. TPS is 20 while the mean fits in 50 ms, else
1000 / mean.

**Process.** RSS and CPU time of the server's process tree from `/proc`, once a second over the
measured window (100 % is one CPU). Start is the time from launch to the first answer to a
status request.

## Results (2026-11-08)

**Machine.** AMD EPYC 9454 (48 cores, 96 threads), a shared production host. One container on
CPUs 80 to 95 under `nice -n 19`: the server on CPUs 80 to 87, the bots on CPUs 88 to 95. The
1-minute load average of the host was 4.7 to 28 at the start and the end of each run. It was 64
and 56 at the ends of `vanilla explore 1` and `vanilla idle 1`, from other work on the host; the
next run then waited until it fell under 40. The cpuset does not keep other processes off
CPUs 80 to 95, so the times are an upper bound. Warm-up 60 s, measured window 120 s, 23 or 24
tick windows a run.

**What ran.** mcrs at 49524647f, a release build for x86-64 with no target-cpu flags. Vanilla
26.4-snapshot-7 on Temurin 25.0.5. In every run, every bot joined and stayed to the end.

MSPT, P50, P95, P99 and worst P99 are in milliseconds. RSS and CPU cover the server's process
tree. 100 % CPU is one CPU.

| Behaviour | Bots | Server | MSPT | P50 | P95 | P99 | Worst P99 | TPS | RSS mean / max | CPU | Start |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| walk | 1 | mcrs | 0.86 | 0.72 | 2.07 | 3.11 | 3.7 | 20 | 1.24 / 1.25 GiB | 21 % | 2.8 s |
| walk | 1 | vanilla | 2.83 | 2.67 | 3.82 | 7.4 | 17.4 | 20 | 3.24 / 3.25 GiB | 17 % | 7.2 s |
| walk | 10 | mcrs | 1.33 | 0.94 | 2.82 | 3.79 | 4.7 | 20 | 1.41 / 1.42 GiB | 23 % | 1.2 s |
| walk | 10 | vanilla | 4.76 | 4.49 | 6.64 | 9.19 | 19.4 | 20 | 3.24 / 3.24 GiB | 25 % | 7.1 s |
| walk | 25 | mcrs | 1.91 | 1.54 | 3.93 | 5.02 | 6.4 | 20 | 1.53 / 1.55 GiB | 28 % | 1.2 s |
| walk | 25 | vanilla | 6.92 | 6.77 | 9.19 | 12.82 | 24.7 | 20 | 3.32 / 3.39 GiB | 41 % | 7.2 s |
| walk | 50 | mcrs | 2.68 | 2.44 | 5.10 | 6.22 | 7.4 | 20 | 1.72 / 1.74 GiB | 31 % | 0.9 s |
| walk | 50 | vanilla | 9.72 | 10.1 | 13.88 | 16.95 | 24.7 | 20 | 3.41 / 3.44 GiB | 65 % | 7.3 s |
| explore | 1 | mcrs | 0.74 | 0.71 | 0.86 | 1.80 | 7.2 | 20 | 1.21 / 1.25 GiB | 8 % | 1.1 s |
| explore | 1 | vanilla | 3.64 | 3.51 | 4.61 | 9.7 | 24.8 | 20 | 3.27 / 3.27 GiB | 21 % | 7.3 s |
| explore | 10 | mcrs | 0.99 | 0.84 | 1.87 | 3.07 | 7.7 | 20 | 1.76 / 2.08 GiB | 25 % | 0.9 s |
| explore | 10 | vanilla | 6.81 | 6.65 | 8.12 | 13.29 | 23.4 | 20 | 3.30 / 3.35 GiB | 51 % | 7.0 s |
| explore | 25 | mcrs | 1.09 | 0.90 | 2.26 | 3.26 | 6.3 | 20 | 1.98 / 2.32 GiB | 29 % | 0.9 s |
| explore | 25 | vanilla | 8.80 | 8.62 | 10.65 | 18.22 | 39.9 | 20 | 3.37 / 3.46 GiB | 69 % | 7.3 s |
| explore | 50 | mcrs | 1.39 | 1.11 | 2.87 | 3.65 | 4.9 | 20 | 2.30 / 2.68 GiB | 33 % | 0.9 s |
| explore | 50 | vanilla | 11.95 | 11.93 | 14.43 | 19.74 | 53.8 | 20 | 3.43 / 3.50 GiB | 94 % | 7.3 s |
| idle | 1 | mcrs | 0.75 | 0.74 | 0.84 | 1.04 | 3.9 | 20 | 1.11 / 1.11 GiB | 3 % | 0.9 s |
| idle | 1 | vanilla | 2.83 | 2.76 | 3.34 | 4.64 | 17.0 | 20 | 3.22 / 3.22 GiB | 13 % | 7.1 s |
| idle | 10 | mcrs | 0.83 | 0.81 | 0.95 | 1.14 | 2.0 | 20 | 1.15 / 1.15 GiB | 3 % | 0.9 s |
| idle | 10 | vanilla | 3.55 | 3.48 | 4.24 | 5.28 | 17.7 | 20 | 3.25 / 3.25 GiB | 18 % | 7.1 s |
| idle | 25 | mcrs | 0.79 | 0.78 | 0.89 | 0.95 | 1.4 | 20 | 1.26 / 1.26 GiB | 3 % | 0.9 s |
| idle | 25 | vanilla | 4.03 | 3.91 | 4.89 | 6.57 | 17.0 | 20 | 3.25 / 3.26 GiB | 24 % | 7.4 s |
| idle | 50 | mcrs | 0.84 | 0.82 | 0.99 | 1.07 | 1.8 | 20 | 1.43 / 1.43 GiB | 4 % | 1.0 s |
| idle | 50 | vanilla | 6.29 | 6.20 | 7.55 | 9.04 | 22.9 | 20 | 3.31 / 3.41 GiB | 44 % | 7.2 s |
| mixed | 1 | mcrs | 0.88 | 0.72 | 2.03 | 3.35 | 4.4 | 20 | 1.25 / 1.25 GiB | 22 % | 1.0 s |
| mixed | 1 | vanilla | 3.08 | 2.96 | 4.18 | 7.57 | 16.6 | 20 | 3.24 / 3.25 GiB | 18 % | 7.7 s |
| mixed | 10 | mcrs | 1.20 | 0.91 | 2.64 | 3.69 | 5.6 | 20 | 1.52 / 1.65 GiB | 25 % | 0.9 s |
| mixed | 10 | vanilla | 5.70 | 5.50 | 7.33 | 12.29 | 22.3 | 20 | 3.29 / 3.34 GiB | 37 % | 7.4 s |
| mixed | 25 | mcrs | 1.59 | 1.20 | 3.35 | 4.64 | 6.2 | 20 | 1.81 / 2.04 GiB | 29 % | 0.9 s |
| mixed | 25 | vanilla | 8.63 | 8.39 | 10.70 | 15.10 | 28.1 | 20 | 3.35 / 3.42 GiB | 62 % | 7.3 s |
| mixed | 50 | mcrs | 2.20 | 1.87 | 4.26 | 5.74 | 8.8 | 20 | 2.21 / 2.54 GiB | 35 % | 0.9 s |
| mixed | 50 | vanilla | 12.88 | 12.77 | 15.98 | 20.73 | 37.7 | 20 | 3.44 / 3.50 GiB | 97 % | 7.1 s |

### Equal-work checks

Chunks a second and first-time chunks a second are sums over the bots. "Pulled back" counts the
teleports after the first. The last column counts the vanilla log lines "moved wrongly" and
"moved too quickly" over the whole run, warm-up included.

| Behaviour | Bots | Server | Chunks/s | First-time chunks/s | Resent | Pulled back | Waits for chunk | Moves | Log moved wrongly / too quickly |
|---|---:|---|---:|---:|---:|---:|---:|---:|---:|
| walk | 1 | mcrs | 9.78 | 0.26 | 0 | 0 | 0 | 2400 | |
| walk | 1 | vanilla | 9.97 | 0.28 | 0 | 0 | 0 | 2400 | 0 / 0 |
| walk | 10 | mcrs | 101.48 | 0.03 | 0 | 0 | 0 | 24000 | |
| walk | 10 | vanilla | 100.78 | 0.04 | 0 | 0 | 0 | 24000 | 0 / 0 |
| walk | 25 | mcrs | 254.65 | 0.02 | 0 | 0 | 0 | 60000 | |
| walk | 25 | vanilla | 253.48 | 0.08 | 0 | 0 | 0 | 60000 | 0 / 0 |
| walk | 50 | mcrs | 512.42 | 0.02 | 0 | 0 | 0 | 120000 | |
| walk | 50 | vanilla | 518.86 | 0.07 | 0 | 0 | 0 | 120000 | 0 / 0 |
| explore | 1 | mcrs | 1.15 | 1.15 | 0 | 0 | 0 | 2400 | |
| explore | 1 | vanilla | 1.15 | 1.15 | 0 | 0 | 0 | 2400 | 0 / 0 |
| explore | 10 | mcrs | 14.57 | 5.73 | 0 | 0 | 0 | 24000 | |
| explore | 10 | vanilla | 14.38 | 5.35 | 0 | 0 | 0 | 24000 | 0 / 1 |
| explore | 25 | mcrs | 36.23 | 6.42 | 0 | 0 | 0 | 60000 | |
| explore | 25 | vanilla | 36.23 | 5.88 | 0 | 0 | 0 | 60000 | 0 / 0 |
| explore | 50 | mcrs | 72.83 | 6.57 | 0 | 0 | 0 | 120000 | |
| explore | 50 | vanilla | 71.10 | 6.13 | 0 | 4563 | 0 | 120000 | 0 / 0 |
| idle | 1 | mcrs | 0 | 0 | 0 | 0 | 0 | 2400 | |
| idle | 1 | vanilla | 0 | 0 | 0 | 0 | 0 | 2400 | 0 / 0 |
| idle | 10 | mcrs | 0 | 0 | 0 | 0 | 0 | 23999 | |
| idle | 10 | vanilla | 0 | 0 | 0 | 0 | 0 | 24000 | 0 / 0 |
| idle | 25 | mcrs | 0 | 0 | 0 | 0 | 0 | 60000 | |
| idle | 25 | vanilla | 0 | 0 | 0 | 0 | 0 | 60000 | 0 / 0 |
| idle | 50 | mcrs | 0 | 0 | 0 | 0 | 0 | 120000 | |
| idle | 50 | vanilla | 0 | 0 | 0 | 0 | 0 | 120000 | 0 / 1 |
| mixed | 1 | mcrs | 9.97 | 0.27 | 0 | 0 | 0 | 2400 | |
| mixed | 1 | vanilla | 9.97 | 0.28 | 0 | 0 | 0 | 2400 | 0 / 0 |
| mixed | 10 | mcrs | 64.53 | 2.26 | 0 | 0 | 0 | 24000 | |
| mixed | 10 | vanilla | 63.18 | 1.93 | 0 | 0 | 0 | 24000 | 0 / 0 |
| mixed | 25 | mcrs | 159.73 | 3.63 | 0 | 0 | 0 | 60000 | |
| mixed | 25 | vanilla | 158.21 | 3.21 | 0 | 0 | 0 | 60000 | 0 / 0 |
| mixed | 50 | mcrs | 319.45 | 5.90 | 0 | 0 | 0 | 120000 | |
| mixed | 50 | vanilla | 323.96 | 5.51 | 0 | 252 | 0 | 120000 | 0 / 0 |

Chunks a second agree within 3 % in every pair, and no server sent a chunk twice. The idle bots
held all their columns before the window, so they received none during it. In `explore` and
`mixed` from 10 bots up, first-time chunks are 7 to 17 % higher on mcrs, so mcrs generated a
little more terrain. In `walk` they are under 0.1 a second on both. The likely cause is the
different spawn point (below).

### Where the work is not equal

- **Spawn point.** mcrs puts every player at (0, 128, 0). Vanilla finds a spawn point on the
  surface, here (96.5, 145, -31.5). The bots therefore fly over different terrain. This is the
  likely cause of the vanilla pull-backs in `explore 50` (4563, about 2 bots stuck against
  terrain for the window) and `mixed 50` (252). Vanilla logs no "moved wrongly" line for them.
  mcrs pulled back no bot; this run does not show whether mcrs refuses the same moves.
- **Mobs.** Vanilla spawns no mobs here (`spawn_mobs` and `spawn_monsters` off), but the animals
  that world generation places still tick. mcrs has no mobs.
- **Saving.** Vanilla writes chunks to disk when they unload (`sync-chunk-writes=true`). mcrs
  does not save a generated world.
- **Locator bar.** Off on vanilla, absent on mcrs.
- **Compression.** Off on both, so neither server pays for zlib. mcrs once sent a compressed
  frame below the threshold of 256 during a join (FRAME-006), which the strict client of
  `mcrs_minecraft_network` rejects.
- **Tick time.** Vanilla measures itself with `/tick query`, rounded to 0.1 ms. mcrs is
  measured by its own loop over the same window size. Both exclude the wait for the next tick.
  The mcrs time excludes the socket hand-over between ticks, and the vanilla time includes the
  packet handling inside its tick.
- **Heap.** Vanilla's RSS is mostly its fixed 4 GB heap (`-Xms4g`).

### Findings

- Both servers held 20 TPS in every run.
- mcrs's mean tick was 3.3 to 8.6 times shorter than vanilla's: 2.68 against 9.72 ms at 50
  walking bots, 1.39 against 11.95 ms at 50 exploring bots, and 2.20 against 12.88 ms at 50
  mixed bots. Its worst window P99 was 8.8 ms or less in every run; vanilla's was 16.6 to 53.8 ms.
- mcrs's mean tick grows by about 0.037 ms a walking bot (0.86 to 2.68 ms from 1 to 50), and
  vanilla's by about 0.14 ms a bot (2.83 to 9.72 ms).
- mcrs used 1.1 to 2.3 GiB RSS, against 3.2 to 3.4 GiB for vanilla. mcrs's RSS grows with the
  columns it holds: 2.3 GiB at 50 exploring bots.
- mcrs used less CPU from 10 bots up: 31 to 35 % at 50 bots against 65 to 97 %. With 1 walking
  bot mcrs used more (21 % against 17 %).
- mcrs answered a status request 0.9 to 2.8 s after launch, vanilla 7.0 to 7.7 s.
