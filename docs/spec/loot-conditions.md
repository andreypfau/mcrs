# Loot conditions and context number providers: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the result of each loot condition and of each context number provider. For
each type, it states the context parameters and the world values that the type reads. It states
the result when a value is absent, and each random draw that the type takes.

The scope of this document is:

- the condition types of REG-FLD-702 to REG-FLD-719;
- a key or a tag of the registry `predicate`;
- the context int providers and the context float providers of REG-FLD-722 to REG-FLD-737;
- the range predicates of REG-FLD-720 and REG-FLD-721;
- the scoreboard name provider of REG-FLD-738;
- the resolvable numbers of REG-FLD-739 and REG-FLD-740;
- the level-based value of REG-FLD-386.

`registry-fields.md`, sections 6 and 9, states how the server decodes each of these types. This
document does not repeat the decode.

## 0. Conventions

### 0.1 Terms

- **Condition**: a loot condition (REG-FLD-700). One test of a condition gives the result
  **pass** or **fail**.
- **Context**: the values that one evaluation of a loot table, an item modifier or a condition can
  read. `loot.md` states the context, its parameters and the sets of parameters of each use
  (LOOT-CTX).
- **Parameter**: one value of the context, by its id, for example `this_entity` or `origin`. A
  parameter is **present** or **absent**.
- **Dimension of the evaluation**: the dimension of the context. The weather, the clocks, the
  environment attributes and the blocks come from this dimension.
- **Rain strength**, **thunder strength**: as `command-weather.md`, section 0, defines them.
- **Provider**: a context int provider (REG-FLD-722) or a context float provider (REG-FLD-723).
  An **int provider** gives an i32. A **float provider** gives an f32.
- **Key of a provider**: a key of the registry `context_int_provider` or `context_float_provider`
  in a provider field. The provider of the key is the value of that registry entry.
- **Evaluation of a provider**: the computation of the value of a provider. It gives a value or an
  **evaluation error**.
- **Read of a provider**: an evaluation of a provider by a condition, a range predicate, an item
  modifier or a component. LOOT-NUM-001 states how a read changes an evaluation error.
- **f32**, **f64**, **i32**, **i64**, **widen**, **narrow**, `toI32`, `toI64`, `toF32`, **step
  order**: as `numeric.md`, section 0.1, defines them.
- **Finite**: an f32 that is not a NaN and not an infinity.
- **Generator**: the random generator of the evaluation. `loot.md` states which generator an
  evaluation uses, and how the generator makes each draw (LOOT-RND).
- **Draw**: one value that the generator gives. Each draw advances the generator. This document
  uses the float draw `F` and the bounded draw `B(n)`, with the notation of LOOT-RND-006
  (`loot.md`).
- **Entity target**: the enum string of a field `entity` or `target` (REG-FLD-706). LOOT-CND-020
  gives the parameter of each target.
- **Level of an enchantment on an item**, **level of an enchantment on an entity**: as
  LOOT-CND-022 states.
- **Score holder**: a name with scores on the scoreboard of the server (LOOT-CND-023).
- **Entity predicate**, **location predicate**, **damage source predicate**, **item predicate**,
  **block predicate**: as `predicates.md` (PRED) states them. This document states only the
  values that a condition gives to them.

An integer range `a..b` includes `a` and `b`.

### 0.2 Observers

Each rule of this document has these observers, unless the rule states other observers:

- a data pack: the drops of a loot table, the result of `execute if predicate`, and the values
  of the items that an item modifier changes;
- a user of the library: the generator state after an evaluation in the exact mode of `loot.md`.
  Each draw of each rule changes that state. The order of the draws is thus part of each rule.

In the keyed mode of `loot.md` (LOOT-RND-002), only the distribution of the results is compared.
The block source (LOOT-RND-001 of `block-loot.md`) and each other source without a saved seed
(LOOT-RND-009) use that mode. In that mode, the order and the count of the draws are not compared.

### 0.3 Provenance

The labels are those of `registry.md`, section 0.3:

- `black_box_confirmed`: shown by a game-produced artefact that the rule cites.
- `source_suggested_black_box_confirmed`: taken from the reference, and consistent with a cited
  artefact.
- `source_suggested`: taken from the reference, with no artefact yet. The confidence is at most
  medium.
- `design`: a choice of mcrs. The rule states the reason.
- `unresolved`: not decided.

The reference that the author read is the game version above. Each rule states its kind, as
`docs/spec/README.md` defines it.

### 0.4 Artefacts cited

- The data pack of the client jar: `data/minecraft/predicate/` (3 entries),
  `data/minecraft/context_int_provider/` and `data/minecraft/context_float_provider/`, and the
  conditions inside `data/minecraft/loot_table/`, `advancement/` and `enchantment/`.
  `tools/mcrs_minecraft_fixtures` installs it.

