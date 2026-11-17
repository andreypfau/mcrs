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
