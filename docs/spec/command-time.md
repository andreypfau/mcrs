# The `time` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser ids | `brigadier:float` 1, `minecraft:resource_location` 36, `minecraft:time` 43, `minecraft:resource` 46 |
| Packet | PLAY, clientbound, `set_time`, id 115 |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, registry `minecraft:command_argument_type`. Source of the
packet id: `assets/mcrs/reports/packets.json`.

This document states the tree of the `time` command, the clocks that it reads and changes, the
packets that the server sends after a change, and the messages. It also states the argument
types `minecraft:time` and `minecraft:resource`, because no other document states them. It does
not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Node**, **literal**, **argument**, **executable**, **source**, **error**, **suggestion**,
  **remaining input**: as `command-parser.md` defines them.
- **Operator**, **system message**, **suggestion type**: as `commands-packet.md` defines them.
- **Identifier**: as `registry.md`, REG-ID-001, defines it.
- **Clock**: an entry of the registry `minecraft:world_clock`. The entry has no fields. The
  name of a clock is its identifier, for example `minecraft:overworld`.
- **Clock state**: the four values that the server keeps for one clock (CMD-TIME-006): the
  total ticks, the partial tick, the rate and the paused flag.
- **Total ticks**: the number of whole ticks of a clock, a signed 64-bit integer.
- **Partial tick**: the fraction of a tick of a clock, a binary32 number in `[0, 1)`.
- **Rate**: the clock ticks that a clock advances in one server tick, a positive binary32
  number.
- **Timeline**: an entry of the registry `minecraft:timeline`. Its fields that this document
  reads are `clock`, `period_ticks` and `time_markers`.
- **Period**: the value of `period_ticks` of a timeline, a positive integer. A timeline can have
  no period.
- **Time marker**: an entry of the field `time_markers` of a timeline. It has an identifier,
  a tick count `ticks`, and a flag `show_in_commands`. A time marker belongs to the clock of
  its timeline, and it has the period of its timeline.
- **Default clock**: the clock that the field `default_clock` of a dimension type names. A
  dimension type can have no default clock.
- **Source dimension**: the dimension of the source.
- **Selected clock**: the clock on which a branch of the command acts (CMD-TIME-008).
- **Game time**: as `block-simulation.md` defines it. All dimensions of one server read the
  same game time (SIM-TICK-003).
- **Clock subtree**: the children of `time` that CMD-TIME-001 lists under that name.
- **`t mod p`**: the remainder of the integer division of `t` by `p`, with the sign of `t`.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". None of the experiments has been run.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CMD-TIME` | the `time` command, its two argument types and the clocks |

---

## 1. The tree

### CMD-TIME-001: Tree

```
time                                                    require command_level gamemasters
├── <clock subtree>
├── query                                               (the literal of the clock subtree)
│   └── gametime                                        executable
└── of
    └── <clock: minecraft:resource>                     registry minecraft:world_clock
        └── <clock subtree>

clock subtree:
├── set
│   ├── <time: minecraft:time>                          executable; min 0
│   └── <timemarker: minecraft:resource_location>       executable; suggestion type minecraft:ask_server
├── add
│   └── <time: minecraft:time>                          executable; min -2147483648
├── pause                                               executable
├── resume                                              executable
├── rate
│   └── <rate: brigadier:float>                         executable; min 1.0E-5, max 1000
└── query
    ├── time                                            executable
    └── <timeline: minecraft:resource>                  executable; registry minecraft:timeline; suggestion type minecraft:ask_server
        └── repetition                                  executable
```

- The children of `time` are in this order: `set`, `add`, `pause`, `resume`, `rate`, `query`,
  `of`.
- The node `query` under `time` is one node. Its children are in this order: `time`,
  `<timeline>`, `gametime`. The node `query` under `<clock>` has no child `gametime`.