The vanilla data pack uses these condition types: `all_of`, `any_of`, `inverted`,
`random_chance`, `random_chance_with_enchanted_bonus`, `entity_properties`, `killed_by_player`,
`survives_explosion`, `match_block`, `match_tool`, `table_bonus`, `damage_source_properties`,
`location_check`, `weather_check` and `enchantment_active_check`. It uses no `entity_scores`,
`time_check`, `int_value_check`, `float_value_check` or `environment_attribute_check`. In its
providers, it uses `uniform`, `binomial`, `conditional`, `number_dispatcher`, `weighted_list`,
`div` and `enchantment_level`.

### 0.5 ID prefixes

| Prefix | Subject |
|--------|---------|
| `LOOT-CND` | the conditions, the range predicates and the common values that conditions read |
| `LOOT-NUM` | the providers, the level-based values and the resolvable numbers |
| `LOOT-CDV` | the intentional deviations of this document |

LOOT-CND-001 to LOOT-CND-009 have the ids of `block-loot.md`, section 4. This document states
them for each loot table type, not only for block tables.

## 1. Summary of the conditions

A condition field that is absent passes. A condition reads only the parameters of this table,
and the parameters that its nested conditions and providers read. The validation of `loot.md`
checks these parameters against the parameter set of the use.

| Type | Parameters | Draws | Rule |
|------|------------|-------|------|
| a key of `predicate` | those of the entry | those of the entry | LOOT-CND-001 |
| `inverted`, `all_of`, `any_of` | those of the terms | those of the terms that it tests | LOOT-CND-002 |
| `survives_explosion` | `explosion_radius` | `F` when the radius is present | LOOT-CND-003 |
| `table_bonus` | `tool` | `F` | LOOT-CND-004 |
| `random_chance` | none | those of `chance`, then `F` | LOOT-CND-005 |
| `match_block` | `block_state`; also `block_entity` when the block predicate reads it | none | LOOT-CND-006 |
| `match_tool` | `tool` | none | LOOT-CND-007 |
| `location_check` | `origin` | none | LOOT-CND-008 |
| `entity_properties` | `origin`, the parameter of `entity` | none | LOOT-CND-009 |
| `random_chance_with_enchanted_bonus` | `attacking_entity` | `F` | LOOT-CND-010 |
| `killed_by_player` | `last_damage_player` | none | LOOT-CND-011 |
| `entity_scores` | the parameter of `entity` | those of the ranges that it reads | LOOT-CND-012 |
| `damage_source_properties` | `origin`, `damage_source` | none | LOOT-CND-013 |
| `weather_check` | none | none | LOOT-CND-014 |
| `time_check` | none | those of the range | LOOT-CND-015 |
| `int_value_check` | none | those of `value` and of the range | LOOT-CND-016 |
| `float_value_check` | none | those of `value` and of the range | LOOT-CND-017 |
| `enchantment_active_check` | `enchantment_active` | none | LOOT-CND-018 |
| `environment_attribute_check` | `origin` when the attribute is positional | none | LOOT-CND-019 |

## 2. Conditions

### LOOT-CND-001: A key of the registry `predicate`

- **Inputs:** a key of the registry `predicate` in a condition field.
- **Result:** the result of the condition that the registry entry holds. The test of a key is
  the test of that condition, with the same context, the same draws and the same order.
- **Load:** a key that refers, through its entry, back to itself fails the load. This includes a
  key in a nested condition, a term and a provider. The reference also reports this as a fatal
  problem, except through the field `chance` of `random_chance`. LOOT-CDV-004 records that case.
- **Example:** `minecraft:tool/can_silk_touch` is a `match_tool` condition. It passes for a tool
  with Silk Touch at level 1 or more. `minecraft:tool/can_shear` passes for the tool `shears`.
  `minecraft:block/fast_cooking` is a `match_block` condition for `smoker` and `blast_furnace`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-002: `inverted`, `all_of` and `any_of`

- **Inputs:** `term` for `inverted`. `terms` for `all_of` and `any_of`: a list of conditions or
  keys, or a tag of the registry `predicate` (REG-FLD-701).
- **Result:**
  - `inverted` tests `term` one time. It passes when `term` fails, and fails when `term` passes.
  - `all_of` tests the terms in order. It fails at the first term that fails, and does not test
    the terms after it. It passes when each term passes. It passes when it has no terms.
  - `any_of` tests the terms in order. It passes at the first term that passes, and does not
    test the terms after it. It fails when no term passes. It fails when it has no terms.
- **Order:** the order of a list is the order of the file. The order of a tag is the member
  order of REG-TAG-005.
- **Draws:** the draws of each term that the condition tests, in the order of the tests. A term
  that the condition does not test takes no draw.
- **Example:** `any_of` with the terms `random_chance` 0.5 and `random_chance` 0.5 takes one `F`
  when the first `F` is less than 0.5, and two `F` otherwise.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-003: `survives_explosion`

- **Reads:** the parameter `explosion_radius`, an f32 `R`.
- **Result:** when `explosion_radius` is absent, the condition passes and takes no draw.
  Otherwise it takes `F` and passes when `F ≤ 1 / R`. The division is in f32.
