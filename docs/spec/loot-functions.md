# Item modifiers: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states what each item modifier does to a stack. For each type, it states the change
of the item, of the count and of the component patch. It states the parameters that the type
reads, and the result when a parameter is absent. It states each random draw, with its order and its
kind.

Scope: each type of the registry `minecraft:loot_function_type`, a list of modifiers, and a key of
the registry `minecraft:item_modifier`. These documents state the parts that this document cites:

- `registry-fields.md`, REG-FLD-741 to REG-FLD-789: how the server decodes each type. This document
  does not repeat a decode rule.
- `loot.md`: the context, the entries, the evaluation of a table and the generator (LOOT-CTX,
  LOOT-EVL, LOOT-RND).
- `loot-conditions.md`: the conditions and the number providers (LOOT-CND, LOOT-NUM).
- `predicates.md`: the item predicate (PRED).
- `item-stack.md`: the stack, the component patch and the validation (STK).

This document replaces section 5 of
`block-loot.md`. The rules LOOT-MOD-001 to LOOT-MOD-008 keep their ids.

## 0. Conventions

### 0.1 Terms

- **Stack**, **empty stack**, **count**, **component patch**, **value entry**, **removal entry**,
  **effective component**, **maximum stack size**: as `item-stack.md`, section 0.1, defines them.
- **Tag**, **tag type**, **numeric tag**, **compound**, **key**, **list**, **array**: as `snbt.md`,
  section 0.1, defines them. An array is not a list.
- **Modifier**: an item modifier (REG-FLD-741). Its **type** is the value of its field `type`.
- **Chain**: an ordered sequence of modifiers that the server applies to one work stack.
- **Work stack**: the value that a chain changes. It has an item, a count and a component patch.
  Its count can be 0 or less, and it can be larger than the maximum stack size.
- **Empty work stack**: a work stack with a count of 0 or less, or with the item `minecraft:air`
  (LOOT-MOD-011).
- **`c`**: the count of the work stack before the modifier of the rule. `c` is 0 for an empty work
  stack (LOOT-MOD-011).
- **Context**, **parameter**, **generator**: as `loot.md`, section 0.1, defines them. A parameter is
  for example `tool`. This document names a parameter by its id without the namespace.
- **Entity target**: a string that names one parameter that holds an entity:

  | Entity target | Parameter |
  |---------------|---------------|
  | `this` | `this_entity` |
  | `attacker` | `attacking_entity` |
  | `direct_attacker` | `direct_attacking_entity` |
  | `attacking_player` | `last_damage_player` |
  | `target_entity` | `target_entity` |
  | `interacting_entity` | `interacting_entity` |

- **Block entity target**: the string `block_entity`. It names the parameter `block_entity`.
- **Item stack target**: the string `tool`. It names the parameter `tool`.
- **Provider**: a number provider (REG-FLD-722, REG-FLD-723).
- **Read**: the **int read** or the **float read** of a provider, as LOOT-NUM-001
  (`loot-conditions.md`) states. An evaluation error gives 0. A float read also gives 0 for a NaN
  or an infinity.
- **Draw**: one value that a rule takes from the generator. A rule names the kind of each draw with the
  notation of LOOT-RND-006 (`loot.md`): the float draw `F`, the bounded draw `B(n)` and the draw in
  a range `between(a, b)`.
- **Registry order**: the order of the numeric ids of a registry (REG-SET-001, REG-ENTRY-004).
- **Set order**: the order of the entries of an entry set. A tag has its resolved order
  (REG-TAG-005). A list has the order of the list.
- **Level of an enchantment in a component**: the level that the component holds for that
  enchantment, or 0 when the component does not hold it.
- **Service**: an operation that needs more than the context and the loaded data, for example a
  search for a structure (section 8).
- **Event**: the action that makes a source evaluate a table, for example the open of a container
  or the death of a mob.

An integer range `a..b` includes `a` and `b`. An integer division drops the fraction. `round(x)` is
`floor(x + 0.5)`. `clamp(x, a, b)` is `a` when `x < a`; otherwise it is the smaller of `x` and `b`.
Where a rule says f32, each operation of the formula gives an f32 value.

### 0.2 Provenance

- `black_box_confirmed`: shown by a game-produced artefact. The artefact is the vanilla data pack.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with such an artefact.
- `source_suggested`: taken from the reference implementation, with no artefact. The
  confidence is at most medium.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided.

Each rule states its kind, as `docs/spec/README.md` defines it.

The observer of a draw is a player who opens a container whose loot has a seed. With the same seed,
the player gets the same stacks from the vanilla server. A test of the library sees the same thing:
it compares the state of the generator after an evaluation. Each draw thus has a rule.

### 0.3 Artefacts cited

- The data pack of the client jar: `data/minecraft/loot_table/` and
  `data/minecraft/villager_trade/`. `tools/mcrs_minecraft_fixtures` installs it. The data pack has
  no entry of the registry `minecraft:item_modifier`.

The vanilla data pack uses these 23 types: `apply_bonus`, `copy_components`, `copy_state`,
`discard`, `enchant_randomly`, `enchant_with_levels`, `enchanted_count_increase`,
`exploration_map`, `explosion_decay`, `filtered`, `furnace_smelt`, `limit_count`, `set_components`,
`set_count`, `set_damage`, `set_enchantments`, `set_instrument`, `set_name`,
`set_ominous_bottle_amplifier`, `set_potion`, `set_random_dyes`, `set_random_potion` and
`set_stew_effect`. Only a custom data pack uses the other types.

### 0.4 ID prefixes

| Prefix | Topic |
|--------|-------|
| `LOOT-MOD` | The modifiers, the NBT providers and the NBT paths |
| `LOOT-MDV` | The intentional deviations of this document |

## 1. The chain

**LOOT-MOD-009** (`source_suggested`, semantic, confidence medium). A modifier tests its common
field `condition` (REG-FLD-742) first. The draws of the condition (`loot-conditions.md`) come before
each draw of the modifier. When the condition fails, the modifier does not change the work stack. It
then takes no draw and evaluates no provider. A modifier without `condition` always applies.

**LOOT-MOD-010** (`source_suggested`, semantic, confidence medium). A value of the type item
modifier has one of four forms:

| Form | Result |
|------|--------|
| an object with a `type` | That modifier applies. |
| a list | Each modifier of the list applies in list order, on the result of the modifier before it. An empty list changes nothing. |
| `sequence` (REG-FLD-780) | When its `condition` passes, the modifiers of `functions` apply as a list. |
| a key of `minecraft:item_modifier` | The registry entry of that key applies. |

A command can apply a registry entry to a stack with the same rules. `loot.md` states the order of
the chains of a table, of a pool and of an entry.

**LOOT-MOD-011** (`source_suggested`, semantic, confidence low). A work stack whose count becomes 0
or less keeps its item and its component patch. In the evaluation of a table, it stays a work stack
through each modifier of LOOT-EVL-011 (`loot.md`). These are the modifiers of the entry, of each
composite entry, of the pool and of the table, and for a nested table also the modifiers around its
`loot_table` entry. For an empty work stack, these rules apply:

- `c` is 0.
- A modifier that reads the item reads `minecraft:air`.
- A modifier that reads an effective component finds none.
- A modifier that writes a component writes it to the component patch.

A later modifier can thus make the count more than 0 again. The work stack then has its item and
its component patch again. For example, `set_count` to `-1` and then `set_count` with `add` and 2
give the count 2. In the evaluation of a table, the division (LOOT-OUT-001) is the one point where
an empty work stack becomes the empty stack (STK-EMP-001). For a content stack of `set_contents`,
that point is the division of LOOT-MOD-053. For each other chain, for example a chain of
`modify_contents`, that point is the end of the chain. No modifier limits the count to the maximum
stack size.

