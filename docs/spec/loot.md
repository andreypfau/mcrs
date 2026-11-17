# Loot: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states how the server evaluates a loot table of any type. It states the context and
its parameters, the evaluation of a table, a pool, a roll and each entry type, and the division of a
stack. It also states the spread of a drop list over the slots of a container, and each random draw
of an evaluation.

Other documents state the parts that a table contains and the sources that evaluate a table:

- `loot-conditions.md`: each condition (prefix `LOOT-CND`) and each number provider (prefix
  `LOOT-NUM`).
- `loot-functions.md`: each item modifier (prefix `LOOT-MOD`).
- `predicates.md`: the entity, location, damage source, item and block predicates (prefix `PRED`).
- `block-loot.md`: the block source, its context values and its item entities.
- `registry-fields.md`, section 9: the decode of a loot table and of each part of it (REG-FLD-790
  to REG-FLD-805).
- `registry.md`: the load, the load faults and the tags (REG-ERR-001, REG-TAG-005).

## Loot documents

The loot rules are in seven documents. This table gives the document of each prefix. A rule
that moves to another document keeps its id.

| Prefix | Numbers | Document |
|--------|---------|----------|
| `LOOT-SRC` | all | `block-loot.md` |
| `LOOT-CTX` | 001 | `block-loot.md` |
| `LOOT-CTX` | 010 and up | `loot.md` |
| `LOOT-EVL` | all | `loot.md` |
| `LOOT-OUT` | 001 | `loot.md` |
| `LOOT-OUT` | 002 to 004 | `block-loot.md` |
| `LOOT-RND` | 001, 003 | `block-loot.md` |
| `LOOT-RND` | 002, 004 and up | `loot.md` |
| `LOOT-CTR` | all | `loot.md` |
| `LOOT-DEV` | 001 to 099 | `block-loot.md` |
| `LOOT-DEV` | 100 to 199 | `loot.md` |
| `LOOT-CND` | all | `loot-conditions.md` |
| `LOOT-NUM` | all | `loot-conditions.md` |
| `LOOT-CDV` | all | `loot-conditions.md` |
| `LOOT-MOD` | all | `loot-functions.md` |
| `LOOT-MDV` | all | `loot-functions.md` |
| `PRED` | all | `predicates.md` |
| `CTL` | all | `container-loot.md` |
| `BDR` | all | `block-drops.md` |


## 0. Conventions

### 0.1 Terms

- **Server**: as `login.md`, section 0.1, defines it.
- **Stack**, **empty stack**, **count**, **maximum stack size**: as `item-stack.md`, section 0.1,
  defines them. A stack with a count of 0 or less is an empty stack.
- **Loot table**, **table**: a value of the registry `minecraft:loot_table` (REG-FLD-799).
- **Pool**, **entry**, **condition**, **modifier**: a loot pool (REG-FLD-800), a loot pool entry
  (REG-FLD-790), a loot condition (REG-FLD-700) and an item modifier (REG-FLD-741).
- **Leaf entry**: an entry of the type `item`, `empty`, `tag`, `loot_table`, `dynamic` or `slots`.
- **Composite entry**: an entry of the type `alternatives`, `sequence` or `group`.
- **Nested table**: a table that a `loot_table` entry names or contains.
- **Source**: the game mechanism that asks for an evaluation of a table. Examples are the break of a
  block, the death of a mob and the first opening of a container.
- **Parameter**: one named value that a context can hold (LOOT-CTX-010).
- **Key set**: a value of the static registry `minecraft:context_key_set`. A key set names the
  required parameters and the allowed parameters (LOOT-CTX-011).
- **Type of a table**: the key set that the field `type` of the table names.
- **Context**: the parameters, the luck and the dynamic names that one evaluation can read.
- **Luck**: a 32-bit float of the context (LOOT-CTX-015).
- **Drop list**: the ordered list of stacks that one evaluation of a table gives.
- **Candidate**: a leaf result that one roll can select (LOOT-EVL-003).
- **Generator**: the random number generator that one evaluation takes its draws from
  (LOOT-RND-004).
- **Dimension random**: as `block-simulation.md`, section 0.1, defines it (SIM-RNG-001).
- **Draw**: one value that the evaluation takes from the generator. LOOT-RND-006 states the kinds
  and their notation. Each loot document uses this notation.
- **Keyed stream**: a sequence of random values that is a function of a key. The `i`-th draw of an
  evaluation is the `i`-th value of the stream. LOOT-RND-001 and LOOT-RND-009 state the keys.
- **Free slot**: a slot of a container that holds the empty stack when the spread starts.
- **Reference chain**, **length** of a reference chain: as LOOT-CTX-017 defines them.

An integer range `a..b` includes `a` and `b`. `floor` rounds toward negative infinity.

### 0.2 Provenance

The labels are those of `registry.md`, section 0.3. The author read the reference of the game
version above. Each rule states its kind, as `docs/spec/README.md` defines it.

### 0.3 Artefacts cited

- The data pack of the client jar: `data/minecraft/loot_table/` (1474 tables).
  `tools/mcrs_minecraft_fixtures` installs it.
- `assets/mcrs/reports/registries.json`: the 31 entries of `minecraft:context_key_set` and the 9
  entries of `minecraft:loot_pool_entry_type`.
- `assets/mcrs/reports/datapack.json`: the registry `minecraft:loot_table` accepts tags.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `LOOT-CTX` | the context, the parameters, the key sets and the load checks |
| `LOOT-EVL` | the evaluation of a table, a pool, a roll and each entry |
| `LOOT-OUT` | the division of the stacks of the drop list |
| `LOOT-CTR` | the spread of a drop list over the slots of a container |
| `LOOT-RND` | the generator and the draws |
| `LOOT-DEV` | the intentional deviations of this document, numbers 100 to 199 |

