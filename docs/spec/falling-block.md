# Falling block: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the falling block: a block that loses the support below it, becomes an
entity, falls, and becomes a block again or an item. It states which blocks fall and when, the
start of a fall, the tick of the entity, the landing, the time limit and the packets.

`entity-physics.md` states the motion of the category
"falling block" (PHY-ORD-005): gravity, the move, the block effects, the ground rule and the drag.
`entity-tracking.md` states the spawn bundle, the updates and the removal of an entity.
`block-simulation.md` states the sets, the shape updates and the scheduled ticks. This document
refers to the rules of these documents by id.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **viewer**, **spawn bundle**: as `entity-tracking.md`, section 0.1,
  defines them. A viewer of a falling block is a player that tracks it.
- **Set**, **shape update**, **placement side effect**, **scheduled tick**: as `block-simulation.md`
  defines them.
- **Falling block**: an entity of the type `minecraft:falling_block`.
- **Carried state**: the block state that a falling block holds.
- **Gravity block**: a block of FBK-TRG-001. A gravity block falls when the block below it is free.
- **Free**: a property of a block state (FBK-TRG-004).
- **Concrete powder**: a block that is a key of the concrete pairs (FBK-TRG-002). The **concrete**
  of a concrete powder is the value of its pair.
- **Cell**: the block position that contains the position of the falling block. Each coordinate
  of the cell is the floor of the coordinate of the position.
- **Age**: the number of ticks of a falling block (FBK-DAT-001).
- **Drop flag**: a value of a falling block. When it is true, a falling block that does not land
  becomes an item.
- **Break flag**: a value of a falling block. When it is true, the falling block breaks at its
  landing and makes no block (FBK-LND-005).
- **Maximum age**: the value of the setting `mcrs:falling_block_max_age` (FBK-TCK-004).
- **Landing**: the step of FBK-LND-001.
- **Target cell**: the position at which a landing tries to place the carried state (FBK-LND-001).
- **Replaceable at a landing**: a property of a block state (FBK-LND-003).
- **Drop of a falling block**: the item entity of FBK-LND-006.

### 0.2 Provenance

- `black_box_confirmed`: shown by a game-produced artefact that is cited by path.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with an artefact that is cited by path.
- `source_suggested`: taken from the reference implementation, with no artefact of this version.
  The confidence is at most medium.
- `documentation`: taken from public documentation or from a public bug report.
- `design`: a choice of this project or of the owner. The rule states the reason.
- `unresolved`: not decided. Section 9.3 collects these items.

Each rule states its kind, as `docs/spec/README.md` defines it.

### 0.3 Artefacts cited

- `assets/mcrs/reports/registries.json`: the registry report. It gives the wire id 52 of the entity
  type `minecraft:falling_block`.
- `assets/mcrs/reports/packets.json`: the packet id report. Play, clientbound: `add_entity` 1,
  `block_update` 8, `add_transient_block` 37, `level_event` 47, `remove_entities` 78.
- `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json`: the type artefact. It gives
  the dimensions, the tracking range, the update interval and the entity data layout of
  `minecraft:falling_block`.
- `crates/mcrs_minecraft_collision_table/tests/fixtures/physics/e3_drop_stone.json` and
  `e3_drop_slime_block.json`: captures of a falling block of `sand` with the drop flag false.
- The data pack of mcrs, `assets/mcrs/datapacks/mcrs/`: the tags of FBK-TRG-002 and the file
  `mcrs/falling_block/concrete_pairs.json`.
- The data pack of the game: the block tags `minecraft:concrete_powders`, `minecraft:anvil`,
  `minecraft:fire` and `minecraft:water` (a fluid tag), and the game rule `minecraft:entity_drops`.
- The block definitions under `assets/mcrs/block_definition/`: the component
  `minecraft:replaceable`.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `FBK-DAT` | the values of a falling block, its type data and the `summon` keys |
| `FBK-TRG` | the gravity blocks and the trigger of a fall |
| `FBK-SPN` | the start of a fall |
| `FBK-TCK` | the tick of a falling block and its time limits |
| `FBK-LND` | the landing |
| `FBK-NET` | the packets |

