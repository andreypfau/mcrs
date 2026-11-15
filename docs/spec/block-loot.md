# Block loot: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states what a block drops when it breaks. It states how the server finds the loot
table of the block, which values the table reads, and how each result becomes an item entity. It
does not prescribe an architecture.

Scope: the block source, for the causes of section 2. `loot.md` states the evaluation of a table of
any type, the division of a stack and the random draws. `loot-conditions.md` states the conditions
and the number providers. `loot-functions.md` states the item modifiers. `block-breaking.md` states
when a survival break drops loot (BRK-LOOT-001, BRK-LOOT-005), the experience (BRK-LOOT-004) and the
packets. `item-entity.md` states the start values of a block drop (IEN-SPN-001, row "drop of a
block") and `entity-physics.md` states the start position and velocity (PHY-CMP-025).
`registry-fields.md`, section 9, states how the server decodes a loot table and each part of it.
This document refers to the rules of these documents by id. Section 8 lists what this document does
not state.

### Rules that moved

Earlier versions of this document held the rules of this table. Each rule keeps its id in its new
document. LOOT-MOD-008 keeps its meaning, but the table of the number providers is now LOOT-NUM-023
(last row), and LOOT-MOD-008 cites it.

| Rule ids | Document |
|----------|----------|
| LOOT-EVL-001 to LOOT-EVL-007 | `loot.md` |
| LOOT-OUT-001 | `loot.md` |
| LOOT-RND-002 | `loot.md` |
| LOOT-CND-001 to LOOT-CND-009 | `loot-conditions.md` |
| LOOT-MOD-001 to LOOT-MOD-008 | `loot-functions.md` |
| the number providers of the block tables, earlier a part of LOOT-MOD-008 | `loot-conditions.md`, LOOT-NUM-023 |

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**: as `login.md`, section 0.1, defines them.
- **Stack**, **empty stack**, **count**, **maximum stack size**: as `item-stack.md`, section 0.1,
  defines them.
- **Survival break**, **harvest**, **main-hand item**: as `block-breaking.md`, section 0.1, defines
  them.
- **Loot table**, **pool**, **entry**, **condition**, **modifier**, **leaf entry**, **composite
  entry**, **context**, **parameter**, **luck**, **draw**, **drop list**: as `loot.md`, section 0.1,
  defines them.
- **Block table**: the loot table that the block definition names in its component `minecraft:loot`
  (`assets/mcrs/block_definition/README.md`). A block without this component has no block table.
- **Tool**: the stack of the parameter `tool`. The tool can be the empty stack.
- **Level of an enchantment**: the level of that enchantment in the component
  `minecraft:enchantments` of the tool. The level is 0 when the tool does not have the enchantment.
- **Origin block**: the block position whose block table the server evaluates.

An integer range `a..b` includes `a` and `b`.

### 0.2 Provenance

- `black_box_confirmed`: shown by a game-produced artefact. The artefact is a captured result of
  this document, of `block-breaking.md` or of `item-entity.md`, or the vanilla data pack.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with a captured result.
- `source_suggested`: taken from the reference implementation, with no captured result. The
  confidence is at most medium. Section 10 lists the experiments that ran.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided. Section 9.3 collects these items.

The reference that the author read is an earlier release than the game version above. The block
tables of that release use other field names, for example `conditions` and `functions`. The meaning
of each type that this document states did not change between the releases, as far as the data pack
shows. Each rule from the reference thus has the label `source_suggested`.

Each rule states its kind, as `docs/spec/README.md` defines it.

### 0.3 Artefacts cited

- The data pack of the client jar: `data/minecraft/loot_table/blocks/` (1226 block tables) and
  `data/minecraft/predicate/tool/`. `tools/mcrs_minecraft_fixtures` installs it.
- `assets/mcrs/block_definition/`: the component `minecraft:loot` of each block.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `LOOT-SRC` | the causes and the block table |
| `LOOT-CTX` | the context of a block table, number 001 |
| `LOOT-OUT` | the item entities, numbers 002 to 004 |
| `LOOT-RND` | the random values of the block source, numbers 001 and 003 |
| `LOOT-DEV` | the intentional deviations of this document, numbers 001 to 099 |

`docs/spec/README.md`, section "Loot documents", lists the document of each other loot rule.

## 1. What the block tables use

**LOOT-SRC-001** (`black_box_confirmed`, semantic, confidence high). The block tables of the vanilla
data pack use only the types of this table. The server evaluates each type in a block table, also a
type that this table does not list, as `loot.md`, `loot-conditions.md` and `loot-functions.md` state.

| Part | Types | Document |
|------|-------|----------|
| entry | `item`, `alternatives`, `dynamic` | `loot.md` |
| condition | `survives_explosion`, `match_block`, `table_bonus`, `random_chance`, `any_of`, `all_of`, `inverted`, `match_tool`, `location_check`, `entity_properties`, a key of the registry `predicate` | `loot-conditions.md` |
| modifier | `set_count`, `explosion_decay`, `copy_components`, `apply_bonus`, `limit_count`, `copy_state`, a list of modifiers | `loot-functions.md` |
| number provider | a bare number, `uniform`, `binomial` | `loot-conditions.md` |

Each pool has `rolls` 1 and no `bonus_rolls`. No entry has `weight` or `quality`. A `condition`
field is one condition: an object, or a key of the registry `predicate`. A `modifier` field is one
object or a list. The key `minecraft:tool/can_silk_touch` occurs 124 times and
`minecraft:tool/can_shear` 14 times. Fourteen block tables have no pools, for example `fire` and
`spawner`. Nine block tables have a table modifier, for example `wheat`.

## 2. Causes

**LOOT-SRC-002** (`source_suggested`, semantic, confidence medium). The server evaluates a block
table for these causes. The table gives the context values of each cause. "No value" means that the
context does not have the value.

| Cause | `tool` | `this_entity` | `explosion_radius` | Where |
|-------|--------|---------------|--------------------|-------|
| survival break with a harvest | the main-hand item before the break damages it (LOOT-SRC-004) | the breaker | no value | BRK-LOOT-001 |
| a block that a change of a neighbour removes, for example a torch without support | the empty stack | no value | no value | BRK-DRP-004 |
| `setblock` or `fill` with the mode `destroy` | the empty stack | no value | no value | result R6 of `item-entity.md` |
| an explosion | the empty stack | the direct source of the explosion | the radius, only for an explosion with decay | section 8 |

A survival break without a harvest does not evaluate the table (BRK-LOOT-001). A creative break does
not evaluate the table (BRK-DRP-001). When the game rule `block_drops` is false, the server can
evaluate the table, but no item entity appears (LOOT-OUT-003).

**LOOT-SRC-003** (`source_suggested`, semantic, confidence medium). The server evaluates the block
table of the state that the block had before the break. A block without a block table drops nothing
from loot, for example `air`, `water` and `bedrock`. A block table without pools drops nothing.

**LOOT-SRC-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). In a survival
break, the tool is a copy of the main-hand item before the break damages it (BRK-DMG-001). Thus an
item that breaks on this use still gives its enchantments to the table. Result R37 of
`block-breaking.md` shows a wooden pickaxe with the damage 58 that breaks and still drops
`cobblestone`. Result L6 shows such a pickaxe with Silk Touch that breaks and drops `stone`.