A work stack with the item `minecraft:air` is an empty work stack for each count, and `c` is 0 for
it. A `dynamic` entry can give such a work stack, for an empty slot of a shulker box (LOOT-EVL-007
of `loot.md`). Each modifier of its chain still tests its condition and takes the draws that its
rule states for an empty work stack. A modifier can give it a count of more than 0 and a component
patch. No modifier gives it another item, so it stays an empty work stack to the end of the chain.
At the point that the paragraph above names, it becomes the empty stack, with no component patch.
This paragraph has the confidence medium.

**LOOT-MOD-012** (`source_suggested`, semantic, confidence medium). A modifier takes its draws from
the generator in the order that its rule states. A modifier takes no other draw. Where a rule says
"no draw", the state of the generator does not change. A rule that can meet a bound `n` outside
`1..2³¹ − 1` for `B(n)` states its result for that bound, and that result takes no draw
(LOOT-MOD-004). Section 9 lists the draws of each type.

**LOOT-MOD-008** (`source_suggested_black_box_confirmed`, semantic, confidence high). The number
providers of a modifier are the providers of `loot-conditions.md` (LOOT-NUM). The block tables use
the bare number, `uniform` and `binomial` (LOOT-NUM-023).

**LOOT-MOD-059** (`source_suggested`, semantic, confidence medium). A modifier evaluates each of its
providers one time, at the place that its rule states. A rule that says "the draws of the provider"
refers to the draws of that one evaluation. A modifier that does not apply evaluates no provider.

## 2. Count, item and damage

**LOOT-MOD-001** (`source_suggested_black_box_confirmed`, semantic, confidence high). `set_count`
(REG-FLD-743) sets the count to `v`, or to `c + v` when `add` is true. `v` is the int read of the
provider `count`. The draws are the draws of the provider. The count can become 0 or less. For
example, the tables of `glow_lichen` and `sculk_vein` add 1 for each face and then `-1`. Result L3
of `block-loot.md` shows counts below 1, which make no item.

**LOOT-MOD-003** (`source_suggested_black_box_confirmed`, semantic, confidence high). `limit_count`
(REG-FLD-763) limits the count to the int limit `limit` (REG-FLD-764):

| `min` | `max` | New count | Draws |
|-------|-------|-----------|-------|
| absent | absent | `c` | none |
| absent | present | the smaller of `c` and `max` | the provider `max` |
| present | absent | the larger of `c` and `min` | the provider `min` |
| present | present | `clamp(c, min, h)`, where `h` is the larger of `min` and `max` | the provider `min`, then the provider `max` |

Each bound is the int read of its provider. For example, the table of `glowstone` limits
`glowstone_dust` to `1..4`, and the table of `brown_mushroom_block` limits the count to 0 or more.

**LOOT-MOD-002** (`source_suggested`, semantic, confidence medium). `explosion_decay` (REG-FLD-767)
does nothing and takes no draw when the context has no `explosion_radius`. Otherwise `p` is `1 /
explosion_radius` as f32. The modifier takes `c` draws of `F`, one for each item. It keeps an item
when its draw gives `F ≤ p`. The count becomes the number of items that it keeps. For `c ≤ 0`, it
takes no draw and the count becomes 0.

**LOOT-MOD-004** (`source_suggested_black_box_confirmed`, semantic, confidence high). `apply_bonus`
(REG-FLD-765) does nothing and takes no draw when the context has no `tool`. Otherwise `L` is the
level of `enchantment` in the component `minecraft:enchantments` of the tool. The modifier applies
one formula, also with `L = 0`:

| `formula` | `parameters` | New count | Draws |
|-----------|--------------|-----------|-------|
| `minecraft:ore_drops` | none | `c` when `L ≤ 0`; otherwise `c × (max(b - 1, 0) + 1)` with `b = B(L + 2)` | none when `L ≤ 0`; otherwise one `B(L + 2)` |
| `minecraft:uniform_bonus_count` | `bonusMultiplier` `m` | `c + B(m × L + 1)`; `c` when `m × L + 1` is outside `1..2³¹ − 1` | one `B(m × L + 1)`, also when the bound is 1; none when the bound is outside `1..2³¹ − 1` |
| `minecraft:binomial_with_bonus_count` | `extra` `e`, `probability` `p` | `c` plus the number of draws that give `F < p` | `L + e` draws of `F` |

For `ore_drops`, `L` is at most 255 (STK-SHP-015), so the bound `L + 2` is at most 257. The bound
is thus always in `1..2³¹ − 1`. For `ore_drops` with `L > 0`, the factor is 1 with the chance
`2 / (L + 2)`, and each value of `2..L + 1` with the chance `1 / (L + 2)`. The draw of `uniform_bonus_count` for a bound of 1 has the label `source_suggested`. The
bound `m × L + 1` is an exact integer. It is outside `1..2³¹ − 1` only for a negative `m` or a very
large `m`, which no vanilla table has. LOOT-MDV-003 records the reference for such a bound.

**LOOT-MOD-013** (`source_suggested`, semantic, confidence medium). `enchanted_count_increase`
(REG-FLD-751) reads the parameter `attacking_entity`. It does these steps:

1. When the value is absent or is not a living entity, the modifier stops. It takes no draw.
2. `L` is the level of `enchantment` on the entity. For this level, take each equipment slot that
   the field `slots` of the enchantment definition names. `L` is the highest level of the
   enchantment in the component `minecraft:enchantments` of the stacks in these slots.
3. When `L` is 0, the modifier stops. It does not evaluate the provider.
4. `v` is the float read of the provider `count`. The draws are the draws of the provider.
5. The count becomes `c + round(L × v)`, with the product as f32.
6. When `limit` is larger than 0 and the count is larger than `limit`, the count becomes `limit`.

**LOOT-MOD-014** (`source_suggested`, semantic, confidence medium). `discard` (REG-FLD-767) gives
the empty stack. The item and the component patch do not stay. It takes no draw. For example, each
`exploration_map` of the vanilla data pack has a `filtered` with `on_fail` `discard` after it.

**LOOT-MOD-015** (`source_suggested`, semantic, confidence medium). `set_item` (REG-FLD-744)
replaces the item of the work stack with `item`. The count and the component patch stay. The
effective components then come from the new item (STK-PAT-002). An empty work stack does not
change. It takes no draw.

**LOOT-MOD-016** (`source_suggested`, semantic, confidence medium). `set_damage` (REG-FLD-752)
applies only to a work stack that can take damage (STK-DMG-001). For other work stacks, it changes
nothing and does not evaluate the provider. Otherwise, with the maximum damage `M` and the damage
`d` of the stack, it does these steps in f32:

1. `v` is the float read of the provider `damage`. The draws are the draws of the provider.
2. `base` is `1 - d / M` when `add` is true. Otherwise `base` is 0.
3. `p` is `1 - clamp(v + base, 0, 1)`.
4. The modifier writes the damage value `floor(p × M)` (STK-DMG-003).

The value `v` is thus the part of the durability that stays. A `damage` of 1 gives an item with no
damage.

**LOOT-MOD-017** (`source_suggested`, semantic, confidence medium). `furnace_smelt` (REG-FLD-750)
reads the smelting recipes of the loaded data. It takes no draw. It does not change an empty work
stack. It searches the smelting recipe whose ingredient accepts the work stack. When no recipe
accepts it, or the result of the recipe is the empty stack, the work stack does not change.
Otherwise the work stack becomes the result of the recipe, with a new count:

- `n` is `c` when `use_input_count` is true. Otherwise `n` is 1.
- The count is the smaller of `n × k` and the maximum stack size of the result. `k` is the count of
  the result of the recipe.

The component patch of the input does not stay. Item U5 records which recipe wins when more than one
accepts the stack.

## 3. Enchantments

**LOOT-MOD-018** (`source_suggested`, semantic, confidence medium). Three rules of this section use
these two operations.