## 1. What the data pack uses

**LOOT-EVL-008** (`black_box_confirmed`, semantic, confidence high). The tables of the vanilla data
pack use these parts. A server must evaluate each entry type of section 3, also a type that no
vanilla table uses.

| Part | Use in the vanilla data pack |
|------|------------------------------|
| table types | `block` 1226, `entity` 116, `chest` 68, `shearing` 22, `gift` 21, `archaeology` 6, `block_interact` 5, `fishing` 4, `equipment` 3, `barter` 1, `entity_interact` 1 |
| entry types | `item` 2591, `alternatives` 87, `loot_table` 57, `empty` 39, `dynamic` 1, `tag` 1 |
| `rolls` | a bare int in 1537 pools, `uniform` in 80 pools |
| `bonus_rolls` | no pool |
| `quality` | 3 entries, all in `gameplay/fishing` |
| `random_sequence` | each table names its own id |

The one `tag` entry is in `entities/creeper` and has `expand` true. The one `dynamic` entry is in
`blocks/decorated_pot`. No table uses `sequence`, `group` or `slots`. No `loot_table` entry has
`expand` or a list `value`.

## 2. The context

**LOOT-CTX-010** (`source_suggested`, semantic, confidence medium). A context can hold these
parameters. The "Name in data" column gives the name that a field `entity` or `source` of a
condition, a modifier or a slot source uses for the parameter.

| Parameter | Value | Name in data |
|-----------|-------|--------------|
| `this_entity` | an entity | `this` |
| `interacting_entity` | an entity | `interacting_entity` |
| `target_entity` | an entity | `target_entity` |
| `last_damage_player` | a player | `attacking_player` |
| `damage_source` | a damage source | none |
| `attacking_entity` | an entity | `attacker` |
| `direct_attacking_entity` | an entity | `direct_attacker` |
| `origin` | a position, three 64-bit floats | none |
| `block_state` | a block state | none |
| `block_entity` | a block entity | `block_entity` |
| `tool` | a stack, which can be the empty stack | `tool` |
| `explosion_radius` | a 32-bit float | none |
| `enchantment_level` | an int | none |
| `enchantment_active` | a boolean | none |
| `additional_cost_component_allowed` | a flag without a value: present or absent | none |
| `container` | an entity or a block entity that has slots | `container` |

**LOOT-CTX-011** (`source_suggested`, semantic, confidence medium). The registry
`minecraft:context_key_set` holds these key sets. A data pack cannot add a key set. "Required" lists
the parameters that a context of the key set always holds. "Optional" lists the other parameters
that it can hold. A parameter in neither column is not allowed.

| Key set | Required | Optional |
|---------|----------|----------|
| `empty` | none | none |
| `generic` | each parameter of LOOT-CTX-010 | none |
| `chest` | `origin` | `this_entity` |
| `command` | `origin` | `this_entity` |
| `command_slot_source` | `origin`, `container` | `this_entity` |
| `command_compute_default` | `origin` | `this_entity` |
| `command_compute_position` | `origin`, `block_state` | `this_entity`, `block_entity` |
| `command_compute_entity` | `origin`, `target_entity` | `this_entity` |
| `selector` | `origin`, `this_entity` | none |
| `villager_trade` | `origin`, `this_entity`, `additional_cost_component_allowed` | none |
| `fishing` | `origin`, `tool` | `this_entity` |
| `entity` | `this_entity`, `origin`, `damage_source` | `attacking_entity`, `direct_attacking_entity`, `last_damage_player` |
| `equipment` | `origin`, `this_entity` | none |
| `archaeology` | `origin`, `this_entity`, `tool` | none |
| `gift` | `origin`, `this_entity` | none |
| `barter` | `this_entity` | none |
| `vault` | `origin` | `this_entity`, `tool` |
| `advancement_reward` | `this_entity`, `origin` | none |
| `advancement_entity` | `this_entity`, `origin` | none |
| `advancement_location` | `this_entity`, `origin`, `tool`, `block_state` | `block_entity` |
| `block_use` | `this_entity`, `origin`, `block_state` | none |
| `block` | `block_state`, `origin`, `tool` | `this_entity`, `block_entity`, `explosion_radius` |
| `shearing` | `origin`, `this_entity`, `tool` | none |
| `entity_interact` | `target_entity`, `tool` | `interacting_entity` |
| `block_interact` | `block_state`, `origin` | `block_entity`, `interacting_entity`, `tool` |
| `container_process` | `block_entity`, `block_state`, `container`, `origin` | none |
| `enchanted_damage` | `this_entity`, `enchantment_level`, `origin`, `damage_source` | `direct_attacking_entity`, `attacking_entity` |
| `enchanted_item` | `tool`, `enchantment_level` | none |
| `enchanted_location` | `this_entity`, `enchantment_level`, `origin`, `enchantment_active` | none |
| `enchanted_entity` | `this_entity`, `enchantment_level`, `origin` | none |
| `hit_block` | `this_entity`, `enchantment_level`, `origin`, `block_state` | none |

The names of the key sets come from `assets/mcrs/reports/registries.json`. The contents come from
the reference.

`generic` is the default type of a table (LOOT-CTX-013). The load also checks each entry of the
registries `predicate`, `item_modifier`, `slot_source`, `context_int_provider` and
`context_float_provider` against `generic`. Thus such an entry can read each parameter. No vanilla
source evaluates a table with the key set `generic`. A context for `generic` must hold each
parameter of LOOT-CTX-010.

