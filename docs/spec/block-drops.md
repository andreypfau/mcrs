# Block drops and block experience: behavioural specification

| Field | Value |
|-------|-------|
| Version | see `assets/minecraft/version.json` |

This document states what a removed block drops and the experience that it pays. It states only the topics that other documents do not state.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**: as `login.md`, section 0.1, defines them.
- **Stack**, **empty stack**, **count**, **maximum stack size**, **component**: as `item-stack.md`,
  section 0.1, defines them.
- **Break**, **survival break**, **harvest**, **main-hand item**: as `block-breaking.md`, section
  0.1, defines them.
- **Creative break**: a break that a player in creative mode causes.
- **Block table**, **tool**, **origin block**, **level of an enchantment**: as `block-loot.md`,
  section 0.1, defines them.
- **Drop list**, **context**, **parameter**, **key set**, **draw**: as `loot.md`, section 0.1,
  defines them.
- **Explosion**, **power**, **block interaction mode**, **causing entity**, **source entity**: as
  `explosions.md`, section 0.1, and EXP-SRC-001 define them.
- **Orb**: as `experience.md`, section 0.1, defines it.
- **Dimension random**: as `block-simulation.md`, section 0.1, defines it (SIM-RNG-001).
- **Value effect**: an enchantment value effect (REG-FLD-356 of `registry-fields.md`).
- **Neighbour removal**: the removal of a block because a change of a neighbour makes the block
  invalid, for example a torch without support. It is the second row of LOOT-SRC-002.
- **Command destroy**: the removal of a block by `setblock` or `fill` with the mode `destroy`. It is
  the third row of LOOT-SRC-002.
- **Removal**: a survival break, a creative break, a neighbour removal, a command destroy, or the
  change to air at a position of an explosion.
- **Experience block**: a block of a row of the table of BDR-XP-002. These are the blocks of the
  table of BRK-LOOT-004, and six ores with the amount 0.
- **Held items**: the stacks that the block entity of a block holds (BDR-CNT-001).
- **Explosion kind**: one of the values `none`, `block`, `mob`, `tnt` and `trigger`. These are the
  values of the field `block_interaction` of the enchantment effect `minecraft:explode`
  (`registry-fields.md`).
- **Collected stack**: a stack of the list that an explosion builds from the drop lists of its
  positions (BDR-EXL-002). A collected stack has the position of the block that started it.

Each draw uses the notation of LOOT-RND-006 of `loot.md`: `F`, `B(n)` and `between(a, b)`. A draw of
the experience or of the held items comes from the dimension random. It is not a draw
of a loot evaluation, and a parity test compares only its distribution.

### 0.2 Provenance

The labels are those of `docs/spec/registry.md`, section 0.3:

- `black_box_confirmed`: shown by a game-produced artefact.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with a captured result.
- `source_suggested`: taken from the reference implementation, with no captured result. The
  confidence is at most medium. Section 6 gives the experiment of each such rule.
- `unresolved`: not decided. Section 5.2 collects these items.

Section 5.1 records a deviation from the reference.

No experiment of section 6 ran. Each rule from the reference
thus has the label `source_suggested`. Each rule states its kind, as `docs/spec/README.md` defines
it.

### 0.3 Artefacts cited

- `assets/mcrs/registry_definition/game_rule.json`: the names and the defaults of the game rules
  `block_drops`, `block_explosion_drop_decay`, `mob_explosion_drop_decay`,
  `tnt_explosion_drop_decay`, `tnt_explodes` and `mob_griefing`.