- The clock subtree under `<clock>` is a second copy of the nodes, not a redirect.
- Only the node `time` has a requirement. Every other node has none.
- The two arguments with the suggestion type `minecraft:ask_server` have their own suggestion
  source on the server (CMD-TIME-012, CMD-TIME-017). Each other argument has no suggestion type.

Properties on the wire (`commands-packet.md`, CMDP-ARG-001):

| Argument | Properties |
|----------|------------|
| `<time>` under `set` | Int, 4 bytes, big-endian: `0` |
| `<time>` under `add` | Int, 4 bytes, big-endian: `-2147483648` |
| `<rate>` | flags `0x03`, then the binary32 value nearest to `1.0E-5`, then `1000.0` |
| `<clock>` | Identifier: `minecraft:world_clock` |
| `<timeline>` | Identifier: `minecraft:timeline` |
| `<timemarker>` | none |

The order of the children is an output of the merge rule of `command-parser.md`, CMD-TREE-003:
the game registers the node `time` two times, and the second registration adds `gametime` to
the node `query` of the first registration. An implementation can select another order of the children if the client
accepts it, because the parse prefers a literal that matches (CMD-TIME-002).

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `time` in `commands.json` with the tree above.
The report sorts the children by name, so it does not show their order. Then decode the node
`timemarker` of a captured `commands` packet: it has flag `0x10` and `minecraft:ask_server`.

### CMD-TIME-002: Choice between siblings

- **Behavior:** the general parse rules decide every choice (`command-parser.md`, CMD-PARSE-002
  and CMD-PARSE-005). These results follow:
  - After `set`, `<time>` comes first. A text that both `<time>` and `<timemarker>` read to the
    end is a tick count. `time set 1d` sets 24000 ticks. It does not look for a time marker
    `minecraft:1d`.
  - A text that `<time>` rejects and that is a valid identifier is a time marker. The command
    then fails at run time with `commands.time.no_time_marker_found`, not with an error of
    `<time>`.
  - After `query`, the word `time` or `gametime` always selects the literal. A timeline with the
    identifier `minecraft:time` is reachable only as `minecraft:time`.
- **Edge cases:**

| Command line | Branch | Result |
|--------------|--------|--------|
| `time set 1000` | `set <time>` | total ticks 1000 |
| `time set day` | `set <timemarker>` | time marker `minecraft:day` |
| `time set 5x` | `set <timemarker>` | `commands.time.no_time_marker_found` with `minecraft:5x` |
| `time set -1` | `set <timemarker>` | `commands.time.no_time_marker_found` with `minecraft:-1` |
| `time query time` | `query time` | the total ticks of the default clock |
| `time query day` | `query <timeline>` | the timeline `minecraft:day` |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator in the overworld, send `time set 5x` and `time set -1`. Both replies are
`commands.time.no_time_marker_found`, with `minecraft:5x` and `minecraft:-1`.

---

## 2. Argument types

### CMD-TIME-003: `minecraft:time`

- **Properties:** a minimum, a signed 32-bit integer.
- **Units:**

| Unit | Ticks for one unit |
|------|--------------------|
| `d` | 24000 |
| `s` | 20 |
| `t` | 1 |
| no unit | 1 |

- **Read:**
  1. Read a float (`command-parser.md`, CMD-RD-005). Its errors are the errors of the read.
  2. Read an unquoted string (CMD-RD-006). This is the unit. The unit is case-sensitive.
  3. If the unit is not in the table, fail with `argument.time.invalid_unit`. The error
     position is the cursor after the unit.
  4. Multiply the float by the ticks for one unit, in binary32 arithmetic.
  5. Round the product to the nearest integer. A product with a fraction of exactly 0.5 rounds
     up, toward positive infinity. A product outside the signed 32-bit range becomes the
     nearest bound of that range.
  6. If the result is less than the minimum, fail with `argument.time.tick_count_too_low`,
     with the minimum and the result, in that order. The error position is the cursor after
     the unit.
  7. The value is the result, a number of ticks.
