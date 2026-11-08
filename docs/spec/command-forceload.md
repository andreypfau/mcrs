# The `forceload` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Parser ids | `minecraft:column_pos` 9 |

Source of these values: `assets/minecraft/version.json` and `assets/mcrs/reports/registries.json`,
registry `minecraft:command_argument_type`.

This document states the forms `add`, `remove` and `query` of `forceload`, the effect of a force
load on a column, and the messages. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Column**: the chunk column with the chunk coordinates `(cx, cz)`. The column of a block
  coordinate `x` is `floor(x / 16)`.
- **Forced column**: a column of a dimension that holds the force load mark.
- **Chunk text**: the text `[cx, cz]` of a column, for example `[0, -1]`.
- **Range**: the columns from the column of `from` to the column of `to`, on both axes, both ends
  included.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-FL`. None of the experiments has
  been run.

---

### CMD-FL-001: Tree

```
forceload                                        require command_level gamemasters
├── add
│   └── <from: minecraft:column_pos>             executable
│       └── <to: minecraft:column_pos>           executable
├── remove
│   ├── <from: minecraft:column_pos>             executable
│   │   └── <to: minecraft:column_pos>           executable
│   └── all                                      executable
└── query                                        executable
    └── <pos: minecraft:column_pos>              executable
```

The permission is PERM-LEVEL-001, level `gamemasters`.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `forceload` in `commands.json` with the tree above.

### CMD-FL-002: `minecraft:column_pos`

- **Properties:** none.
- **Read:** two block coordinates x and z, with one space between them (`argument-coordinates.md`,
  ARG-COORD-010). Input with only one coordinate fails with `argument.pos2d.incomplete`.
- **Value:** the block position of the coordinates against the source. A relative coordinate uses
  the source position. The command reads only x and z.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CMD-FL-003: `add` and `remove` of a range

- **Behavior:**
  1. Let the block range be the box from the smaller to the larger x and z of `from` and `to`.
     Without `to`, `to` is `from`.
  2. If the smallest x or z is less than `−30 000 000`, or the largest is `30 000 000` or more,
     fail with `argument.pos.outofworld`.
  3. Let `c` be the number of columns of the range. If `c` is more than 256, fail with
     `commands.forceload.toobig`, with 256 and `c`. The command changes no column.
  4. For each column of the range, in order of `cx`, then `cz`: `add` sets the mark, and
     `remove` clears the mark. A column changes when its mark changes.
  5. `add` loads each column that changed at once, before the command ends.
- **Postconditions:** the command works in the dimension of the source. Each change applies
  before the next command of the tick.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: run
`forceload add 0 0 255 255`. The range has 256 columns, and the command succeeds. Run
`forceload add 0 0 256 255`. The range has 272 columns. The reply is `commands.forceload.toobig`
with 256 and 272.

### CMD-FL-004: The effect of a forced column

- **Behavior:** while a column is a forced column, these are true:
  - The column stays loaded.
  - The column ticks its entities. Each mob of the column does its AI step (PHY-ORD-011). The
    other ticks of the column are out of scope.
  - The dimension stays active. A dimension with no player stops the tick of its entities 300
    ticks after it became empty, unless a ticket keeps it active. A forced column is such a
    ticket.
  - The save of the dimension keeps the mark, so the column stays forced after a restart.
- **Postconditions:** a column next to a forced column is loaded, but it does not tick entities. A
  mob that walks into such a column stops: it does no AI step and no move until its column ticks
  again (TRK-LIFE-005). A mob in the last forced column next to such a column still ticks.
- **Edge cases:** a forced column does not stop the pause of a server with no player. The server
  property `pause-when-empty-seconds` controls that pause. In the pause, the server runs console
  commands, but it ticks no dimension. A benchmark with no player must set the property to `-1`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a server with
`pause-when-empty-seconds=-1` and no player, run `forceload add 0 0` and `summon pig 8 -60 8`. Wait
600 ticks. Run `data get entity @n[type=pig] Pos` two times, 100 ticks apart. The positions differ
when the pig walked.

### CMD-FL-005: Feedback of `add` and `remove`

| Columns that changed | `add` | `remove` |
|----------------------|-------|----------|
| none | error `commands.forceload.added.failure` | error `commands.forceload.removed.failure` |
| one | `commands.forceload.added.single`: the chunk text, the dimension identifier | `commands.forceload.removed.single`: the same arguments |
| more than one | `commands.forceload.added.multiple`: the count, the dimension identifier, the chunk text of the smallest corner of the range, the chunk text of the largest corner | `commands.forceload.removed.multiple`: the same arguments |

- The count is an int primitive. A chunk text and a dimension identifier are literal texts.
- The single message names the column that changed. The multiple message names the corners of the
  whole range, also when only some columns changed.
- The success message has the broadcast flag set. The result is the number of columns that
  changed.
- `remove all` clears each mark of the dimension. It sends `commands.forceload.removed.all` with
  the dimension identifier, with the broadcast flag set. Its result is 0.

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: run `forceload add 0 0 31 31` two times. The first reply is
`commands.forceload.added.multiple` with 4, `minecraft:overworld`, `[0, 0]` and `[1, 1]`. The
second reply is `commands.forceload.added.failure`.

### CMD-FL-006: `query`

- **Behavior of `query <pos>`:** let `p` be the column of `pos`. If `p` is a forced column of the
  source dimension, send `commands.forceload.query.success` with the chunk text and the dimension
  identifier, with the broadcast flag clear. The result is 1. Else fail with
  `commands.forceload.query.failure`, with the same arguments.
- **Behavior of `query`:** let `k` be the number of forced columns of the source dimension. The
  list is the chunk texts of these columns, joined with `, `. The columns are in ascending order of
  the signed 64-bit key `(cz << 32) | (cx & 0xFFFFFFFF)`.
  - `k = 0`: send the failure message `commands.forceload.added.none` with the dimension
    identifier. This is a message, not an error. The result is 0.
  - `k = 1`: send `commands.forceload.list.single` with the dimension identifier and the list.
  - `k > 1`: send `commands.forceload.list.multiple` with `k`, the dimension identifier and the
    list.

  The success messages have the broadcast flag clear. The result is `k`.

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: run `forceload add -16 0 15 0`, then `forceload query`. The reply is
`commands.forceload.list.multiple` with 2, `minecraft:overworld` and `[0, 0], [-1, 0]`.

---

## 1. Use in a benchmark

- One command forces at most 256 columns, a square of 16 × 16 columns or 256 × 256 blocks. A
  larger pen needs more than one command.
- Force the area before the summons (TRK-SUM-006). Then each summoned pig ticks from its first
  tick.
- Force one more ring of columns around the pen. Then no pig is in a column that does not tick
  entities, also when a push moves it to the fence.