- **Edge cases:** `R = 0` gives `1 / R = +∞`, so the condition passes. A negative `R` gives a
  negative bound, so the condition fails.
- **Draws:** one `F`, only when `explosion_radius` is present.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-004: `table_bonus`

- **Reads:** the parameter `tool`.
- **Result:** let `L` be the level of `enchantment` on the tool (LOOT-CND-022). `L` is 0 when
  `tool` is absent. Let `c` be the element `min(L, len − 1)` of `chances`, where `len` is the
  number of elements. The condition takes `F` and passes when `F < c`.
- **Draws:** one `F`, also when `c` is 0 or 1, and also when `tool` is absent.
- **Example:** `gravel` drops `flint` with the chance 0.1 for an empty hand.

Provenance: `source_suggested_black_box_confirmed`. Kind:
semantic. Confidence: high.

### LOOT-CND-005: `random_chance`

- **Inputs:** `chance`, a float provider.
- **Result:** read `chance` (LOOT-NUM-001) to get `c`. Then take `F`. The condition passes when
  `F < c`.
- **Order:** the draws of `chance` come before the `F` of the condition.
- **Draws:** the draws of `chance`, then one `F`. The condition takes the `F` also when `c ≤ 0`
  or `c ≥ 1`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-006: `match_block`

- **Reads:** the parameter `block_state`, and the parameter `block_entity` when the block
  predicate reads the block entity (`predicates.md`).
- **Result:** the condition fails when `block_state` is absent. Otherwise it passes when the block
  predicate of its fields matches the state, and then matches the block entity. The block entity
  is absent when the parameter `block_entity` is absent. The block predicate states the result
  for an absent block entity.
- **Example:** the table of `tall_grass` tests `"blocks": "minecraft:tall_grass"` with the
  `state` `{"half": "lower"}`. In `context_int_provider/compostable/low.json`, the condition
  tests a `composter` with the `level` 0.
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-007: `match_tool`

- **Reads:** the parameter `tool`.
- **Result:** the condition fails when `tool` is absent. Otherwise it passes when `predicate` is
  absent, or when the tool matches the item predicate `predicate`. The tool can be the empty
  stack. The item predicate states the result for the empty stack.
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-008: `location_check`

- **Reads:** the parameter `origin`, a position `(x, y, z)` of three f64 values.
- **Result:** the condition fails when `origin` is absent. Otherwise it passes when `predicate`
  is absent. Otherwise it gives the position `(x + offsetX, y + offsetY, z + offsetZ)` and the
  dimension of the evaluation to the location predicate. Each addition is in f64. The condition
  passes when the location predicate matches.
- **Timing:** the location predicate reads the world at the time of the test.
- **Example:** the origin of a block table is the centre of the block. The tables of tall
  plants, for example `tall_grass`, test the other half of the plant with `offsetY` 1 or `-1`
  (LOOT-SRC-006 of `block-loot.md`).
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-009: `entity_properties`

- **Reads:** the parameter of `entity` (LOOT-CND-020) and the parameter `origin`.
- **Result:**
  - When `predicate` is absent, the condition passes. It passes also when the entity is absent.
  - Otherwise it gives the entity, the position `origin` and the dimension of the evaluation to the
    entity predicate. The entity or the position can be absent. The condition passes when the
    entity predicate matches. The entity predicate does not match an absent entity.
- **Example:** an empty object `{}` is a `predicate` that is present. The table of `snow` has
  `entity_properties` with `entity` `this` and the predicate `{}`. Thus a snow layer drops
  `snowball` or `snow` when a player breaks it, and drops nothing for `setblock` with `destroy`.
- **Draws:** none.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high for a present `predicate`, medium for an absent
`predicate`.

### LOOT-CND-010: `random_chance_with_enchanted_bonus`

- **Reads:** the parameter `attacking_entity`.
- **Result:** let `L` be the level of `enchantment` on the attacking entity (LOOT-CND-022). `L`
  is 0 when `attacking_entity` is absent or is not a living entity. When `L > 0`, the chance `c`
  is the level-based value `enchanted_chance` at `L` (LOOT-NUM-021). When `L = 0`, `c` is
  `unenchanted_chance`. The condition takes `F` and passes when `F < c`.
- **Draws:** one `F`.
- **Example:** the vanilla entity tables use the enchantment `minecraft:looting`, for example
  for the rare drops of a zombie.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-011: `killed_by_player`

- **Reads:** the parameter `last_damage_player`.
- **Result:** the condition passes when `last_damage_player` is present, and fails when it is
  absent.
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-012: `entity_scores`

- **Reads:** the parameter of `entity` (LOOT-CND-020), and the scoreboard of the server.
- **Result:**
  1. When the entity is absent, the condition fails and reads no score.
  2. Go through the entries of `scores` in the order of the file. For each entry, do these
     steps:
     1. When the scoreboard has no objective with the name of the entry, the condition fails.
     2. When the entity has no score for that objective (LOOT-CND-023), the condition fails.
     3. Test the score with the int range of the entry (LOOT-CND-021). When the test fails, the
        condition fails.
  3. When each entry passes, the condition passes. A `scores` object with no entries passes.