- **Suggest:** read a float from the remaining input. If the read fails, suggest nothing.
  Otherwise make a builder at the position after the float. Suggest `d`, `s` and `t` there,
  with the filter of `command-parser.md`, CMD-SUG-008, for text candidates. The unit without a
  text adds no suggestion.
- **Examples:** `0d`, `0s`, `0t`, `0`.
- **Edge cases:**

| Text at cursor | Minimum | Result |
|----------------|---------|--------|
| `1d` | 0 | 24000 |
| `1.5s` | 0 | 30 |
| `0.1d` | 0 | 2400 |
| `0.5` | 0 | 1 |
| `-0.5` | -2147483648 | 0 |
| `100000d` | 0 | 2147483647 |
| `1D` | 0 | `argument.time.invalid_unit` |
| `1e3` | 0 | `argument.time.invalid_unit`: the float is `1`, the unit is `e3` |
| `-1` | 0 | `argument.time.tick_count_too_low` with `0` and `-1` |
| `d` | 0 | `reader_expected_float` |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, send `time add 1D`. The reply has `argument.time.invalid_unit`, with the context after
`1D`. Then send `time set 100000d` and `time query time`. The total ticks are 2147483647.

### CMD-TIME-004: `minecraft:resource`

- **Properties:** the registry, an Identifier on the wire.
- **Read:**
  1. Read an identifier (`argument-resource.md`, ARG-RES-001).
  2. If the registry has no entry with that identifier, fail with
     `argument.resource.not_found`, with the identifier and the registry identifier. The error
     position is the cursor after the identifier.
  3. The value is the entry.
- **Postconditions:** the read needs the registry contents of the source. The client uses the
  registries that the server synchronized. The server uses its own registries. The registries
  `minecraft:world_clock` and `minecraft:timeline` are synchronized, so the client checks a
  clock and a timeline when it parses.
- **Suggest:** the entries of the registry, with ARG-RES-002, and no tags. The source finds the
  registry as ARG-RES-007 states. A node with its own suggestion source uses that source.
- **Text of the value:** the identifier of the entry.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as an
operator, type `/time of minecraft:nope query time` in the client. The client underlines
`minecraft:nope`. Send it: the reply has `argument.resource.not_found` with `minecraft:nope` and
`minecraft:world_clock`.

---

## 3. Clocks

### CMD-TIME-005: Scope of a clock

- **Behavior:**
  - The server keeps one clock state for each clock of the registry. The clock states are
    global to the server. A dimension has no clock state of its own.
  - A clock that the save does not contain starts with total ticks 0, partial tick 0, rate 1
    and the paused flag false.
  - The server saves each clock state with the world. The save format specification owns the
    layout.
  - Every value that the server derives from a clock, for example an environment attribute,
    reads the new clock state at once after a change. No value from before the change remains
    in use for the rest of the tick.
- **Postconditions:** a command in one dimension can change a clock that another dimension
  reads. `execute in minecraft:the_nether run time of minecraft:overworld set day` changes the
  day of the overworld.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the Nether,
as an operator, run `time of minecraft:overworld set night`. Then go to the overworld. It is
night.

### CMD-TIME-006: Advance of a clock

- **Behavior:** in each server tick that runs normally, when the game rule `advance_time` is
  true, the server advances each clock whose paused flag is false:
  1. Add the rate to the partial tick, in binary32 arithmetic.
  2. Let `n` be the largest integer that is not greater than the partial tick.
  3. Subtract `n` from the partial tick. Add `n` to the total ticks.
- **Postconditions:** a server tick that does not run normally, for example a tick under
  `tick freeze`, does not advance a clock. When `advance_time` is false, no clock advances. The
  clock states keep their values.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`tick freeze`, `time rate 0.25`, `time set 100`, `tick step 10` and `time query time`. The total
ticks are 102.

### CMD-TIME-007: The time markers of a clock

- **Inputs:** a clock `C`.
- **Behavior:** the time markers of `C` are the time markers of every timeline whose `clock` is
  `C`. The server derives this set from the loaded registry `minecraft:timeline`. It does not
  use a list of identifiers that it does not read from the data.
