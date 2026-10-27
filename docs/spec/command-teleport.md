# The `teleport` and `tp` commands: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states the tree of `teleport` and its alias `tp`, and how each form computes the
destination. It also states the packets that a player receives after a move, and the messages.
It does not prescribe an architecture.

## 0. Conventions

- **Node**, **redirect**, **source**, **error**: as `command-parser.md` defines them.
- **Victim**: an entity that the command moves.
- **Relative set**: the set of flags that the `player_position` packet carries
  (CMD-TP-006).
- **Spawnable bounds**: the block positions with `−30 000 000 ≤ x < 30 000 000`,
  `−30 000 000 ≤ z < 30 000 000` and `−20 000 000 ≤ y < 20 000 000`.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-TP`. None of the experiments
  has been run.

---

### CMD-TP-001: Tree

```
teleport                                               require command_level gamemasters
├── <location: minecraft:vec3>                         executable
├── <destination: minecraft:entity>                    executable; one entity (0x01)
└── <targets: minecraft:entity>                        entities (0x00)
    ├── <location: minecraft:vec3>                     executable
    │   ├── <rotation: minecraft:rotation>             executable
    │   └── facing
    │       ├── entity
    │       │   └── <facingEntity: minecraft:entity>   executable; one entity (0x01)
    │       │       └── <facingAnchor: minecraft:entity_anchor>   executable
    │       └── <facingLocation: minecraft:vec3>       executable
    └── <destination: minecraft:entity>                executable; one entity (0x01)
