# Block loot: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`.

This document states what a block drops when it breaks. It states how the server finds the loot
table of the block and which values the table reads. It also states how the server evaluates the
table, and how each result becomes an item entity. It states only the parts of the loot table format
that the block tables of the vanilla data pack use. It does not prescribe an architecture.

Scope: the loot tables of the type `minecraft:block`, for the causes of section 2.
`block-breaking.md` states when a survival break drops loot (BRK-LOOT-001, BRK-LOOT-005), the
experience (BRK-LOOT-004) and the packets. `item-entity.md` states the start values of a block drop
(IEN-SPN-001, row "drop of a block") and `entity-physics.md` states the start position and velocity
(PHY-CMP-025). `registry-fields.md`, section 9, states how the server decodes a loot table and each
part of it. This document refers to the rules of these documents by id. Section 8 lists what this
document does not state.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**: as `login.md`, section 0.1, defines them.
- **Stack**, **empty stack**, **count**, **maximum stack size**: as `item-stack.md`, section 0.1,
  defines them.
- **Survival break**, **harvest**, **main-hand item**: as `block-breaking.md`, section 0.1, defines
  them.
- **Loot table**: a value of the registry `minecraft:loot_table` (REG-FLD-799).
- **Block table**: the loot table that the block definition names in its component `minecraft:loot`
  (`assets/mcrs/block_definition/README.md`). A block without this component has no block table.
- **Pool**, **entry**, **condition**, **modifier**: a loot pool (REG-FLD-800), a loot pool entry
  (REG-FLD-790), a loot condition (REG-FLD-700) and an item modifier (REG-FLD-741).
- **Leaf entry**: an entry of the type `item`, `dynamic`, `empty`, `tag`, `loot_table` or `slots`.
- **Composite entry**: an entry of the type `alternatives`, `sequence` or `group`.
- **Context**: the values that one evaluation of a table can read (LOOT-CTX-001).
- **Tool**: the stack of the context value `tool`. The tool can be the empty stack.
- **Level of an enchantment**: the level of that enchantment in the component
  `minecraft:enchantments` of the tool. The level is 0 when the tool does not have the enchantment.
- **Draw**: one random value that an evaluation takes. `r` is a uniform f32 value in `[0, 1)`. `I(a,
  b)` is a uniform integer in `a..b`; when `a ≥ b`, `I(a, b)` is `a` and takes no draw.
- **Drop list**: the ordered list of stacks that one evaluation of a table gives.
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
| `LOOT-CTX` | the context |
| `LOOT-EVL` | the evaluation of a table, a pool and an entry |
| `LOOT-CND` | the conditions |
| `LOOT-MOD` | the modifiers and the number providers |
| `LOOT-OUT` | the drop list and the item entities |
| `LOOT-RND` | the random values |

## 1. What the block tables use

**LOOT-SRC-001** (`black_box_confirmed`, semantic, confidence high). The block tables of the vanilla
data pack use only the types of this table. An implementation that supports these types drops the
vanilla items for each block. An implementation can reject or ignore another type in a block table.

| Part | Types | Rules |
|------|-------|-------|
| entry | `item`, `alternatives`, `dynamic` | LOOT-EVL-004 to LOOT-EVL-007 |
| condition | `survives_explosion`, `match_block`, `table_bonus`, `random_chance`, `any_of`, `all_of`, `inverted`, `match_tool`, `location_check`, `entity_properties`, a key of the registry `predicate` | section 4 |
| modifier | `set_count`, `explosion_decay`, `copy_components`, `apply_bonus`, `limit_count`, `copy_state`, a list of modifiers | section 5 |
| number provider | a bare number, `uniform`, `binomial` | LOOT-MOD-008 |

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
these values:

| Value | Content | Present |
|-------|---------|---------|
| `origin` | the centre of the origin block, `(x + 0.5, y + 0.5, z + 0.5)` | always |
| `block_state` | the state of LOOT-SRC-003 | always |
| `tool` | LOOT-SRC-002 | always |
| `this_entity` | LOOT-SRC-002 | for some causes |
| `block_entity` | the block entity of the origin block before the break | when the block had one |
| `explosion_radius` | LOOT-SRC-002 | for some causes |

The luck of the context is 0. A condition or a modifier that reads a value which is absent acts as
its rule states for that case.

**LOOT-EVL-001** (`source_suggested`, semantic, confidence medium). An evaluation of a table makes
the drop list. For each pool, in the order of `pools`, the server does LOOT-EVL-002. Each stack that
a pool gives goes through the modifier of the table, when the table has one. Then the stack goes to
the end of the drop list.

**LOOT-EVL-002** (`source_suggested`, semantic, confidence medium). A pool does these steps:

1. Test the condition of the pool. When it fails, the pool gives nothing.
2. Let `n` be the value of `rolls` plus `floor(bonus_rolls × luck)`. With the luck 0, `n` is the
   value of `rolls`.
3. Do LOOT-EVL-003 `n` times. Each stack that a roll gives goes through the modifier of the pool,
   when the pool has one.

**LOOT-EVL-003** (`source_suggested`, semantic, confidence medium). One roll of a pool does these
steps:

1. Expand each entry of `entries`, in order (LOOT-EVL-004). Collect the leaf entries that the
   expansions select. These are the candidates.
2. The weight of a candidate is `max(floor(weight + quality × luck), 0)`. Remove each candidate with
   the weight 0. With the luck 0, the weight is the field `weight`, default 1.
3. When no candidate remains, the roll gives nothing.
4. When one candidate remains, it makes its stacks (LOOT-EVL-006). This takes no draw.
5. When more candidates remain, let `t` be the sum of their weights. Take `k = I(0, t - 1)`. Go
   through the candidates in order, and subtract the weight of each from `k`. The first candidate
   for which `k` becomes less than 0 makes its stacks.

Thus one roll makes the stacks of at most one leaf entry.

**LOOT-EVL-004** (`source_suggested`, semantic, confidence medium). An expansion of an entry selects
leaf entries and gives a result, true or false. It first tests the condition of the entry. When the
condition fails, the expansion selects nothing and gives false. Otherwise:

- a leaf entry selects itself and gives true;
- `alternatives` expands its children in order, until one gives true. It gives true when a child
  gave true, and false when no child did, or when it has no children;
- `sequence` expands its children in order, until one gives false. The leaf entries that the earlier
  children selected stay selected. It gives false when a child gave false, and true otherwise;
- `group` expands each child. It gives true.

The expansion tests each condition one time in each roll.

**LOOT-EVL-005** (`source_suggested_black_box_confirmed`, semantic, confidence high). An
`alternatives` entry thus selects the first child whose condition passes. For example, the table of
`stone` gives `stone` when the tool can Silk Touch, and otherwise `cobblestone`. Result R37 of
`block-breaking.md` shows both.

**LOOT-EVL-006** (`source_suggested`, semantic, confidence medium). A leaf entry makes its stacks:

- `item` makes one stack of the item `name`, with the count 1 and no component patch;
- `empty` makes nothing;
- `dynamic` makes the stacks of LOOT-EVL-007.

Each stack goes through the modifier of the entry, when the entry has one. Then it goes to the pool.

**LOOT-EVL-007** (`source_suggested`, semantic, confidence medium). The only `dynamic` name in a
block table is `minecraft:sherds`, in the table of `decorated_pot`. It makes four stacks, one for
each side of the pot, in the order back, left, right, front. The stack of a side is the item of that
side in the component `minecraft:pot_decorations` of the block entity, with the count 1. A side
without a decoration gives `brick`. Without a block entity, it makes nothing. The entry has the
condition that the pot is cracked, so a pot that is not cracked drops itself (LOOT-MOD-006).

## 4. Conditions

A condition passes or fails. A condition field that is absent passes.

**LOOT-CND-001** (`source_suggested`, semantic, confidence medium). A key of the registry
`predicate` is the condition that the registry entry holds. For example,
`minecraft:tool/can_silk_touch` is a `match_tool` condition that passes for a tool with Silk Touch
at level 1 or more. `minecraft:tool/can_shear` passes for the tool `shears`.

**LOOT-CND-002** (`source_suggested`, semantic, confidence medium). `inverted` passes when its
`term` fails. `all_of` tests its `terms` in order and fails at the first term that fails. It passes
when each term passes, and also when it has no terms. `any_of` tests its `terms` in order and passes
at the first term that passes. It fails when no term passes, and also when it has no terms. A term
that is not tested takes no draw.

**LOOT-CND-003** (`source_suggested`, semantic, confidence medium). `survives_explosion` passes when
the context has no `explosion_radius`. Otherwise it takes a draw `r` and passes when `r ≤ 1 /
explosion_radius`.

**LOOT-CND-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). `table_bonus`
reads the level `L` of its `enchantment`. Let `c` be the element `min(L, len - 1)` of `chances`,
where `len` is the number of elements. The condition takes a draw `r` and passes when `r < c`. For
example, `gravel` drops `flint` with the chance 0.1 for an empty hand. Result L2 shows the chances
of `oak_leaves`.

**LOOT-CND-005** (`source_suggested`, semantic, confidence medium). `random_chance` takes a draw `r`
and passes when `r < chance`. `chance` is a float provider (LOOT-MOD-008).

**LOOT-CND-006** (`source_suggested`, semantic, confidence medium). `match_block` passes when the
context state is a block of `blocks` and each property of `state` has the value that `state` gives.
In a block table, `blocks` is the block of the table, and each value of `state` is one exact value
as a string. For example, `{"half": "lower"}` and `{"type": "double"}`.