**LOOT-SRC-005** (`source_suggested_black_box_confirmed`, semantic, confidence high). With an empty
main hand, the tool is the empty stack. The tool is present in the context, and it has no
enchantments. Thus the level of each enchantment is 0, and each `match_tool` of a block table fails
(LOOT-CND-007). `apply_bonus` still applies its formula with the level 0 (LOOT-MOD-004). For
example, the table of `wheat` gives ripe wheat an extra `wheat_seeds` count of 0 to 3 for an empty
hand. Result L3 shows this distribution.

**LOOT-SRC-006** (`source_suggested_black_box_confirmed`, semantic, confidence high). A block of two
parts evaluates the table of one part, as follows.

- A survival break of a tall plant evaluates the block table of the hit part before it removes
  either part. The context is the context of the break. The removal of the other part then evaluates
  the table of the other part, but its `location_check` fails, because the hit part is gone. For
  example, `tall_grass` and `sunflower` drop one time.
- A door or a bed evaluates the table of each part when that part is removed. The table drops only
  for one part, because of its `match_block` condition: the lower part of a door, and the head of a
  bed. When the break hits the other part, the removal of the part that drops is a neighbour change.
  Its context is the second row of LOOT-SRC-002: the empty stack and no `this_entity`.

The item entity appears at the position of the part whose table dropped it. Result L4 shows one item
for each break. For each hit part, the item of a door is at the lower part, and the item of a bed is
at the head. The item of `tall_grass` is at the hit part. Item U1 records what result L4 does not
show.