**LOOT-CTX-012** (`source_suggested`, semantic, confidence medium). A source makes the context of
an evaluation with one key set, which the source document states. The context must hold each
required parameter of that key set. It must not hold a parameter that the key set does not allow. A
library that takes a context from its user returns an error for a context that breaks this rule, and
does not evaluate the table. The type of the table does not change the context. Thus a source can
evaluate a table of another type, and a part that reads an absent parameter acts as its rule states
for that case (`loot-conditions.md`, `loot-functions.md`).

**LOOT-CTX-013** (`design`, semantic). Each part of a table can read only the parameters that the
type of the table allows. A part that reads another parameter is a fault of the load, as a fatal
problem of REG-ERR-001. The check follows these steps:

1. Check each pool, entry, condition, modifier, number provider and slot source of the table against
   the type of the table.
2. Check each nested table against the type of the table that contains it. This applies also to a
   table that a key or a tag of tables names, and also to a nested table in a nested table.
3. Check each table also against its own type. An inline table has no own type for this check.

The default type is `generic`, which allows each parameter. Reason: the owner decided in issue #72
that a load with a validation issue is refused. The vanilla data pack has no table that this rule
refuses. LOOT-DEV-100 records the difference from the vanilla server.

**LOOT-CTX-014** (`source_suggested`, semantic, confidence medium). A reference chain
(LOOT-CTX-017) refers back to itself when it enters a table, an item modifier, a predicate or a
named provider that it already entered. That is a fault of the load, as a fatal problem of REG-ERR-001. The reference
chain does not enter it again. For example, a table `t:a` with the entry
`{"type": "minecraft:loot_table", "value": "t:a"}` fails the load. The same applies through a tag
of tables, an inline table, a key of an item modifier, a key of a predicate and a key of a provider.

A table can name the same nested table two times in different places, for example in two pools.
That is not a reference back to itself, because each place continues its own reference chain.

**LOOT-CTX-017** (`design`, semantic). A **reference chain** is a sequence of tables, item
modifiers, predicates, named providers and nested inline providers. A named provider is an entry of
the registry `context_int_provider` or `context_float_provider`. A nested inline provider is an
inline provider in a provider place inside another provider, as the statements below define it. An
evaluation enters each member of the sequence from the member before it. The **length** of a reference chain is the number of its members. These
statements define the length:

- A reference chain starts at each table, item modifier, predicate and named provider that the load
  checks (LOOT-CTX-016). The start adds 1.
- Each table that an evaluation enters adds 1. A table that a key names and an inline table add 1
  alike. A `loot_table` entry enters its tables, in a pool and also in the `entries` of
  `set_contents` (LOOT-EVL-010, LOOT-MOD-053 of `loot-functions.md`).
- Each item modifier that an evaluation enters adds 1. The evaluation enters an item modifier when
  a key of the registry `item_modifier` is in the place of a modifier (LOOT-MOD-010 of
  `loot-functions.md`).
- Each predicate that an evaluation enters adds 1. The evaluation enters a predicate when a key of
  the registry `predicate` is in the place of a condition (LOOT-CND-001 of `loot-conditions.md`).
- Each named provider that an evaluation enters adds 1. The evaluation enters a named provider when
  a key of a provider is in the place of a provider (REG-FLD-722, REG-FLD-723, and section 0.1 of
  `loot-conditions.md`).
- A key, a tag and a list add 0 by themselves. An inline modifier and an inline condition add 0.
- A **provider place** is a field, or an element of a list field, whose rule in `registry-fields.md`
  names the registry `context_int_provider` or `context_float_provider` as its target. A field that
  takes a plain number is not a provider place, for example `weight` of `weighted_list`
  (REG-FLD-732). An inline provider is the value in a provider place that is not a key: an object
  with `type`, or a bare number.
- The **scope** of a provider is its value: its fields, the elements of its list fields, and each
  inline condition in them, at each level of nesting. The scope ends at a key. Thus the scope does
  not continue into a named predicate, a named provider or any other member that a key names.
- An inline provider in the scope of another provider adds 1. The other provider can be a named
  provider or an inline provider. A bare number in such a place adds 1.
- An inline provider in no scope of a provider adds 0, for example a provider in a field of a table,
  of an item modifier or of a condition of a pool. This also applies to the outermost inline
  provider of a named predicate that a provider names by key: the key of the predicate adds 1, and
  the scope starts again in the predicate.
- A `loot_table` entry that names several tables starts one reference chain for each table. A tag
  or a list of item modifiers, of predicates or of named providers does the same.
- The modifier `set_loot_table` names a table, but no evaluation enters that table. It adds 0.

A reference chain has a length of at most 64. A reference chain that gets the length 65 is a fault
of the load, as a fatal problem of REG-ERR-001. The place of the fault is the part that gives the
length 65: the `loot_table` entry that names or holds the table, or the key of the item modifier, of
the predicate or of the named provider, or the inline provider. The load reports the fault one time for each place where a reference chain from
some start gets the length 65.

For example, take these parts of a data pack:

- The table `t:a` has one pool. Its `loot_table` entry has the `value` list of the key `t:b` and an
  inline table `X`.
- The pool of the table `t:b` has the modifier key `t:m`. The item modifier `t:m` is a
  `set_contents` with one `loot_table` entry for `t:c`.
- `X` has one `loot_table` entry with the tag `#t:g`, which holds only `t:c`.
- The pool of the table `t:c` has the condition key `t:p` of a predicate.

From the start `t:a`, the reference chain `t:a`, `t:b`, `t:m`, `t:c`, `t:p` has the length 5. The
reference chain `t:a`, `X`, `t:c`, `t:p` has the length 4. The list, the keys and the tag add 0.

In a second example, each table `t:ci` names `t:c(i+1)`. The tables `t:c1` to `t:c64` load. The
tables `t:c1` to `t:c66` give two faults: at the entry of `t:c64` from the start `t:c1`, and at the
entry of `t:c65` from the start `t:c2`.

