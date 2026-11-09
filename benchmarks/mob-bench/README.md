# mob-bench: 50,000 pigs

This benchmark loads one server with 50,000 pigs and measures its tick, its CPU and the motion of
the pigs. It compares mcrs, vanilla 26.4-snapshot-7 and Horizon (a Rust server of Java 26.3).
The harness is `tools/mob-bench`. Each folder here holds the reports of one session: one JSON
report a run, `summary.csv` (one row a phase) and `runs.txt` (start and end of each run, with the
load average).

## How to rerun

```sh
cargo build --release -p mcrs_mob_bench -p mcrs --bins
# Horizon, in its own checkout, with its own target folder:
(cd ~/src/github.com/horizon && CARGO_TARGET_DIR=target-mobbench \
    cargo build --release -p horizon-client --no-default-features && \
    CARGO_TARGET_DIR=target-mobbench cargo build --release -p horizon-bots)
MINECRAFT_EULA=true tools/mob-bench/run.sh
```

The full matrix (3 servers, 2 client modes, 8 phases, 50,000 pigs) takes about 2.5 hours on an
M4 Max: about 75 minutes for vanilla, 40 for Horizon and 30 for mcrs. A slow server takes longer,
because each warm-up lasts at least 120 game ticks and each sprint 100 ticks.