- `assets/mcrs/reports/registries.json`: the registry `block_entity_type`.
- The data pack of the client jar: the enchantment `minecraft:silk_touch` and the block tag
  `minecraft:jukebox_containers`.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BDR-XP` | the experience of a removed block |
| `BDR-EXL` | the blocks that an explosion destroys |
| `BDR-CNT` | the held items |
| `BDR-ORD` | the order of the item entities and the orbs of one removal |
| `BDR-DEV` | the intentional deviations of this document |

## 1. Experience

**BDR-XP-001** (`source_suggested`, semantic, confidence medium). The experience that a removal pays
depends on its cause, as this table states. "Tool" is the stack that BDR-XP-002 reads. The amount of an
experience block is the result of BDR-XP-002.

| Cause | Blocks that pay | Tool | Rule |
|-------|-----------------|------|------|
| survival break with a harvest | each experience block | the tool of LOOT-SRC-004 | BRK-LOOT-004 |
| survival break of a creaking heart, with or without a harvest | a creaking heart with `natural=true` | none | BRK-LOOT-006 |
| survival break without a harvest, other blocks | none | none | BRK-LOOT-004 |
| creative break | none | none | this rule |
| neighbour removal, command destroy | each experience block | the empty stack | this rule |
| explosion with a player as the causing entity | each experience block | the empty stack | this rule |
| explosion with a player as the causing entity | a creaking heart with `natural=true`, when the player is not in creative mode or spectator mode | none | this rule |
| explosion with another causing entity or with none | none | none | this rule |

A creaking heart pays `between(20, 24)`, as in BRK-LOOT-006. It pays nothing for a neighbour removal
and for a command destroy. The game mode of the player has no effect on the experience blocks in an
explosion. An explosion in the block interaction mode `keep` or `trigger` pays nothing
(BDR-EXL-001).

When the game rule `block_drops` is false, no cause pays experience. The orbs appear at the centre
of the block, as BRK-LOOT-001 states, and the split is that of XP-VAL-001.

**BDR-XP-002** (`source_suggested`, semantic, confidence medium). The experience of an experience
block comes from these steps:

1. Draw the amount of the block from the table below.
2. When the block is a spawner, the amount is the experience. Stop.
3. Let `v` be the amount as an f32 value.
4. Take each enchantment of the component `minecraft:enchantments` of the tool.
5. For each enchantment, take each conditional effect of its effect `minecraft:block_experience`,
   in the order of the list.
6. Test the `requirements` of the conditional effect. Use the key set `enchanted_item`, with `tool`
   set to the tool and `enchantment_level` set to the level of the enchantment.
7. When the effect has no `requirements`, or they pass, set `v` to the result of the value effect
   (BDR-XP-003) at the level of the enchantment.
8. Convert `v` to an i32 toward zero. The result is the experience.

| Block | Amount |
|-------|--------|
| coal ore, deepslate coal ore | `between(0, 2)` |
| nether gold ore | `between(0, 1)` |
| diamond ore, deepslate diamond ore, emerald ore, deepslate emerald ore | `between(3, 7)` |
| lapis ore, deepslate lapis ore, nether quartz ore | `between(2, 5)` |
| redstone ore, deepslate redstone ore, lit or not lit | `between(1, 5)` |
| spawner | `15 + B(15) + B(15)`, from 15 to 43 |
| sculk | 1 |
| sculk sensor, calibrated sculk sensor, sculk shrieker, sculk catalyst | 5 |
| iron ore, deepslate iron ore, gold ore, deepslate gold ore, copper ore, deepslate copper ore | 0 |

The amount of a spawner is not uniform: 15 and 43 each have the chance 1/225, and 29 has the chance
15/225. The tool has no effect on a spawner, so Silk Touch does not remove its experience. Other
blocks, for example ancient debris and stone, are not experience blocks. An effect `add` thus gives
experience to iron ore, but not to stone.

A result of 0 or less makes no orb. A NaN gives 0. A value above 2³¹ − 1 gives 2³¹ − 1. The empty
stack has no enchantments, so it keeps the amount. The component `minecraft:stored_enchantments` has
no effect. In the vanilla data pack, only Silk Touch has this effect: `set` with the value 0.0 and no
`requirements`. The draws of `requirements` and of the value effects come from the dimension random.

The order of the enchantments in step 4 is unspecified (kind: unspecified). The reference does not
take them in the order of the component. The order changes the result only when two enchantments of
the tool have this effect. A parity test uses a tool with at most one such enchantment.

**BDR-XP-003** (`source_suggested`, semantic, confidence medium). A value effect changes an input `v`
at a level `L` as this table states. Each field is a level-based value at `L` (LOOT-NUM-021 of
`loot-conditions.md`). Each step is in f32, unless the row states otherwise.

| Type | Result | Draws |
|------|--------|-------|
| `add` | `v + value` | none |
| `multiply` | `v × factor` | none |
| `set` | `value` | none |
| `exponential` | the f32 narrow of the f64 product of `v` and the f64 power `base` to `exponent`. Each operand is the f64 widen of its f32 value. | none |
| `all_of` | each effect of `effects`, in the order of the list, each on the result of the one before | the draws of each effect, in that order |
| `remove_binomial` | `v − k`, with `k` as below | below |

For `remove_binomial`, let `n` be `v` and `p` the value of `chance`. Calculate `k` as follows:

- When `n ≤ 128`, or `n × p < 20`, or `n × (1 − p) < 20`: take one `F` for each integer `i` with
  `0 ≤ i < n`. `k` is the number of draws with `F < p`. For `n ≤ 0`, take no draw, and `k` is 0.
- Otherwise: let `m = floor(n × p)` and `s = sqrt(n × p × (1 − p))`. The products are in f32, and
  `floor`, `sqrt` and the steps below are in f64. Let `G` be one draw of the standard normal
  distribution. `k` is `m + G × s`, rounded to the nearest integer, with a half rounded toward
  positive infinity. Then limit `k` to the range from 0 to `n` converted to an i32 toward zero.

`G` is not a kind of LOOT-RND-006. A parity test compares only the distribution of `k`. This rule
states the value effects of `minecraft:block_experience` only.

## 2. Explosions

**BDR-EXL-001** (`source_suggested`, semantic, confidence medium). The explosion kind of an exploder
and the game rules give the block interaction mode, as this table states. The defaults are those of
`game_rule.json`.

| Kind | Block interaction mode | Exploders |
|------|------------------------|-----------|
| `none` | `keep` | an effect `minecraft:explode` with this kind; a `sulfur_cube` when `mob_griefing` is false |
| `block` | `destroy_with_decay` when `block_explosion_drop_decay` is true (default), else `destroy` | a bed, a respawn anchor, an `end_crystal` |
| `mob` | `keep` when `mob_griefing` is false (default true). Else `destroy_with_decay` when `mob_explosion_drop_decay` is true (default), else `destroy` | a `creeper`, a `fireball`, a `wither`, a `wither_skull` |
| `tnt` | `destroy_with_decay` when `tnt_explosion_drop_decay` is true (default false), else `destroy` | a primed TNT (`tnt`), a `tnt_minecart`, a `sulfur_cube` when `mob_griefing` is true |
| `trigger` | `trigger` | a `wind_charge`, a `breeze_wind_charge`, an entity with the effect `wind_charged` when it dies |

An effect `minecraft:explode` of an enchantment has the kind of its field `block_interaction`. For
example, the effect of Wind Burst has the kind `trigger`. A `sulfur_cube` explodes only when the game
rule `tnt_explodes` is true. The mode `trigger` changes no block to air, evaluates no block table
and pays no experience (EXP-BLK-003). A block can act on the hit in that mode, for example a door
that opens. This document does not state that action.

Only the mode `destroy_with_decay` gives the parameter `explosion_radius`. Its value is the power of
the explosion. In the mode `destroy`, each `survives_explosion` condition passes with no draw
(LOOT-CND-003 of `loot-conditions.md`). Each `explosion_decay` modifier then keeps each item with no
draw (LOOT-MOD-002 of `loot-functions.md`). In the mode `destroy_with_decay`, a `survives_explosion`
condition takes one `F` and passes when `F ≤ 1 / power`. For example, with the defaults, a creeper
of power 3 keeps a `survives_explosion` drop when `F ≤ 1/3`. With the defaults, a primed TNT keeps
each drop.

**BDR-EXL-002** (`source_suggested`, semantic, confidence medium). An explosion collects the drop
lists of its positions into one list of collected stacks. It takes the positions in the order of
EXP-BLK-003, and the stacks of each drop list in the order of the list. For each stack, it does these
steps:

1. Take each collected stack in the order of the list.
2. When the collected stack has the same item and the same components as the stack, and the sum of
   the two counts is not more than the maximum stack size of the item, move `min(m - c, n)` items
   from the stack to the collected stack. `c` is the count of the collected stack, `n` is the count
   of the stack, and `m` is the smaller of 16 and the maximum stack size. When `c` is `m` or more,
   move no item (BDR-DEV-001).
3. When the stack is empty, stop.
4. After the last collected stack, add the rest of the stack to the end of the list as a new
   collected stack. Its position is the origin block of the stack.

A new collected stack keeps its count, also a count above 16. After the last position, each
collected stack becomes one item entity, in the order of the list, as LOOT-OUT-002 states for its
position. When the game rule `block_drops` is false, no collected stack becomes an item entity
(LOOT-OUT-003). EXP-BLK-003 cites this rule for the merge.

Examples:

- 20 positions of `dirt`, each with 1 `dirt`, give the collected stacks 16 and 4.
- A collected stack of 14 `dirt` and a new stack of 4 `dirt` give the collected stack 16 and a new
  collected stack of 2.
- A collected stack of 14 `snowball` and a new stack of 4 `snowball` do not join, because 18 is more
  than 16, the maximum stack size of `snowball`. The new stack becomes a new collected stack of 4.
- An item with the maximum stack size 1 never joins a collected stack.

**BDR-EXL-003** (`source_suggested`, semantic, confidence medium). EXP-BLK-003 states the steps of
an explosion at one position in the mode `destroy` or `destroy_with_decay`. This rule adds these
facts to those steps:

- Before step 1, a creaking heart pays the experience of its row of BDR-XP-001.
- In step 1, the experience is that of BDR-XP-001, with the empty stack as the tool.
- In step 2, the drop list goes into the collected stacks (BDR-EXL-002).
- In step 3, the held items drop at once (BDR-CNT-001). After them, each block that the change makes
  invalid is a neighbour removal. It drops and pays at once, with the context of a neighbour
  removal: no `explosion_radius` and no `this_entity`. Its stacks do not join the collected stacks.

The block `tnt` skips steps 1 and 2 (EXP-BLK-003, EXP-TNT-003). A position that is air when the
explosion reaches it drops nothing. For example, take a torch on a stone, both in the set. When the
stone comes first in the order, the torch is a neighbour removal. Its item has no decay and does not
join the collected stacks. When the torch comes first, its table gets `explosion_radius`, and its
stack joins the collected stacks.

## 3. Held items

**BDR-CNT-001** (`source_suggested`, semantic, confidence medium). The block
entities of these types of the registry `block_entity_type` hold items that drop at a removal:
`barrel`, `blast_furnace`, `brewing_stand`, `campfire`, `chest`, `chiseled_bookshelf`, `crafter`,
`decorated_pot`, `dispenser`, `dropper`, `furnace`, `hopper`, `jukebox`, `lectern`, `shelf`, `smoker`
and `trapped_chest`. A `shulker_box` keeps its items in its own item (BRK-DRP-003), and its held
items do not drop.

The held items drop at each removal of the block: a survival break with or without a harvest, a
creative break, a neighbour removal, a command destroy, and an explosion of the mode `destroy` or
`destroy_with_decay`. `setblock` and `fill` in each other mode replace the block, and the held items
do not drop. The game rule `block_drops` has no effect on the held items (BRK-LOOT-005).

The held items are not part of a drop list. Thus an explosion does not decay them and does not merge
them (BDR-EXL-002).

**BDR-CNT-002** (`source_suggested`, semantic, confidence medium). Each held stack becomes item
entities as IEN-SPN-005 states, with the pickup delay 0, except for these two types:

- `lectern`: the book is one item entity at `(x + 0.5 + 0.25 × dx, y + 1, z + 0.5 + 0.25 × dz)`.
  `(dx, dz)` is the unit step of the property `facing` of the lectern.
- `jukebox`: the disc is one item entity. Its position depends on the block above the jukebox:
  - When that block is in the block tag `minecraft:jukebox_containers` and its property `type` is
    `single`, the position is `(x + 0.5, y + h, z + 0.5)`. `h` is the f64 widen of the f32 value
    2.2, 2.200000047683716.
  - Otherwise, take two draws `F₁` and `F₂`, in this order. The position is `(x + 0.5 + a, y + 1.01,
    z + 0.5 + b)`, with `a = (F₁ − 0.5) × 0.7` and `b = (F₂ − 0.5) × 0.7` in f32.

`(x, y, z)` is the block position. The item entity of a lectern or a jukebox has the velocity of the
default form (IEN-SPN-008) and the pickup delay 10.

## 4. Order

**BDR-ORD-001** (`source_suggested`, semantic, confidence medium). The `add_entity` packets of one
removal reach a client in the order of this table. Each step can be empty. This rule owns the order
of the orbs against the item entities. LOOT-OUT-004 and BRK-LOOT-006 cite it for the orbs of a
creaking heart, which come first.

| Cause | Order |
|-------|-------|
| survival break | 1. the orbs of a creaking heart (BRK-LOOT-006). 2. the held items. 3. the items and the orbs of each neighbour removal (BRK-LOOT-003). 4. the drop list (LOOT-OUT-004). 5. the orbs of BRK-LOOT-004. |
| creative break | 1. the item of a shulker box with contents (BRK-DRP-003) or of a beehive (BRK-DRP-008). 2. the held items (BRK-DRP-007). 3. the items and the orbs of each neighbour removal (BRK-DRP-004). |
| neighbour removal, command destroy | 1. the drop list. 2. the orbs. 3. the held items. 4. the items and the orbs of each neighbour removal that this removal causes. |
| explosion | For each position, in the order of EXP-BLK-003: 1. the orbs of a creaking heart. 2. the orbs of step 1 of EXP-BLK-003. 3. the held items. 4. the items and the orbs of each neighbour removal. 5. the entity of step 4 of EXP-BLK-003, for example the primed TNT of EXP-TNT-003. After the last position: 6. the collected stacks (BDR-EXL-002). |

The held items of a chest come before the chest, as step 2 and
step 4 of a survival break state.

## 5. Intentional deviations and unresolved items

### 5.1 Intentional deviations

**BDR-DEV-001.**

- **Rule**: BDR-EXL-002, step 2.
- **Reference**: a collected stack with a count above 16 that meets a stack of the same item keeps
  16 items. Its other items join the stack, and they become a new collected stack. The total count
  does not change.
- **Visible to the client**: no, with the vanilla data pack, because no block table of it gives a
  stack of more than 16 items of an item that can join. With a data pack that gives such a stack, the
  player sees other counts on the item entities of one explosion.
- **Test**: a differential test that finds this difference accepts it when a collected stack had
  more than 16 items. It then compares only the total count of each item.

### 5.2 Unresolved

- **U1**: item U1 of `block-loot.md`, the context of the part of a door or a bed that drops when the
  break hits the other part. The reference suggests the context of a neighbour removal: the empty
  stack and no `this_entity`. Experiment E7 uses a data pack whose table reads the tool and the
  entity, so it can show the context.

## 6. Experiments

None of these experiments ran. The tools of `docs/spec/README.md` start the vanilla server only when
the environment accepts the Minecraft EULA (`MINECRAFT_EULA=true`). The environment of the author did
not accept it, and the author did not accept it for the owner.

Use the vanilla server of the game version above, on a flat world. Read an orb with `data get entity
<orb> Value` and an item entity with `data get entity <item> Item`. Before each step, kill each item
entity and each orb. For E5, run `tick freeze` before the step, and run `tick step 1` after the
command that starts the explosion. Then read the item entities before they move or merge. For E6 and
E11, record the `add_entity` packets with `tools/mcrs_minecraft_client_recorder`.

| Id | Steps | Rules |
|----|-------|-------|
| E1 | Run `setblock` with `spawner` and then `setblock` with `air` and `destroy`, 400 times. Sum the orb values of each run, and count each sum. With a survival player, break a spawner 20 times with a diamond pickaxe with Silk Touch. Sum the orb values of each break. | BDR-XP-002, BDR-XP-001 |
| E2 | Do the command destroy 100 times each on `coal_ore`, `diamond_ore`, `sculk_sensor` and `creaking_heart` with `natural=true`. Sum the orb values of each run. | BDR-XP-001 |
| E3 | In creative mode, break `coal_ore`, `spawner` and `creaking_heart` with `natural=true`. Count the orbs. | BDR-XP-001 |
| E4 | Put `diamond_ore` and `creaking_heart` with `natural=true` in a stone floor. Summon `tnt` with `fuse` 0 and the UUID of a survival player as `owner`. Repeat with a creative owner, with no owner, and with `block_drops` false. Sum the orb values. | BDR-XP-001 |
| E5 | Fill a row of 20 `dirt`, and explode a `tnt` at its centre. Read the count and the position of each item entity. Add a data pack whose table of `dirt` gives 3 `snowball`, and repeat. | BDR-EXL-002 |
| E6 | Put a chest with 64 `stone` in slot 0 and a torch on a stone. Explode a `tnt` next to both, 20 times. Record the packets. Put `creaking_heart` with `natural=true` and `coal_ore` next to a `tnt` with a survival owner, and record again. Set `block_drops` to false and repeat. | BDR-EXL-003, BDR-CNT-001, BDR-ORD-001 |
| E7 | Add a data pack whose tables of `oak_door` and `red_bed` have no `match_block` condition. Add a pool that gives `diamond` with a `match_tool` condition for `diamond_pickaxe`, and a pool that gives `emerald` with an `entity_properties` condition on `this` with an empty predicate. Break the upper part of a door and the foot of a bed with a diamond pickaxe. Read each item entity and its position. | U1, LOOT-SRC-006 |
| E8 | Fill a 9 by 9 by 3 box with `dirt`. Explode a `creeper` (`Fuse` 0, `ignited` true) at its centre. Then run `damage` with the amount 1 on an `end_crystal` at its centre. Count the destroyed positions and the `dirt` items. Repeat with `mob_explosion_drop_decay` and `block_explosion_drop_decay` false, and with `mob_griefing` false. | BDR-EXL-001 |
| E9 | Add a data pack with the enchantment `example:xp`. Its effect `minecraft:block_experience` has two conditional effects: `add` with `linear` (`base` 2, `per_level_above_first` 1), then `multiply` with `factor` 3. Give a survival player a netherite pickaxe with `example:xp` at level 1, and break `coal_ore`, `iron_ore`, `stone` and `spawner` 50 times each. Repeat at level 2. Sum the orb values of each break. Expected at level 1: 6, 9 or 12 for `coal_ore`, 6 for `iron_ore`, no orb for `stone`, and 15 to 43 for `spawner`. | BDR-XP-002, BDR-XP-003 |
| E10 | Put a lectern with a book for each value of `facing`, and do the command destroy. Read `Pos` and `PickupDelay` of the item entity. Put a jukebox with a disc, and do the command destroy 20 times. Repeat with a single `chest` above the jukebox, and with one half of a double chest above it. Do the command destroy on a chest with 64 `stone`, and read `PickupDelay`. | BDR-CNT-002 |
| E11 | With a survival player and an iron pickaxe, break a chest with 3 `diamond`, with a torch on it. Break `coal_ore` with a torch on it, and `creaking_heart` with `natural=true` and a torch on it. Do the command destroy on the chest and the `coal_ore` setups. In creative mode, break the chest setup and a `beehive` with `honey_level=5`. Record the packets, and note the entity type of each `add_entity`. | BDR-ORD-001 |