**LOOT-CND-007** (`source_suggested`, semantic, confidence medium). `match_tool` passes when the
tool matches `predicate`. It fails when the context has no tool. The block tables use two parts of
the item predicate, and neither part matches the empty stack:

- `items`: an item, a tag of items or a list. The tool must be one of them. For example
  `"#minecraft:cluster_max_harvestables"` in the table of `amethyst_cluster`.
- `predicates`, key `minecraft:enchantments`: a list. For each element, the tool must have one of
  its `enchantments` at a level in its `levels` range.

**LOOT-CND-008** (`source_suggested`, semantic, confidence medium). `location_check` reads the block
at the origin block plus `(offsetX, offsetY, offsetZ)`, each 0 by default. The block tables use only
the `block` part of the location predicate. The block there must be one of `blocks` and match
`state`, as in LOOT-CND-006. The tables of tall plants, for example `tall_grass`, use it to test the
other part (LOOT-SRC-006). The server reads the block at the time of the evaluation.

**LOOT-CND-009** (`source_suggested_black_box_confirmed`, semantic, confidence high).
`entity_properties` with `entity` `this` and an empty `predicate` passes when the context has a
`this_entity`, and fails when it does not. The only block table with it is `snow`. Thus a snow layer
drops `snowball` or `snow` when a player breaks it, and drops nothing for a neighbour change or for
`setblock` with `destroy`. Result L5 shows both cases.

## 5. Modifiers and number providers

A modifier changes one stack. The common field `condition` (REG-FLD-742) applies to each modifier:
when it fails, the modifier does not change the stack. A list of modifiers applies each modifier in
order. In the rules of this section, `c` is the count of the stack before the modifier.

**LOOT-MOD-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). `set_count`
sets the count to `v`, or to `c + v` when `add` is true. `v` is the value of the int provider
`count`. The count can become 0 or less. For example, the tables of `glow_lichen` and `sculk_vein`
add 1 for each face and then `-1`. Result L3 shows the counts below 1 of `brown_mushroom_block`,
which make no item.

**LOOT-MOD-002** (`source_suggested`, semantic, confidence medium). `explosion_decay` does nothing
when the context has no `explosion_radius`. Otherwise it takes `c` draws, one for each item, and
keeps each item when its draw `r ≤ 1 / explosion_radius`. The count becomes the number of items
kept.

**LOOT-MOD-003** (`source_suggested_black_box_confirmed`, semantic, confidence high). `limit_count`
limits the count to the range `limit`: a bare int is an exact range, and an object has `min` and
`max`, each optional. A count below `min` becomes `min`. A count above `max` becomes `max`. For
example, the table of `glowstone` limits `glowstone_dust` to `1..4`, and the table of
`brown_mushroom_block` limits the count to 0 or more. Result L3 shows both.

**LOOT-MOD-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). `apply_bonus`
reads the level `L` of its `enchantment`. When the context has no tool, it does nothing. Otherwise
it applies one formula, also with `L = 0`:

| `formula` | `parameters` | New count | Draws |
|-----------|--------------|-----------|-------|
| `minecraft:ore_drops` | none | `c` when `L = 0`; otherwise `c × (max(I(0, L + 1) - 1, 0) + 1)` | 1 when `L > 0` |
| `minecraft:uniform_bonus_count` | `bonusMultiplier` `m` | `c + I(0, m × L)` | 1 when `m × L > 0` |
| `minecraft:binomial_with_bonus_count` | `extra` `e`, `probability` `p` | `c` plus the number of draws `r < p`, for `L + e` draws | `L + e` |

For `ore_drops` with `L > 0`, the factor is 1 with the chance `2 / (L + 2)`, and each value of `2..L
+ 1` with the chance `1 / (L + 2)`. Result L1 shows `ore_drops` with `L = 3`, and result L3 shows
`binomial_with_bonus_count` with `L = 0`.

**LOOT-MOD-005** (`source_suggested`, semantic, confidence medium). `copy_state` adds to the
component `minecraft:block_state` of the stack each property of `properties` that the context state
has, with the value of the context state. Other entries of the component stay. Without a context
state, it does nothing. For example, the tables of `beehive` and `bee_nest` copy `honey_level`.

**LOOT-MOD-006** (`source_suggested_black_box_confirmed`, semantic, confidence high).
`copy_components` with `source` `block_entity` copies components of the block entity of the context
to the stack. It copies each component type of `include` that the block entity has, except each type
of `exclude`. Without `include`, it copies each type that the block entity has, except each type of
`exclude`. Without a block entity, it does nothing. For example, the table of `chest` copies
`minecraft:custom_name`, so a named chest drops a named item. Result L7 shows it.