The **source list** of a modifier is its `options` in set order. Without `options`, it is each
entry of the registry `minecraft:enchantment` in registry order.

The **raise** of an enchantment `E` to a level `l` on a work stack does these steps:

1. The component is `minecraft:stored_enchantments` when the item is `minecraft:enchanted_book`.
   Otherwise it is `minecraft:enchantments`.
2. When the work stack has no such effective component, nothing changes.
3. When `l` is 0 or less, nothing changes.
4. The level of `E` in the component becomes the larger of its current level and the smaller of `l`
   and 255. Other enchantments of the component stay.

**LOOT-MOD-019** (`source_suggested`, semantic, confidence medium). `enchant_randomly` (REG-FLD-746)
does these steps:

1. Make the candidate list from the source list. When `only_compatible` is true and the item is not
   `minecraft:book`, keep only each enchantment whose `supported_items` contains the item.
2. When the candidate list is empty, the modifier stops. It takes no draw.
3. Draw `B(n)`, where `n` is the length of the candidate list. The draw is the index of the
   enchantment `E`.
4. The level `l` is `between(1, max_level)`, with the `max_level` of `E`. It takes no draw when
   `max_level` is 1 or less.
5. When the item is `minecraft:book`, the work stack becomes `minecraft:enchanted_book` with the
   count 1 and an empty component patch.
6. Raise `E` to `l` (LOOT-MOD-018).
7. When `include_additional_cost_component` is true and the context has
   `additional_cost_component_allowed`, draw `x = B(5 + 10 × l)`. Then set the component
   `minecraft:additional_trade_cost` to `2 + x + 3 × l`.

Step 5 loses the count and the component patch of the book. Item U1 asks the owner about this
result.

**LOOT-MOD-020** (`source_suggested`, semantic, confidence medium). `enchant_with_levels`
(REG-FLD-745) does these steps:

1. `n` is the int read of the provider `levels`. The draws are the draws of the provider.
2. When the work stack has no effective component `minecraft:enchantable`, go to step 9. The
   modifier takes no more draws. Otherwise `e` is the `value` of that component.
3. Take four draws in this order: `b1 = B(e / 4 + 1)`, `b2 = B(e / 4 + 1)`, `f1 = F`, `f2 = F`. The
   division is an integer division.
4. Calculate the cost `q`. With `n2 = n + 1 + b1 + b2` and `s = (f1 + f2 - 1) × 0.15` in f32, `q` is
   `clamp(round(n2 + n2 × s), 1, 2147483647)`. The sum and the product are f32.
5. Make the candidate list from the source list, in the order of the source list. Keep an
   enchantment when the item is `minecraft:book`. For other items, keep it when its
   `supported_items` contains the item, and its `primary_items` is absent or contains the item.
6. Give each candidate the highest level `l` in `1..max_level` for which `min_cost(l) ≤ q ≤
   max_cost(l)`. Remove a candidate that has no such level. A cost at the level `l` is `base +
   per_level_above_first × (l - 1)`.
7. When the candidate list is empty, go to step 9.
8. Select the results (LOOT-MOD-021).
9. When the item is `minecraft:book`, the work stack becomes `minecraft:enchanted_book` with the
   count 1 and an empty component patch. This step also applies when there is no result.
10. Raise each result to its level, in the order of the selection (LOOT-MOD-018).
11. Set the component `minecraft:additional_trade_cost` to `n` when each of these is true:
    `include_additional_cost_component` is true, the context has
    `additional_cost_component_allowed`, the work stack is not empty, and `n > 0`.

For example, the table `gameplay/fishing/treasure` has `levels` 30 and `options`
`#minecraft:on_random_loot`. Item U1 also applies to step 9.

**LOOT-MOD-021** (`source_suggested`, semantic, confidence medium). The selection of
`enchant_with_levels` uses the candidate list and the cost `q` of LOOT-MOD-020. A **weighted pick**
does these steps:

1. `T` is the sum of the `weight` of each candidate.
2. When `T` is 0, the pick gives no result and takes no draw.
3. Draw `k = B(T)`. Go through the candidates in order, and subtract the `weight` of each from `k`.
   The result is the first candidate for which `k` becomes less than 0.

The selection does these steps:

1. Do one weighted pick. Add its result to the results.
2. Draw `d = B(50)`. When `d > q`, the selection ends. The draw stays taken.
3. Remove from the candidate list each enchantment that is not compatible with the last result.
4. When the candidate list is empty, the selection ends with no more draws.
5. Do one weighted pick. Add its result to the results.
6. Divide `q` by 2 with an integer division. Go to step 2.

Two enchantments are compatible when they are different and neither is in the `exclusive_set` of
the other. Step 3 does nothing when there is no result yet.

**LOOT-MOD-022** (`source_suggested`, semantic, confidence medium). `set_enchantments`
(REG-FLD-747) does these steps:

1. When the item is `minecraft:book`, the item becomes `minecraft:enchanted_book`. The count and the
   component patch stay.
2. The component is `minecraft:stored_enchantments` for `minecraft:enchanted_book`. Otherwise it is
   `minecraft:enchantments`.
3. When the work stack has no such effective component, the modifier stops. It evaluates no
   provider.
4. For each entry of `enchantments`, `v` is the int read of its provider. With `add` true, the new
   level is `clamp(l + v, 0, 255)`, where `l` is the current level. Otherwise it is `clamp(v, 0,
   255)`.
5. A new level of 0 removes the enchantment from the component.

**LOOT-MOD-023** (`design`, unspecified). `set_enchantments` evaluates the providers of its entries
in the order of the keys in the file. The reference gives no stable order. A parity test must not
compare the generator state after a `set_enchantments` with more than one provider that takes a
draw. No `set_enchantments` of the vanilla data pack has a provider that takes a draw. Entry
LOOT-MDV-002 records the
reference.

## 4. Components that the modifier states

In this section, "the component, or its default" means the effective component of the work stack.
When the work stack has none, it means the default that the rule states. Each modifier of this
section sets a value entry. STK-PAT-005 keeps the component patch in normal form.

**LOOT-MOD-024** (`source_suggested`, semantic, confidence medium). `set_components` (REG-FLD-749)
applies its `components` to the work stack: each value entry sets the component, and each removal
entry removes it. It then validates the work stack strictly (STK-VAL-001). When the validation
rejects the result, the component patch from before the modifier stays. It takes no draw. For
example, the table `equipment/trial_chamber` sets `minecraft:trim`.

**LOOT-MOD-025** (`source_suggested`, semantic, confidence medium). `set_custom_data` (REG-FLD-748)
merges `tag` into the compound of `minecraft:custom_data`, or into an empty compound. The merge is
the compound merge of LOOT-MOD-049. When the result is an empty compound, the stack has no
`minecraft:custom_data`. It takes no draw.

**LOOT-MOD-026** (`source_suggested`, semantic, confidence medium). `set_attributes` (REG-FLD-753)
starts from an empty list when `replace` is true. Otherwise it starts from the list of the
component `minecraft:attribute_modifiers`, or from an empty list. For each element of `modifiers`
(REG-FLD-754), in list order, it does these steps:

1. Draw `B(k)`, where `k` is the length of `slot`. The draw is the index of the slot group. It also
   takes this draw when `k` is 1.
2. `v` is the float read of the provider `amount`. The draws are the draws of the provider.
3. Remove from the list each entry with the same `attribute` and the same `id`.
4. Append an entry with `attribute`, `id`, the amount `v`, `operation` and the slot group.

The modifier then sets the component to the list.

**LOOT-MOD-027** (`source_suggested`, semantic, confidence medium). `set_name` (REG-FLD-755) does
nothing when `name` is absent. Otherwise it sets the component `minecraft:custom_name` or
`minecraft:item_name`, as `target` states, to the text. When `entity` is present and the context has
the entity of that entity target, the server resolves the text first (LOOT-MOD-028). Otherwise it
sets the text as the file states it. It takes no draw. For example, the table
`chests/abandoned_camp_common_chest` sets `item_name` to a translation.

