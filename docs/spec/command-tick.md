# The `tick` command

| Field | Value |
|-------|-------|
| Parser ids | `brigadier:float` 1, `minecraft:time` 43 |
| Packets | PLAY, clientbound: `ticking_state` 130, `ticking_step` 131 |

Version: see `assets/minecraft/version.json`. Source of the parser ids: `assets/mcrs/reports/registries.json`, registry
`minecraft:command_argument_type`. Source of the packet ids: `assets/mcrs/reports/packets.json`.

This document states all forms of the `tick` command: `query`, `rate`, `freeze`, `unfreeze`,
`step`, `step stop`, `sprint` and `sprint stop`. It states the tick time samples, the text of the
query, the tick length, the frozen game, a step, a sprint and its report, and the packets that
tell the clients the tick state. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Tick rate**: the target number of ticks in one second, an f32. The default is 20.
- **Target tick time**: `trunc(10^9 / tick rate)` nanoseconds, with the division in f64. It is
  50 000 000 at the default tick rate.
- **Frozen**: a state of the game that `tick freeze` sets and `tick unfreeze` clears. A new
  server starts with the game not frozen.
- **Steps that remain**: the number of ticks that the frozen game still runs normally. It is 0
  at the start of the server. **A step runs** while the steps that remain are more than 0.
- **Runs normally**: a tick runs normally when the game is not frozen, or when a step runs at the
  start of the tick (`block-simulation.md`, SIM-TICK-004).
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
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-TICK`.

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
- **Edge cases:**
  - In the first 100 ticks, the slots that no tick wrote hold 0, so the percentiles are low.
  - A step does not change the text. While the game is frozen, the state is
    `commands.tick.status.frozen`, also while a step runs. A step that remains after
    `tick unfreeze` does not change the text.

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: run `tick query` on an idle server. The first message is
`commands.tick.status.running`. The second has `20.0` and `50.0` as its first and third
arguments.

### CMD-TICK-003: `tick sprint <time>`

- **Behavior:**
  1. If a sprint runs, it ends with no report. The command then sends
     `commands.tick.sprint.stop.success`, with the broadcast flag set.
  2. Remember whether the game is frozen, as the **frozen state after the sprint**. Unfreeze the
     game.
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
  2. Set the frozen state to the frozen state after the sprint (CMD-TICK-003, step 2, and
     CMD-TICK-009).
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

### CMD-TICK-006: `tick rate <rate>`

- **Behavior:** set the tick rate to `<rate>`. Send `commands.tick.rate.success`, with the
  broadcast flag set. The argument is the text of `<rate>` with one digit after the decimal
  point, for example `12.3` for `12.345`. The result is `<rate>`, rounded toward 0.
- **Postconditions:**
  - The target tick time changes at once. The next wait between two ticks uses it
    (CMD-TICK-007).
  - Each player receives `minecraft:ticking_state` (CMD-TICK-013).
  - Let `a = max(100, trunc(300 × tick rate))` ticks. When `a` is less than the ticks until the
    next autosave, the next autosave comes after `a` ticks. The end of a sprint applies the
    same rule.
- **Edge cases:** the command does not fail while the game is frozen or while a sprint runs. A
  sprint ignores the tick rate. The new tick rate applies after the end of the sprint.

Provenance: `source_suggested`. Kind: semantic; the key and its argument are wire. Confidence:
high. Experiment: run `tick rate 12.345`, then `tick query`. The reply is "Set the target tick
rate to 12.3 per second", and the query shows `12.3`. Run `tick freeze`, then `tick rate 30`.
The command succeeds, and the query shows "The game is frozen" with `30.0`. Run
`execute store result score #t r run tick rate 33.9`. The score is 33.

### CMD-TICK-007: The tick schedule

- **Behavior:** outside a sprint, the planned start of a tick is one target tick time after the
  planned start of the previous tick. The server waits until the planned start. A tick that
  starts late starts at once, with no wait, until the server is on time again.
- **Edge cases:** when the server is more than `10^9 + 20 × L` nanoseconds late, with `L` the
  target tick time, it drops the delay in whole ticks of `L`. It then does not make up these
  ticks. It drops a delay at most one time in `10^10 + 100 × L` nanoseconds.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`tick rate 40` with no player online. Run `time query gametime` two times, 10 s apart. The
difference is 40 for each second between the two replies, about 417 for 10.4 s.

### CMD-TICK-008: `tick freeze`

- **Behavior:**
  1. If a sprint runs, end it with its report (CMD-TICK-004).
  2. If a step runs, set the steps that remain to 0. This sends no message.
  3. Freeze the game.
  4. Send `commands.tick.status.frozen`, with the broadcast flag set. The result is 1.
- **Postconditions:** the next tick does not run normally (CMD-TICK-012).
- **Edge cases:**
  - When the game is already frozen, the command succeeds with the same message.
  - A debug world starts frozen.

Provenance: `source_suggested`. Kind: semantic; the key and the result are wire. Confidence:
high. Experiment: run `tick sprint 1000000`, then `tick freeze`. The replies are the sprint
report, then "The game is frozen". Run `execute store result score #f r run tick freeze`. The
score is 1.

### CMD-TICK-009: `tick unfreeze`

- **Behavior:**
  1. Unfreeze the game.
  2. If a sprint runs, set its frozen state after the sprint to not frozen.
  3. Send `commands.tick.status.running`, with the broadcast flag set. The result is 0.
- **Postconditions:** the next tick runs normally. A sprint continues. The steps that remain do
  not change, and they continue to decrease by 1 in each tick. While a step runs,
  `tick step stop` succeeds (CMD-TICK-011).