- **Invariants** (the registry load checks them, and fails on a violation):
  - The identifier of a time marker occurs one time only among the time markers of one clock.
    Two clocks can have time markers with the same identifier.
  - In a timeline with a period `p`, each time marker has `0 ≤ ticks < p`.
  - Without a period, each time marker has `ticks ≥ 0`.
- **Data shape:** a time marker in the JSON is a non-negative integer, which is `ticks` with
  `show_in_commands` false. It can also be an object with `ticks` and the optional
  `show_in_commands`, false by default.
- **Occurs:** a time marker with `ticks` = `m` occurs at total ticks `t` when:
  - the time marker has no period, and `t = m`;
  - the time marker has the period `p`, and `t mod p = m`.
- **Next total ticks:** for a time marker that does not occur at `t`:
  - without a period, the result is `m`. This result can be less than `t`;
  - with the period `p`, let `d = m − (t mod p)`. The result is `t + d` when `d > 0`, and
    `t + p + d` otherwise. The result is always greater than `t`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: add a data pack
with a timeline whose clock is `minecraft:overworld`, with no period, and with the time marker
`test:start` at 100. Run `time set 500`, then `time set test:start`. The total ticks are 100.

### CMD-TIME-008: The selected clock

- **Behavior:**
  - Under `of <clock>`, the selected clock is the value of `<clock>`.
  - Under `time` directly, the selected clock is the default clock of the dimension type of the
    source dimension. If that dimension type has no default clock, fail with
    `commands.time.no_default_clock`, with the identifier of the dimension type.
  - The branch resolves the selected clock before any other check of CMD-TIME-010 to
    CMD-TIME-016.
- **Edge cases:** with the vanilla data, the dimension type `minecraft:the_nether` has no
  default clock. `time set day` in the Nether fails with `commands.time.no_default_clock` and
  `minecraft:the_nether`. The server console has the overworld as its source dimension.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
in the Nether, run `time query time`. The reply is `commands.time.no_default_clock` with
`minecraft:the_nether`.

### CMD-TIME-009: The packet `set_time`

- **Layout** (kind: wire):

| Field | Type | Notes |
|-------|------|-------|
| game time | Long | the game time |
| clock count | VarInt | the number of entries that follow |
| for each entry: clock | VarInt | the index of the clock in the registry `minecraft:world_clock` as the server synchronized it |
| for each entry: total ticks | VarLong | |
| for each entry: partial tick | Float | |
| for each entry: rate | Float | 0 when the clock does not advance (see below) |

- **Rate on the wire:** the rate field is 0 when the paused flag is true, or when the game rule
  `advance_time` is false. Otherwise it is the rate of the clock.
- **After a change by this command:** every player of the server, in every dimension, receives
  `set_time` with one entry: the clock that changed, with its new clock state. The player
  receives it in the tick of the command, before the next server tick. The order of this packet
  against the feedback message is free.
- **Other occasions** (context, not owned by this document): the server sends `set_time` with
  every clock when a player joins or changes dimension, and when the value of `advance_time`
  changes. Every 20 ticks it sends `set_time` with no entries.

Provenance: `source_suggested`. `black_box_confirmed` (`assets/mcrs/reports/packets.json`) for
the packet id. Kind: wire for the layout, semantic for the occasions. Confidence: medium.
Experiment: with two players in two dimensions, one runs `time pause`. Both receive `set_time`
with one entry, and its rate is 0.

---

## 4. The branches

Each branch acts on the selected clock (CMD-TIME-008). A branch that changes a clock state
sends `set_time` (CMD-TIME-009). The feedback keys and their arguments are in CMD-TIME-018, and
the errors in CMD-TIME-019.

### CMD-TIME-010: `set <time>`