**LOOT-MOD-007** (`source_suggested`, semantic, confidence medium). The components of a block entity
are the values that the block entity stores as components. Examples are its custom name, its
contents, its lock and its bees. No specification states them for each block entity yet. Item U2
records this dependency.

**LOOT-MOD-008** (`source_suggested_black_box_confirmed`, semantic, confidence high). The block
tables use these number providers:

| Provider | Int value | Float value | Draws |
|----------|-----------|-------------|-------|
| a bare number `v` | `v` | `v` | 0 |
| `uniform` with `min` `a` and `max` `b` | `I(a, b)` | `a + r × (b - a)` | 1, or 0 for an int with `a ≥ b` |
| `binomial` with `n` and `p` | the number of draws `r < p`, for `n` draws | the int value | `n` |

The `min`, the `max`, `n` and `p` are number providers too. For example, the table of `melon_stem`
gives `binomial` with `n` 3 and `p` `0.06666667` for the age 0. Results L2 and L3 show `uniform`.

## 6. The drop list and the item entities

**LOOT-OUT-001** (`source_suggested`, semantic, confidence medium). The server divides each stack of
the drop list, in order, before it makes item entities. A stack with a count less than its maximum
stack size stays one stack. A larger stack becomes stacks of the maximum stack size, and the rest
when it is not 0. For example, 70 `snowball` with the maximum 16 become four stacks of 16 and one of
6.

**LOOT-OUT-002** (`source_suggested_black_box_confirmed`, semantic, confidence high). Each stack of
LOOT-OUT-001 that is not empty becomes one item entity, in the order of the list. A stack with the
count 0 or less makes nothing. The start values are the row "drop of a block" of IEN-SPN-001. The
position and the velocity are those of PHY-CMP-025 at the origin block, and the pickup delay is 10.
Result R37 of `block-breaking.md` shows one item entity for each stack.

**LOOT-OUT-003** (`source_suggested`, semantic, confidence medium). When the game rule `block_drops`
is false, no stack of a block table becomes an item entity (BRK-LOOT-005).

**LOOT-OUT-004** (`source_suggested`, semantic, confidence medium). The order of the item entities
of one break is the order of the drop list. `add_entity` reaches the breaker in that order. The
experience orbs come after the items (BRK-LOOT-001). The item entities of a block that loses its
support come before those of the broken block (BRK-LOOT-003).

## 7. Random values

**LOOT-RND-001** (`design`, semantic). Each draw of an evaluation is a keyed random value. The
stream is a function of the world seed, the id of the block table and the origin block. It is also a
function of the game tick of the evaluation and of the cause. The `i`-th draw of one evaluation is
the `i`-th value of its stream. Thus a drop does not depend on the order of the breaks in a tick or
on the thread count. Reason: the keyed random values of `entity-ai-timing.md` (AIT-LAG-008) and
`entity-physics.md` (PHY-CMP-025). LOOT-DEV-001 records the difference from the vanilla server.

**LOOT-RND-002** (`source_suggested`, semantic, confidence medium). A parity test compares the
distribution of the drops over many breaks. The support, the probability of each value and the
independence of the draws of sections 3 to 5 are the behaviour. The generator and the number of
draws are not. Kind of the generator: unspecified.

**LOOT-RND-003** (`source_suggested`, semantic, confidence medium). The position and the velocity of
an item entity of a drop take their own random values (PHY-CMP-025, PHY-DEV-013). They are not draws
of the evaluation.

## 8. What this document does not state

- How the server decodes a loot table and its parts: `registry-fields.md`, section 9.
- The types that no block table uses. Examples are the entries `tag`, `loot_table` and `slots`, and
  the conditions `killed_by_player`, `entity_scores`, `damage_source_properties`, `weather_check`,
  `time_check` and `enchantment_active_check`. Most item modifiers are also such types.
- The loot tables of other types, for example `entity`, `chest`, `fishing`, `archaeology`,
  `block_use`, `block_interact`, `shearing` and `vault`. The loot of a trial spawner.
- The advancements, and the statistic of a mined block.
- An explosion: which explosions have decay, the merge of the stacks of one explosion, and the
  experience of an explosion. This document states only what `explosion_radius` changes in a table
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
  `minecraft:blocks/stone`. The vanilla server keeps one random stream for each sequence id in the
  dimension. It seeds the stream from the world seed and an MD5 hash of the id, and saves its state
  with the world. Each evaluation of the table continues the stream. The drops thus depend on the
  order of all earlier evaluations of the same table.
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
  table of a door or a bed reads the tool or the entity, so no experiment can show it.
- **U2**: the full list of components of a block entity for LOOT-MOD-007. The required result does
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
| E2 | Evaluate the table of `oak_leaves` 2000 times with an empty hand. | L2 | LOOT-CND-004, LOOT-MOD-008 |
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