## 3. Context and evaluation

**LOOT-CTX-001** (`source_suggested`, semantic, confidence medium). The context of a block table has
the key set `block` (LOOT-CTX-011 of `loot.md`) and these parameters:

| Parameter | Content | Present |
|-----------|---------|---------|
| `origin` | the centre of the origin block, `(x + 0.5, y + 0.5, z + 0.5)` | always |
| `block_state` | the state of LOOT-SRC-003 | always |
| `tool` | LOOT-SRC-002 | always |
| `this_entity` | LOOT-SRC-002 | for some causes |
| `block_entity` | the block entity of the origin block before the break | when the block had one |
| `explosion_radius` | LOOT-SRC-002 | for some causes |

The luck of the context is 0 (LOOT-CTX-015). The block entity gives the dynamic names of LOOT-EVL-007.
The server evaluates the block table as `loot.md` states, with the division of LOOT-OUT-001. A
condition or a modifier that reads an absent parameter acts as its rule states for that case.

Sections 4 and 5 of earlier versions of this document moved to other documents (section "Rules that
moved"). The other sections keep their numbers.

## 6. The item entities

**LOOT-OUT-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). Each stack of
the drop list (LOOT-OUT-001) that is not empty becomes one item entity, in the order of the list. A
stack with the count 0 or less makes nothing. The start values are the row "drop of a block" of
IEN-SPN-001. The position and the velocity are those of PHY-CMP-025 at the origin block, and the
pickup delay is 10. Result R37 of `block-breaking.md` shows one item entity for each stack.

**LOOT-OUT-003** (`source_suggested`, semantic, confidence medium). When the game rule `block_drops`
is false, no stack of a block table becomes an item entity (BRK-LOOT-005).

**LOOT-OUT-004** (`source_suggested`, semantic, confidence medium). The order of the item entities
of one break is the order of the drop list. `add_entity` reaches the breaker in that order. The
experience orbs come after the items (BRK-LOOT-001), except the orbs of a creaking heart, which come
first (BRK-LOOT-006). The item entities of a block that loses its support come before those of the
broken block (BRK-LOOT-003). BDR-ORD-001 of `block-drops.md` states the full order of each cause.

## 7. Random values

**LOOT-RND-001** (`design`, semantic). Each draw of an evaluation of a block table is a keyed random
value. The stream is a function of the world seed, the id of the block table and the origin block.
It is also a function of the game tick of the evaluation and of the cause. The `i`-th draw of one
evaluation is the `i`-th value of its stream. The draws, their order and their kinds are those of
`loot.md` (LOOT-RND-006, LOOT-RND-008). This is the keyed mode of LOOT-RND-002. Thus a drop does not
depend on the order of the breaks in a tick or on the thread count. Reason: the keyed random values
of `entity-ai-timing.md` (AIT-LAG-008) and `entity-physics.md` (PHY-CMP-025). LOOT-DEV-001 records
the difference from the vanilla server. LOOT-RND-009 of `loot.md` applies the same choice to each
other source without a saved seed.