**LOOT-MOD-028** (`source_suggested`, semantic, confidence low). The resolution of a text for
`set_name` and `set_lore` replaces each part that needs a world value. Such parts are a selector, a
score and an NBT source. The entity of the entity target is the source of the resolution, with the
permissions of a game master. When the resolution fails, the modifier sets the text as the file
states it. The resolution takes no draw. No specification states the resolution yet (item U4), and
no modifier of the vanilla data pack has `entity`.

**LOOT-MOD-029** (`source_suggested`, semantic, confidence medium). `set_lore` (REG-FLD-768) takes
the lines of the component `minecraft:lore`, or an empty list. It makes the new lines from `lore`,
as `set_name` makes its text, with the same `entity`. It then applies the list operation
(LOOT-MOD-030) with the maximum 256 and sets the component. It takes no draw.

**LOOT-MOD-030** (`source_suggested`, semantic, confidence medium). A list operation (REG-FLD-769,
REG-FLD-770) makes a list from the current list `old`, of the length `n`, and the new values `new`,
of the length `k`. Some modifiers give a maximum length.

| `mode` | Result | The result is `old` when |
|--------|--------|--------------------------|
| `replace_all` | `new` | never |
| `replace_section` | `old[0..offset)`, then `new`, then `old[offset + size..n)` | `offset > n`, or the result is longer than the maximum |
| `insert` | `old[0..offset)`, then `new`, then `old[offset..n)` | `offset > n`, or `n + k` is larger than the maximum |
| `append` | `old`, then `new` | `n + k` is larger than the maximum |

Without `size`, `replace_section` uses `k`. A section that ends after `n` ends at `n`. When the
result is `old`, the server writes an error line to its log, and the evaluation continues. The
modifier still sets the component to the result. Thus a stack without lore gets an empty
`minecraft:lore` after an `insert` with `offset` 1.

The maximum also bounds the decode (REG-FLD-769). A list `new` that is longer than the maximum
fails the load. A `size` that is larger than the maximum fails the load. `offset` has no maximum at
the load. Thus `replace_all` never gives `old`, because `k` is at most the maximum.

These examples use the maximum 256 of `set_lore`. `old` has 255 lines. An `append` of 2 lines gives
`old`, and an `append` of 1 line gives 256 lines. For `set_writable_book_pages`, `old` has 99 pages.
An `append` of 2 pages gives `old`, and a `replace_all` of 100 pages gives 100 pages.

These modifiers use a list operation:

| Modifier | List | Maximum |
|----------|------|---------|
| `set_lore` | the lines | 256 |
| `set_writable_book_pages` | the pages | 100 |
| `set_written_book_pages` | the pages | none |
| `set_fireworks` | `explosions` | none (item U8) |
| `set_custom_model_data` | `floats`, `flags`, `strings`, `colors` | none |

**LOOT-MOD-031** (`source_suggested`, semantic, confidence medium). `set_banner_pattern`
(REG-FLD-775) sets the component `minecraft:banner_patterns`. When `append` is true, the value is
the current layers and then `patterns`. Otherwise the value is `patterns`. It takes no draw.

**LOOT-MOD-032** (`source_suggested`, semantic, confidence medium). `set_potion` (REG-FLD-776) takes
the component `minecraft:potion_contents`, or its default with no potion, no colour, no effect and
no name. It sets the potion to `id`. The other parts of the component stay. It takes no draw.

**LOOT-MOD-033** (`source_suggested`, semantic, confidence medium). `set_random_potion`
(REG-FLD-778) selects one potion and then does the steps of `set_potion`. The candidates are
`options` in set order. Without `options`, they are each entry of the registry `minecraft:potion` in
registry order. When there is no candidate, the modifier does nothing and takes no draw. Otherwise
it draws `B(n)`, where `n` is the number of candidates. The draw is the index of the potion.

**LOOT-MOD-034** (`source_suggested`, semantic, confidence medium). `set_random_dyes` (REG-FLD-777)
does these steps:

1. `n` is the int read of the provider `number_of_dyes`. The draws are the draws of the provider.
2. When `n` is 0 or less, the modifier stops.
3. Take `n` draws of `B(16)`. Each draw is the index of a dye in the table below.
4. Make the colour list: the colour of the effective `minecraft:dyed_color` when the work stack has
   one, then the colour of each dye in draw order.
5. Mix the colour list (LOOT-MOD-035) and set `minecraft:dyed_color` to the result.
6. The count of a work stack that is not empty becomes 1.

| Index | Dye | Colour | Index | Dye | Colour |
|-------|-----|--------|-------|-----|--------|
| 0 | white | `F9FFFE` | 8 | light_gray | `9D9D97` |
| 1 | orange | `F9801D` | 9 | cyan | `169C9C` |
| 2 | magenta | `C74EBD` | 10 | purple | `8932B8` |
| 3 | light_blue | `3AB3DA` | 11 | blue | `3C44AA` |
| 4 | yellow | `FED83D` | 12 | brown | `835432` |
| 5 | lime | `80C71F` | 13 | green | `5E7C16` |
| 6 | pink | `F38BAA` | 14 | red | `B02E26` |
| 7 | gray | `474F52` | 15 | black | `1D1D21` |

The modifier does not test that the item can have a dye. The leatherworker trades of the vanilla
data pack use this modifier. Item U2 asks the owner about step 6 and about the test.

**LOOT-MOD-035** (`source_suggested`, semantic, confidence medium). The mix of `k` colours gives one
RGB colour:

1. `R`, `G` and `B` are the sums of the red, green and blue channels. `S` is the sum of the largest
   channel of each colour.
2. `r = R / k`, `g = G / k` and `b = B / k`, each as an integer division.
3. `a = S / k` in f32. `m` is the largest of `r`, `g` and `b`.
4. Each channel becomes `x × a / m` in f32, without its fraction. The product comes before the
   division.

When `m` is 0, each channel is 0.

**LOOT-MOD-036** (`source_suggested`, semantic, confidence medium). `set_instrument` (REG-FLD-779)
selects one instrument of `options`, in set order. When the set is empty, the modifier does nothing
and takes no draw. Otherwise it draws `B(n)`, where `n` is the size of the set. It sets the
component `minecraft:instrument` to the instrument at that index. For example, the table
`chests/pillager_outpost` has `options` `#minecraft:regular_goat_horns`.

**LOOT-MOD-037** (`source_suggested`, semantic, confidence medium). `set_stew_effect` (REG-FLD-757)
applies only when the item is `minecraft:suspicious_stew` and `effects` is not empty. Otherwise it
does nothing and takes no draw. It does these steps:

1. Draw `B(n)`, where `n` is the length of `effects`. The draw is the index of the entry
   (REG-FLD-758).
2. `t` is the int read of the provider `duration` of the entry. The draws are the draws of the
   provider.
3. When the effect is not instantaneous, `t` becomes `t × 20`.
4. Append the effect with the duration `t` to the component `minecraft:suspicious_stew_effects`, or
   to an empty list.

**LOOT-MOD-038** (`source_suggested`, semantic, confidence medium). `set_ominous_bottle_amplifier`
(REG-FLD-788) sets the component `minecraft:ominous_bottle_amplifier` to `clamp(v, 0, 4)`. `v` is
the int read of the provider `amplifier`. The draws are the draws of the provider.

**LOOT-MOD-039** (`source_suggested`, semantic, confidence medium). `set_loot_table` (REG-FLD-766)
sets the component `minecraft:container_loot` to the loot table key and the seed of the modifier. It
does not change an empty work stack. It takes no draw. It does not evaluate the loot table.

**LOOT-MOD-040** (`source_suggested`, semantic, confidence medium). These modifiers change parts of
one component and take no draw. A field that is absent leaves its part as it is.

