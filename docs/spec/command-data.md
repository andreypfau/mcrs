# The `data get` command: behavioural specification

| Field | Value |
|-------|-------|
| Parser ids | `brigadier:double` 2, `minecraft:entity` 6, `minecraft:block_pos` 8, `minecraft:nbt_path` 23 |

Version: see `assets/minecraft/version.json`. Source of the parser ids: `assets/mcrs/reports/registries.json`, registry
`minecraft:command_argument_type`.

This document states the forms `data get entity <target> [path] [scale]` and
`data get block <target> [path] [scale]`: the data that a query reads, the replies, the result
and the errors. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **source**, **error**: as `command-parser.md` defines them.
- **Display name**: as `command-execute.md`, section 0, defines it.
- **Path**, **node of a path**, **selection**: as `loot-functions.md`, LOOT-MOD-047 and
  LOOT-MOD-048, define them.
- **Path text**: the text of the `<path>` argument, as it is in the command line.
- **Text form**: the styled text of a tag that SNBT-PRINT-004 to SNBT-PRINT-006 (`snbt.md`) state.
- **Entity data**: the compound that a query reads from an entity (CMD-DATA-002).
- **Block entity data**: the compound that a query reads from a block entity (CMD-DATA-003).
- **Queried data**: the entity data or the block entity data of the command.
- **Scale text**: the text of the scale that CMD-DATA-008 states.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-DATA`.

## 1. Scope

The forms `data merge`, `data modify`, `data remove` and the target `storage` are out of scope,
except in the tree. The parse and the suggestions of the argument type `minecraft:nbt_path` are
out of scope (section "Unresolved").

---

### CMD-DATA-001: Tree

```
data                                                 require command_level gamemasters
├── merge                                            (out of scope)
├── get
│   ├── entity
│   │   └── <target: minecraft:entity>               executable; one entity (0x01)
│   │       └── <path: minecraft:nbt_path>           executable
│   │           └── <scale: brigadier:double>        executable
│   ├── block
│   │   └── <target: minecraft:block_pos>            executable
│   │       └── <path: minecraft:nbt_path>           executable
│   │           └── <scale: brigadier:double>        executable
│   └── storage                                      (out of scope)
├── remove                                           (out of scope)
└── modify                                           (out of scope)
```

The permission is PERM-LEVEL-001 (`command-permissions.md`), level `gamemasters`. The node
`data` and the node `get` are not executable. `<scale>` has no minimum and no maximum.

`<target>` of `entity` finds one entity. A selector that can find more than one entity fails at
the parse with `argument.entity.toomany` (ARG-ENT-010, `argument-entity.md`).

Provenance: `source_suggested`. Kind: wire. Confidence: high. Experiment: on a vanilla server, run
`data get entity @e[type=pig]`. The reply is `argument.entity.toomany` with the cursor mark after
the selector.

### CMD-DATA-002: Entity data

- **Behavior:** the entity data is the saved data of the entity without the key `id`. It is the
  compound that the selector option `nbt` compares (ARG-ENT-015). For a player, the entity data
  also has the key `SelectedItem` when the stack in the selected slot is not empty.
- **Postconditions:** a query reads the entity data when the command runs.
- **Edge cases:** the path `id` selects nothing, also for a mob.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: summon a pig with
`{Tags:["t"],CustomName:"Bob"}`. Run `data get entity @e[tag=t,limit=1] id`. The reply is
`arguments.nbtpath.nothing_found` with `id`. Run `data get entity @e[tag=t,limit=1] CustomName`.
The reply has `"Bob"`.

### CMD-DATA-003: Block entity data and the block target

- **Behavior:**
  1. Do the loaded check of ARG-COORD-013 (`argument-coordinates.md`) on `<target>`, in the source
     dimension.
  2. If the block at `<target>` has no block entity, fail with `commands.data.block.invalid`.
  3. The block entity data is the saved data of the block entity with these keys:
     - `id`: a string, the identifier of the block entity type;
     - `x`, `y`, `z`: ints, the block position;
     - `components`: a compound, the components of the block entity, also when it is empty;
     - the keys of the block entity type.
- **Postconditions:** steps 1 and 2 run before the path. A block with no block entity fails with
  `commands.data.block.invalid` with each path, also a path that the block cannot have.

Provenance: `source_suggested`. Kind: semantic. Confidence: high. Experiment: place a chest at
`0 100 0` and stone at `1 100 0`. Run `data get block 0 100 0`. The text form has the keys
`components`, `x`, `y`, `Items`, `z` and `id`. Run `data get block 1 100 0 x`. The reply is
`commands.data.block.invalid`. Run `data get block 100000 100 0` and `data get block 0 400 0`. The
replies are `argument.pos.unloaded` and `argument.pos.outofworld`.

### CMD-DATA-004: The single value of a path

- **Behavior:** a query with `<path>` selects tags from the queried data as LOOT-MOD-048 states.
  1. After each node of the path, if the selection is empty, fail with
     `arguments.nbtpath.nothing_found`. The argument is a text primitive: the path text from its
     start to the end of that node. The nodes after that node do not run.
  2. If the selection has more than one tag, fail with `commands.data.get.multiple`.
  3. Otherwise the tag of the selection is the **value** of the query.
- **Edge cases:**

| Path | Queried data | Result |
|------|--------------|--------|
| `Pos[5].x` | `Pos` has three elements | `arguments.nbtpath.nothing_found` with `Pos[5]` |
| `a.b.c` | no key `a` | `arguments.nbtpath.nothing_found` with `a` |
| `{NoGravity:0b}` | `NoGravity` is `1b` | `arguments.nbtpath.nothing_found` with `{NoGravity:0b}` |
| `Tags[]` | `Tags` has two elements | `commands.data.get.multiple` |

Provenance: `source_suggested`. Kind: semantic; the keys and the arguments are wire. Confidence:
high. Experiment: summon an armor stand with `{NoGravity:1b,Tags:["a","b"]}`. Run the paths of the
table. The replies agree with the table.

### CMD-DATA-005: Query without a path

- **Behavior:** the command sends one success message, with the broadcast flag clear:
  - `entity`: `commands.data.entity.query`, with the display name of the entity and the text form
    of the entity data;
  - `block`: `commands.data.block.query`, with the `x`, `y` and `z` of `<target>` as int
    primitives, and the text form of the block entity data.
- **Postconditions:** the result is 1.

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: run
`execute store result storage t:s r int 1 run data get entity @e[tag=t,limit=1]`. Run
`data get storage t:s r`. The value is 1.

### CMD-DATA-006: Query with a path

- **Behavior:** the command finds the value (CMD-DATA-004). It then sends one success message, with
  the broadcast flag clear. The message is that of CMD-DATA-005, with the text form of the value in
  place of the text form of the queried data. The key does not depend on the tag type of the value.
- **Result:**

| Tag type of the value | Result |
|-----------------------|--------|
| byte, short, int, long, float, double | the value as f64, rounded toward negative infinity, then limited to the range of i32 |
| byte array, int array, long array, list | the number of elements |
| compound | the number of keys |
| string | the length of the string in code units |

- **Postconditions:** NaN gives the result 0. The limit sends a value above the range of i32 to
  `2147483647`, and a value below it to `-2147483648`. A long with more than 53 significant bits
  loses precision in the conversion to f64.
- **Edge cases:** `Pos[1]` with the value `-10.7d` gives −11. The string `"x😀y"` gives 4.
  The int array `UUID` gives 4.

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: summon an armor stand at `0.5 -10.7 0.5` with
`{NoGravity:1b,Tags:["a","x😀y"]}`. Store the result of `data get` of `Pos[1]`, `Tags[1]`, `Pos`
and `UUID[0]` in storage with `execute store result`. The values are −11, 4, 3 and the first
element of `UUID`. The reply of `Pos` is `commands.data.entity.query` with `[0.5d, -10.7d, 0.5d]`.

### CMD-DATA-007: Query with a scale

- **Behavior:** the command finds the value (CMD-DATA-004).
  1. If the value is not a byte, short, int, long, float or double, fail with
     `commands.data.get.invalid`. The argument is the path text, as a text primitive.
  2. Let `r` be the value as f64 times `<scale>`, in f64. Round `r` toward negative infinity and
     limit it to the range of i32, as CMD-DATA-006 states. This is the result.
  3. Send one success message, with the broadcast flag clear:
     - `entity`: `commands.data.entity.get`, with the path text, the display name of the entity,
       the scale text and the result;
     - `block`: `commands.data.block.get`, with the path text, the `x`, `y` and `z` of `<target>`,
       the scale text and the result.
- **Postconditions:** the path text and the scale text are text primitives. The coordinates and
  the result are int primitives. Step 2 of CMD-DATA-004 runs before step 1 of this rule, so `Tags[]`
  with two elements fails with `commands.data.get.multiple`.
- **Edge cases:**

| Value | Scale | Result |
|-------|-------|--------|
| `-10.7d` | `1` | −11 |
| `-10.7d` | `10000000000` | −2147483648 |
| `-10.7d` | `-10000000000` | 2147483647 |
| `-10.7d` | `0` | 0 |
| `1b` | `-0.004` | −1 |
| `-90.5f` | `1` | −91 |
| `[0.5d, -10.7d, 0.5d]` | `2` | `commands.data.get.invalid` with `Pos` |
| `"a"` | `1` | `commands.data.get.invalid` with `Tags[0]` |

Provenance: `source_suggested`. Kind: semantic; the keys and the argument order are wire.
Confidence: high. Experiment: on the armor stand of CMD-DATA-006, with `Rotation:[-90.5f,0f]`, run
the cases of the table. The replies agree with the table.

### CMD-DATA-008: Scale text

- **Behavior:** the scale text has exactly two digits after the decimal mark `.`.
  1. Take the decimal of SNBT-PRINT-001 that prints the scale as a double. Use its digits, not its
     notation: `1.0E10` is the decimal 10000000000.
  2. Round the decimal to two digits after the mark. A tie rounds away from zero.
  3. Write the integer part in decimal, with no group separator. Write `-` before a negative scale,
     also when the rounded text is zero.
- **Edge cases:**

| Scale | Scale text |
|-------|------------|
| `10` | `10.00` |
| `0.125` | `0.13` |
| `1.005` | `1.01` |
| `2.675` | `2.68` |
| `1.115` | `1.12` |
| `0.005` | `0.01` |
| `-0.005` | `-0.01` |
| `-0.004` | `-0.00` |
| `-0.0` | `-0.00` |
| `1234567.891` | `1234567.89` |
| `10000000000` | `10000000000.00` |

- **Postconditions:** the round uses the decimal of step 1, not the exact binary value. The double
  nearest to `1.005` is below `1.005`, but the text is `1.01`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`data get entity @e[tag=q,limit=1] NoGravity <scale>` with each scale of the table. Each reply has
the scale text of the table.

### CMD-DATA-009: Errors

| Situation | Translation key | Arguments |
|-----------|-----------------|-----------|
| the selector finds no entity | `argument.entity.notfound.entity` | none |
| the chunk of the block position is not loaded | `argument.pos.unloaded` | none |
| the block position is outside the world | `argument.pos.outofworld` | none |
| the block has no block entity | `commands.data.block.invalid` | none |
| a node of the path selects nothing | `arguments.nbtpath.nothing_found` | the path text to the end of that node |
| the path selects more than one tag | `commands.data.get.multiple` | none |
| a query with a scale finds a value that is not a number | `commands.data.get.invalid` | the path text |

- **Behavior:** the checks run in the order of the table. The first check that fails stops the
  command. A failed command sends no success message.
- **Postconditions:** the key `commands.data.get.unknown` exists in the language file. A `data get`
  never sends it, because a path never selects a tag with no type.

Provenance: `source_suggested`. Kind: semantic; the keys are wire. Confidence: high. Experiment: run
`data get entity @e[type=zombie,tag=none,limit=1]`. The reply is `argument.entity.notfound.entity`.
The experiments of CMD-DATA-003, CMD-DATA-004 and CMD-DATA-007 show the other keys.

### CMD-DATA-010: The text form of a whole query

- **Behavior:** the second argument of `commands.data.entity.query` and the fourth argument of
  `commands.data.block.query` are the text form of the queried data.
- **Postconditions:** a test can rely on these properties of the text form:
  - the set of keys of each compound, and the value and the tag type of each key;
  - the order of the elements of each list and of each array;
  - the forms of SNBT-PRINT-004: `, ` between two entries, `: ` after a key, the suffixes of the
    numbers, the quotes of the strings and the folds of SNBT-PRINT-006.
- **Edge cases:** a test must not rely on these properties:
  - the order of the entries of a compound (SNBT-PRINT-004). Two queries of the same entity can
    give two orders;
  - a value that changes from tick to tick, for example `Motion` of an entity that falls, or `UUID`
    of a new entity.

A test that compares a whole query parses the text form and compares the compounds. A test that
compares one value uses a query with a path (CMD-DATA-006).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: summon a pig and a
cow. Run `data get entity` on each. The two texts start with the same keys in the same order, and
that order is not the order of the code units of the keys.

### CMD-DATA-011: The entity data of a player

- **Behavior:** the entity data of a player has the keys of the player file of SAVE-PLAYER-001
  (`persistence.md`), and `SelectedItem` of CMD-DATA-002. This table lists the keys that a test of
  movement, health, food and inventory reads:

| Key | Tag type | Content |
|-----|----------|---------|
| `Pos` | list of 3 doubles | x, y, z of the position |
| `Motion` | list of 3 doubles | the velocity, in blocks per tick |
| `Rotation` | list of 2 floats | yaw, pitch, in degrees |
| `OnGround` | byte | `1b` on the ground, else `0b` |
| `Health` | float | the health |
| `foodLevel` | int | the food level, 0 to 20 |
| `foodSaturationLevel` | float | the saturation |
| `foodExhaustionLevel` | float | the exhaustion |
| `foodTickTimer` | int | the food timer (`hunger.md`, HNG-SAV-001) |
| `playerGameType` | int | the game mode: 0 survival, 1 creative, 2 adventure, 3 spectator |
| `Dimension` | string | the identifier of the dimension |
| `SelectedItemSlot` | int | the selected hotbar slot, 0 to 8 |
| `Inventory` | list of compounds | an entry for each slot of 0 to 35 that is not empty |
| `SelectedItem` | compound | the stack in the selected slot; absent when the slot is empty |

- **The inventory entry:** each entry of `Inventory` is a compound with these keys:
  - `Slot`: a byte, the slot index, 0 to 35;
  - `id`: a string, the identifier of the item;
  - `count`: an int, also when the count is 1;
  - `components`: a compound, the component patch; absent when the patch is empty
    (STK-SAV-001, `item-stack.md`).
- **Postconditions:**
  - The entries of `Inventory` are in ascending order of `Slot`.
  - `SelectedItem` has the keys of an inventory entry without `Slot`.
  - The armour slots and the offhand slot are in `equipment`, not in `Inventory` (`inventory.md`,
    INV-SAV-001).
- **Sample:** a creative player in the Overworld, with a damaged sword in slot 0 and five stone
  in slot 4. Slot 0 is selected. The sample shows only the keys of the table. Its values and its
  key order are a sample, not a requirement.

```
{Pos: [0.5d, -60.0d, 0.5d], Motion: [0.0d, 0.0d, 0.0d], Rotation: [90.0f, 0.0f],
 OnGround: 1b, Health: 20.0f, foodLevel: 20, foodSaturationLevel: 5.0f,
 foodExhaustionLevel: 0.0f, foodTickTimer: 0, playerGameType: 1,
 Dimension: "minecraft:overworld", SelectedItemSlot: 0,
 Inventory: [{Slot: 0b, id: "minecraft:diamond_sword", count: 1, components: {"minecraft:damage": 5}},
             {Slot: 4b, id: "minecraft:stone", count: 5}],
 SelectedItem: {id: "minecraft:diamond_sword", count: 1, components: {"minecraft:damage": 5}}}
```

Provenance: `source_suggested`. Kind: semantic; the key names and the tag types are wire.
Confidence: medium. Experiment: summon an armor stand at `0.5 -10.7 0.5` with
`{NoGravity:1b,Rotation:[-90.5f,0f]}`. Run `data get` of `Pos`. The value is
`[0.5d, -10.7d, 0.5d]`. Run `data get` of `Rotation[0]` with the scale 1. The result is −91. This
experiment covers only the keys that a player shares with each entity.

---

## Unresolved

- **Q1:** no specification states the argument type `minecraft:nbt_path` as a command argument:
  its read from the command line, its parse errors, their positions and its suggestions.
  LOOT-MOD-047 states the grammar of a path in a data pack field. The vanilla client parses the
  argument, so this is a rule of the kind client agreement. A `data get` with a valid path does not
  need it.
- **Q2:** no experiment has queried a player. The keys of CMD-DATA-011 that only a player has, and
  `SelectedItem`, need a run with a vanilla client on the vanilla server.