In a third example, the named provider `t:n` is a `number_dispatcher` whose `default` is a
`weighted_list`. The `data` of one element of its `distribution` is the bare number `2`. A pool of the table `t:d` has
`rolls` equal to the key `t:n`. From the start `t:d`, the reference chain `t:d`, `t:n`, the
`weighted_list`, the number `2` has the length 4. The same provider written inline in `rolls` gives
the length 3: the `number_dispatcher` adds 0, the `weighted_list` adds 1 and the number adds 1.

A reference back to itself (LOOT-CTX-014) needs a key. An inline provider cannot name itself, so it
cannot make such a cycle by itself. An inline provider can be a member of a cycle only between two
members that keys name.

Inline conditions and inline item modifiers add 0. An inline condition inside an inline condition,
for example `inverted` inside `inverted`, is a step of the evaluation that the chain does not count.
The nesting depth of one file bounds these steps (REG-FLD-010 of `registry-fields.md`).

Reason: an evaluation must not exhaust the stack of its thread, and the check of the load must
stop. An inline provider is a step of the evaluation like a named provider, so the limit must count
it. Otherwise a provider with deep inline nesting passes the check and can exhaust the stack. The
longest reference chain of the vanilla data pack has the length 3, also when inline providers count,
for example from `equipment/trial_chamber_melee` and from the named provider
`cooking/time_wool_slabs`. LOOT-DEV-105 records the difference from the vanilla server.

**LOOT-CTX-016** (`design`, semantic). Each loot validation problem that the server finds at the
load is a fault of the load, as a fatal problem of REG-ERR-001. The load or the reload fails. The
check of the validation goes over all loaded tables, predicates, item modifiers, slot sources and
providers. These are the loot validation problems:

| Problem | Rule |
|---------|------|
| a part reads a parameter that the type of its table does not allow | LOOT-CTX-013 |
| a table, a predicate, an item modifier or a provider refers back to itself | LOOT-CTX-014, REG-ERR-001 |
| a reference chain has a length of more than 64 | LOOT-CTX-017 |
| a composite entry without children | this rule |
| a child of an `alternatives` entry, other than the last child, that has no `condition` | this rule |
| an aggregate provider (LOOT-NUM-011, LOOT-NUM-012) whose `inputs` is empty | this rule |

These faults of the load are not loot validation problems, and REG-ERR-001 lists them in other rows:

- A key of a table, a predicate, an item modifier or a provider that has no entry is an unresolved
  reference (REG-REF-002).
- A `time_check` with `period` 0 fails the decode of its entry (REG-FLD-715).
- An NBT path that the grammar of LOOT-MOD-047 (`loot-functions.md`) does not accept fails the
  decode of its entry.

Reason: the owner decided in issue #72 that a load with a loot validation issue is refused. The
vanilla data pack has none of these problems. LOOT-DEV-100 and LOOT-DEV-102 record the differences
from the vanilla server.

**LOOT-CTX-015** (`source_suggested`, semantic, confidence medium). The luck of a context is a
32-bit float. The source gives it. A source that gives no luck gives 0. The vanilla sources give
these values:

| Source | Luck |
|--------|------|
| a player opens a container with a table, or a container entity with a table | the attribute `minecraft:luck` of that player; 0 without a player |
| a mob dies, and a player killed it | the attribute `minecraft:luck` of that player |
| fishing | the attribute `minecraft:luck` of the angler, plus the fishing luck bonus of the rod |
| brushing a block | the attribute `minecraft:luck` of the player |
| a vault | the attribute `minecraft:luck` of the player |
| each other source, for example a block break (LOOT-CTX-001) | 0 |

The luck changes only the number of rolls (LOOT-EVL-002) and the weights (LOOT-EVL-003). A
condition or a modifier can read it when its own rule says so.

## 3. Evaluation

**LOOT-EVL-001** (`source_suggested`, semantic, confidence medium). An evaluation of a table makes
the drop list. It does these steps:

1. For each pool, in the order of `pools`, do LOOT-EVL-002.
2. Each stack that a pool gives goes through the modifier of the table, when the table has one.
3. When the table is not a nested table, divide the stack (LOOT-OUT-001).
4. Put each stack that results at the end of the drop list.

A table without pools gives an empty drop list. LOOT-EVL-012 states the order of the stacks and of
the draws.

**LOOT-EVL-002** (`source_suggested`, semantic, confidence medium). A pool does these steps:

1. Test the condition of the pool, when the pool has one. When it fails, the pool gives nothing and
   takes no other draw.
2. Let `a` be the int read of `rolls` (LOOT-NUM-001 of `loot-conditions.md`), with the draws of
   the provider. An evaluation error gives 0.
3. Let `b` be the float read of `bonus_rolls` (LOOT-NUM-001), with the draws of the provider. An
   evaluation error, a NaN and an infinity give 0. The default is the constant 0. The pool reads
   `b` also when the luck is 0.
4. Let `n` be `a + floor(b × luck)`. The product is a 32-bit float product.
5. Do LOOT-EVL-003 `n` times. When `n` is 0 or less, the pool does no roll.
6. Each stack that a roll gives goes through the modifier of the pool, when the pool has one.

**LOOT-EVL-003** (`source_suggested`, semantic, confidence medium). One roll of a pool does these
steps:

1. Expand each entry of `entries`, in order (LOOT-EVL-004). The leaf results that the expansions
   give are the candidates, in the order in which the expansions give them.
2. The weight of a candidate is `max(floor(weight + quality × luck), 0)`. The sum and the product
   are 32-bit float operations. `weight` defaults to 1 and `quality` defaults to 0.