- **Order:** the condition stops at the first entry that fails. It does not read the entries
  after it. The decode must keep the order of the entries of the file.
- **Draws:** the draws of the ranges that the condition tests, in the order of the entries.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-013: `damage_source_properties`

- **Reads:** the parameters `damage_source` and `origin`.
- **Result:** the condition fails when `damage_source` or `origin` is absent. Otherwise it passes
  when `predicate` is absent. Otherwise it gives the damage source, the position `origin` and the
  dimension of the evaluation to the damage source predicate. It passes when that predicate matches.
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-014: `weather_check`

- **Reads:** the weather of the dimension of the evaluation, as `command-weather.md` states it
  (CMD-W-003).
- **Result:**
  - The dimension **has weather** when its dimension type has sky light, has no ceiling, and the
    dimension is not `minecraft:the_end`.
  - It **rains** in the dimension when the dimension has weather and its rain strength is more
    than 0.2.
  - It **thunders** in the dimension when the dimension has weather and the product of its
    thunder strength and its rain strength is more than 0.9.
  - When `raining` is present and differs from "it rains in the dimension", the condition fails.
  - When `thundering` is present and differs from "it thunders in the dimension", the condition
    fails.
  - Otherwise the condition passes. With no `raining` and no `thundering`, it passes.
- **Timing:** the condition reads the rain strength and the thunder strength of the current tick.
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-015: `time_check`

- **Reads:** the total ticks `t` of the clock `clock` in the dimension of the evaluation, an i64
  (`command-time.md`, terms). A clock that the world does not hold has the total ticks 0
  (CMD-TIME-005).
- **Result:**
  1. When `period` `p` is present, let `t` be the remainder of `t / p`. The quotient truncates
     toward zero, so the remainder has the sign of `t`.
  2. Let `v` be the low 32 bits of `t`, as an i32.
  3. The condition passes when `v` is in the range `value` (LOOT-CND-021).
- **Load:** a `time_check` with `period` 0 fails the decode of its entry (REG-FLD-715). Thus the
  evaluation never meets `p = 0`. This is a `design` choice. LOOT-CDV-001 records the reference.
- **Draws:** the draws of the range `value`.
- **Example:** `"clock": "minecraft:overworld", "period": 24000, "value": {"min": 13000}` passes
  at night.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-016: `int_value_check`

- **Inputs:** `value`, an int provider, and `test`, an int range.
- **Result:** the condition passes when the read of `value` is in the range `test`. LOOT-CND-021
  states the order of the reads. When `test` has neither `min` nor `max`, the condition passes
  and does not read `value`.
- **Draws:** those of LOOT-CND-021, with `value` as the input provider.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-017: `float_value_check`

- **Inputs:** `value`, a float provider, and `test`, a float range.
- **Result:** as LOOT-CND-016, with float providers and f32 comparisons.
- **Draws:** those of LOOT-CND-021, with `value` as the input provider.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-018: `enchantment_active_check`

- **Reads:** the parameter `enchantment_active`, a boolean.
- **Result:** the condition fails when `enchantment_active` is absent. Otherwise it passes when
  the parameter equals `active`.
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-019: `environment_attribute_check`

- **Reads:** the value of the environment attribute `attribute` in the dimension of the evaluation.
- **Result:**
  - For a positional attribute (REG-FLD-253), the value is the position value at `origin`
    (ENVA-LAYER-002, ENVA-LAYER-003). The biome layer takes the biome of the cell of `origin`,
    with no spatial interpolation. When `origin` is absent, the value is the default value of
    the attribute (ENVA-TYPE-006).
  - For an attribute that is not positional, the value is the dimension value (ENVA-LAYER-002).
  - The condition passes when the value equals `value`. Two values are equal when they are the
    same value of the attribute type (ENVA-TYPE-001). Two f32 values are equal when they have the
    same bits. Thus `0.0` and `-0.0` are different.
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

## 3. Common values of the conditions

### LOOT-CND-020: Entity targets

- **Behavior:** an entity target names one parameter of the context:

| Entity target | Parameter |
|---------------|-----------|
| `this` | `this_entity` |
| `attacker` | `attacking_entity` |
| `direct_attacker` | `direct_attacking_entity` |
| `attacking_player` | `last_damage_player` |
| `target_entity` | `target_entity` |
| `interacting_entity` | `interacting_entity` |

- **Absent values:** the entity of a target is absent when its parameter is absent.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-021: Int range and float range

- **Inputs:** a range (REG-FLD-720, REG-FLD-721) and an input. The input is a value, or a
  provider that the range reads.