- **Edge cases:** when the game is not frozen, the command succeeds with the same message.

Provenance: `source_suggested`. Kind: semantic, with item 2 a deviation (CMD-TICK-DEV-001); the
key and the result are wire. Confidence: high. Experiment: run `tick freeze`, `tick step 100`,
`tick unfreeze`, then `tick step stop`. The last reply is "Interrupted the current tick step".
Run `execute store result score #u r run tick unfreeze`. The score is 0.

### CMD-TICK-010: `tick step [<time>]`

- **Behavior:** `<time>` is a number of ticks (`command-time.md`, CMD-TIME-003). The form
  without `<time>` uses 1.
  1. If the game is not frozen, fail with `commands.tick.step.fail`.
  2. Set the steps that remain to `<time>`. This replaces the steps that remain. It does not add
     to them.
  3. Send `commands.tick.step.success`, with the broadcast flag set. The argument is `<time>` as
     an int primitive. The result is 1.
- **The run:** at the start of each tick while a step runs, the steps that remain decrease by 1,
  and the tick runs normally. A step of `n` ticks thus runs exactly `n` ticks that run normally,
  at the tick rate. The game stays frozen after them.
- **Edge cases:** while a sprint runs, the game is not frozen, and the command fails.

Provenance: `source_suggested`. Kind: semantic; the keys and the argument are wire. Confidence:
high. Experiment: with the game not frozen, run `tick step`. The reply is "Unable to step the
game - the game must be frozen first". Force-load the column at 0 0, and summon a pig at y 200.
Run `tick freeze`. The game time and the position of the pig do not change across several
commands. Run `tick step 2`. The game time increases by 2, and the pig falls. Then
`tick step stop` fails.

### CMD-TICK-011: `tick step stop`

- **Behavior:** if a step runs, set the steps that remain to 0. Send
  `commands.tick.step.stop.success`, with the broadcast flag set. The result is 1.
- **Edge cases:** when no step runs, fail with `commands.tick.step.stop.fail`. A step that ran
  all its ticks does not run.

Provenance: `source_suggested`. Kind: semantic; the keys are wire. Confidence: high. Experiment:
on a new server, run `tick step stop`. The reply is "No tick step in progress".

### CMD-TICK-012: The frozen game

- **Behavior:** while a tick does not run normally, these items continue to tick:
  - each player;
  - each entity that a player rides, directly or through other passengers;
  - each passenger of such an entity, also an entity that is not a player.

  No other entity ticks. Such an entity does not move or age by itself, and it does not despawn.
  The server continues to track it, and a client in range continues to see it.
- **Postconditions:**
  - A player does not push other entities.
  - The speed check of a move does not apply (`movement-checks.md`, MVC-VAN-009).
  - Commands continue to run. Their block changes have their full neighbor and shape updates.
  - `block-simulation.md`, SIM-TICK-004, lists the other work that stops and continues. Game
    time, the world clocks, the weather, the world border, scheduled and random ticks, spawning,
    block events, block entities and the `#minecraft:tick` functions stop.
- **Edge cases:** the client freezes the same entities (`client-entities.md`, CENTS-TIME-003).

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: see CMD-TICK-010.
A pig that falls keeps its position while the game is frozen.

### CMD-TICK-013: The tick state packets

- **Behavior:**
  - `minecraft:ticking_state` carries the tick rate and the frozen flag. The server sends it to
    each player of the server, in all dimensions, when one of these occurs:
    - `tick rate` sets the tick rate;
    - `tick freeze` or `tick unfreeze` sets the frozen flag, also to the same value;
    - a sprint starts or ends.
  - `minecraft:ticking_step` carries the steps that remain. The server sends it to each player
    of the server when `tick step` sets them. It also sends it when `tick step stop` or
    `tick freeze` sets them to 0 while a step runs.
  - A player receives both packets when it joins and when it changes dimension
    (`dimension-change.md`, rows 13 and 14).
- **Postconditions:**
  - A packet carries the state after the command. When one command sends more than one packet of
    a type, the last packet carries the final state.
  - The server does not send a packet for each tick of a step. The client counts the steps that
    remain itself.
  - A sprint is not visible in the packets. During a sprint, the frozen flag is false, and the
    tick rate is the tick rate.

Provenance: `source_suggested`. Kind: wire for the packets and their content; the number of
packets for one command is unspecified. Confidence: high. Experiment: record a vanilla client
with `tools/mcrs_minecraft_client_recorder`. Run `tick freeze`, `tick step 5`, `tick rate 40`
and `tick unfreeze`. The client receives `ticking_state` with `20.0` and true, `ticking_step`
with 5, `ticking_state` with `40.0` and true, then `ticking_state` with `40.0` and false.

---

## Intentional deviations

### CMD-TICK-DEV-001: `tick unfreeze` during a sprint

- **Rule:** CMD-TICK-009, item 2.
- **Reference:** the sprint keeps the frozen state from its start. When the game was frozen at
  the start of the sprint, the game is frozen again at the end of the sprint. This occurs also
  after `tick unfreeze` reported "The game is running normally".
- **Reason:** the last command of a player decides the frozen state.
- **Visible to the client:** yes. After the sprint report, the game is not frozen, and
  `tick query` shows `commands.tick.status.running`. The last `ticking_state` has the frozen
  flag false.
- **Bug reference:** none.
- **Test:** a differential test does not compare the frozen state after a sprint in which
  `tick unfreeze` ran.

## Unresolved

None.