| Modifier | Component | Default | Change |
|----------|-----------|---------|--------|
| `set_fireworks` (REG-FLD-782) | `minecraft:fireworks` | flight duration 0, no explosion | `flight_duration` replaces the flight duration. `explosions` applies its list operation to the explosions. |
| `set_firework_explosion` (REG-FLD-783) | `minecraft:firework_explosion` | shape `small_ball`, no colour, no fade colour, no trail, no twinkle | Each of `shape`, `colors`, `fade_colors`, `trail` and `twinkle` replaces its part. |
| `set_book_cover` (REG-FLD-784) | `minecraft:written_book_content` | empty title, empty author, generation 0, no page | Each of `title`, `author` and `generation` replaces its part. The pages stay. |
| `set_written_book_pages` (REG-FLD-785) | `minecraft:written_book_content` | as `set_book_cover` | The list operation applies to the pages. The other parts stay. |
| `set_writable_book_pages` (REG-FLD-786) | `minecraft:writable_book_content` | no page | The list operation applies to the pages, with the maximum 100. |
| `toggle_tooltips` (REG-FLD-787) | `minecraft:tooltip_display` | the tooltip shows, no hidden component | For each entry of `toggles`: `true` shows the tooltip part of that component type, and `false` hides it. |

**LOOT-MOD-041** (`source_suggested`, semantic, confidence medium). `set_custom_model_data`
(REG-FLD-789) takes the component `minecraft:custom_model_data`, or its default with four empty
lists. It does these steps:

1. When `floats` is present, evaluate each provider of `floats.values` in list order for its float
   value.
2. When `colors` is present, evaluate each provider of `colors.values` in list order for its int
   value.
3. Apply the list operation of each present field to its list. `flags` and `strings` take no draw.

The draws of step 1 come before the draws of step 2. The modifier takes these draws also when the
list operation then leaves the list as it is.

## 5. Values from the context

**LOOT-MOD-005** (`source_suggested`, semantic, confidence medium). `copy_state` (REG-FLD-774) does
nothing when the context has no `block_state`. Otherwise it takes the component
`minecraft:block_state`, or an empty one. For each name of `properties` that the context state has,
it sets that name to the value of the context state, as a string. Other entries of the component
stay. It takes no draw. For example, the tables of `beehive` and `bee_nest` copy `honey_level`.

**LOOT-MOD-006** (`source_suggested_black_box_confirmed`, semantic, confidence high).
`copy_components` (REG-FLD-781) copies components from its `source` to the work stack. The `source`
is an entity target, the block entity target or the item stack target. When the context does not
have that value, the modifier does nothing. It takes no draw. Otherwise it copies each component
type that the source has and that passes these two tests:

- `include` is absent, or `include` contains the type.
- `exclude` is absent, or `exclude` does not contain the type.

Each copy is a value entry on the work stack. For example, the table of `chest` copies
`minecraft:custom_name` from `block_entity`, so a named chest drops a named item. The sources other than `block_entity` have the label `source_suggested`.

**LOOT-MOD-007** (`source_suggested`, semantic, confidence medium). The components of a source are
these:

| Source | Components |
|--------|------------|
| a block entity | The values that the block entity stores as components. Examples are its custom name, its contents, its lock and its bees. |
| a stack | Its effective components. |
| an entity | The values that the entity gives as components, for example its custom name. |

No specification states the list for each block entity and each entity yet. Item U3 and item U2 of
`block-loot.md` record this dependency.

**LOOT-MOD-042** (`source_suggested`, semantic, confidence medium). `copy_name` (REG-FLD-759) reads
the custom name of its `source`. The `source` is an entity target or the block entity target. It
takes no draw.

| Source | Result |
|--------|--------|
| absent from the context | no change |
| a block entity that cannot have a custom name | no change |
| a source with a custom name | `minecraft:custom_name` becomes that name |
| a source that can have a custom name and has none | the work stack has no `minecraft:custom_name` |

**LOOT-MOD-043** (`source_suggested`, semantic, confidence medium). `fill_player_head`
(REG-FLD-771) applies only when the item is `minecraft:player_head` and the entity of `entity` is a
player. It then sets the component `minecraft:profile` to the profile of that player: the id, the
name and the properties. Otherwise it does nothing. It takes no draw.

**LOOT-MOD-044** (`source_suggested`, semantic, confidence medium). An NBT provider (REG-FLD-773)
gives one compound, or nothing:

| Provider | Value |
|----------|-------|
| a bare entity target, or `context` with that `target` | The saved form of the entity, without the key `id`. For a player whose main-hand stack is not empty, the compound also has the key `SelectedItem` with that stack. |
| the bare string `block_entity`, or `context` with that `target` | The saved form of the block entity, with its `id` and its position. |
| `storage` with `source` | The compound of that command storage. It is an empty compound when the storage does not exist. |

A `context` provider gives nothing when the context does not have its value. The command storage
is a world value that the evaluation reads. It is not a service (section 8). Item U3 records the
saved form of an entity as a dependency.

**LOOT-MOD-045** (`source_suggested`, semantic, confidence medium). `copy_custom_data` (REG-FLD-772)
does nothing when its `source` provider gives nothing. Otherwise it applies each operation of `ops`
in list order. It takes no draw. One operation does these steps:

1. Select the tags of the path `source` in the compound of the provider (LOOT-MOD-048). When the
   selection is empty, the operation ends.
2. Take the work compound. It is a copy of the compound of `minecraft:custom_data`, or an empty
   compound. Each later operation of the same modifier uses the same work compound.
3. Apply the `op` of the table below to the path `target` in the work compound.

| `op` | Result |
|------|--------|
| `replace` | Set the path to a copy of the last selected tag (LOOT-MOD-051). |
| `append` | Get or create the tags of the path, with a new list for the last node (LOOT-MOD-050). Append a copy of each selected tag, in order, to each of those tags that is a list. |
| `merge` | Get or create the tags of the path, with a new compound for the last node (LOOT-MOD-050). Merge each selected compound, in order, into each of those tags that is a compound (LOOT-MOD-049). A selected tag that is not a compound has no effect. |

When one operation took the work compound, the modifier sets `minecraft:custom_data` to it after
the last operation. When the work compound is empty, the stack then has no `minecraft:custom_data`.
A step of LOOT-MOD-050 or LOOT-MOD-051 that fails ends only its operation.

**LOOT-MOD-046** (`design`, unspecified). An operation of `copy_custom_data` that fails can leave
the tags that it created before the fault in the work compound. An implementation selects whether
these tags stay. A parity test must not compare the custom data after an operation that fails.

## 6. NBT paths

No other document states the NBT path (`text-component.md`, open question Q2). This section states
the part that `copy_custom_data` needs.

**LOOT-MOD-047** (`source_suggested`, semantic, confidence medium). An NBT path is a sequence of one
or more nodes. The grammar is this:

```
path       = first *next
first      = root-node / name-node / index-node
next       = "." name-node / [ "." ] index-node
root-node  = pattern
name-node  = name [ pattern ]
index-node = "[" [ integer / pattern ] "]"
name       = 1*unquoted-char / quoted-string
```

- A `pattern` is the SNBT text of one compound (SNBT-GR-001).
- An `unquoted-char` is each character except the space, `"`, `'`, `[`, `]`, `.`, `{` and `}`.
- A `quoted-string` starts with `"` or `'` and has a quoted string body (CMD-RD-007). The body must
  have one or more characters. The names `""` and `''` fail.
- An `integer` is the longest run of the characters `0` to `9`, `-` and `.` after the `[`. The run
  must be an optional `-` and one or more digits. Its value must be in the range of i32.

The `integer` has these edge cases:

| Text | Result |
|------|--------|
| `[007]` | the index 7. Leading zeros are allowed. |
| `[-0]` | the index 0, the first element. It does not count from the end. |
| `[+1]` | fails. `+` is not in the run, so the run is empty. |
| `[1.0]`, `[1-]`, `[--1]` | fails. The run is not an optional `-` and digits. |
| `[2147483648]` | fails. The value is outside i32. |