- **Result:**
  - The point form passes when the input equals the read of the point provider. The f32
    comparison is IEEE equality, so a NaN equals nothing and `-0.0` equals `0.0`.
  - The range form passes when the input is not less than the read of `min` and not more than
    the read of `max`. An absent `min` or `max` has no limit. A range form with neither `min`
    nor `max` passes.
- **Order:**
  1. When the input is a provider, read it first. Exception: a range form with neither `min` nor
     `max` does not read the input provider.
  2. Point form: read the point provider.
  3. Range form: read `min` when it is present. When the input is less than `min`, the range
     fails and does not read `max`.
  4. Range form: read `max` when it is present.
- **Draws:** the draws of each read, in the order above. Each read is a separate read of
  LOOT-NUM-001. Thus an evaluation error in `min` gives 0 for `min` only.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-022: Level of an enchantment

- **Behavior:**
  - The level of an enchantment on an item is the level of that enchantment in the component
    `minecraft:enchantments` of the item. It is 0 when the item does not have the enchantment.
    The component `minecraft:stored_enchantments` does not count. The empty stack has the level
    0.
  - The level of an enchantment on an entity is the highest level of that enchantment on the
    items of the entity. Only the equipment slots that the field `slots` of the enchantment
    (REG-FLD-122) names count, and only the slots that are not empty. It is 0 when no such item
    has the enchantment.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-CND-023: Score of a score holder

- **Behavior:**
  - The score holder of a player is the name of the player. The score holder of another entity
    is its UUID, as text with hyphens.
  - The score holder of the scoreboard name provider `fixed` is its `name`.
  - A score holder has a score for an objective when the scoreboard holds a score of that holder
    in that objective.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

## 4. Context number providers

A field that is a provider is a number, an object with a `type`, or a key of a provider
(REG-FLD-722, REG-FLD-723). A bare number is the type `constant`. A key of a provider is the
provider of its entry, with the same draws.

In this section, `a` and `b` are the values of the two operands of a type, in the order of the
evaluation. "Error" means an evaluation error.

### LOOT-NUM-001: Evaluation errors and the read of a provider

- **Behavior:**
  - An evaluation error of one operand is an evaluation error of each provider that contains
    that operand. The evaluation stops at the error. It takes no draw after the error. The draws
    before the error stay taken.
  - The read of an int provider gives the value of the evaluation. It gives 0 for an evaluation
    error.
  - The read of a float provider gives the value of the evaluation when the value is finite. It
    gives 0 for an evaluation error, and for a NaN or an infinity.
  - Inside an evaluation, a float value can be a NaN or an infinity. Only the read, `from_float`
    (LOOT-NUM-004) and `binomial` (LOOT-NUM-018) test it.
  - Each use of a provider outside another provider is a read. A pool reads `rolls` with the int
    read and `bonus_rolls` with the float read (LOOT-EVL-002 of `loot.md`). Each condition of this
    document and each modifier (`loot-functions.md`) reads its providers in the same way.
  - A condition inside a provider (LOOT-NUM-013, LOOT-NUM-014) reads its own providers. An error
    inside such a condition does not reach the outer provider.
- **Example:** `{"type": "minecraft:div", "left": 1, "right": 0}` reads as 0.
  `{"type": "minecraft:add", "inputs": [1, {"type": "minecraft:div", "left": 1, "right": 0}]}`
  also reads as 0, not as 1.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-002: Evaluation order of operands

- **Behavior:** a provider evaluates each operand one time, in this order:
  - `left`, then `right`;
  - `base`, then `exponent`;
  - `min`, then `max`;
  - `n`, then `p`;
  - the elements of `inputs`, in their order. The order of the elements of a tag is the member
    order of REG-TAG-005.
- **Exceptions:** the float `mod` (LOOT-NUM-007), `conditional` (LOOT-NUM-013),
  `number_dispatcher` (LOOT-NUM-014), `weighted_list` (LOOT-NUM-015), `storage` (LOOT-NUM-017)
  and `score` (LOOT-NUM-019) state their own order.
- **Draws:** the draws of the operands, in this order. The draw of the type itself comes after
  the draws of its operands.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-003: `constant`

- **Result:** the int `value` or the float `value`. A bare number is a `constant`.
- **Draws:** none.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high.

### LOOT-NUM-004: Int unary types

- **Result:** `a` is the value of `input`.

| Type | Value | Error |
|------|-------|-------|
| `abs` | the absolute value of `a` | `a = −2^31` |
| `negate` | `−a` | `a = −2^31` |
| `from_float` | `a` truncated toward zero. `a` is the value of a float provider. | `a` is not finite, or the truncated value is outside the i32 range |

- **Draws:** those of `input`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-005: Float unary types

- **Result:** `a` is the value of `input`. No float unary type gives an error of its own.