3. Remove each candidate with the weight 0.
4. When no candidate remains, the roll gives nothing. This takes no draw.
5. When one candidate remains, it makes its stacks. This takes no draw.
6. When more candidates remain, let `t` be the sum of their weights. When `t` is more than
   2³¹ − 1, the roll gives nothing and takes no draw. LOOT-DEV-103 records the reference.
7. Otherwise take the draw `k = B(t)` (LOOT-RND-006). Go through the candidates in order, and
   subtract the weight of each from `k`. The first candidate for which `k` becomes less than 0
   makes its stacks.

Thus one roll makes the stacks of at most one candidate. A candidate with the weight 0 still had its
condition tested in step 1. A `tag` or `loot_table` entry with `expand` true can give more than one
candidate (LOOT-EVL-009, LOOT-EVL-010). Each of these candidates has the weight and the quality of
the entry.

**LOOT-EVL-004** (`source_suggested`, semantic, confidence medium). An expansion of an entry gives
leaf results and a result, true or false. It first tests the condition of the entry, when the entry
has one. When the condition fails, the expansion gives no leaf result and gives false. Otherwise:

- `item`, `empty`, `dynamic` and `slots` give one leaf result and give true;
- `tag` and `loot_table` give the leaf results of LOOT-EVL-009 and LOOT-EVL-010, and give true, also
  when they give no leaf result;
- `alternatives` expands its children in order, until one gives true. It gives true when a child
  gave true. It gives false when no child gave true, and also when it has no children;
- `sequence` expands its children in order, until one gives false. The leaf results that the earlier
  children gave stay. It gives false when a child gave false. It gives true otherwise, and also when
  it has no children;
- `group` expands each child in order. It gives true, also when it has one child that gives false.
  LOOT-DEV-104 records the reference.

A child that the expansion does not reach is not tested and takes no draw. The expansion tests each
condition again in each roll. A leaf result of a child keeps the modifier of each composite entry
that contains it (LOOT-EVL-011).

In a roll, the expansion of each entry of the pool ends before a candidate makes its stacks
(LOOT-EVL-003). The modifier `set_contents` uses the same expansion, but each leaf result makes its
stacks when the expansion gives it (LOOT-MOD-053 of `loot-functions.md`).

**LOOT-EVL-005** (`source_suggested_black_box_confirmed`, semantic, confidence high). An
`alternatives` entry thus selects the first child whose condition passes. For example, the table of
`stone` gives `stone` when the tool can Silk Touch, and otherwise `cobblestone`.

**LOOT-EVL-006** (`source_suggested`, semantic, confidence medium). A selected candidate makes its
stacks:

- `item` makes one stack of the item `name`, with the count 1 and no component patch;
- `empty` makes nothing;
- `tag` makes the stacks of LOOT-EVL-009;
- `loot_table` makes the stacks of LOOT-EVL-010;
- `dynamic` makes the stacks of LOOT-EVL-007;
- `slots` makes the stacks of LOOT-EVL-013.

Each stack then goes through the modifiers of LOOT-EVL-011.

**LOOT-EVL-007** (`source_suggested`, semantic, confidence medium). A `dynamic` entry makes the
stacks that the source gives for the name `name`. A source can give a list of stacks for each name.
When the source gives nothing for the name, the entry makes nothing. The vanilla sources give these
names, and only in the evaluation of a block table for a block with this block entity
(`block-loot.md`):

| Name | Block entity | Stacks, in order |
|------|--------------|------------------|
| `minecraft:sherds` | a decorated pot | a stack of the decoration of each side that has one in the component `minecraft:pot_decorations`, with the item, the count and the components of the decoration, in the order left, back, front, right |
| `minecraft:contents` | a shulker box | the stack of each slot, in the order of the slots, also the empty stacks |

A side without a decoration gives no stack. A pot from an item without changes has `brick` on each
side, so it gives four `brick`. The table of `decorated_pot` has `dynamic` only for a cracked pot.
A pot that is not cracked drops itself (LOOT-MOD-006, `loot-functions.md`). No vanilla table uses
`minecraft:contents`.

A source can give the empty stack for a name, for example for an empty slot of a shulker box. In
the modifiers of LOOT-EVL-011, that stack is an empty work stack with the item `minecraft:air`
(LOOT-MOD-011 of `loot-functions.md`). Each modifier tests its condition and takes the draws that
its rule states for an empty work stack. A modifier can give the empty work stack a count of more
than 0 and a component patch, for example `set_count` 3 and `set_name`. Its item stays
`minecraft:air`, so it stays an empty work stack until the division. The division makes it the
empty stack, with no component patch, and keeps it in the drop list (LOOT-OUT-001).

**LOOT-EVL-009** (`source_suggested`, semantic, confidence medium). A `tag` entry names a set of
items in `items`: a tag, one item or a list of items. The order of a tag is the member order of
REG-TAG-005. The order of a list is the order of the file.

- With `expand` false, the entry gives one leaf result. When it is selected, it makes one stack of
  each item of the set, in order, each with the count 1 and no component patch.
- With `expand` true, the entry gives one leaf result for each item of the set, in order. When one
  of them is selected, it makes one stack of its item, with the count 1 and no component patch.

An empty set makes nothing. With `expand` true, an empty set gives no candidate. For example, the
table of `creeper` gives one disc of the tag `minecraft:creeper_drop_music_discs`, with an equal
chance for each disc.

**LOOT-EVL-010** (`source_suggested`, semantic, confidence medium). A `loot_table` entry names a
set of tables in `value`. Each element is the key of a table, or an inline table object. `value`
can also be a tag of tables. The order is as in LOOT-EVL-009.

- With `expand` false, the entry gives one leaf result. When it is selected, it evaluates each table
  of the set, in order.