- **Inputs:** the value `v` of `<time>`, `0 ≤ v ≤ 2147483647`.
- **Behavior:**
  1. If the total ticks of the clock are `v`, fail with `commands.time.set.already_at_time`.
     The partial tick does not take part in this check.
  2. Set the total ticks to `v`, and the partial tick to 0.
  3. Send `commands.time.set.absolute`, with the broadcast flag set.
- **Result:** `v`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`time set 1000` two times. The second reply is `commands.time.set.already_at_time` with
`minecraft:overworld` and `1000`.

### CMD-TIME-011: `set <timemarker>`

`<timemarker>` is the first node of the server that uses the argument type
`minecraft:resource_location`. It reads with `argument-resource.md`, ARG-RES-003. The read
does not check that the time marker exists.

- **Behavior:**
  1. Look for the time marker with the identifier of the value among all time markers of the
     clock (CMD-TIME-007). The flag `show_in_commands` does not take part in this look-up.
  2. If there is none, fail with `commands.time.no_time_marker_found`.
  3. If the time marker occurs at the total ticks of the clock, fail with
     `commands.time.set.already_at_time_marker`.
  4. Set the total ticks to the next total ticks of the time marker, and the partial tick to 0.
  5. Send `commands.time.set.time_marker`, with the broadcast flag set.
- **Result:** the new total ticks, wrapped (CMD-TIME-017).
- **Suggestions:** the server makes them, because the node has the suggestion type
  `minecraft:ask_server` (`commands-packet.md`, CMDP-SUG). The source:
  1. Resolve the selected clock from the parsed context. If that fails, the node gives no
     suggestion (`command-parser.md`, CMD-SUG-003).
  2. Take the time markers of the clock whose `show_in_commands` is true.
  3. Suggest their identifiers with ARG-RES-002, with no prefix.
- **Edge cases:**
  - A time marker with `show_in_commands` false is not suggested, but `set` accepts it. With
    the vanilla data, `time set minecraft:wake_up_from_sleep` sets the total ticks to the next
    multiple of 24000.
  - With the vanilla data, the clock `minecraft:the_end` has no timeline. Thus
    `execute in minecraft:the_end run time set day` fails with
    `commands.time.no_time_marker_found`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a vanilla
server, send a suggestion request with `/time set `. The reply holds the time markers of
`data/minecraft/timeline/day.json` whose `show_in_commands` is true, and no other time marker.

### CMD-TIME-012: `add <time>`

- **Inputs:** the value `v` of `<time>`, a signed 32-bit integer.
- **Behavior:**
  1. Set the total ticks to the larger of `total ticks + v` and 0. The partial tick does not
     change.
  2. Send `commands.time.set.absolute` with the new total ticks, with the broadcast flag set.
- **Edge cases:** `time add 0` succeeds. It sends `set_time` and the message.
- **Result:** the new total ticks, wrapped (CMD-TIME-017).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`time set 100`, then `time add -1d`. The reply is `commands.time.set.absolute` with
`minecraft:overworld` and `0`.

### CMD-TIME-013: `pause` and `resume`

- **Behavior:**
  1. `pause` sets the paused flag to true. `resume` sets it to false.
  2. If the paused flag already has that value, fail. `pause` fails with
     `commands.time.pause.already_paused`. `resume` fails with
     `commands.time.pause.already_running`.
  3. Send `commands.time.pause` or `commands.time.resume`, with the broadcast flag set.
- **Edge cases:** the game rule `advance_time` does not take part. When it is false, `resume`
  succeeds, and the clock still does not advance (CMD-TIME-006).
- **Result:** 1.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`gamerule advance_time false`, then `time pause`. The reply is `commands.time.pause`, because
the game rule does not set the paused flag. Run `time resume`: the reply is
`commands.time.resume`, and `time query time` gives the same value two ticks later.

### CMD-TIME-014: `rate <rate>`

- **Inputs:** the value `r` of `<rate>`, a binary32 number in `[1.0E-5, 1000]`
  (`command-parser.md`, CMD-ARG-002).