**LOOT-RND-003** (`source_suggested`, semantic, confidence medium). The position and the velocity of
an item entity of a drop take their own random values (PHY-CMP-025, PHY-DEV-013). They are not draws
of the evaluation.

## 8. What this document does not state

- How the server decodes a loot table and its parts: `registry-fields.md`, section 9.
- The evaluation of a table, of each entry, condition, modifier and number provider, and the
  division of a stack: `loot.md`, `loot-conditions.md` and `loot-functions.md`.
- The sources of the loot tables of other types, for example `entity`, `chest`, `fishing`,
  `archaeology`, `block_use`, `block_interact`, `shearing` and `vault`. The loot of a trial spawner.
- The advancements, and the statistic of a mined block.
- An explosion: which explosions have decay, the merge of the stacks of one explosion, and the
  experience of an explosion. This document states only that an explosion gives `explosion_radius`
  (LOOT-SRC-002). `loot-conditions.md` and `loot-functions.md` state what it changes in a table
  (LOOT-CND-003, LOOT-MOD-002).
- The contents of a container that drop with the block (BRK-LOOT-003, IEN-SPN-005). They do not come
  from the block table.
- The experience of a block (BRK-LOOT-004, BRK-LOOT-006).
- The items that a block drops in a creative break (BRK-DRP-007, BRK-DRP-008).
- Feature flags. Each item of the vanilla data pack is enabled.
- The command `/loot` and the command `/random`.

## 9. Intentional deviations, open questions and unresolved items

### 9.1 Intentional deviations

**LOOT-DEV-001.**

- **Rule**: LOOT-RND-001.
- **Reference**: each block table has a field `random_sequence`, for example
  `minecraft:blocks/stone`. The vanilla server keeps one random stream for each sequence id on the
  server. It seeds the stream from the world seed and an MD5 hash of the id, and saves its state
  with the world (LOOT-RND-004, LOOT-RND-005 of `loot.md`). Each evaluation of the table continues
  the stream. The drops thus depend on the order of all earlier evaluations of the same table, also
  in other dimensions.
- **Visible to the client**: no, as a distribution.
- **Test**: a differential test compares the distribution of the drops over many breaks.

### 9.2 Open questions

- **Q1**: the server can take the field `random_sequence` as part of the key of LOOT-RND-001 in
  place of the table id. In the vanilla data pack, each block table names its own id. A custom data
  pack can share one sequence between tables. This document uses the table id, because the
  difference has no effect that a player can see.

### 9.3 Unresolved

- **U1**: the context of the part of a door or a bed that drops when the break hits the other part
  (LOOT-SRC-006). The reference suggests the empty stack and no `this_entity`. No current block
  table of a door or a bed reads the tool or the entity. Experiment E7 of `block-drops.md` adds a
  data pack whose tables read them, so it can show the context. E7 did not run.
- **U2**: the full list of components of a block entity for LOOT-MOD-007 (`loot-functions.md`). The
  required result does
  not depend on this item: LOOT-MOD-006 and result L7 apply. A server that does not store the
  components of a block entity does not conform to LOOT-MOD-006. For example, its named chest drops
  a chest without a name.

## 10. Experiments

All experiments ran on 2026-11-13 against the vanilla server of `26.4-snapshot-7`, on a flat world,
with one scripted client in survival mode. The attribute `minecraft:block_break_speed` of the player
was 1000, so each start action broke the block at once. Experiments E1, E2, E3 and E8 evaluate the
block table with the command `execute as <player> run loot spawn <pos> mine <block> mainhand`. That
command evaluates the same table, with the main-hand item of the player as the tool and the player
as `this_entity`. Each run of the command writes one line to the log, for example `Dropped 2 [Coal]
from loot table minecraft:blocks/coal_ore`.