- With `expand` true, the entry gives one leaf result for each table of the set. When one of them is
  selected, it evaluates its table.

A nested table uses the context of the table that contains it: the same parameters, the same luck,
the same dynamic names and the same generator. The type and the `random_sequence` of a nested table
have no effect on the evaluation. A nested table applies its own pools and its own table modifier
(LOOT-EVL-001), but it does not divide its stacks (LOOT-OUT-001). For example, the table
`gameplay/fishing` selects one of `gameplay/fishing/junk`, `gameplay/fishing/treasure` and
`gameplay/fishing/fish`, by weight and quality.

**LOOT-EVL-011** (`source_suggested`, semantic, confidence medium). Each stack that a candidate makes
goes through these modifiers, in this order. A part without a modifier passes the stack unchanged.

1. The modifier of the leaf entry.
2. The modifier of each composite entry that contains the leaf entry, from the nearest to the
   farthest.
3. The modifier of the pool (LOOT-EVL-002).
4. The modifier of the table (LOOT-EVL-001).

For a stack of a nested table, steps 1 to 4 of the nested table come first. Then the stack goes
through the modifiers of the `loot_table` entry and of the parts around it. `loot-functions.md`
states what each modifier does and its draws.

A stack whose count becomes 0 or less keeps its item and its component patch through all these
modifiers, also through the modifiers around a nested table (LOOT-MOD-011 of `loot-functions.md`).
A later modifier can make its count more than 0 again. The division (LOOT-OUT-001) is the one point
where a stack with a count of 0 or less, or with the item `minecraft:air`, becomes the empty stack.

**LOOT-EVL-012** (`source_suggested`, semantic, confidence medium). The evaluation makes one stack at
a time. Each stack goes through all its modifiers of LOOT-EVL-011 and then through the division
(LOOT-OUT-001), and reaches the drop list, before the evaluation makes the next stack. Thus the draws of the modifiers
of a stack come before each draw that the evaluation takes for the next stack. For example, a
`loot_table` entry evaluates the first pool of its nested table. The modifiers of the outer pool
apply to each stack of that first pool before the nested table tests the condition of its second
pool.

**LOOT-EVL-013** (`source_suggested`, semantic, confidence medium). A `slots` entry makes a copy of
the stack of each slot that its `slot_source` selects, in the order of the selection. It removes
each empty stack after the selection. A slot source selects slots as follows. A slot source takes no
draw.

| Slot source | Selection |
|-------------|-----------|
| `slot_range` | the slots of the range `slots` of the parameter that `source` names, in the order of the range. `source` is a name of LOOT-CTX-010, default `container`. A slot of the range that the entity or block entity does not have is not selected. When the parameter is absent, or has no slots, nothing is selected. |
| `group` | the slots of each source of `terms`, in order, one after the other |
| `filtered` | the slots of `slot_source` whose stack matches the item predicate `item_filter` (`predicates.md`) |
| `limit_slots` | the first `limit` slots of `slot_source` |
| `contents` | for each slot of `slot_source`, in order, the slots of the contents of its stack in the component `component`, in the order of that component. A stack without the effective component `component` gives no slot. |
| `empty` | nothing |

The names of a slot range are those of the command `/item`, for example `container.*` and
`armor.head`. An empty slot counts for `limit_slots`, because the entry removes the empty stacks
only at the end.

## 4. Division

**LOOT-OUT-001** (`source_suggested`, semantic, confidence medium). The evaluation of a table that is
not a nested table divides each stack, after the modifier of the table:

- A stack with a count of 0 or less, or with the item `minecraft:air`, becomes the empty stack. The
  empty stack has no component patch. It stays in the drop list, so the drop list can hold empty
  stacks. Each source states what it does with them.
- Another stack whose item is not enabled by the feature flags of the world is removed.
- Another stack with a count less than its maximum stack size stays one stack.
- A larger stack becomes stacks of the maximum stack size, in order, and then the rest when it is
  not 0.

For example, 70 `snowball` with the maximum 16 become four stacks of 16 and one stack of 6. A
nested table does not divide its stacks (LOOT-EVL-010), so the modifiers of the containing table see
the stacks before the division.

## 5. The spread over a container

**LOOT-CTR-001** (`source_suggested`, semantic, confidence medium). A source can fill a container
from a table, for example at the first opening of a chest that has a table. The source evaluates
the table with its context (LOOT-EVL-001). Then it spreads the drop list over the container, with
the same generator (LOOT-RND-004). The spread does the steps of LOOT-CTR-002 to LOOT-CTR-005, in
this order. A slot that is not free stays as it is.

**LOOT-CTR-002** (`source_suggested`, semantic, confidence medium). The spread lists the free slots
of the container in ascending order of the slot number. Then it shuffles that list (LOOT-RND-007).

**LOOT-CTR-003** (`source_suggested`, semantic, confidence medium). The spread prepares two lists
from the drop list. It goes through the drop list in order:

- it removes each empty stack;
- it moves each stack with a count of more than 1 to the end of the split list;
- it keeps each stack with the count 1 in the kept list, in order.

Let `f` be the number of free slots. While `f` minus the length of both lists is more than 0, and the
split list is not empty, the spread does these steps:

1. Take the draw `i = between(0, s − 1)`, where `s` is the length of the split list. Remove the
   stack at the position `i` of the split list. Call it P, with the count `c`.
2. Take the draw `k = between(1, floor(c / 2))`. Make a new stack Q with the count `k`. Q is a copy
   of P with all its components. The count of P becomes `c − k`.
3. When the count of P is more than 1, take the draw `Bool`. When it is true, put P at the end of
   the split list. Otherwise, or when the count of P is 1, put P at the end of the kept list.
4. Do step 3 for Q.

