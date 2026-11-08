# The `tick` command: `query` and `sprint`

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Parser ids | `brigadier:float` 1, `minecraft:time` 43 |

Source of these values: `assets/minecraft/version.json` and `assets/mcrs/reports/registries.json`,
registry `minecraft:command_argument_type`.

This document states the forms `tick query`, `tick sprint <time>` and `tick sprint stop`: the tick
time samples, the text of the query, the run of a sprint and its report. The forms `rate`, `step`,
`freeze` and `unfreeze` are out of scope, except in the tree. It does not prescribe an
architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Tick rate**: the target number of ticks in one second. The default is 20.
- **Target tick time**: `1000 / tick rate` milliseconds.
- **Tick work**: the work of one server tick from its start to its end. It contains the tick of
  each dimension, the console commands and the packets that the tick handles. It does not contain
  the wait for the next tick.
- **Tick time**: the wall time of the tick work of one tick, in nanoseconds.
- **Sample table**: the tick times of the last 100 ticks. A slot that no tick wrote yet holds 0.
- **Average tick time**: the sum of the sample table divided by `min(100, t)`, with `t` the number
  of ticks since the start of the server, at least 1. The division is an integer division.
- **Sprint**: a run of a fixed number of ticks with no wait between them.
- **Millisecond text**: a number of nanoseconds divided by `10^6` in f32, with one digit after the
  decimal point and `.` as the decimal mark, for example `12.3`.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-TICK`. None of the experiments
  has been run.

---

### CMD-TICK-001: Tree

```
tick                                         require command_level admins
├── query                                    executable
├── rate
│   └── <rate: brigadier:float>              executable; min 1, max 10000
├── step                                     executable
│   ├── stop                                 executable
│   └── <time: minecraft:time>               executable; min 1
├── sprint
│   ├── stop                                 executable
│   └── <time: minecraft:time>               executable; min 1
├── unfreeze                                 executable
└── freeze                                   executable
```

The permission is PERM-LEVEL-001, level `admins`. The server console has this level.
`command-time.md`, CMD-TIME-003, states `minecraft:time`.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `tick` in `commands.json` with the tree above.

### CMD-TICK-002: `tick query`

- **Behavior:** the command sends three success messages, each with the broadcast flag clear. All
  arguments are text primitives, except the sample count.
  1. The state:
     - while a sprint runs: `commands.tick.status.sprinting`;
     - else, while the game is frozen: `commands.tick.status.frozen`;
     - else, when the target tick time is less than the average tick time:
       `commands.tick.status.lagging`;
     - else: `commands.tick.status.running`.
  2. The rate:
     - while a sprint runs: `commands.tick.query.rate.sprinting`, with the tick rate and the
       millisecond text of the average tick time;
     - else: `commands.tick.query.rate.running`, with the tick rate, the millisecond text of the
       average tick time and the millisecond text of the target tick time.
  3. The percentiles: `commands.tick.query.percentiles`, with the millisecond texts of the
     entries 50, 95 and 99 of the sample table in ascending order, counted from 0, and the size
     of the table, 100, as an int primitive.
- **Postconditions:** the text of the tick rate has one digit after the decimal point, for example
  `20.0`. The result is the tick rate, rounded toward 0.
- **Edge cases:** in the first 100 ticks, the slots that no tick wrote hold 0, so the percentiles
  are low.

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: run `tick query` on an idle server. The first message is
`commands.tick.status.running`. The second has `20.0` and `50.0` as its first and third
arguments.

### CMD-TICK-003: `tick sprint <time>`

- **Behavior:**
  1. If a sprint runs, it ends with no report. The command then sends
     `commands.tick.sprint.stop.success`, with the broadcast flag set.
  2. Remember whether the game is frozen. Unfreeze the game.
  3. Set the sprint length and the remaining sprint ticks to `<time>`. Set the sprint time to 0.
  4. Send `commands.tick.status.sprinting`, with the broadcast flag set. The result is 1.
- **The run:** at the start of each tick while the remaining sprint ticks are more than 0:
  1. Decrease the remaining sprint ticks by 1.
  2. Run the tick at once, with no wait after the previous tick. The tick does no optional work
     that needs spare time before the next tick.
  3. Add the wall time from the start of this tick to the end of its work to the sprint time.
     This time includes the packets and the queued tasks that the server handles in the tick.
- **The end:** at the start of the first tick in which the remaining sprint ticks are 0, the
  server ends the sprint (CMD-TICK-004). That tick then runs at the tick rate.
- **Postconditions:**
  - A sprint of `n` ticks runs exactly `n` ticks. Each tick does the full work of a normal tick.
    Only the wait between the ticks is gone.
  - While a sprint runs, a server with no player does not pause
    (`command-forceload.md`, CMD-FL-004).

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: run
`tick sprint 200` on an idle server. Run `time query gametime` before and after the report. The
difference is 200 plus the ticks between the commands and the report.

### CMD-TICK-004: The report of a sprint

- **Behavior:** at the end of a sprint, let `c` be the number of ticks that the sprint ran, and
  `m = max(1, s) / 10^6`, with `s` the sprint time in nanoseconds, in f64.
  1. Send `commands.tick.sprint.report` from the server source, with the broadcast flag set. The
     arguments are, in order:
     - `trunc(1000 × c / m)`, the ticks in one second, as an int primitive;
     - the milliseconds per tick as a text with two digits after the decimal point: `m / c`, or
       the target tick time when `c` is 0.
  2. Set the frozen state to the state of CMD-TICK-003, step 2.
- **Postconditions:** the server source writes the report to the server log. The broadcast sends
  it to each operator (`commands-packet.md`, CMDP-EXEC-004).

Provenance: `source_suggested`. Kind: semantic; the key and the argument order are wire.
Confidence: high.

### CMD-TICK-005: `tick sprint stop`

- **Behavior:** if a sprint runs, end it now with its report (CMD-TICK-004). The report counts the
  ticks that ran. Then send `commands.tick.sprint.stop.success`, with the broadcast flag set. The
  result is 1.
- **Edge cases:** with no sprint, fail with `commands.tick.sprint.stop.fail`.

Provenance: `source_suggested`. Kind: semantic. Confidence: high.

---

## 1. Use as a probe of equal work

A benchmark can compare two servers with `tick sprint <n>`. Each server runs the same `n` ticks of
the same scene, with no wait, and reports the ticks in one second. For a fair comparison:

- The tick work of this server must contain the same work as the vanilla tick. The AI stage of
  `entity-ai-timing.md` runs beside the hot tick. Its work counts in the sprint time only when the
  hot tick waits for it. Thus the report of this server measures the wall time of the ticks, not
  the sum of the work of all threads.
- The sprint time contains the handling of packets and console commands. A script must not send
  commands during the sprint.
- The sample table of `tick query` measures a shorter span than the sprint time: it does not
  contain the packets that the server handles before the tick work. The two numbers are not equal.