| Type | Value |
|------|-------|
| `abs` | `a` with the sign bit cleared |
| `negate` | `a` with the sign bit changed |
| `floor` | the largest integer value that is not more than `a`, as an f32. NaN, an infinity and a zero stay as they are. |
| `ceil` | `toF32` of the ceiling of NUM-CONV-001. The value goes through an i32, so a NaN gives 0 and the result stays in `[−2^31, 2^31]`. |
| `round` | `toF32(n)`. `n` is the nearest integer to `a`; a tie goes toward +∞. A NaN gives `n = 0`. `n` stays in the i32 range, as `toI32` states. |
| `truncate` | `floor` of `a` when `a > 0`, and `ceil` of `a` in f32 otherwise, with no i32 step. `-0.5` gives `-0.0`. A NaN stays a NaN. |
| `sqrt` | the square root of `a`, in f32. A negative `a` gives a NaN. |
| `sin` | the sine of NUM-TRIG-003 at `widen(a)` |
| `cos` | the cosine of NUM-TRIG-003 at `widen(a)` |
| `from_int` | `toF32(a)`. `a` is the value of an int provider. An error of `a` is an error of `from_int`. |

- **Draws:** those of `input`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-006: Int binary types

- **Result:** `a` is the value of `left`, and `b` is the value of `right`.

| Type | Value | Error |
|------|-------|-------|
| `sub` | `a − b` | the exact result is outside the i32 range |
| `mod` | the remainder of the quotient that truncates toward zero. It has the sign of `a`. `−2^31 mod −1` is 0. | `b = 0` |
| `floor_mod` | the floor modulus of NUM-CONV-002. It has the sign of `b`. | `b = 0` |
| `div` | the quotient that truncates toward zero. `−2^31 / −1` is `−2^31`. | `b = 0` |
| `floor_div` | the floor quotient of NUM-CONV-002 | `b = 0`, or `a = −2^31` and `b = −1` |

- **Draws:** those of `left`, then those of `right`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-007: Float binary types

- **Result:** each step is in f32.
  - `sub` evaluates `left`, then `right`. The value is `a − b`.
  - `div` evaluates `left`, then `right`. The value is `a / b`, with the IEEE results for a
    division by zero.
  - `mod` evaluates `right` first. When the value of `right` is `0.0` or `-0.0`, the value is a
    NaN, and `mod` does not evaluate `left`. Otherwise it evaluates `left`. The value is the
    exact remainder of `left / right` with a quotient that truncates toward zero. The value has
    the sign of `left`.
- **Draws:** `sub` and `div`: those of `left`, then those of `right`. `mod`: those of `right`,
  then those of `left` when it evaluates `left`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-008: `pow`

- **Result:** `a` is the value of `base`, and `b` is the value of `exponent`.
  - Int: when `a = 0` and `b = 0`, error. When `b < 0`, error. When the exact value of `a` to the
    power `b` is outside the i32 range, error. Otherwise the value is that exact power.
  - Float: when `a` and `b` are both `0.0` or `-0.0`, the value is a NaN. Otherwise the value is
    the narrow of the f64 power of `widen(a)` and `widen(b)`. The f64 power has the IEEE 754
    special cases, with two exceptions. The power of 1 to a NaN is a NaN. The power of 1 or
    `-1` to an infinity is a NaN.
- **Draws:** those of `base`, then those of `exponent`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-009: Int `uniform`

- **Result:** `a` is the value of `min`, and `b` is the value of `max`. When `a ≥ b`, the value
  is `a`, with no draw. Otherwise the value is `a + B(b − a + 1)`.
- **Edge case:** when `b − a + 1` is more than `2^31 − 1`, the provider gives an error and takes
  no draw. This is a `design` choice. LOOT-CDV-002 records the reference.
- **Draws:** those of `min`, then those of `max`, then one `B(b − a + 1)` when `a < b`.
- **Example:** `{"type": "minecraft:uniform", "min": 2, "max": 4}` gives 2, 3 or 4 with the chance
  1/3 each.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high for the distribution, medium for the draw.

### LOOT-NUM-010: Float `uniform`

- **Result:** `a` is the value of `min`, and `b` is the value of `max`. When `a ≥ b`, the value
  is `a`, with no draw. Otherwise the value is `F × (b − a) + a`, in f32 and in this step order.
  A NaN `a` or `b` is not `a ≥ b`, so it takes the draw.
- **Draws:** those of `min`, then those of `max`, then one `F` when `a ≥ b` is false.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-011: Int aggregate types

- **Result:** the provider evaluates each element of `inputs`, in order.

| Type | Value | Value with no elements | Error |
|------|-------|------------------------|-------|
| `add` | the exact sum | 0 | the sum is outside the i32 range |
| `mul` | the product, in i64 steps that keep the low 64 bits | 1 | the product is outside the i32 range |
| `min` | the smallest element | `2^31 − 1` | none |
| `max` | the largest element | `−2^31` | none |
| `avg` | the exact sum divided by the number of elements, truncated toward zero | none | no elements |

- **Note:** an empty `inputs` is a fault of the load (LOOT-CTX-016 of `loot.md`). Only a provider
  that a library user makes in code meets the column "Value with no elements".
- **Draws:** those of each element, in order.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-012: Float aggregate types