A string that this grammar does not accept fails the decode of its entry, as REG-ERR-001 states for
an entry that fails to decode. A path that is empty, that ends with `.`, or that has a space after a
node thus fails the decode. The rejection of these three forms is a `design` choice. Entry
LOOT-MDV-001 records the reference.

**LOOT-MOD-048** (`source_suggested`, semantic, confidence medium). The selection of a path starts
with one tag, the root. Each node replaces the current tags with the tags that it selects from each
of them, in order. The result is the tags after the last node. When a node selects no tag, the
selection is empty.

| Node | Tags that it selects from one tag `t` |
|------|----------------------------------------|
| `{p}` as the first node | `t`, when `t` is a compound that matches `p` |
| `name` | the value of the key `name`, when `t` is a compound that has it |
| `name{p}` | the value of the key `name`, when it is a compound that matches `p` |
| `[]` | each element of `t`, when `t` is a list or an array |
| `[i]` | the element at the index `i`, when `t` is a list or an array and the index is in range |
| `[{p}]` | each element of `t` that is a compound that matches `p`, when `t` is a list |

An index below 0 counts from the end: `-1` is the last element.

An element of an array is a numeric tag of the element type of the array: byte, int or long. An
array gives nothing to `[{p}]`. No node can write into an element that it selects from an array.
Only a last node of LOOT-MOD-051 changes an array.

**LOOT-MOD-049** (`source_suggested`, semantic, confidence medium). Two operations compare and join
tags.

A tag `t` **matches** a pattern tag `p` when one of these is true:

- Both are compounds, and for each key of `p`, `t` has the key with a value that matches the value
  in `p`.
- Both are lists, `p` is empty and `t` is empty.
- Both are lists, `p` is not empty, `t` has at least as many elements as `p`, and each element of
  `p` matches at least one element of `t`.
- They have the same tag type, are not compounds or lists, and are equal. Two arrays are equal when
  they have the same length and the same elements in the same order.

Two numeric tags of different tag types do not match: `1b` does not match `1`, and `1.0f` does not
match `1.0d`. Two floats, or two doubles, are equal when they have the same value, with two
exceptions. Each NaN is equal to each NaN, and the bits of a NaN do not matter. `0.0` is not equal
to `-0.0`. An SNBT pattern cannot hold NaN or `-0.0` (SNBT-VAL-004), but a target tag can.

This is the match of ARG-BLK-011 (`argument-block.md`). In a list match, the order of the elements
does not matter. One element of `t` can match two or more elements of `p`. These examples show the
list match:

| `p` | `t` | Result |
|-----|-----|--------|
| `[1,1]` | `[1]` | no match: `t` has fewer elements than `p` |
| `[1,1]` | `[1,2]` | match: the element `1` of `t` matches the two elements of `p` |
| `[1]` | `[2,1,1]` | match |
| `[]` | `[1]` | no match |
| `[1]` | `[I;1]` | no match: an array is not a list |

The **compound merge** of a compound `s` into a compound `t` does this for each key of `s`. When
both values are compounds, it merges the value of `s` into the value of `t`. Otherwise the key of
`t` gets a copy of the value of `s`. Other keys of `t` stay.

**LOOT-MOD-050** (`source_suggested`, semantic, confidence medium). The operation "get or create"
of a path goes through the nodes as the selection does. A node that finds nothing in a tag `t` can
create a tag. The **new tag** for the last node is the tag that the caller gives. For another node,
the new tag depends on the next node:

| Next node | New tag |
|-----------|---------|
| `name`, `name{p}` | an empty compound |
| `[]`, `[i]`, `[{p}]` | an empty list |

Each creation makes a separate new tag.

| Node | When it creates |
|------|-----------------|
| `{p}` as the first node | never |
| `name` | `t` is a compound without the key: it puts the new tag at the key |
| `name{p}` | `t` is a compound without the key: it puts a copy of `p` at the key. A value that does not match `p` gives nothing. |
| `[]` | `t` is an empty list: it adds the new tag |
| `[i]` | never |
| `[{p}]` | `t` is a list with no element that matches `p`: it appends a copy of `p` |

Get or create never creates an element of an array. In an array, `[]` and `[i]` give the elements
that the selection gives (LOOT-MOD-048), and an empty array gives nothing. Each new tag is a list or
a compound, and an element of an array is a numeric tag. `append` and `merge` of LOOT-MOD-045 thus
never change an array.

In this rule and in LOOT-MOD-051, the **last node** is the last node of the whole path. A node that
gives no tag has one of these results:

- A node before the last node gives no tag: the operation fails. LOOT-MOD-046 applies to the tags
  that the operation created. This result is the same for "get or create", for "set", and for
  `append` and `merge` of LOOT-MOD-045.
- The last node gives no tag in "get or create": the operation does not fail. It gives no tag, and
  the tags that the nodes before it created stay.

A list that a node creates for a next node `[i]` is empty, so `[i]` gives no tag in it. These
examples start with the compound `{}`:

| Path | Result |
|------|--------|
| `a[0]` | `a` gets an empty list. The last node `[0]` gives no tag. The operation does not fail, and the result is `{a:[]}`. |
| `a[0].b` | `a` gets an empty list. The node `[0]` gives no tag before the last node, so the operation fails. |
| `{z:1}.a` | `{}` does not match `{z:1}`, so the first node gives no tag. The operation fails and creates nothing. The result is `{}`. |

The paths `a[0].b` and `{z:1}.a` fail in the same way under "set", `append` and `merge`.

**LOOT-MOD-051** (`source_suggested`, semantic, confidence medium). The operation "set" of a path to
a value does "get or create" for each node except the last node of the path. A node of these that
gives no tag fails the operation, as LOOT-MOD-050 states. The last node then writes the value
to each tag `t` that this gives. Each write after the first one uses a copy of the value.

| Last node | Write |
|-----------|-------|
| `{p}` as the first node | none |
| `name` | `t` is a compound: the key gets the value |
| `name{p}` | `t` is a compound whose value at the key matches `p`: the key gets the value |
| `[]` | `t` is an empty list: it adds the value. `t` is a list that is not empty: each element of the list becomes the value. `t` is an array: see the table below. |
| `[i]` | `t` is a list and the index is in range: that element becomes the value. `t` is an array: see the table below. |
| `[{p}]` | `t` is an empty list: it adds the value. Otherwise each element that matches `p` becomes the value. An array gets no write. |

A write into an array needs a numeric value. The array converts the value to its element type. A
value that is not a numeric tag gives no write, and the array does not change. LOOT-MDV-004 records
the reference.

| Last node | Array `t` | Write |
|-----------|-----------|-------|
| `[]` | empty | it adds one element, the converted value |
| `[]` | not empty | each element becomes the converted value. The length does not change. |
| `[i]` | the index is in range | that element becomes the converted value |
| `[i]` | the index is not in range | none |

The conversion of a numeric value `v` to the element type is this:

| Element type | `v` is a byte, a short, an int or a long | `v` is a float or a double |
|--------------|-------------------------------------------|----------------------------|
| byte | the low 8 bits of `v`, as a signed value | `floor(v)`, clamped to `-128..127` |
| int | the low 32 bits of `v`, as a signed value | `floor(v)`, clamped to the range of i32 |
| long | `v` | `floor(v)`, clamped to the range of i64 |

A NaN gives 0. An infinity gives the bound of its sign.

When the last node writes nothing, the operation does not fail. The tags that the nodes before it
created stay, as LOOT-MOD-050 states. For example, "set" of `a[0]` in `{}` gives `{a:[]}`.

The operation fails when the value is too deep. This test occurs before the first node, so a value
that is too deep changes nothing. The **level** of a tag in the
value is this:

- The value itself has the level 0.
- A value of a compound and an element of a list have the level of their container plus 1.
- An element of an array has no level.

The value is too deep when the number of nodes of the path plus the highest level in the value is
512 or more. For example, a path of one node accepts a value with the highest level 510, and it
rejects 511. The value `{a:[1]}` has the highest level 2. This count is not the depth of
SNBT-LIM-001: the level starts at 0, and a tag of each type has a level. Only "set" has this test.
`append` and `merge` of LOOT-MOD-045 do not test the level.

## 7. Nested modifiers and contents

**LOOT-MOD-052** (`source_suggested`, semantic, confidence medium). `filtered` (REG-FLD-762) tests
the item predicate `item_filter` on the work stack (`predicates.md`). When the test passes, it
applies `on_pass`. Otherwise it applies `on_fail`. A branch that is absent changes nothing. The
draws are the draws of the branch that applies. For example, the table `chests/underwater_ruin_big`
discards a stack without `minecraft:map_id`.

**LOOT-MOD-053** (`source_suggested`, semantic, confidence medium). `set_contents` (REG-FLD-760)
does not change an empty work stack, and then takes no draw. Otherwise it makes a list of stacks:

1. Expand each entry of `entries` in list order, as LOOT-EVL-004 states.
2. Each leaf result makes its stacks at the time when the expansion gives it, as LOOT-EVL-006 states.
   Each stack goes through the modifier of its leaf entry and of each composite entry that contains
   it (LOOT-EVL-011, steps 1 and 2). This occurs before the expansion tests the condition of the
   next child or of the next entry.
3. Each stack then goes through the division of LOOT-OUT-001 (`loot.md`), and the results go at the
   end of the list. A stack with a count of 0 or less, or with the item `minecraft:air`, becomes the
   empty stack there.

There is no weight, no selection and no roll: each leaf result makes its stacks. `weight` and
`quality` have no effect. The draws thus follow the order of the expansion. For example, take a
`group` with two `item` children A and B. Each child has a `random_chance` condition and a
`set_count` with a `uniform` count. When both conditions pass, the draws are the `F` of A, the draw
of the count of A, the `F` of B and the draw of the count of B. A roll of a pool takes the `F` of A and the `F` of B first
(LOOT-EVL-003).

The modifier then writes the list to the component `component` (LOOT-MOD-055). The contents from
before the modifier do not stay.

**LOOT-MOD-054** (`source_suggested`, semantic, confidence medium). `modify_contents` (REG-FLD-761)
does not change an empty work stack. It also does nothing when the work stack has no effective
component `component`. Otherwise it applies `modifier` to each content stack that is not empty, in
the order of the contents. It then limits the count of each result to its maximum stack size. It
writes the results to the component (LOOT-MOD-055). The draws are the draws of `modifier` for each
content stack, in that order.

**LOOT-MOD-055** (`source_suggested`, semantic, confidence medium). A list of stacks becomes the
value of a contents component in this way:

| `component` | Value |
|-------------|-------|
| `minecraft:container` | Slot `i` holds the stack `i` of the list. Empty stacks at the end of the list make no slot. |
| `minecraft:bundle_contents` | The bundle takes each stack of the list in order, by the rule for an insert into a bundle (item U6). |
| `minecraft:charged_projectiles` | The stacks of the list that are not empty, in order. |

## 8. Services

**LOOT-MOD-056** (`design`, semantic). A modifier reads the work stack, its own fields, the
registries, the generator and the parameters that its rule names. It changes only the work stack
and the state of the generator. The modifiers of this table also read or change other values. The
result of each modifier is the work stack that its rule states. Reason: issue #74 asks for an
evaluation in the library, and this table names each value of the world that a modifier observes.

| Modifier | What it reads or changes | Kind |
|----------|--------------------------|------|
| `exploration_map` | It reads the structures of the dimension. It adds new map data with a new map id to the world. | service (LOOT-MOD-057) |
| `set_name`, `set_lore` with `entity` | It reads the values of the world that the text resolves against. | service (LOOT-MOD-028, item U4) |
| `copy_custom_data` with a `storage` provider | It reads the compound of a command storage of the world. | world value (LOOT-MOD-044) |
| `copy_custom_data` with a `context` provider | It reads the saved form of the entity or of the block entity of a parameter. | parameter (LOOT-MOD-044) |
| `copy_components`, `copy_name`, `fill_player_head`, `enchanted_count_increase` | It reads the components, the name, the profile or the equipment of a parameter. | parameter |
| `furnace_smelt` | It reads the smelting recipes. | loaded data |
| `set_contents` | It evaluates its entries and their conditions, which can read each part of the context. | evaluation (`loot.md`) |

**LOOT-MOD-057** (`source_suggested`, semantic, confidence medium). `exploration_map` (REG-FLD-756)
takes no draw. It does not change an empty work stack, and it does nothing when the context has no
`origin`. Otherwise it uses the service of LOOT-MOD-056. The item can be each item. The **search**
of the modifier finds a start of one of the structures of `destination`, as CMD-LOC-006 states. The
search starts at the block position of `origin`, in the dimension of the context. The limit of the
area is `search_radius` cells in place of 100 cells. These are the results:

1. When `skip_existing_chunks` is true, the search skips a start that cannot take one more
   reference. The world adds one reference to the start that the search finds.
2. When the search finds nothing, the work stack does not change, and the world does not change.
3. The world gets new map data with a new map id. LOOT-MOD-060 states the id.
4. The centre of the map data is the x and the z of the found position, and its scale is `zoom`.
   The map tracks the position, also without a limit. Its dimension is the dimension of the context.
5. The component `minecraft:map_id` of the work stack becomes the map id of result 3.
6. The map data holds the preview of the biomes.
7. Take the component `minecraft:map_decorations`, or an empty one. Set its key `+` to the type
   `decoration`, the x and the z of the found position, and the rotation 180.

When the search finds a start, the work stack after the modifier is the input with the two
components of results 5 and 7. When the search finds nothing, the work stack is the input with no
change. The count and the other components stay in both cases. Because of result 1, a second map
from the same position can show a different structure. The number of references that a start can
take is world data, and this document does not state it.

The world gets the changes of results 1, 3 and 6 when the evaluation is complete (LOOT-MOD-060), in
the order of the maps. A search sees the references that each earlier search of the same
evaluation added.

In an evaluation of the block source (`block-loot.md`), of `item modify` and of `loot`,
`exploration_map` does not change the work stack, and the world does not change. The evaluation
gives no error. LOOT-MDV-005 and LOOT-MDV-006 record the reference.

**LOOT-MOD-058** (`source_suggested`, semantic, confidence medium). The modifiers after
`exploration_map` apply to the work stack of LOOT-MOD-057, with or without a found start. The
modifiers around the entry (LOOT-EVL-011 of `loot.md`), the division (LOOT-OUT-001) and, for a
container, the spread (LOOT-CTR-001) then use that result. For example, each `exploration_map` of
the vanilla data pack has exactly one modifier after it: a `filtered` that tests
`minecraft:map_id`, with `on_fail` `discard`. When the search finds nothing, that modifier
discards the map. The drop list then has one stack less, and the spread takes other draws.

An evaluation that reaches a search gives the result of one evaluation of the table, with the
generator in the state that it had at the event. For a keyed stream (LOOT-RND-009), the key holds
the game tick of the event. The same applies to a generator that the source gives and to a sequence
stream (the rows of LOOT-RND-004). Thus the ticks that the search takes do not change a draw.

**LOOT-MOD-060** (`design`, semantic). A search (LOOT-MOD-057) can end some ticks after the event.
The number of ticks is unspecified. These sources wait for the search of their evaluation:

- a container (`container-loot.md`, CTL-TRG-001);
- the death of an entity;
- fishing;
- a trade of a villager.