When the loop stops, the spread puts the stacks of the split list, in order, at the end of the kept
list. Then it shuffles the kept list (LOOT-RND-007).

**LOOT-CTR-004** (`source_suggested`, semantic, confidence medium). The spread puts each stack of the
kept list, in order, in a free slot. The slot is the last slot of the shuffled list of LOOT-CTR-002.
The spread then removes that slot from the list. When no free slot remains, the spread stops. The
stacks that remain are lost, and no item entity appears for them.

**LOOT-CTR-005** (`source_suggested`, semantic, confidence medium). The draws of the spread come
after all draws of the evaluation. Their order is: the shuffle of the free slots, the draws of the
loop, and the shuffle of the kept list. For example, a drop list of three stacks with the count 1
and a container with 27 free slots takes 26 draws for the first shuffle, no draw in the loop, and
2 draws for the second shuffle.

## 6. Random values

**LOOT-RND-002** (`source_suggested`, semantic, confidence medium). The draws of an evaluation are a
sequence. Each draw has its place in the order and its kind (LOOT-RND-006). The rules of this
document, of `loot-conditions.md` and of `loot-functions.md` state each draw. An implementation
takes the same draws, in the same order, from the generator that the caller gives. Two modes use
this one sequence:

- Exact mode. The caller gives the generator of LOOT-RND-004 in its state before the evaluation.
  The drop list, the contents of the container and the state of the generator after the evaluation
  must be those of the vanilla server.
- Keyed mode. The caller gives a keyed stream, for example the stream of LOOT-RND-001 for a block or
  of LOOT-RND-009 for another source. The support and the probability of each result must be those
  of the vanilla server.

A parity test in exact mode compares the stacks and the state of the generator. A parity test in
keyed mode compares the distribution of the results over many evaluations.

**LOOT-RND-004** (`source_suggested`, semantic, confidence medium). In the vanilla server, an
evaluation takes its draws from one of these generators. The first row that applies selects the
generator.

| Condition | Generator |
|-----------|-----------|
| the source gives a generator, for example the dimension random (SIM-RNG-001) for the display item of a vault | that generator |
| the source gives a seed that is not 0, for example the save field `LootTableSeed` of a container or `DeathLootTableSeed` of a mob | a new 48-bit generator (BMR-PICK-003), set to that seed |
| the table has a `random_sequence` | the sequence stream of that id (LOOT-RND-005) |
| otherwise | the dimension random (SIM-RNG-001) |

The seed 0 means "no seed". A nested table uses the generator of the table that contains it
(LOOT-EVL-010). The spread over a container continues with the same generator (LOOT-CTR-001). In
mcrs, LOOT-RND-009 replaces the generators of the first, the third and the fourth row by a keyed
stream.

**LOOT-RND-009** (`design`, semantic). In mcrs, an evaluation for a source without a seed in the
save takes its draws from a keyed stream. A seed of 0 is no seed. This applies to each source, for
example the death of a mob without `DeathLootTableSeed`. LOOT-RND-001 states the stream of the
block source. For each other source, the key holds these values:

- the world seed;
- the id of the table;
- the game tick of the evaluation;
- a value that the source document states, which tells the source apart from each other source of
  the same table in the same tick, for example the UUID of the mob.

This is the keyed mode of LOOT-RND-002. A source with a seed that is not 0 keeps the 48-bit
generator of LOOT-RND-004. mcrs keeps no sequence stream and saves none, and `random_sequence` has
no effect on the draws. Thus a result does not depend on the order of the evaluations in a tick, on
the thread count, or on the earlier evaluations of the same table. Reason: the owner decided in
issue #367 that loot is at the tier "independent of the scheduler".

A caller that holds the state of a sequence stream (LOOT-RND-005) can give that generator in exact
mode (LOOT-RND-002). The result is then the result of the vanilla server. LOOT-DEV-101 records the
difference from the vanilla server.

**LOOT-RND-005** (`source_suggested`, semantic, confidence medium). The vanilla server keeps one
sequence stream for each sequence id. There is one set of streams for the server, and the world
saves it. Each evaluation with the id continues its stream. A new stream is a Xoroshiro128++
generator with a 128-bit state `(lo, hi)`. The server makes the state from the world seed `w` and
the id, as follows. All operations are on 64-bit integers, modulo 2⁶⁴.

1. Let `a = w XOR 0x6A09E667F3BCC909` and `b = a + 0x9E3779B97F4A7C15`.
2. Take the MD5 hash of the UTF-8 text of the id, with its namespace, for example
   `minecraft:blocks/stone`. Let `p` be its first 8 bytes and `q` its last 8 bytes, each read as a
   big-endian integer.
3. Let `lo = mix(a XOR p)` and `hi = mix(b XOR q)`.

`mix(z)` does `z = (z XOR (z >> 30)) × 0xBF58476D1CE4E5B9`, then `z = (z XOR (z >> 27)) ×
0x94D049BB133111EB`, then gives `z XOR (z >> 31)`. Each shift is a logical shift. The command
`/random reset` can change this derivation. That command is out of the scope of this document.

**LOOT-RND-006** (`source_suggested`, semantic, confidence medium). A draw is of one of the kinds of
this table. The loot documents (section "Loot documents" of `loot.md`) use this notation
and no other kind of draw. A rule can give the value of a draw a name, for example `k = B(t)`.

| Notation | Kind | Value | Draws |
|----------|------|-------|-------|
| `F` | float draw | a uniform 32-bit float in `[0, 1)` | one |
| `B(n)`, for `1 ≤ n ≤ 2³¹ − 1` | bounded draw | a uniform int in `0..n − 1`. `B(1)` gives 0. | one |
| `between(a, b)` | draw in a range | `a` when `a ≥ b`. Otherwise `a + B(b − a + 1)`. | none when `a ≥ b`; otherwise one `B` |
| `Bool` | boolean draw | true or false, with the chance 0.5 each | one |
| `shuffle` of a list | shuffle | LOOT-RND-007 | the `B` draws of LOOT-RND-007 |