- **Result:** the provider evaluates each element of `inputs`, in order. Each step is in f32.

| Type | Value |
|------|-------|
| `add` | `s = +0.0`, then `s = s + v` for each element `v` |
| `mul` | `s = 1.0`, then `s = s × v` for each element `v` |
| `min` | `s = 3.4028235e38`, then `s = min(s, v)` for each element `v` |
| `max` | `s = −3.4028235e38`, then `s = max(s, v)` for each element `v` |
| `avg` | the value of `add`, divided by `toF32` of the number of elements. With no elements, a NaN. |
| `length` | `s = +0.0`, then `s = s + v × v` for each element `v`. The value is the square root of `s`. |

- `min` and `max` give a NaN when one of their arguments is a NaN. For them, `-0.0` is less
  than `0.0`.
- An empty `inputs` is a fault of the load, as LOOT-NUM-011 states.
- **Draws:** those of each element, in order.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-013: `conditional`

- **Result:** test `condition`. When it passes, the value is the value of `on_true`. When it
  fails, the value is the value of `on_false`. An absent `on_false` is the constant 0.
- **Order:** the provider tests the condition first. Then it evaluates only the selected
  operand.
- **Draws:** those of `condition`, then those of the selected operand.
- **Example:** `context_float_provider/cooking/speed_default.json` selects a fast speed for a
  `smoker` or a `blast_furnace`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-014: `number_dispatcher`

- **Result:** test the condition of each element of `cases`, in order, until one passes. The
  value is the value of the `value` of that case. When no case passes, the value is the value
  of `default`. An absent `default` is the constant 0.
- **Order:** the provider does not test the cases after the case that passes. It evaluates only
  the selected value.
- **Draws:** those of each condition that it tests, in order, then those of the selected value.
- **Example:** `context_int_provider/compostable/low.json` gives 1 for an empty `composter`.
  Otherwise it gives 1 with the weight 30 and 0 with the weight 70.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-015: `weighted_list`

- **Result:** let `W` be the sum of the `weight` of each element of `distribution`. Take `k =
  B(W)`. Go through the elements in order, and subtract the weight of each from `k`. The first
  element for which `k` becomes less than 0 is the selected element. The value is the value of
  its `data`.
- **Draws:** one `B(W)`, then those of the selected `data`. The provider does not evaluate the
  other elements.
- **Note:** the decode makes sure that `W` is at least 1 and at most `2^31 − 1` (REG-FLD-732). An
  element with the weight 0 is never selected.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-016: `environment_attribute`

- **Result:** read the value of `attribute` as LOOT-CND-019 states. The int provider accepts only
  an attribute of the type `integer`, and the value is that integer. The float provider accepts
  an attribute of the type `float`, `angle_degrees` or `integer`. The value is the f32 value,
  or `toF32` of the integer.
- **Reads:** the parameter `origin` when the attribute is positional.
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-017: `storage`

- **Reads:** the command storage `storage` of the server. A storage that does not exist is an
  empty compound.
- **Result:**
  1. Select the tags of the storage at the NBT path `path`, as the command `data get` selects
     them.
  2. When the path selects exactly one tag, and that tag is a number, the value comes from that
     tag:
     - int provider: a byte, a short or an int gives its value. A long gives its low 32 bits. A
       float or a double gives `toI32` of its value.
     - float provider: a byte, a short, an int or a long gives the nearest f32. A double gives
       its narrow. A float gives its value.
  3. Otherwise the value is the value of `fallback`. An absent `fallback` is the constant 0. A
     path that selects no tag, more than one tag or a tag that is not a number gives the
     fallback.
- **Draws:** those of `fallback`, only when the provider evaluates it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-018: `binomial`

- **Result:** evaluate `n`, an int provider, to get `n`. Then evaluate `p`, a float provider, to
  get `p`. When `p` is not finite, the provider gives an error. Otherwise take `n` float draws.
  The value is the number of draws `F` with `F < p`. When `n ≤ 0`, the value is 0 and the
  provider takes no draw.
- **Draws:** those of `n`, then those of `p`, then `n` draws `F`.
- **Example:** the table of `melon_stem` gives `binomial` with `n` 3 and `p` `0.06666667` for the
  age 0.

Provenance: `source_suggested_black_box_confirmed` (the block tables of `block-loot.md`, for the
distribution). Kind: semantic. Confidence: medium.

### LOOT-NUM-019: `score`

- **Reads:** the scoreboard of the server, and the parameter of `target` when `target` is an
  entity target.
- **Result:**
  1. Get the score holder of `target`. A bare entity target string and the type `context` give
     the entity of the target (LOOT-CND-020, LOOT-CND-023). The type `fixed` gives `name`.
  2. In these cases, the value is the value of `fallback`:
     - the entity of the target is absent;
     - the scoreboard has no objective with the name `score`;
     - the holder has no score for that objective.

     An absent `fallback` is the constant 0.
  3. Otherwise the value is the score.