## 1. The values of a falling block

**FBK-DAT-001** (`source_suggested`, semantic, confidence medium). A falling block has these values
in addition to the values of each entity.

| Value | Range | Default | Meaning |
|-------|-------|---------|---------|
| carried state | a block state | the default state of `sand` | the block that falls |
| age | a 32-bit signed integer, 0 to 2147483647 | 0 | FBK-TCK-001, FBK-TCK-004, FBK-TCK-005 |
| drop flag | true or false | true | FBK-TCK-004, FBK-LND-006 |
| break flag | true or false | false | FBK-LND-005 |
| start position | a block position | `(0, 0, 0)` | the entity data entry of FBK-DAT-003 |

In normal play, the age of a falling block does not come near 2147483647: FBK-TCK-004 ends it
after 600 ticks by default. The age after a tick that starts at 2147483647 is unspecified. A
negative `Time` of `summon` is also unspecified.

**FBK-DAT-002** (`black_box_confirmed`, wire, confidence high). The type data (TRK-TYPE-001) of
`minecraft:falling_block` has these values. The registry report and the type artefact give them.

| Wire id | Category | Width × height | Eye height | Range | Interval | Summonable |
|---------|----------|----------------|------------|-------|----------|------------|
| 52 | falling block (PHY-ORD-005) | `0.98F × 0.98F`, fixed | `0.83300006F` | 10 | 20 | true |

A falling block is not attackable: an attack of a player on it has no effect. A falling block is a
blocker of a placement (`block-placing.md`, PLC-ENT-001).

**FBK-DAT-003** (`source_suggested_black_box_confirmed`, wire, confidence high). The entity data of a
falling block has the entries 0 to 7 of the part "entity" (`entity-tracking.md`, TRK-DATA-002), and
one more entry:

| Index | Meaning | Serializer | Default |
|-------|---------|------------|---------|
| 8 | start position | 10, block position | `(0, 0, 0)` |

The type artefact confirms the index and the serializer. The vanilla client sets the start position
from the position of `add_entity` itself. Thus the entry has no visible effect for the vanilla
client.

**FBK-DAT-004** (`source_suggested`, semantic, confidence medium). The command
`summon minecraft:falling_block <pos> [<nbt>]` makes a falling block at `<pos>` with the velocity 0
and the defaults of FBK-DAT-001. The start position keeps its default `(0, 0, 0)`. The compound of
`<nbt>` can hold these keys, in addition to the keys that `entity-tracking.md` allows for each
entity:

| Key | Tag | Value |
|-----|-----|-------|
| `BlockState` | compound with the string `Name` and the optional compound `Properties` | the carried state |
| `Time` | int | the age |
| `DropItem` | byte | the drop flag |
| `CancelDrop` | byte | the break flag |

A summon with no `BlockState` key makes a falling block of `sand`. A summon with the carried state
`air` makes a falling block that its first tick removes (FBK-TCK-001). TRK-TYPE-005 of
`entity-tracking.md` has the type row that `summon` of this type uses.

## 2. Gravity blocks and the trigger

**FBK-TRG-001** (`source_suggested`, semantic, confidence medium). These blocks are gravity blocks
in this issue. Each has the delay 2.

| Blocks | Break flag at the start | Event at the landing |
|--------|-------------------------|----------------------|
| `sand`, `red_sand`, `gravel` | false | none |
| the blocks of the tag `minecraft:concrete_powders` | false | the change to concrete (FBK-TRG-002, FBK-LND-004) |
| the blocks of the tag `minecraft:anvil` | false | the sounds of FBK-LND-007 |
| `suspicious_sand`, `suspicious_gravel` | true | the break of FBK-LND-005 |


**FBK-TRG-002** (`design`, semantic). The set of gravity blocks and the blocks with the break flag
come from two block tags of the data pack of mcrs: `mcrs:gravity_blocks` and
`mcrs:breaks_after_fall`. Their default members are the rows of FBK-TRG-001. Reason: a server can
give gravity to another block, for example `snow` for the Bedrock world rule of issue #669, without
a change of code.