tp                                                     require command_level gamemasters; redirect to teleport
```

The children are in the order of the tree. `tp` has no command, no source modifier and no fork.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the nodes `teleport` and `tp` in `commands.json` with the
tree above. The report sorts the children by name, so it does not show their order.

### CMD-TP-002: Choice between the first-level forms

`teleport 1 2 3` matches `<location>`. `teleport Steve` matches `<destination>`, and also
`<targets>`, which is not executable. Both parse to the end with no error, so the first,
`<destination>`, wins (`command-parser.md`, CMD-PARSE-005). In `teleport Steve 1 2 3`,
`<destination>` reads `Steve` and leaves ` 1 2 3`, but `<targets> <location>` reads to the end,
so the second wins.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: with the
players `A` and `Steve` online, `A` runs `teleport Steve`. `A` moves to `Steve`. Then `A` runs
`teleport Steve 1 2 3`. `Steve` moves to `1.5 2 3.5`, and `A` does not move.

### CMD-TP-003: Victims

- The forms without `<targets>` move the source entity. If the source has no entity, the
  command fails with `permissions.requires.entity`.
- The forms with `<targets>` move each entity that the selector finds, in that order. If it
  finds none, the command fails with `argument.entity.notfound.entity`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: from the server
console, run `tp 0 64 0`. It fails with `permissions.requires.entity`. Then run
`tp @e[tag=nobody] 0 64 0`. It fails with `argument.entity.notfound.entity`.

### CMD-TP-004: Destination of each form

| Form | Dimension | Position | Rotation | Facing |
|------|-----------|----------|----------|--------|
| `<location>` | the source dimension | `location` against the source | the victim keeps its rotation | none |
| `<destination>` | the dimension of `destination` | the position of `destination` | the yaw and pitch of `destination` | none |
| `<targets> <location>` | the source dimension | `location` against the source | the victim keeps its rotation | none |
| `<targets> <location> <rotation>` | the source dimension | `location` | `rotation` against the source | none |
| `... facing entity <facingEntity> [<facingAnchor>]` | the source dimension | `location` | the victim keeps its rotation | look at the anchor of `facingEntity`, `feet` when absent |
| `... facing <facingLocation>` | the source dimension | `location` | the victim keeps its rotation | look at `facingLocation` |
| `<targets> <destination>` | the dimension of `destination` | the position of `destination` | the yaw and pitch of `destination` | none |

`location`, `rotation` and `facingLocation` resolve against the source once, before the first
victim (`argument-coordinates.md`, section 3). `destination` and `facingEntity` must each find
exactly one entity (`argument-entity.md`, ARG-ENT-016). The command finds `facingEntity` before
the first victim, and it reads the anchor position of `facingEntity` after each move. When
`facingEntity` is also a victim, the facing target of a later victim is unspecified.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CMD-TP-005: Move one victim

- **Inputs:** a victim, a dimension, a position `(x, y, z)`, a yaw, a pitch, a relative set, an
  optional facing target.
- **Behavior:**
  1. If the block that contains the position is outside the spawnable bounds, the command
     fails with `commands.teleport.invalidPosition`. All victims have the same position, so
     the command fails before it moves any victim.
  2. (kind: wire) Each relative flag of the position and the rotation selects a relative
     value. A relative value is the destination value minus the current value of the victim.
     Each other value is absolute.
     The yaw and the pitch to apply are wrapped to `[−180, 180)`. The wrap occurs first.
     The final pitch of the victim is clamped to `[−90, 90]`. Without the pitch flag, the final
     pitch is the pitch to apply, then clamped. With the pitch flag, the final pitch is the
     current pitch plus the pitch to apply, then clamped. `player_position` carries the
     wrapped values, not the clamped values. The client clamps the pitch in the same way.
  3. The victim moves:
     - A player that sleeps wakes up first. A player that spectates through another entity gets
       its own camera back.
     - The victim leaves its vehicle.
     - The passengers of a victim that is not a player move with it. In another dimension,
       they mount the victim again after the move.
  4. If the move fails, steps 5 to 7 do not occur for this victim. A move fails when the
     victim is removed, or when the game cannot create the victim in the other dimension.
  5. If there is a facing target, the victim turns to face it (CMD-TP-011). A player also
     receives `player_look_at` after `player_position` (kind: wire, CMD-TP-012).
  6. Unless the victim is a living entity that glides with elytra, its vertical velocity
     becomes 0, and it is on the ground.
  7. A mob stops its path.
- **Postconditions for a player in the same dimension** (kind: wire, except the head yaw):
  - The player receives `player_position` with a new teleport id and the values of step 2.
  - The packet has a velocity of zero and the relative set of CMD-TP-006.
  - The head yaw of the player becomes its final yaw.
- **Postconditions for a player in another dimension:** the player changes dimension as
  `dimension-change.md`, DIM-CHG-001 to DIM-CHG-008, states. `player_position` of
  DIM-CHG-003, step 5, carries the values of step 2 and the relative set of CMD-TP-006. The
  head yaw of the player becomes its final yaw. CMD-TP-013 states the values.
- **Postconditions for an entity that is not a player, in another dimension:** a new entity
  replaces the victim (`dimension-change.md`, DIM-CHG-009). Steps 5 to 7 apply to the new
  entity (CMD-TP-008).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
at `0.5 64 0.5`, run `tp ~1 ~ ~`, and capture `player_position`. Its x is 1.0 with the flag for
x set, and its y and z are 0.0 with their flags set. Then run `tp @s ~ ~ ~ 0 270`. The pitch in
`player_position` is −90.0, and the player looks straight up. Then run `tp @s ~ ~ ~ 0 100`. The
pitch in the packet is 100.0, and the pitch of the player is 90.0 on the server and the client.

### CMD-TP-006: The relative set

- **Behavior:** the relative set is the union of:
  - for each axis of the position that is relative: the flag of that axis, when the victim is
    in the destination dimension;
  - for each axis of the position that is relative: the flag of the velocity on that axis;
  - the flags of yaw and pitch, when the form has no `<rotation>`. Otherwise the flag of yaw
    when the yaw of `rotation` is relative, and the flag of pitch when its pitch is relative.
- The flag bits on the wire:

| Bit | Flag |
|-----|------|
| `0x001` | x |
| `0x002` | y |
| `0x004` | z |
| `0x008` | yaw |
| `0x010` | pitch |
| `0x020` | velocity x |
| `0x040` | velocity y |
| `0x080` | velocity z |
| `0x100` | rotate the velocity |

- **Postconditions:** local coordinates (`^`) are relative on all three axes. The `<destination>`
  forms have an empty relative set.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

### CMD-TP-007: Feedback

- **Behavior:** after the last victim, when at least one victim moved, the command sends one
  success message with the broadcast flag set. The victim and the count are those of the
  victims that moved:

| Form | One victim moved | More than one victim moved |
|------|------------|----------------------|
| to a position | `commands.teleport.success.location.single`: display name of the victim, x, y, z | `commands.teleport.success.location.multiple`: count, x, y, z |
| to an entity | `commands.teleport.success.entity.single`: display name of the victim, display name of the destination | `commands.teleport.success.entity.multiple`: count, display name of the destination |

- **Postconditions:** x, y and z are the resolved position of CMD-TP-004. Each has six digits
  after the decimal point and a `.` as the decimal mark, for example `1.500000`. The
  result of the command is the number of victims that moved.
- **Edge cases:**
  - A victim whose move failed (CMD-TP-005, step 4) does not count. It does not count in the
    result or in the choice between the single and the multiple message.
  - When no victim moved, the result is 0. The command sends no message and gives no error.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run `tp 1 2 3`,
and read `1.500000`, `2.000000` and `3.500000` in the message.

### CMD-TP-011: Facing

- **Inputs:** a victim after its move, the source anchor, a facing target.
- **Behavior:**
  - The forms with `facing` have no `<rotation>`. Thus the relative set has the yaw flag and
    the pitch flag, and the yaw and the pitch to apply are 0. `player_position` does not change
    the rotation of the player.
  - The source anchor is the anchor of the command source. Each source starts with `feet`.
    Only `execute anchored` changes it. A command that a player types in chat has `feet`.
  - The facing target is a position:
    - `facing <facingLocation>`: the resolved `facingLocation`.
    - `facing entity <facingEntity> [<facingAnchor>]`: the position of `facingEntity`. With
      the anchor `eyes`, add the eye height of `facingEntity` to y.
  - The victim looks from its own anchor that the source anchor names. With `feet`, this is
    the position of the victim. With `eyes`, add the eye height of the victim to y.
  - Let `(dx, dy, dz)` be the facing target minus the anchor of the victim, and
    `h = √(dx² + dz²)`. In degrees:
    - the pitch of the victim is `−atan2(dy, h)`, wrapped to `[−180, 180)`;
    - the yaw of the victim is `atan2(dz, dx) − 90`, wrapped to `[−180, 180)`;
    - the head yaw of the victim is its yaw.
- **Postconditions:** the server sends this rotation to the player in no packet. The client
  computes it from `player_look_at` (CMD-TP-012).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator at
`0.5 64 0.5` with yaw 30, run `tp @s ~ ~ ~ facing 0.5 64 10.5`. Capture `player_position`: its
yaw and pitch are 0.0, with the flags `0x008` and `0x010` set. The player then has yaw 0 and
pitch 0. Then run `execute anchored eyes run tp @s ~ ~ ~ facing 0.5 64 10.5`. The pitch is
positive, so the player looks down.

### CMD-TP-012: The packet `player_look_at`

- **Behavior** (kind: wire): a player victim receives `player_look_at` after
  `player_position` of the same move. The fields are in this order:

| Field | Type | Value |
|-------|------|-------|
| from anchor | VarInt | the source anchor |
| x | double | x of the facing target |
| y | double | y of the facing target |
| z | double | z of the facing target |
| at entity | boolean | true for `facing entity`, false for `facing <facingLocation>` |
| entity id | VarInt | only when at entity is true: the network id of `facingEntity` |
| to anchor | VarInt | only when at entity is true: `facingAnchor`, `feet` when absent |

| Anchor | Id |
|--------|----|
| `feet` | 0 |
| `eyes` | 1 |

- x, y and z are the facing target of CMD-TP-011, which the server computes after the move.
- **Postconditions** (kind: client agreement): the client turns from its own anchor that the
  from anchor names. When at entity is true and the client knows the entity, the client uses
  its own copy of that entity and the to anchor. Otherwise, it uses x, y and z.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: as an operator, run
`tp @s ~ ~ ~ facing entity @e[type=cow,limit=1] eyes`. Capture `player_look_at`. Its from anchor
is 0, at entity is true, its entity id is the id of the cow, and its to anchor is 1.


### CMD-TP-013: A victim in another dimension than the destination

- **Inputs:** a victim whose dimension is not the destination dimension of CMD-TP-004.
- **Behavior:**
  - `location`, `rotation` and `facingLocation` resolve against the source, as CMD-TP-004
    states. The dimension of the victim does not change these values. `~` and `^` refer to the
    source, not to the victim.
  - The relative set has no flag of x, y or z (CMD-TP-006). Thus `player_position` carries the
    absolute position of the destination.
  - The relative set keeps the velocity flags of the relative axes. The victim keeps its
    velocity on those axes, and its velocity on the other axes becomes 0.
  - The rotation follows CMD-TP-005, step 2, with the yaw and the pitch of the victim. Without
    `<rotation>`, the victim keeps its rotation.
  - A facing target in the form `facing entity` is the position of `facingEntity` in its own
    dimension. The command does not check that `facingEntity` is in the destination
    dimension. The victim turns toward that position after the move (CMD-TP-011).
- **Examples:**

| Command, in the Overworld | Victim | Result |
|---------------------------|--------|--------|
| `execute in minecraft:the_end run tp @s 0 100 0` | the source player | the End at `0.5 100 0.5`, rotation kept, relative set `0x018` |
| `tp @a ~ ~ ~` | every player | each player in the Overworld at the source position, also a player from the Nether |
| `tp @s Steve`, `Steve` in the Nether | the source player | the Nether at the position of `Steve`, with the yaw and the pitch of `Steve`, relative set empty |

Provenance: `source_suggested`. Kind: semantic, except the relative set, which is wire.
Confidence: medium. Experiment: as an operator in the Overworld at `10.5 64 10.5`, run
`execute in minecraft:the_end run tp @s 0 100 0`. Capture `player_position`: x is 0.5, y is
100.0, z is 0.5, yaw and pitch are 0.0, and the flags are `0x018`. Then run
`execute in minecraft:the_nether run tp @s ~1 ~ ~`. The flags are `0x0f8`: no flag of x, y or z,
the three velocity flags, and yaw and pitch. The x is the scaled source x plus 1.

### CMD-TP-014: Order of the moves and the feedback

- **Behavior:**
  1. The command moves the victims one after the other, in the order of the selector.
  2. The move of a victim ends before the move of the next victim starts. A move to another
     dimension ends when the victim is in the new dimension (`dimension-change.md`,
     DIM-CHG-001).
  3. After the last move, the command sends the feedback of CMD-TP-007 and gives its result.
- **Postconditions:**
  - (kind: wire) A player that is a victim and also the output receives all packets of its own
    move before the success message.
  - (kind: semantic) Each search that occurs after the command finds each moved victim in its
    destination dimension. This includes a later run of the same chain
    (`command-execute.md`, CMD-EX-035) and a later command of the same tick.
  - (kind: semantic) The result counts the victims that moved. The count is known when the
    command returns, so `execute store` writes it at once.
  - (kind: unspecified) The order of the packets of two victims on two connections. Each
    connection has its own order. A client cannot see the order of packets on another
    connection.
  - (kind: semantic) The order of the moves decides what a later victim sees. An example is a
    victim that is also `destination` or `facingEntity` (CMD-TP-004).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with the players
`A` in the Overworld and `B` in the Nether, `A` runs `scoreboard objectives add r dummy` and
`execute store result score @s r run tp @a 0 64 0`. The score is 2, and the success message is
`commands.teleport.success.location.multiple` with 2. Then `A` runs
`execute if entity @a[distance=..2]` at once. The reply is `commands.execute.conditional.pass`
with the count 2.

### CMD-TP-015: Failures of a move to another dimension

- **Behavior:**
  - The destination dimension always exists: it is the source dimension or the dimension of an
    entity. The command has no error for an absent destination dimension. `execute in` fails
    first when its dimension does not exist (`command-execute.md`, CMD-EX-021).
  - A destination chunk that is not loaded or not generated does not stop the move. The server
    loads the chunks after the move (`dimension-change.md`, DIM-CHG-007).
  - The spawnable bounds of CMD-TP-005, step 1, apply in every dimension. The world border does
    not apply.
  - A victim that is removed before its move, for example a player that leaves, does not move.
    That move fails as CMD-TP-005, step 4, states. The other victims still move.
- **Postconditions:** a failed move counts as CMD-TP-007 states.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the
Overworld, run `execute in minecraft:the_end run tp @s 0 25000000 0`. The reply is
`commands.teleport.invalidPosition`, and the player stays in the Overworld.

---

## Intentional deviations

### CMD-TP-008: Steps after a move to another dimension

- **Rule:** CMD-TP-005, steps 5 to 7, and its postconditions for an entity that is not a
  player in another dimension.
- **Reference:** the game applies steps 5 to 7 to the removed victim, not to the new entity.
  The new entity does not turn to the facing target. The command does not clear its vertical
  velocity and does not put it on the ground.
- **Visible to the client:** yes. A cow that moves to the Nether with `facing` keeps its old
  yaw in the reference.
- **Test:** a differential test can find this difference after a move to another dimension of
  an entity that is not a player. Then the test does not compare the rotation, the vertical
  velocity or the ground flag.

### CMD-TP-009: Head yaw after a relative yaw

- **Rule:** CMD-TP-005, postconditions for a player in the same dimension.
- **Reference:** when the yaw flag is in the relative set, the game first sets the head yaw to
  the final yaw. It then adds the yaw of step 2 to that head yaw. The yaw change counts two
  times, so the head yaw is the final yaw plus the yaw of step 2. The next tick of the player
  sets the head yaw to the yaw of the player again.
- **Visible to the client:** not known. The wrong head yaw lasts until the next tick of the
  player. Whether a packet sends it to other players before that tick is not verified.
- **Test:** a differential test does not compare the head yaw of a player after a move with a
  relative yaw.

### CMD-TP-010: A failed move counts

- **Rule:** CMD-TP-007.
- **Reference:** the game counts each victim, also a victim whose move failed. The result is
  then the number of victims, and a success message names a victim that did not move.
- **Visible to the client:** yes, in the message and in the command result. A failed move
  occurs only for a removed victim, or for a victim that the game cannot create in the other
  dimension.
- **Test:** a differential test with a failed move does not compare the result or the message.