Such a source gives no stack of the evaluation until the evaluation is complete. Then it gives the
stacks as its own rule states, with the context of the event. An evaluation that reaches no search
does not wait for an evaluation that reaches a search.

A trade whose modifier reaches a search is not in the trade list of the villager until the
evaluation is complete. Then the offer is in the list. The other offers of the same level do not
wait for it. A specification of the villager trades states when the server sends a trade list to a
player. LOOT-MDV-007 records the reference.

These sources do not wait: the block source (LOOT-MDV-005), and the commands `item modify` and
`loot` (LOOT-MDV-006). Their stack does not change (LOOT-MOD-057). The result and the feedback of
such a command are those of the tick of the command. Reason: a function can store the result of a
command in the same tick.

A map of an evaluation takes the next free map id of the world at the time when the evaluation is
complete. These rules give the order of the map ids:

- The maps of one evaluation take consecutive ids, in the order of the drop list. A map in the
  contents of a stack (LOOT-MOD-053) takes its id at the place of that stack, in the order of the
  contents.
- Evaluations that are complete in the same tick take their ids in the order of their events.
- Each other new map id of that tick keeps the order in which the server applies its action, for
  example a player who fills an empty map.

The search of an evaluation sees the references that each evaluation of an earlier event added.
Reason: the tick never waits for a search. The map ids and the references then follow the order of
the events, with no gap in the map ids of one evaluation.

## 9. The draws of each type

This table repeats the draws of the rules above, in order. The draws of `condition` come first for
each type (LOOT-MOD-009). A type that this table does not list takes no draw.

| Type | Draws in order | Rule |
|------|----------------|------|
| `set_count` | the provider `count` | LOOT-MOD-001 |
| `limit_count` | the provider `min`, the provider `max`, each when present | LOOT-MOD-003 |
| `explosion_decay` | `c` draws of `F`, with `explosion_radius` | LOOT-MOD-002 |
| `apply_bonus` | one `B`, or `L + e` draws of `F`, by formula, with `tool` | LOOT-MOD-004 |
| `enchanted_count_increase` | the provider `count`, when the level is not 0 | LOOT-MOD-013 |
| `set_damage` | the provider `damage`, when the stack can take damage | LOOT-MOD-016 |
| `enchant_randomly` | `B(n)`, `between(1, max_level)`, then `B(5 + 10 × l)` for the cost | LOOT-MOD-019 |
| `enchant_with_levels` | the provider `levels`, `B`, `B`, `F`, `F`, then the selection | LOOT-MOD-020, LOOT-MOD-021 |
| `set_enchantments` | the provider of each entry | LOOT-MOD-022, LOOT-MOD-023 |
| `set_attributes` | for each element: `B(k)`, then the provider `amount` | LOOT-MOD-026 |
| `set_random_potion` | `B(n)` | LOOT-MOD-033 |
| `set_random_dyes` | the provider `number_of_dyes`, then `n` draws of `B(16)` | LOOT-MOD-034 |
| `set_instrument` | `B(n)` | LOOT-MOD-036 |
| `set_stew_effect` | `B(n)`, then the provider `duration` of the entry | LOOT-MOD-037 |
| `set_ominous_bottle_amplifier` | the provider `amplifier` | LOOT-MOD-038 |
| `set_custom_model_data` | each provider of `floats`, then each provider of `colors` | LOOT-MOD-041 |
| `filtered`, `sequence`, `modify_contents` | the draws of the modifiers that apply | LOOT-MOD-052, LOOT-MOD-010, LOOT-MOD-054 |
| `set_contents` | the draws of its entries | LOOT-MOD-053 |

## 11. Intentional deviations and unresolved items

### 11.1 Intentional deviations

**LOOT-MDV-001.**

- **Rule**: LOOT-MOD-047.
- **Reference**: the read of a path stops at a space and accepts the text before it. The reference
  also accepts a `.` at the end, and an empty string as a path with no node. A path with no node
  fails each time that a modifier uses it as `target`.
- **Visible to the client**: no. The path is a field of a data pack.

**LOOT-MDV-002.**

- **Rule**: LOOT-MOD-023.
- **Reference**: the order of the entries of `enchantments` is the order of an unordered map. It is
  not stable between two starts of the server.
- **Visible to the client**: no. The result differs only when two entries have a provider that
  takes a draw.

**LOOT-MDV-003.**

- **Rule**: LOOT-MOD-004.
- **Reference**: `uniform_bonus_count` calculates `m × L + 1` as a 32-bit int, which wraps. A
  bound of 0 or less after the wrap stops the evaluation with an error that nothing catches. The
  action that evaluates the modifier fails. A positive bound after the wrap takes a draw with that
  bound.
- **Visible to the client**: no. The author of the data pack sees the count `c` in place of an
  error. No vanilla table has a negative `bonusMultiplier`.

**LOOT-MDV-004.**

- **Rule**: LOOT-MOD-051.
- **Reference**: when the last node is `[]`, `t` is an array that is not empty, and the value is
  not a numeric tag, the reference removes each element of the array. The array becomes empty.
- **Visible to the client**: no. The author of the data pack sees an empty array in the custom data.
  No vanilla table writes into an array.

**LOOT-MDV-005.**

- **Rule**: LOOT-MOD-057.
- **Reference**: the block source also runs the search during the evaluation. The work stack gets a
  map of the found structure.
- **Visible to the client**: yes, only with a data pack that adds `exploration_map` to a block
  table. No vanilla block table has it. The block then drops the stack with no map. With the
  `filtered` of the vanilla tables after it, the player gets no map.

**LOOT-MDV-006.**

- **Rule**: LOOT-MOD-060.
- **Reference**: `item modify` and `loot` with a modifier or a table that reaches a search run the
  search in the tick of the command. The stack gets a map of the found structure.
- **Visible to the client**: yes. With `item modify`, only for an item modifier of a data pack
  that has `exploration_map`: the stack of the slot gets no map. With `loot`, also for a vanilla
  table, for example `chests/shipwreck_map`: the command gives the stack with no map. With the
  `filtered` of the vanilla tables after `exploration_map`, the stack is discarded.

**LOOT-MDV-007.**

- **Rule**: LOOT-MOD-060, the trade of a villager.
- **Reference**: the vanilla server runs the search in the tick of the level change of the
  villager. The offer is in the trade list from that tick.
- **Visible to the client**: yes. An offer with a map, for example of a cartographer, is in the
  trade list some ticks after the level change.

### 11.2 Unresolved

- **U1**: `enchant_randomly` and `enchant_with_levels` replace a `book` with a new `enchanted_book`
  (LOOT-MOD-019, LOOT-MOD-020). The count and the component patch of the book do not stay, but
  `set_enchantments` keeps them. Does the owner want this result? Each `book` of the vanilla data
  pack has the count 1 and an empty patch at that step. Parity thus does not depend on the answer.
- **U2**: `set_random_dyes` sets the count to 1 and does not test that the item can have a dye
  (LOOT-MOD-034). Does the owner want this result? The vanilla trades use the count 1 and leather
  armor.
- **U3**: the components that an entity gives to `copy_components`, and the saved form of an entity
  for an NBT provider (LOOT-MOD-007, LOOT-MOD-044). No specification states them.
- **U4**: the resolution of a text (LOOT-MOD-028). No specification states it. The owner decides
  whether it is a service or a read of the world before the evaluation.
- **U5**: the smelting recipe that `furnace_smelt` uses when more than one recipe accepts the stack
  (LOOT-MOD-017). No specification of recipes exists.
- **U6**: the rule for an insert into a bundle (LOOT-MOD-055). No specification states it.
- **U8**: `set_fireworks` has no maximum for `explosions` at the time of the evaluation
  (LOOT-MOD-030). The component holds at most 256 explosions. Does the result stay as the old list
  above 256?
- **U9**: the reads of an empty work stack (LOOT-MOD-011). The confidence is low.