The concrete pairs come from the file `falling_block/concrete_pairs.json` in the namespace `mcrs`
of each data pack. In the data pack of mcrs, the file is
`assets/mcrs/datapacks/mcrs/mcrs/falling_block/concrete_pairs.json`. The file is a JSON object:

| Key | Type | Required | Meaning |
|-----|------|----------|---------|
| `replace` | boolean | no, default false | true discards the pairs of the packs that load before this pack |
| `values` | object | yes | each key is the identifier of a powder block; its value is the identifier of the concrete block |

The server reads the file of each pack in the load order of the packs, as it reads a tag. A pack
with `replace` false adds its pairs to the pairs before it. When a key is already present, the value
of the later pack replaces the earlier value. Thus a pack can add a pair without a change of code.

The load validates each pair of each file of each pack. This includes a pair whose value a later
pack replaces, and a pair that a later file with `replace` true discards. Reason: the result of the
validation does not depend on the order of the packs. When a key or a value is not a block of the
block registry, the load of the data pack fails. The error names the pack, the file and the
identifier. The server does not use a part of the file. A key that is equal to its value also makes
the load fail.

The default pairs are 16. For each colour `c` of `white`, `orange`, `magenta`, `light_blue`,
`yellow`, `lime`, `pink`, `gray`, `light_gray`, `cyan`, `purple`, `blue`, `brown`, `green`, `red`
and `black`, the pair is `minecraft:<c>_concrete_powder` → `minecraft:<c>_concrete`. For example:

```json
{
  "values": {
    "minecraft:white_concrete_powder": "minecraft:white_concrete",
    "minecraft:orange_concrete_powder": "minecraft:orange_concrete"
  }
}
```

A block of the pairs that is not a gravity block does not fall. The pairs only decide the water
contact (FBK-TCK-003) and the change to concrete (FBK-LND-004).

**FBK-TRG-003** (`source_suggested`, semantic, confidence medium). Two events schedule a block tick
of a gravity block at its position, with the delay 2 (SIM-SCH-003):

- the placement side effect of a set of the gravity block (SIM-SET-002). This includes a placement
  by a player, the `setblock` command and a piston move;
- each shape update that the gravity block receives (SIM-SHP-001), from each direction.

A second schedule while a tick is pending has no effect (SIM-SCH-004). A set through the generation
path makes no tick (SIM-NBR-005). Thus a gravity block of generated terrain with a free block below
it stays in place until an event of this list occurs.

**FBK-TRG-004** (`source_suggested`, semantic, confidence medium). When the scheduled tick runs, the
server reads the state below the gravity block. When that state is free, the gravity block starts to
fall (FBK-SPN-001). When it is not free, nothing occurs. A state is free when one of these
conditions is true:

- the definition of its block has the component `minecraft:replaceable`. For `snow`, this is true
  for each number of layers;
- its block is in the tag `minecraft:fire`;
- its block is `water` or `lava`. A block that holds water as a property, for example a waterlogged
  slab, is not free.

The state below the bottom of the world is `void_air`, which is free. Thus a gravity block at the
minimum `y` of a dimension falls out of the world (FBK-TCK-005).

## 3. The start of a fall

**FBK-SPN-001** (`source_suggested`, semantic, confidence medium). A gravity block at the position
`(x, y, z)` with the state `S` starts to fall in these steps:

1. Set the position to the block of the fluid of `S`, with the flags 3 (SIM-SET-001). The block is
   `water` for a state that holds water, and `air` for each other state. The set makes neighbour
   updates and shape updates.
2. Add a falling block at `(x + 0.5, y, z + 0.5)` with the velocity 0 and the age 0. Its carried
   state is `S`, with the property `waterlogged` false when `S` has it. Its start position is
   `(x, y, z)`. Its drop flag is true. Its break flag is the value of FBK-TRG-001.

The shape updates of step 1 schedule the tick of a gravity block above (FBK-TRG-003). Thus a column
of gravity blocks falls one block after the other, 2 ticks apart.

The new falling block takes its first tick (FBK-TCK-001) in the same server tick, after the block
ticks (PHY-ORD-010). A viewer observes this:

- the `add_entity` of FBK-SPN-002 carries the position `(x + 0.5, y, z + 0.5)` and the velocity 0,
  that is, the values before the first tick;
- the updates of TRK-UPD-001 carry the first move. `entity-tracking.md` states their tick;
- `data get entity` at the end of that server tick gives the age 1 and the position after one move.

The rule is the order of the list: step 1, step 2, then the first tick. An implementation can run
the first tick before step 1 writes the position, in the same server tick, when the observations of
this list stay the same. Before step 1, the gravity block is still at the cell of the falling block.
For the blocks of FBK-TRG-001, the first move goes down, the top of that block is above the bottom
of the box so the collision does not stop a move down, and the block has no effect on an entity in it. Thus no observer of this document sees the
difference.

**FBK-SPN-002** (`source_suggested`, wire, confidence medium). A viewer receives the spawn bundle of
the falling block (TRK-SPN-001) before the `block_update` that empties the position. The
`add_entity` packet has the type 52, the position of FBK-SPN-001, the velocity 0 and the data field
equal to the network id of the carried state (TRK-SPN-002). `set_entity_data` carries the entry 8
when the start position is not `(0, 0, 0)`. In the other order, the client shows an empty position for a short time.


## 4. The tick

**FBK-TCK-001** (`source_suggested`, semantic, confidence medium). In each tick, the server does
these steps for a falling block:

1. When the carried state is `air`, remove the falling block, and stop. Nothing drops.
2. Add 1 to the age.
3. Apply the gravity, the move and the block effects of the row "falling block" of PHY-ORD-005.
4. Find the target cell and the water contact of a concrete powder (FBK-TCK-003).
5. When the on-ground flag is true after the move, or the water contact is true, do the landing
   (FBK-LND-001).
6. Otherwise, apply the time limits (FBK-TCK-004, FBK-TCK-005).
7. Apply the drag of PHY-ORD-005.

A falling block has none of the other steps of the tick of an entity. It does not burn, it has no
air supply, a fluid current does not push it, and the removal below the world does not apply to
it. FBK-TCK-005 replaces that removal.

**FBK-TCK-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). A falling block
that lands in a tick is removed in that tick, except at a target cell that holds `moving_piston`
(FBK-LND-001). The captures `e3_drop_stone.json` and
`e3_drop_slime_block.json` show a falling block of `sand` that is present after tick 17 and absent
after tick 18, the tick in which it reaches the floor.

**FBK-TCK-003** (`source_suggested`, semantic, confidence medium). The target cell is the cell of
the falling block. When the block of the carried state is a concrete powder (FBK-TRG-002), the server
also finds the water contact:

1. First, the water contact is true when the fluid at the cell is in the fluid tag
   `minecraft:water`. Water that flows and a block that holds water as a property count.
2. Then, when the square of the length of the velocity is more than 1.0, test the segment. The
   velocity is the value after the move and the block effects of step 3 of FBK-TCK-001, before the
   drag. The segment starts at the position at the start of the tick, before the move. It ends at
   the position after the move. Thus its length is the distance of the move of this tick, and it has
   no other bound. The test is the segment test of PRJ-TST-001, with one change: a fluid source of
   water or lava also stops the segment. The shape of such a source is a box from the bottom of its
   block to its fluid height.
3. When the segment stops at a block position whose fluid is in the tag `minecraft:water`, that
   block position becomes the target cell, and the water contact is true. This applies also when
   step 1 already found the water contact. When the segment stops at another position, or does not
   stop, the target cell and the result of step 1 stay.

The test of step 2 runs also when step 1 found the water contact. Thus a concrete powder that falls
faster than one block in each tick does not pass through a thin layer of water. 

**FBK-TCK-004** (`design`, semantic). A falling block that does not land in the maximum age drops
as its item. When the maximum age is more than 0, the on-ground flag and the water contact are false,
and the age is more than the maximum age, the server does these steps:

1. When the drop flag is true and the game rule `minecraft:entity_drops` is true, make the item
   entity of FBK-LND-006. The server sends no `level_event`, also for an anvil (FBK-LND-007).