- **Draws:** those of `fallback`, only when the provider evaluates it.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-020: `enchantment_level`

- **Reads:** the parameter `enchantment_level`, an i32 `L`. `L` is 0 when the parameter is
  absent.
- **Result:** the level-based value `amount` at `L` (LOOT-NUM-021).
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-021: Level-based value

- **Inputs:** a level-based value (REG-FLD-386) and a level `L`, an i32.
- **Result:** an f32. Each step is in f32 unless the row states otherwise.

| Type | Value |
|------|-------|
| a bare number `v` | `v` |
| `linear` | `base + per_level_above_first × toF32(L − 1)` |
| `levels_squared` | `toF32(L × L) + added`. `L × L` is an i32 step that keeps the low 32 bits. |
| `clamped` | the value of `value`, limited to the range `min..max`. A NaN stays a NaN. The decode rejects `max ≤ min` (REG-FLD-387). |
| `fraction` | 0 when the value of `denominator` is `0.0` or `-0.0`. Otherwise the value of `numerator` divided by the value of `denominator`. |
| `exponent` | the narrow of the f64 power of the widen of the value of `base` and the widen of the value of `power`, as LOOT-NUM-008 states for f64 |
| `lookup` | the element `L − 1` of `values` when `1 ≤ L ≤ len`. Otherwise the value of `fallback` at `L`. `len` is the number of elements. |

- **Edge case:** `lookup` with `L < 1` uses `fallback`. This is a `design` choice. LOOT-CDV-003
  records the reference.
- **Draws:** none.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-022: Resolvable int and resolvable float

- **Inputs:** a resolvable int (REG-FLD-739) or a resolvable float (REG-FLD-740), and the default
  value of the component that holds it.
- **Result:** a bare number gives its value. A key gives the read of the provider of the key
  (LOOT-NUM-001). A key with no entry in its registry gives the default value of the component.
  The specification of the component states that default value.
- **Draws:** those of the provider of the key.
- **Example:** `context_int_provider/compostable/low.json` is the value of the layers that an item
  adds to a `composter`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### LOOT-NUM-023: The number providers of the block tables

- **Behavior:** the block tables use the bare number (LOOT-NUM-003), `uniform` (LOOT-NUM-009,
  LOOT-NUM-010) and `binomial` (LOOT-NUM-018). Earlier versions of `block-loot.md` stated these
  providers in LOOT-MOD-008. That id is now in `loot-functions.md`, and it cites this rule.
- **Note:** a float `uniform` with `min ≥ max` takes no draw (LOOT-NUM-010).

Provenance: `source_suggested_black_box_confirmed`. Kind:
semantic. Confidence: high.

## 5. Order of the draws in an evaluation

### LOOT-NUM-024: One sequence of draws

- **Behavior:** each draw of this document takes the next value of the generator of the
  evaluation. The conditions and the providers of one evaluation share one generator. Thus the
  order of the draws is the order of the tests and the evaluations that sections 2 to 4 state.
- **Example:** `random_chance` with `"chance": {"type": "minecraft:uniform", "min": 0.2, "max":
  0.4}` takes one `F` for the chance, and then one `F` for the test.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

## 7. Intentional deviations

### 7.1 Intentional deviations

**LOOT-CDV-001.**

- **Rule**: LOOT-CND-015.
- **Reference**: a `time_check` with `period` 0 loads. Its test stops with an arithmetic error.
  When the condition is inside a provider, the nearest read that contains the condition catches
  the error. That read gives 0, as for an evaluation error (LOOT-NUM-001). Otherwise nothing catches the error, and
  the command or the action that evaluates the condition fails.
- **Visible to the client**: no. A data pack author sees a load fault in place of an error at the
  evaluation.

**LOOT-CDV-002.**

- **Rule**: LOOT-NUM-009.
- **Reference**: an int `uniform` with `max − min + 1` more than `2^31 − 1` stops the evaluation
  with an error that nothing catches. The command or the action that evaluates it fails.
- **Visible to the client**: no. A data pack author sees an error in place of a result.

**LOOT-CDV-003.**

- **Rule**: LOOT-NUM-021.
- **Reference**: a `lookup` at a level less than 1 stops the evaluation with an index error.
  An `enchantment_level` with no parameter `enchantment_level` uses the level 0. Thus each
  `lookup` in such a provider meets this case.
- **Visible to the client**: no. A data pack author sees an error in place of a result.

**LOOT-CDV-004.**

- **Rule**: LOOT-CND-001, and LOOT-CTX-013 and LOOT-CTX-016 of `loot.md`.
- **Reference**: the load does not check the provider in the field `chance` of `random_chance`. A
  reference back to itself through that provider loads, and its evaluation fails with a stack
  overflow. A parameter that the provider reads gives no warning, also when the type of the table
  does not allow it.
- **Visible to the client**: no. A data pack author sees a load fault in place of a failed
  evaluation. No vanilla `random_chance` has a `chance` that is not a bare number.