- **Behavior:**
  1. If the rate of the clock equals `r` as a binary32 number, fail with
     `commands.time.rate.already_same`.
  2. Set the rate to `r`. The paused flag does not change.
  3. Send `commands.time.rate`, with the broadcast flag set.
- **Result:** 1.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`time rate 1`. The reply is `commands.time.rate.already_same` with `minecraft:overworld` and
`1.0`.

### CMD-TIME-015: `query time` and `query gametime`

- **`query time`:** send `commands.time.query.absolute` with the total ticks of the clock,
  without the broadcast flag. The result is the total ticks, wrapped (CMD-TIME-017).
- **`query gametime`:** send `commands.time.query.gametime` with the game time, without the
  broadcast flag. The result is the game time, wrapped. This branch has no selected clock, so
  it never fails with `commands.time.no_default_clock`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the Nether,
run `time query gametime`. It succeeds.

### CMD-TIME-016: `query <timeline>` and `query <timeline> repetition`

- **Behavior:**
  1. If the `clock` of the timeline is not the selected clock, fail with
     `commands.time.wrong_timeline_for_clock`.
  2. Let `t` be the total ticks of the clock.
  3. `query <timeline>`: the value is `t mod p` for a timeline with the period `p`, and `t`
     without a period. Send `commands.time.query.timeline` without the broadcast flag.
  4. `query <timeline> repetition`: the value is the quotient of `t` by `p`, rounded toward
     zero, for a timeline with the period `p`, and 0 without a period. Send
     `commands.time.query.timeline.repetitions` without the broadcast flag.
- **Result:** the value, wrapped (CMD-TIME-017).
- **Suggestions:** the server makes them, because the node has the suggestion type
  `minecraft:ask_server`. The source resolves the selected clock from the parsed context. It
  suggests the identifiers of the timelines whose `clock` is that clock, with ARG-RES-002 and
  no prefix. If the clock does not resolve, the node gives no suggestion.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`time set 50000`, then `time query day` and `time query day repetition`. The values are 2000 and
2.

### CMD-TIME-017: Wrapped result

- **Behavior:** a wrapped result of a 64-bit value `v` is `v mod 2147483647`. The divisor is
  `2^31 − 1`, not `2^31`.
- **Edge cases:** `set <time>` does not wrap its result (CMD-TIME-010). `time set 2147483647`
  gives the result 2147483647. `time add` to the total ticks 2147483647 gives the result 0.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`scoreboard objectives add r dummy`, `time set 2147483646`, and
`execute store result score @s r run time add 1`. The score is 0.

---

## 5. Messages

### CMD-TIME-018: Feedback

Each argument is a primitive value in the translation, not a text component. The type column
gives the primitive type.

| Branch | Translation key | Arguments, in order | Broadcast |
|--------|-----------------|---------------------|-----------|
| `set <time>` | `commands.time.set.absolute` | clock name (string), `v` (int) | yes |
| `add <time>` | `commands.time.set.absolute` | clock name (string), new total ticks (long) | yes |
| `set <timemarker>` | `commands.time.set.time_marker` | clock name (string), time marker identifier (string) | yes |
| `pause` | `commands.time.pause` | clock name (string) | yes |
| `resume` | `commands.time.resume` | clock name (string) | yes |
| `rate <rate>` | `commands.time.rate` | clock name (string), `r` (float) | yes |
| `query time` | `commands.time.query.absolute` | clock name (string), total ticks (long) | no |
| `query gametime` | `commands.time.query.gametime` | game time (long) | no |
| `query <timeline>` | `commands.time.query.timeline` | timeline identifier (string), value (long) | no |
| `query <timeline> repetition` | `commands.time.query.timeline.repetitions` | timeline identifier (string), value (long) | no |

- A clock name, a time marker identifier and a timeline identifier are the full text
  `namespace:path`.
- The server sends the rate as a binary32 primitive. The client makes the text, for example
  `1.0` and `1.0E-5`.
- The game also has the keys `commands.time.set` and `commands.time.query`. This command does
  not use them.