There is no draw `B(n)` for a bound `n` outside `1..2³¹ − 1`. A rule that can meet such a bound
states its result for it, and that result takes no draw (LOOT-EVL-003, LOOT-NUM-009 of
`loot-conditions.md`, LOOT-MOD-004 of `loot-functions.md`).

The 48-bit generator and the dimension random make each kind as follows:

- `B(n)` is `next_int(n)` of BMR-PICK-003;
- `F` is `next(24) / 2²⁴`;
- `Bool` is true when `next(1)` is not 0.

A Xoroshiro128++ generator makes the 64-bit value `x` and changes its state as follows: `x =
rotl(lo + hi, 17) + lo`; `h = hi XOR lo`; the new `lo` is `rotl(lo, 49) XOR h XOR (h << 21)`; the
new `hi` is `rotl(h, 28)`. It makes each kind as follows:

- `B(n)`: let `u` be the low 32 bits of `x`, as an unsigned value, and `m = u × n`. When the low
  32 bits of `m` are less than `n`, let `t = (2³² − n) mod n`. While the low 32 bits of `m` are
  less than `t`, take a new `x` and calculate `u` and `m` again. The value is `m >> 32`.
- `F`: `(x >> 40) / 2²⁴`, with a logical shift.
- `Bool`: true when the lowest bit of `x` is 1.

A keyed stream makes each kind with the distribution of the table above. Its method is unspecified,
because keyed mode compares only the distribution (LOOT-RND-002).

**LOOT-RND-007** (`source_suggested`, semantic, confidence medium). A shuffle of a list of length
`n` does these steps. For each `i` from `n` down to 2, take the draw `j = B(i)`. Then swap the
elements at the positions `i − 1` and `j`. Positions start at 0. Thus a shuffle takes `n − 1` draws
when `n ≥ 2`, and no draw otherwise.

**LOOT-RND-008** (`source_suggested`, semantic, confidence medium). The draws of one evaluation of a
table come in this order:

1. For each pool, in order: the draws of the condition of the pool, then of `rolls`, then of
   `bonus_rolls` (LOOT-EVL-002).
2. For each roll: the draws of the conditions of the expansion, in the order of LOOT-EVL-004. Then
   the draw `B(t)` of LOOT-EVL-003, when two or more candidates remain.
3. For the selected candidate: the draws of the nested tables that it evaluates (LOOT-EVL-010). For
   each stack, as LOOT-EVL-012 states, the draws of its modifiers in the order of LOOT-EVL-011.
4. For a spread: the draws of LOOT-CTR-005, after all draws above.

The entry types `item`, `empty`, `tag`, `dynamic` and `slots` take no draw of their own.

## 8. Intentional deviations

### 8.1 Intentional deviations

**LOOT-DEV-100.**

- **Rule**: LOOT-CTX-013.
- **Reference**: a part that reads a parameter that the type of the table does not allow gives a
  warning in the log. The table loads. At the evaluation, the part reads an absent parameter.
- **Visible to the client**: no. The author of the data pack sees a load fault in place of a warning.

**LOOT-DEV-101.**

- **Rule**: LOOT-RND-009.
- **Reference**: an evaluation without a seed in the save continues the sequence stream of the
  `random_sequence` of the table (LOOT-RND-005), or takes the dimension random
  (SIM-RNG-001). The world saves the sequence streams. The drops thus depend on the order of all
  earlier evaluations of the same sequence, also in other dimensions.
- **Visible to the client**: no, as a distribution. A player cannot repeat a drop of the vanilla
  server from the world seed alone.

**LOOT-DEV-102.**

- **Rule**: LOOT-CTX-016.
- **Reference**: a composite entry without children, a child of `alternatives` without `condition`
  before the last child, and an aggregate provider with an empty `inputs` give a warning in the log.
  The load does not fail.
- **Visible to the client**: no. The author of the data pack sees a load fault in place of a warning.

**LOOT-DEV-103.**

- **Rule**: LOOT-EVL-003.
- **Reference**: the sum of the weights is a 32-bit int, and a sum above 2³¹ − 1 wraps. A negative
  sum after the wrap stops the evaluation with an error that nothing catches. The action that
  evaluates the table fails. A sum of 0 after the wrap gives nothing. A positive sum after the wrap
  takes a draw with that bound and selects a candidate with other chances.
- **Visible to the client**: no. The author of the data pack sees no stack in place of an error or
  a different selection. No vanilla table has such weights.

**LOOT-DEV-104.**

- **Rule**: LOOT-EVL-004.
- **Reference**: a `group` with exactly one child gives the result of that child. Thus it gives
  false when the condition of the child fails. A `group` with no child or with two or more
  children gives true.
- **Visible to the client**: no. The author of the data pack sees a different selection when such
  a `group` is a child of `alternatives` or `sequence`. No vanilla table uses `group`.

**LOOT-DEV-105.**

- **Rule**: LOOT-CTX-017.
- **Reference**: the vanilla server has no limit on the length of a reference chain. A reference
  chain of 65 tables loads, and its evaluation gives the stacks of the last table. A much longer
  reference chain exhausts the stack of the thread, at the check of the load or at the evaluation.
  This applies also to a reference chain through keys of item modifiers, of predicates and of
  providers. The vanilla server also has no limit for inline providers inside a provider. Only the
  nesting limit of its JSON reader bounds them.
- **Visible to the client**: no. The author of a data pack sees a load fault for a reference chain
  with a length of 65 or more. No vanilla reference chain has a length of more than 3.