Horizon also needs its prepared game files (`horizon-prepare --prepare --server-jar <26.3 server
jar>`, see Horizon's README). `run.sh` fetches the vanilla jar with `tools/vanilla-server/run.sh
fetch` and checks its SHA-1.

| Variable | Default | Meaning |
|---|---|---|
| `SERVERS` | `vanilla horizon mcrs` | servers to run, one at a time |
| `CLIENTS` | `watcher none` | with one connected client, and with no client |
| `PHASES` | `idle,roam-still,roam-stir,crowd-still,crowd-stir,stimulus,cost-still,cost-stir` | phases of each run, in this order |
| `PIGS` | `50000` | pigs in each phase except idle |
| `SAMPLES` | `32` | named pigs whose position the console reads |
| `IDLE_WARMUP` | `10` | seconds before the idle window |
| `WARMUP` | `30` | seconds before the windows of the roam, crowd and cost phases |
| `STIMULUS_WARMUP` | `20` | seconds of stir and panic hits before the stimulus window |
| `MEASURE` | `60` | seconds of each measured window |
| `PROBE_INTERVAL` | `5` | seconds between console probe rounds |
| `SUMMON_RATE` | `5000` | `summon` lines a second |
| `SPRINT_TICKS` | `100` | ticks of the `tick sprint` after each window (0: none) |
| `STIMULUS_PIGS` | `2000` | pigs in each round of panic hits |
| `STIMULUS_PERIOD` | `5` | seconds between rounds of panic hits |
| `STIR_MODE` | `uuid` | `uuid`: one `damage <uuid>` line a pig; `strip`: one `execute as @e[...]` line a tick |
| `PLAYER_MODE` | `spectator` | game mode of the client's player; `creative` only for a reference run |
| `CONTENDED_CORES` | `1.0` | a window is `contended` when other processes use more cores |
| `PORT` | `25610` | listen port of every server (mcrs reads it as `MCRS_PORT`) |
| `HEAP` | `16g` | vanilla heap (`-Xms` and `-Xmx`) |
| `JAVA` | `java` | the Java 25 runtime |
| `SCALING_THREADS` | empty | thread counts for the thread-scaling runs, e.g. `1 2 4 8 16` |
| `SCALING_SERVERS` | `vanilla` | servers of the thread-scaling runs |
| `SCALING_PHASES` | `crowd-stir` | phases of the thread-scaling runs (with no client) |
| `OUT` | `benchmarks/mob-bench/<UTC date>` | report folder |
| `WORK` | `target/mob-bench` | run folders and server logs |
| `HORIZON_DIR`, `HORIZON_BIN`, `HORIZON_BOTS`, `HORIZON_RESOURCES`, `MCRS_BIN`, `VANILLA_JAR`, `BENCH` | see `run.sh` | binaries and files |

`tools/mob-bench/summary.sh <folder>` rewrites `summary.csv` from the reports.

## Method

### The run

Each run starts one server in a new empty folder, on a port that the harness first checks is
free. The harness waits for an answer to a status request, then for the console to answer
`time query gametime`. It then sends the settings, the forceload lines and the fence through the
console, waits 5 s (100 ticks, so the weather and time commands settle), joins the client, and
runs the phases in order. It stops the server with `stop` (vanilla, Horizon) or SIGINT (mcrs).
A run with a client and a run with no client are two separate server starts.

### The world and its settings

All servers get the same settings. Vanilla and Horizon read them from `server.properties`:

```
online-mode=false            level-type=minecraft\:flat   level-seed=12345
generate-structures=false    gamemode=spectator           force-gamemode=true
difficulty=easy              view-distance=10             simulation-distance=10
max-tick-time=-1             pause-when-empty-seconds=-1  spawn-protection=0
enforce-secure-profile=false network-compression-threshold=-1
sync-chunk-writes=true       entity-broadcast-range-percentage=100
```

`max-tick-time=-1` keeps the watchdog from stopping a slow tick. `pause-when-empty-seconds=-1`
keeps a server with no player online ticking. Vanilla runs with `-Xms16g -Xmx16g`. mcrs gets
`MCRS_WORLD_SEED=12345 MCRS_WORLD_PRESET=flat MCRS_COMPRESSION_THRESHOLD=-1
MCRS_DEFAULT_GAMEMODE=spectator MCRS_LAN_ANNOUNCE=off MCRS_PORT=<PORT>`.

The console then sets these game rules and states, on every server:

```
gamerule spawn_mobs false              gamerule spawn_monsters false
gamerule spawn_patrols false           gamerule spawn_wandering_traders false
gamerule advance_time false            gamerule advance_weather false
gamerule max_entity_cramming 0         gamerule mob_drops false
time set noon                          weather clear
```

`max_entity_cramming 0` turns cramming damage off, so the population stays at 50,000. In mcrs it
also turns on the documented push limit: a pig pushes only its 24 nearest neighbours
(PHY-CMP-019). Each report keeps the answer to every setting line in `settings`.

### The scene

- **The pen**: blocks x and z from -125 to 124 (250 × 250), with one ring of oak fence at y = -60
  outside it. 50,000 pigs give 0.8 pigs a block; a pig is 0.9 × 0.9 blocks.
- **The open area**: blocks x from 380 to 1019, z from -320 to 319 (640 × 640), with no fence.
  50,000 pigs give 0.12 pigs a block.
- **Forceload**: both areas and one more ring of chunks around each, at most 256 chunks a
  `forceload add` line (14 lines, 2,102 chunks). With no spawn chunks in 26.x, this keeps the pigs
  ticking in the runs with no client.
- **Pigs**: one `summon minecraft:pig <x> -60 <z> {UUID:[I;7168627,<g>,0,<i>]}` line a pig, on an
  even 224 × 224 grid over the area, at 5,000 lines a second. `<g>` is the number of the
  population, because vanilla refuses the UUID of a pig killed earlier in the run. Pig `i` thus has
  the UUID `006d6273-0000-<g>-0000-0000<i as 8 hex digits>`, and the console addresses it without an
  entity selector. A selector scans every entity: Horizon takes about 5 s for one over 50,000 pigs
  and does not tick meanwhile (measured with `data get entity @e[name=...,limit=1]`, `execute if
  entity` and `execute as @e[...]`), while a UUID lookup answers at once. 32 pigs spread over the
  grid also get the custom names `mbs0` to `mbs31`. A compound with a key skips the first spawn
  setup, so each pig is an adult of the default variant (`docs/spec/entity-tracking.md`,
  TRK-SUM-003). The UUID and the custom name do not change the idle time (TRK-SUM-007). The pigs
  of the cost phases also get `NoAI:1b`. The harness then polls `execute if entity @e[type=minecraft:pig]` until the count is
  50,000. To move the pigs from the open area to the pen it sends `kill @e[type=minecraft:pig]`,
  waits for a count of 0, and summons them again.

### The phases

Every warm-up also lasts at least 120 game ticks, counted with `time query gametime` from the
end of the summons, so the first `tick query` answer of a window holds no tick of the summons,
the kill or the sprint of the phase before.

| Phase | Pigs | Stir | Warm-up | Window |
|---|---|---|---|---|
| idle | none (pen, fence, forceload only) | no | 10 s | 60 s |
| roam-still | 50,000 in the open area | no | 30 s after the count reaches 50,000 | 60 s |
| roam-stir | the same pigs | yes | 30 s of stir | 60 s |
| crowd-still | 50,000 in the pen | no | 30 s after the count reaches 50,000 | 60 s |
| crowd-stir | the same pigs | yes | 30 s of stir | 60 s |
| stimulus | the same pigs | yes, and panic hits | 20 s of stir and panic hits | 60 s |
| cost-still | 50,000 with `NoAI:1b` in the pen | no | 30 s after the count reaches 50,000 | 60 s |
| cost-stir | the same pigs | yes | 30 s of stir | 60 s |

crowd-stir is the headline phase: 50,000 pigs that stroll and push each other, with no player
influence.

### Why the pigs need a stir

These rules come from `docs/spec/mob-goals.md`:

- The stroll goal of a pig is eligible only in the AI steps 1 to 99 after the last reset of the
  idle time (GOAL-STR-004).
- Only two events reset the idle time: a non-spectator player less than 32 blocks away, and a
  hit (GOAL-STR-004). No flag of the `summon` compound keeps the stroll eligible (TRK-SUM-007).
- A spectator does not reset the idle time and is not a target of the look goal. Thus with a
  spectator watcher, each pig strolls for about 5 s and then only turns its head
  (GOAL-WAKE-019). Experiment E-G13 shows this on vanilla.

Thus in the still phases each pig stands after about 5 s on every server. These phases measure
the honest steady state. In the crowd-still phase only the pushes in the pen move the pigs.

### The stir

The stir hits every pig with `damage <pig> 0 minecraft:drown` once each 80 game ticks
(GOAL-WAKE-020). The hits of one period spread evenly over its 80 ticks: at 50,000 pigs this is
625 hits a tick. A hit resets the idle time, so each pig stays in the stroll window. A hit of 0
`drown` has these effects:

- The health does not change, so the population stays at 50,000.
- `drown` is not in `panic_causes`, so the pig does not panic.
- `drown` has the tags `no_impact` and `no_knockback`, so the hit gives no knockback and no
  velocity.
- The server sends the damage event and the hurt sound to the viewers, and the console logs one
  `Applied ... damage` line a hit. This cost is part of the stir on each server.

The game time paces the stir, not the wall clock. The harness sends `time query gametime` when
the answer to the previous query has arrived. For each game tick since the last served tick, it
sends the lines of that tick. Thus a slow server gets the same lines a game tick as a fast
server, and each pig gets one hit each 80 game ticks on each server. After a stall the harness
serves at most the last 80 ticks.

`STIR_MODE` selects the lines of one tick. The two modes hit the same pigs at the same rate:

- `uuid` (the default): `damage <uuid> 0 minecraft:drown` for each pig whose index modulo 80 is
  the tick modulo 80. At 50,000 pigs this is 625 lines a tick.
- `strip`: one line `execute as @e[type=minecraft:pig,x=..,y=-64,z=..,dx=..,dy=8,dz=..] run damage
  @s 0 minecraft:drown`. Its box is one of 80 strips of the area, each of the full depth of the
  area (`docs/spec/argument-entity.md`, ARG-ENT-009). The strips cover the pen with 1 block of
  margin, and the open area with 32 blocks of margin, for the pigs that stroll out of it. A pig on
  the border of two strips gets two hits in a period.

The default is `uuid`, because a selector scans every entity on Horizon (see the measurements
below). The cost phases measure the cost of the stir lines alone: their pigs have `NoAI:1b`, so
they do no AI step and do not move. The tick time of cost-stir less that of cost-still is the
cost of the stir lines, the hits and their broadcast.

`stir` in each phase report gives `ticks_served`, `lines_sent`, `damage_answers` (the
`Applied ... damage` lines) and `damage_answers_per_second`. The harness counts these answers
and does not keep them in the log or in `server.log`: a run has millions of them.

### The choice of the stir mode

Measured on 2026-11-09 on a busy machine (other agents ran builds), so the numbers are only
coarse. Tick times are the mean of the server's own metric, in ms.

| Server | Pigs | Mode | cost-still | cost-stir | Stir cost a tick | Answers / lines |
|---|---:|---|---:|---:|---:|---|
| vanilla | 5,000 | uuid | 10.19 | 10.84 | 0.65 | 25,063 / 25,063 |
| vanilla | 5,000 | strip | 9.99 | 10.71 | 0.72 | 25,000 / 400 |
| mcrs | 5,000 | uuid | 1.07 | 2.05 | 0.98 | 25,000 / 25,000 |
| mcrs | 5,000 | strip | 1.06 | 1.98 | 0.92 | 25,000 / 400 |
| Horizon | 50,000 | uuid | 94.47 | 244.42 | 150 | 76,875 / 77,500 |
| Horizon | 50,000 | strip | 91.92 | 6,490.28 | 6,398 | 0 / 5 |

On Horizon, one `execute as @e[...]` line with a box takes about 6.4 s: the box does not narrow
the scan of the 50,000 pigs. Horizon also gives no answer to the `damage` of such a line. Thus
each server uses the `uuid` mode. On vanilla and mcrs the two modes cost about the same.

### The motion check

A check run on 2026-11-09 with 5,000 pigs, the spectator watcher and windows of 20 s, on the same
busy machine. Console: the share of the 16 named pigs displaced by more than 0.5 block / their
mean net distance in blocks. Watcher: the tracked pigs / the displaced share / the mean net
distance.

| Phase | vanilla console | vanilla watcher | mcrs console | mcrs watcher |
|---|---|---|---|---|
| roam-still | 0.00 / 0.0 | 988 / 0.00 / 0.0 | 0.00 / 0.0 | 990 / 0.00 / 0.0 |
| roam-stir | 0.94 / 8.9 | 957 / 0.97 / 9.4 | 1.00 / 9.6 | 950 / 0.97 / 10.6 |
| crowd-still | 0.00 / 0.0 | 2,744 / 0.00 / 0.0 | 0.00 / 0.0 | 2,752 / 0.00 / 0.0 |
| crowd-stir | 1.00 / 8.5 | 2,668 / 0.99 / 9.4 | 1.00 / 6.2 | 2,718 / 0.93 / 6.6 |
| stimulus | 1.00 / 9.9 | 2,693 / 1.00 / 9.8 | 1.00 / 10.8 | 125 / 1.00 / 9.5 |
| cost-still, cost-stir | 0.00 / 0.0 | 2,751 / 0.00 / 0.0 | 0.00 / 0.0 | 2,753 / 0.00 / 0.0 |

The still phases are still and the stirred phases move, on both servers. At 5,000 pigs the pen
holds 0.08 pigs a block, so no push moves a pig in crowd-still. The mcrs crowd rows come from a
second run (10 s windows), except the stimulus row. In the first mcrs run, which ran beside the
vanilla run, the watcher tracked only 26 to 125 of the pen pigs in crowd-still, crowd-stir and
stimulus. Two more runs did not show this again.

### Fairness

- No player influences the pigs: the client is a spectator on each server, and a run with no
  client gives the same pig behaviour.
- The stir is the same on each server: the same command, the same pigs and the same hits a game
  tick. The pacing by game time keeps this true at each tick rate.
- The cost of the stir lines is in each stirred phase on each server, and the cost phases
  measure it apart.

### The panic hits

The stimulus phase adds a round of panic hits every 5 s: 2,000 lines `damage <uuid> 0
minecraft:cactus`, for the next 2,000 pigs of a fixed order (pig index `n × 7919 mod 50,000`).
Thus each round hits pigs all over the pen, and 25 rounds hit every pig once. The 2,000 lines of
a round go out evenly over the first 4 s of its 5 s, so no single tick takes a whole round.
`cactus` is in the damage type tag `panic_causes`, so each hit pig panics. The amount 0 takes no
health, and `cactus` gives no knockback. Panic does not spread in vanilla: the neighbours of a
panicking pig move only when it pushes them. The cooldown rejects a hit less than 10 ticks after
the last full hit of the same pig (`docs/spec/entity-damage.md`, DMG-HIT-003), and the console
then gives no `Applied` answer. In the stimulus phase the missing answers are about one quarter of
the panic lines, on vanilla and on mcrs: a panic hit just after a stir hit, or a stir hit just
after a panic hit. A rejected hit still resets the idle time (DMG-HIT-002).

### The client

- **vanilla and mcrs**: the harness's own protocol client (`watcher`, player `mobbench`, of
  mcrs's protocol, 26.4-snapshot-7). It joins in spectator, flies, answers keep-alives, pings and
  teleports, acknowledges chunk batches (64 chunks a tick) and sends its position and
  `client_tick_end` every 50 ms. It tracks every pig the server shows it.
- **Horizon**: Horizon refuses the watcher (`multiplayer.disconnect.incompatible`, 26.3), so the
  client of the Horizon runs is one `horizon-bots --behaviour stand` bot (player `mobbench0`, a
  26.3 client from Horizon's repository). It gives the server the same work of one
  connected player, but the harness cannot read what it receives, so the watcher columns of these
  runs are empty.

After the join, the console sends `gamemode spectator <player>` on each server, also where the
server default already made the player a spectator. The report gives the check in
`player_mode`: the answer, the count of `execute if entity @a[gamemode=spectator]` (1), and for
the watcher the game mode of its last login or game event packet (`Spectator`).

The console teleports the client to the middle of the open area (2 blocks above the floor) for
the roam phases, and to x = -130.5, y = -58, z = 0.5, outside the west fence of the pen, for the
other phases.

### Measurements

All windows exclude the warm-up. Each probe round, every 5 s, sends one `data get entity <uuid> Pos` line
per named pig, `tick query` (vanilla and Horizon) and `time query gametime`, in this order. A round
goes out only when the `time query gametime` answer of the round before has arrived, so a slow
server never builds a backlog of probe commands; `probe_rounds_skipped` counts the rounds not sent. The harness reads answers that arrive up to 3 s after the window.

- **Server tick time (the server's own metric).** Vanilla and Horizon: each `tick query` answer
  gives the mean and the P50, P95 and P99 of the last 100 ticks. The report gives the means of the
  answers in the window and the highest P99 (`worst`). At a slow tick, 100 ticks last longer than
  5 s, so answers overlap. mcrs: one `mcrs_tick` trace line a tick; the report gives the mean and
  the nearest-rank percentiles of every tick in the window, and the longest tick. `tick query`
  leaves out the handling of packets, so these numbers are not the full cost of a tick.
- **External tick probes, which do not trust the server's own clock:**
  - `console_game_time`: the game time of the first and the last `time query gametime` answer
    against their arrival on the harness's clock. It gives the real TPS and the mean wall time a
    tick. It cannot see below 50 ms a tick, because a server that keeps up waits for the next tick.
  - `watcher_set_time`: the `set_time` packets that the watcher receives (one each 20 ticks on
    vanilla). Each pair gives the wall time a tick over those ticks; the report gives their
    percentiles. The same 50 ms floor applies, and the packet arrival adds network jitter.
  - `sprint`: after each window, `tick sprint 100`, timed from the command to the answer
    `Sprint completed ...`. In a sprint the server does not wait between ticks, so the wall time a
    tick is the full cost of a tick, below 50 ms too. The time includes the latency of one console
    answer. The report also gives the server's own number from the answer. Horizon gives no
    `Sprint completed` answer, so its sprint is not measured. The harness sends no console line
    while a sprint runs.
- **CPU.** The busy share of each logical CPU over the window, from the Mach per-CPU tick
  counters (`host_processor_info`). The server's process CPU time over the window from
  `proc_pid_rusage`, as cores used. The harness and its client process are measured the same way.
  `other_cores` is the busy cores less the server and the harness: every other process and the
  kernel. `top_other_processes` lists the heaviest other processes that this user may read.
- **Memory.** The server's resident size, once a second.
- **Contention.** The 1-, 5- and 15-minute load average at the start and the end of each window,
  and the highest 1-minute value. A window is `contended` when `other_cores` is above
  `CONTENDED_CORES`.
- **Motion.** From the named pigs: the path (the sum of the distances between the position
  samples, 5 s apart) a second, the net distance a second, the mean net distance over the window
  (`mean_net_blocks`), the share of pigs whose path is more than 0.1 block (`moved_fraction`), and
  the share of pigs whose net distance is more than 0.5 block (`displaced_fraction`, the test of
  E-G13). Samples 5 s apart miss the turns within those 5 s, so the path is a lower
  bound. From the watcher: the same for every pig tracked at both ends of the window, with the
  path summed over every move packet, and the move packets a second.

### Thread scaling

`SCALING_THREADS="1 2 4 8 16"` adds runs of `SCALING_PHASES` with no client for each count.
Vanilla gets `-XX:ActiveProcessorCount=N`: the JVM sizes its GC, JIT and common pools by it;
the server's tick stays on one thread. mcrs gets `MCRS_COMPUTE_THREADS=N`, the size of its Bevy compute pool. Horizon documents no
thread setting, so the harness refuses `--threads` for it.

### Limits

- The servers run on the same machine as the harness and the client. The client and harness CPU
  is in `harness_cores`.
- Vanilla is 26.4-snapshot-7 and Horizon follows 26.3. The scene uses only commands and rules that
  both have; both gave the same answers to every setting line.
- The recorded results below use the old method: a creative client and the phases roam, crowd
  and stimulus with no stir. In them the pigs near the client strolled and the other pigs stood,
  so they do not compare with the phases of this method.
- The watcher reads the packets of up to 30,000 moving pigs a second. Its reader hands them over
  through a bounded channel, so it drains that channel every millisecond; a slower client falls
  behind, misses keep-alives and is timed out.
- At 50,000 pigs vanilla's `tick query` mean is well below the wall time a tick from the console
  probe and the sprint: the query leaves out work outside its timed part of the tick. Compare
  servers on the external probes first.

## Results: 2026-11-09-provisional

**Provisional.** Every window is marked `contended`: the machine ran a desktop session (about 3
cores of other work: a database, a container VM, browsers), other agents compiled Rust during the
Horizon runs (up to 11.7 other cores), and the user's vanilla server ran beside it (it was idle).
The official runs need a quiet machine. The runs themselves are complete.

**Machine.** Apple M4 Max, 16 logical CPUs (12 performance, 4 efficiency), 128 GB, macOS (Darwin
25.6.0 arm64), OpenJDK 25.0.1. **Versions.** Harness at 222f84a57 (Horizon runs at ba38f660f: the
pig UUIDs had no population number then, the same work). Vanilla 26.4-snapshot-7. Horizon at
e2116fb8c2, `horizon-client` built with `--no-default-features`, `horizon-bots` as its client.
mcrs at 915316881 with no pig AI.

Times in ms. "Server" is the server's own metric (`tick query` means of the answers; mcrs: every
`mcrs_tick`). "Console" is the real TPS and the wall time a tick from the game time. "Sprint" is
the wall time a tick of `tick sprint 100`. "set_time" is the watcher's P50 / P99 wall time a tick.
Motion: share of pigs that moved / mean path in blocks a second (console: the 32 named pigs;
watcher: every tracked pig). CPU in cores: the server process / every other process.

| Run | Phase | Server mean | P50 | P95 | P99 | Console TPS | Console ms | set_time P50 / P99 | Sprint | Motion console | Motion watcher (pigs / moved / path) | Server cores | Other cores | RSS MiB |
|---|---|---:|---:|---:|---:|---:|---:|---|---:|---|---|---:|---:|---:|
| vanilla-watcher | idle | 4.11 | 3.21 | 9.07 | 12.04 | 20.0 | 49.99 | 50.03 / 50.61 | 1.54 | – | – | 0.09 | 2.89 | 7,396 |
| vanilla-watcher | roam | 275.25 | 275.01 | 292.09 | 305.23 | 2.95 | 339.48 | 329.88 / 364.14 | 326.42 | 0.22 / 0.0059 | 9,839 / 0.33 / 0.0135 | 1.02 | 3.04 | 12,284 |
| vanilla-watcher | crowd | 531.68 | 530.80 | 555.72 | 570.84 | 1.35 | 739.13 | 732.07 / 742.93 | 733.97 | 1.00 / 0.0533 | 27,201 / 1.00 / 0.086 | 1.05 | 2.82 | 12,760 |
| vanilla-watcher | stimulus | 591.07 | 589.02 | 621.44 | 681.43 | 1.19 | 841.80 | 841.06 / 857.84 | 822.99 | 1.00 / 0.081 | 27,142 / 1.00 / 0.113 | 1.04 | 3.05 | 12,740 |
| vanilla-none | idle | 4.65 | 3.53 | 9.48 | 12.38 | 20.0 | 49.99 | – | 1.64 | – | – | 0.09 | 3.10 | 6,585 |
| vanilla-none | roam | 254.61 | 253.53 | 267.97 | 290.13 | 4.00 | 249.75 | – | 249.89 | 0.16 / 0.0018 | – | 1.02 | 2.86 | 11,887 |
| vanilla-none | crowd | 483.77 | 481.03 | 507.88 | 551.57 | 2.05 | 486.63 | – | 464.95 | 1.00 / 0.0564 | – | 1.01 | 3.02 | 12,857 |
| vanilla-none | stimulus | 519.12 | 516.40 | 544.10 | 601.28 | 1.90 | 526.80 | – | 508.99 | 1.00 / 0.0976 | – | 1.01 | 3.20 | 12,964 |
| horizon-watcher | idle | 3.13 | 2.95 | 3.64 | 8.45 | 20.0 | 50.00 | – | – | – | – | 1.07 | 4.54 | 329 |
| horizon-watcher | roam | 209.63 | 199.82 | 267.80 | 370.32 | 4.92 | 203.09 | – | – | 0.09 / 0.0021 | – | 0.97 | 8.96 | 985 |
| horizon-watcher | crowd | 259.24 | 243.77 | 356.79 | 488.76 | 3.79 | 264.09 | – | – | 1.00 / 0.0814 | – | 0.94 | 11.74 | 2,587 |
| horizon-watcher | stimulus | 258.62 | 260.62 | 290.58 | 317.08 | 3.88 | 257.97 | – | – | 1.00 / 0.1183 | – | 1.01 | 2.88 | 2,588 |
| horizon-none | idle | 2.67 | 2.61 | 2.97 | 4.63 | 20.0 | 50.00 | – | – | – | – | 1.07 | 3.10 | 304 |
| horizon-none | roam | 145.37 | 145.18 | 152.26 | 164.84 | 6.88 | 145.39 | – | – | 0.00 / 0.0000 | – | 1.00 | 3.13 | 787 |
| horizon-none | crowd | 183.70 | 182.68 | 198.47 | 216.40 | 5.47 | 182.66 | – | – | 1.00 / 0.0774 | – | 1.00 | 3.03 | 1,991 |
| horizon-none | stimulus | 218.12 | 219.13 | 256.01 | 308.02 | 4.45 | 224.61 | – | – | 1.00 / 0.1145 | – | 0.99 | 4.26 | 1,991 |
| mcrs-watcher | idle | 1.25 | 0.80 | 2.73 | 4.72 | 17.66 | 56.62 | – | 0.65 | – | – | 0.03 | 2.84 | 498 |
| mcrs-none | idle | 1.12 | 0.76 | 2.54 | 4.85 | 17.59 | 56.84 | – | 0.86 | – | – | 0.03 | 3.19 | 530 |

mcrs's roam, crowd and stimulus phases were skipped: mcrs refuses `summon` with a `UUID` tag
(`Unable to summon entity`), so the pig count stayed at 0.

**Thread scaling** (vanilla, crowd, no client, `-XX:ActiveProcessorCount=N`):

| N | Server mean | P99 | Console TPS | Console ms | Sprint | Server cores | RSS MiB |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 516.68 | 638.20 | 1.93 | 518.48 | 495.24 | 1.01 | 6,204 |
| 4 | 511.61 | 589.67 | 1.95 | 513.05 | 500.29 | 1.01 | 11,643 |
| 16 | 513.52 | 574.43 | 1.95 | 512.96 | 492.91 | 1.02 | 11,928 |

**Findings.**

- Every server kept the 50,000 pigs: the count was 50,000 before and after each pig phase.
- In the crowd and stimulus phases every sampled pig moved on every server: the crowd pushes them
  with no player near. In the roam phase with no client the pigs stood still (vanilla 16 % of the
  named pigs moved, 0.002 blocks a second; Horizon none), as the 32-block stroll rule says.
- With 50,000 pigs vanilla ran at 1.2 to 4 TPS and Horizon at 3.8 to 6.9 TPS. Horizon's tick was
  1.7 to 3.3 times shorter than vanilla's on the console probe, on one core as vanilla's.
- The watcher cost vanilla 36 % to 60 % more wall time a tick in the pig phases (739 against
  487 ms in the crowd phase), mostly outside what `tick query` times (532 against 484 ms): the tracking and the sending
  of about 30,000 move packets a second.
- Horizon's own `tick query` matches the console probe (within 3 %); vanilla's misses up to 28 %
  with the watcher. Horizon gives no `tick sprint` answer.
- Horizon idles at one full core (1.07 cores with no pig), vanilla at 0.09 and mcrs at 0.03.
- mcrs's idle tick takes 1.1 ms, but the console probe shows 17.6 TPS (56.8 ms a tick): its tick
  loop falls behind the 50 ms pace with no load.
- Vanilla's thread count changes nothing for its tick (the tick is on one thread); fewer CPUs only
  shrink the heap that the JVM commits.
- Horizon takes about 5 s for one entity selector over 50,000 pigs (`@e[...]`) and ticks nothing
  meanwhile; a UUID target answers at once. The scene therefore addresses each pig by UUID.