- Success messages and the broadcast follow `commands-packet.md`, CMDP-EXEC-004.

Provenance: `source_suggested` (`assets/minecraft/lang/en_us.json`, lines 3226–3244 of the
reference resources). Kind: wire for the keys and the argument order, semantic for the
broadcast. Confidence: medium. Experiment: with two operators online, one runs `time rate 2`.
The other receives `chat.type.admin` with `commands.time.rate`, and the text is
`Clock minecraft:overworld will now advance at 2.0x normal rate`.

### CMD-TIME-019: Errors

| Translation key | Arguments, in order | Rule |
|-----------------|---------------------|------|
| `argument.time.invalid_unit` | none | CMD-TIME-003 |
| `argument.time.tick_count_too_low` | minimum (int), result (int) | CMD-TIME-003 |
| `argument.resource.not_found` | identifier, registry identifier | CMD-TIME-004 |
| `commands.time.no_default_clock` | dimension type identifier | CMD-TIME-008 |
| `commands.time.set.already_at_time` | clock name, `v` (int) | CMD-TIME-010 |
| `commands.time.no_time_marker_found` | time marker identifier, clock name | CMD-TIME-011 |
| `commands.time.set.already_at_time_marker` | clock name, time marker identifier | CMD-TIME-011 |
| `commands.time.pause.already_paused` | clock name | CMD-TIME-013 |
| `commands.time.pause.already_running` | clock name | CMD-TIME-013 |
| `commands.time.rate.already_same` | clock name, `r` (float) | CMD-TIME-014 |
| `commands.time.wrong_timeline_for_clock` | timeline identifier, clock name | CMD-TIME-016 |

- The first three are errors of the parse, with a position. The others are errors of the run,
  with no position.
- In `commands.time.no_time_marker_found` and `commands.time.wrong_timeline_for_clock`, the
  clock name comes last. In the other keys with a clock name, it comes first.
- The game also has the key `argument.time.invalid_tick_count`. The argument type does not use
  it.
- A command that fails changes no clock state.

Provenance: `source_suggested`. Kind: wire for the keys and the argument order. Confidence:
medium. Experiment: as an operator in the overworld, send `time set foo:bar`. The text is
`Time marker foo:bar does not exist for clock minecraft:overworld`.

---

## Intentional deviations

### CMD-TIME-020: A failed `set <timemarker>` sends `set_time`

- **Rule:** CMD-TIME-011 and CMD-TIME-009: only a change sends `set_time`.
- **Reference:** when the time marker does not exist, or when it already occurs, the game still
  sends `set_time` with the clock state to every player. It also marks the clock states for a
  save. The clock state does not change.
- **Visible to the client:** no. The packet repeats the clock state that the client has.
- **Test:** a differential test does not compare the `set_time` packets after a command that
  fails with `commands.time.no_time_marker_found` or
  `commands.time.set.already_at_time_marker`.

### CMD-TIME-021: The repetition count is a 32-bit integer

- **Rule:** CMD-TIME-016, step 4: the value is the 64-bit quotient.
- **Reference:** the game truncates the quotient to a signed 32-bit integer before the message
  and the wrap. A quotient of `2^31` or more becomes a different value, which can be negative.
- **Visible to the client:** yes, in the message and in the result, but only after `2^31`
  periods. With a period of 24000, the total ticks must be more than `5.1 × 10^13`.
- **Test:** a differential test does not compare `query <timeline> repetition` when the
  quotient is `2^31` or more.

---

## Open questions

- **Negative total ticks.** The commands never make the total ticks negative. A save can hold
  negative total ticks, because the save format accepts any 64-bit value. The results of
  CMD-TIME-007 and CMD-TIME-016 for a negative `t` are unspecified until an experiment decides
  them. Experiment: write `total_ticks` = `-100` for `minecraft:overworld` in the save, and
  start the server. Run `time query day`, `time query day repetition` and `time set day`, and
  record the replies and the new total ticks.