2. Remove the falling block.

This rule applies also to a falling block that a cobweb or a bubble column holds, and also when the
break flag is true. Thus a falling block of `suspicious_sand` that a cobweb holds drops the item
`suspicious_sand` with no loot. The maximum age 0 disables this rule. It does not disable
FBK-TCK-005.

The maximum age is an integer from 0 to 2147483647, with the default 600. The game rule
`mcrs:falling_block_max_age` of issue #669 holds it. Until that game rule exists, the plugin of the
falling block takes the same value as a setting, with the same default and meaning. With the
default, a falling block exists for at most 600 ticks before it drops, and it drops in tick 601.

Reason: the owner decided this behaviour. The reference has the same
behaviour with the fixed value 600. The bug report MC-260073 states the drop of a suspicious block
in Java Edition, with the resolution "Won't Fix". The bug report MCPE-172500 states that Bedrock
Edition keeps the block, with the resolution "Works As Intended".

**FBK-TCK-005** (`source_suggested`, semantic, confidence medium). When the on-ground flag and the
water contact are false, the age is more than 100, and the `y` of the cell is outside the world
height, the server does the steps of FBK-TCK-004. The test reads the integer `y` of the cell
(section 0.1), not the `y` of the position. Let `min` be the minimum `y` of the dimension and `height` its
height. The `y` of the cell is outside the world height when `y <= min` or `y > min + height - 1`.
Thus the lowest block layer of the dimension counts as outside, and the highest layer
`min + height - 1` counts as inside. The limit 100 is a constant.
This rule does not read the maximum age: `mcrs:falling_block_max_age = 0` does not disable it.
Thus a falling block does not fall without an end below the world. The owner accepted this limit.

## 5. The landing

**FBK-LND-001** (`source_suggested`, semantic, confidence medium). In the landing, the server first
applies the ground rule of PHY-ORD-005 to the velocity: it multiplies the velocity by
`(0.7, -0.5, 0.7)`. This is the only application of the ground rule in the tick. It applies to each
landing, also to a landing by the water contact while the on-ground flag is false. An observer sees
this velocity only when the falling block stays, that is, when the target cell holds
`moving_piston`. Then:

- When the target cell holds `moving_piston`, nothing else occurs. The falling block stays, and it
  can land in a later tick.
- When the break flag is true, FBK-LND-005 applies.
- When the conditions of FBK-LND-002 are true, the carried state becomes a block (FBK-LND-004).
- Otherwise, the falling block breaks into its item (FBK-LND-006).

In each case except the first, the server removes the falling block in this tick.

**FBK-LND-002** (`source_suggested`, semantic, confidence medium). The carried state becomes a block
when each of these conditions is true:

1. The state at the target cell is replaceable at a landing (FBK-LND-003).
2. The carried state can stay at the target cell by its support predicate (`block-support.md`,
   SUP-PRED-001). Each gravity block of FBK-TRG-001 can stay at each position.
3. The state below the target cell is not free (FBK-TRG-004), or the water contact of FBK-TCK-003
   is true.

The table gives examples for a falling block that lands on the top of a block in normal play.

| Block under the falling block | Target cell | Result |
|-------------------------------|-------------|--------|
| `stone` | the cell above the stone | a block |
| `short_grass`, `snow` with 1 layer, water, lava | the cell of that block | a block; the old block is gone |
| a bottom slab, `soul_sand`, `mud`, `farmland`, `dirt_path` | the cell of that block, because the top is below 1 | the item |
| `snow` with 2 to 8 layers | the cell of the snow | the item |
| a block with a `torch`, a `rail` or a carpet on it | the cell of the torch, the rail or the carpet | the item |
| the edge of a block, with the centre of the falling block above a free block | the cell above the free block | the item |

Condition 3 also makes the item for a falling block that lands on an entity, for example a boat.


**FBK-LND-003** (`source_suggested`, semantic, confidence medium). A state is replaceable at a
landing when the replaceable test of `block-placing.md` (PLC-TGT-002, PLC-TGT-006, PLC-TGT-007)
is true for it in this context:

| Term of `block-placing.md` | Value at a landing |
|----------------------------|--------------------|
| clicked position | the target cell |
| face | up |
| held item | the empty stack |
| sneak (secondary use) | false |

With the empty stack, the clicked position and the adjacent position give the same result for each
block, also for `light`. The restriction of PLC-TGT-002 for `light` at the clicked position comes
from the use action of PLC-INT-003, not from the replaceable test, and a landing has no use action.
Thus each held item that is not the item of a block with its own rule in PLC-TGT-002, PLC-TGT-003,
PLC-TGT-006 or PLC-TGT-007 gives the same result as the empty stack, at either position. An item of
a full cube without such a rule, for example `stone`, is such an item. The test gives these results:

- a state whose block definition has the component `minecraft:replaceable` is replaceable, except
  `snow` with more than 1 layer. This includes `light`;
- `glow_lichen`, `sculk_vein` and `resin_clump` are replaceable;
- a `vine` with a face on each of its five sides is not replaceable;
- each other state is not replaceable. This includes a bottom slab, `scaffolding`, a candle, a sea
  pickle, a turtle egg and the pitcher crop.

**FBK-LND-004** (`source_suggested`, semantic, confidence medium). When the carried state becomes a
block, the server does these steps:

1. When the carried state has the property `waterlogged`, and the target cell holds a water source,
   set the property to true.
2. Set the target cell to the carried state, with the flags 3 (SIM-SET-001). The set replaces the
   old state with no drop.
3. Send the packets of FBK-NET-002.
4. Remove the falling block.
5. When the block of the carried state is a concrete powder, and the concrete condition is true,
   set the target cell to the default state of its concrete (FBK-TRG-002), with the flags 3.
6. When the carried state is an anvil, apply FBK-LND-007.

The concrete condition is true when one of these is true:

- the old state of the target cell, before step 2, held a fluid of the tag `minecraft:water`;
- a neighbour of the target cell above it or on one of its four horizontal sides holds a fluid of
  the tag `minecraft:water`, and the face of that neighbour toward the target cell is not sturdy
  (`block-support.md`, SUP-FACE-001).

A neighbour holds water when its state is a water block, source or flowing, or when its state holds
water as a property. Thus a waterlogged neighbour counts when its face toward the target cell is not
sturdy. Examples: a waterlogged bottom slab on a horizontal side counts, because its side face is
not sturdy. A waterlogged top slab above the target cell does not count, because its bottom face is
sturdy. Water below the target cell does not count.

**FBK-LND-005** (`design`, semantic). When the break flag is true, the landing makes no block and
no item. The server removes the falling block. The server also sends `level_event` with the type
2001 and the data equal to the network id of the carried state. The position of the event is the
cell that contains the centre of the box of the falling block. The players of BRK-BRK-006 receive
it. This event applies to each falling block with the break flag, also to a block that a pack adds
to `mcrs:breaks_after_fall`. For an anvil, FBK-LND-007 also applies. The game rule
`minecraft:entity_drops` has no effect on this rule.

Reason: the reference sends this event for its own members of the default tag, `suspicious_sand`
and `suspicious_gravel` (`source_suggested`, confidence medium). The reference sends no event for
other blocks with the break flag, for example `sand` that `summon` makes with `CancelDrop:1b`. mcrs
gives the event to each member, because the event shows the break to a viewer. 

**FBK-LND-006** (`source_suggested`, semantic, confidence medium). When the falling block breaks
into its item, the server removes it. When the drop flag is true and the game rule
`minecraft:entity_drops` is true, the server also makes the drop of the falling block: one item
entity with a stack of 1 item of the carried block, with no components. The item entity has the
default form at the position of the falling block (`item-entity.md`, IEN-SPN-008), and the pickup
delay 10. A block with no item makes no drop. For an anvil, the drop of a landing also sends the
sound of FBK-LND-007.

**FBK-LND-007** (`source_suggested`, semantic, confidence medium). A falling block of an anvil that
is not silent sends `level_event` with the data 0 at the target cell to the players of BRK-BRK-006.
The silent flag is the flag of each entity: the entity data entry 4 (TRK-DATA-002) and the save
and `summon` key `Silent` (byte). Its default is false. A fall that FBK-SPN-001 starts has the
default, so its landing is audible. The events are:

- the type 1031 when the anvil becomes a block;
- the type 1029 when it makes the drop of FBK-LND-006, and when the break flag is true
  (FBK-LND-005). A landing that makes no block and no drop because the drop flag or
  `minecraft:entity_drops` is false sends no event.

An anvil that ends by a time limit (FBK-TCK-004, FBK-TCK-005) sends no event, also when it makes the
drop.

## 6. Packets

**FBK-NET-001** (`source_suggested_black_box_confirmed`, wire, confidence high). The updates of a
falling block follow TRK-UPD-001 and PHY-CMP-002 with the update interval 20. The type artefact
`entity_types.json` gives this update interval. A falling block needs a precise
position after each tick with a collision flag (PHY-CMP-002).

**FBK-NET-002** (`source_suggested`, wire, confidence medium). When the carried state becomes a block
(FBK-LND-004), each viewer of the falling block receives these packets in this order:

1. `block_update` with the target cell and the state that the landing leaves there. For a concrete
   powder that changes in step 5 of FBK-LND-004, this is the concrete (D1).
2. `add_transient_block` with the same position and the same state.
3. `remove_entities` with the falling block (TRK-REM-001).

The fields of `add_transient_block` are the position, encoded as the position of `block_update`, and
the network id of the state as a VarInt. The vanilla client draws that state at that position for
at most 1 second, or until it builds the section again after the packet. Thus a viewer sees no gap
between the end of the entity and the new block.

The set of step 2 of FBK-LND-004 also goes through the client sync of SIM-SET-005. A viewer can thus
receive a second `block_update` with the same state. The second packet has no visible effect. Kind
of the second packet: unspecified.

**FBK-NET-003** (`source_suggested`, wire, confidence medium). Each other end of a falling block
sends `remove_entities` to each viewer (TRK-REM-001): the removal of FBK-TCK-001, the time limits,
the break and the drop. The drop of FBK-LND-006 also sends the spawn bundle of the item entity
(`item-entity.md`, IEN-NET-001).

## 7. Edition differences

**ED1.**

- **Rule**: FBK-TCK-004.
- **Java**: a falling block that does not land in 600 ticks drops as its item (MC-260073).
- **Bedrock**: a suspicious block that a cobweb holds does not drop (MCPE-172500). The time limit
  of Bedrock, if it has one, is not known.
- **Decision**: the maximum age applies to both editions, with the default 600, until the time limit of Bedrock is known.
- **Selected by**: world rule `mcrs:falling_block_max_age`.
- **Visible to the client**: yes. The falling block ends, and an item appears.
- **Prediction risk**: none. The client does not predict the end of a falling block.
- **Parity list or bug reference**: MC-260073, MCPE-172500.

## 9. Intentional deviations and unresolved items

### 9.1 Intentional deviations

**D1.**

- **Rule**: FBK-NET-002.
- **Reference**: for a concrete powder that lands next to water or in water, the `block_update`
  and the `add_transient_block` carry the state of the powder. The ordinary client sync then sends
  the concrete.
- **Visible to the client**: yes. The viewer can see the powder for one tick before the concrete.
- **Test**: a differential test compares the state of the target cell at the end of the tick, not
  the state in the first `block_update`.

### 9.2 Kept by the owner

FBK-TCK-004 keeps the drop of a suspicious block that a cobweb holds (MC-260073), by the decision of
the owner. It has no deviation entry.

FBK-TCK-005 keeps the drop of a falling block outside the world height after 100 ticks. The limit
does not depend on the maximum age. The owner accepted it.

The owner accepted the tags `mcrs:gravity_blocks` and `mcrs:breaks_after_fall` (FBK-TRG-002) and
the deviation D1.

### 9.3 Unresolved

- **U1**: ED1. The time limit of a falling block in Bedrock Edition.
- **U3**: FBK-SPN-002 and FBK-NET-002. The order of the packets rests on captures of
  an earlier snapshot.