| Id | Steps | Result | Rules |
|----|-------|--------|-------|
| E1 | Evaluate the table of `coal_ore` 1000 times with a diamond pickaxe with Fortune III. | L1 | LOOT-MOD-004 |
| E2 | Evaluate the table of `oak_leaves` 2000 times with an empty hand. | L2 | LOOT-CND-004, LOOT-NUM-023 |
| E3 | Evaluate the tables of `glowstone` and `brown_mushroom_block` 500 times each with an empty hand. | L3 | LOOT-MOD-001, LOOT-MOD-003 |
| E8 | Evaluate the table of `wheat` with `age=7` 500 times with an empty hand. Count the items of each evaluation. | L3 | LOOT-SRC-005, LOOT-MOD-004 |
| E4 | Break each part of `oak_door` and `red_bed` with an empty hand, and each part of `tall_grass` with shears. Record `add_entity` and the item. | L4 | LOOT-SRC-006 |
| E5 | Break `snow` with `layers=3` with a wooden shovel. Place it again, and run `setblock` with `air` and `destroy`. | L5 | LOOT-CND-009 |
| E6 | Break `stone` with a wooden pickaxe with Silk Touch and the damage 58. | L6 | LOOT-SRC-004 |
| E7 | Place a chest with the name `Box` and break it with an empty hand. | L7 | LOOT-MOD-006 |

No experiment tests an explosion, the division of LOOT-OUT-001, the order of LOOT-OUT-004 or the
game rule `block_drops`.

## 11. Captured results

**L1.** E1, the count of `coal` in each evaluation. The expected numbers are 400, 200, 200 and 200.

| Count | 1 | 2 | 3 | 4 |
|-------|---|---|---|---|
| Evaluations | 415 | 190 | 180 | 215 |

**L2.** E2, 2000 evaluations of `oak_leaves`. The expected chances of one evaluation are 0.05 for
`oak_sapling`, 0.02 for `stick` and 0.005 for `apple`.

| Result | Evaluations |
|--------|-------------|
| 1 `oak_sapling` | 115 |
| 1 `stick` | 19 |
| 2 `stick` | 16 |
| 1 `apple` | 6 |
| two stacks | 2 |
| nothing | 1842 |

**L3.** E3 and E8, the count of each evaluation. For `brown_mushroom_block`, the log line of a count
0 is `Dropped 0 [Air]`. The expected numbers for it are 388.9, 55.6 and 55.6. For `wheat`, each
evaluation gave 1 `wheat` and one stack of `wheat_seeds`. The script read the item entities of 250
of the 500 evaluations. The expected numbers for the seeds of these 250 are 19.6, 79.1, 104.9 and
46.4.

| Table | Count: evaluations |
|-------|--------------------|
| `glowstone` | 2: 150, 3: 176, 4: 174 |
| `brown_mushroom_block` | 0: 380, 1: 58, 2: 62 |
| `wheat`, `wheat_seeds` | 1: 17, 2: 78, 3: 112, 4: 43 |

**L4.** E4, the one item entity of each break. The `y` of an item is the block `y` plus 0.125 to
0.625.

| Break | Item | Position of `add_entity` | Part at that position |
|-------|------|--------------------------|-----------------------|
| door, upper part at `(2, -59, 0)` | `oak_door` | `(2.41, -59.5, 0.59)` | lower |
| door, lower part at `(2, -60, 0)` | `oak_door` | `(2.53, -59.47, 0.69)` | lower |
| bed, foot at `(2, -60, 0)` | `red_bed` | `(3.64, -59.85, 0.35)` | head |
| bed, head at `(3, -60, 0)` | `red_bed` | `(3.34, -59.79, 0.47)` | head |
| tall grass, upper part at `(2, -59, 0)`, shears | 2 `short_grass` | `(2.75, -58.64, 0.64)` | upper, the hit part |
| tall grass, lower part at `(2, -60, 0)`, shears | 2 `short_grass` | `(2.3, -59.74, 0.34)` | lower, the hit part |

**L5.** E5. The break with the wooden shovel dropped 3 `snowball`. `setblock` with `destroy` dropped
nothing.

**L6.** E6. The pickaxe broke, and the break dropped 1 `stone`.

**L7.** E7. The break dropped one `chest` with the component `"minecraft:custom_name": "Box"`.
