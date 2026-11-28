# Predicates: behavioural specification

This document states what each shared predicate passes for. A predicate is a test that a data pack
writes as data. Loot conditions, enchantment effects, advancements, item components and commands use
the same predicates. This document states the result of each predicate for each input. It also
states the values that a caller and a host must supply. It does not prescribe an architecture.

The scope is these predicates:

- the number bounds;
- the item predicate, with each data component predicate and the collection predicate;
- the block predicate, the state properties predicate and the fluid predicate;
- the light predicate, the location predicate and the distance predicate;
- the NBT predicate and the mob effects predicate;
- the damage source predicate and the damage predicate;
- the entity predicate, with each sub-predicate.

`registry-fields.md`, rules REG-FLD-595 to REG-FLD-646, states how the server decodes each
predicate. This document refers to those rules by id and does not repeat them.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**: as `login.md`, section 0.1, defines them.
- **Predicate**: a value of one of the types of the scope.
- **Subject**: the value that a predicate tests, for example a stack or an entity.
- **Pass**, **fail**: the two results of a predicate and of each test of a predicate.
- **Caller**: the part of the server that evaluates a predicate, for example a loot condition.
- **Host**: the part of the server that supplies the values of the world to a predicate.
- **Absent value**: a value that the host does not have at the time of the evaluation.
- **Bounds**: a number bounds value (REG-FLD-646). It has an optional `min` and an optional `max`.
- **Entry set**: a reference of the kind "entry set" of `registry-fields.md`. It is one id, a list
  of ids or one tag. A value is in the entry set when the list holds its id, or when the tag
  contains it.
- **Stack**, **empty stack**, **count**, **effective component**: as `item-stack.md` defines them
  (STK-EMP-001, STK-PAT-002).
- **Component holder**: a subject that has data components. It is a stack, a block entity or an
  entity.
- **Dimension**: the dimension that the caller supplies with the subject.
- **Loaded position**: a block position that passes the loaded check of `argument-coordinates.md`
  (ARG-COORD-013). The position is inside the build height of the dimension. Its x and z are from
  −30 000 000 to 29 999 999. The server holds the chunk of the position with its full data.
- **Block position of a position**: the block position with the floor of each coordinate.
- **Origin**: an optional position that the caller supplies to an entity predicate (PRED-ENT-003).
- **Saved data of a block entity**: the compound of `argument-block.md` (ARG-BLK-012).
- **Saved data of an entity**: the compound that `argument-entity.md` states for the option `nbt`
  (ARG-ENT-015).
- **Match**: the relation between two NBT tags that `argument-block.md` states (ARG-BLK-011, part
  "Match"). This document uses this word only for that relation.
- **Living entity**, **pose**, **on-ground flag**, **fall distance**, **sprint state**, **swim
  state**, **elytra flight**, **support position**, **movement position**: as `entity-physics.md`
  defines them.
- **Mob**, **line of sight**: as `entity-ai-timing.md`, section 0.1, defines them.
- **Burning**: as `environment-damage.md`, section 0.1, defines it.
- **Entity tick count**: the number of ticks that an entity has lived (`status-effects.md`).
- **Causing entity**, **direct entity**: as `entity-damage.md`, section 0.1, defines them.
- **Active effects**, **amplifier**, **duration**, **ambient**, **visible**: as `status-effects.md`,
  section 0.1, defines them.
- **Raw brightness**: the light value of PRED-LOC-007.
- **Reported movement**: the movement of an entity that PRED-ENT-011 states.

An integer range `a..b` includes `a` and `b`. `in(B, v)` is the bounds test of PRED-NUM-001 or
PRED-NUM-002 for the bounds `B` and the value `v`.

### 0.2 Provenance

- `source_suggested`: taken from the reference implementation, with no experiment. The
  confidence is at most medium.
- `design`: a choice of this project. The rule states the reason.
- `unresolved`: not decided. Section 12.3 collects these items.

Each rule states its kind, as `docs/spec/README.md`
defines it.

### 0.3 Artefacts cited

- The data pack of the client jar: `data/minecraft/predicate/`, `data/minecraft/loot_table/`,
  `data/minecraft/advancement/` and `data/minecraft/enchantment/`.
  `tools/mcrs_minecraft_fixtures` installs it. These files show which predicates the game uses.
  They do not show a result.
- The generated block definitions of this repository: `assets/mcrs/block_definition/`.
  `cargo run -p mcrs_minecraft_update` writes them. They list the values of each property of each
  block, in the order of the block report (PRED-BLK-009).
- The generated item definitions of this repository: `assets/mcrs/item_definition/`.
  `cargo run -p mcrs_minecraft_update` writes them. They give the components of each prototype
  (PRED-ITM-007).

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `PRED-GEN` | rules for all predicates |
| `PRED-NUM` | number bounds |
| `PRED-ITM` | the item predicate and the data component predicates |
| `PRED-COL` | the collection predicate |
| `PRED-BLK` | the block predicate, the state properties predicate and the fluid predicate |
| `PRED-LOC` | the light predicate and the location predicate |
| `PRED-DST` | the distance predicate |
| `PRED-NBT` | the NBT predicate |
| `PRED-EFX` | the mob effects predicate |
| `PRED-DMG` | the damage source predicate and the damage predicate |
| `PRED-ENT` | the entity predicate and its sub-predicates |
| `PRED-HST` | the values that a host supplies |
| `PRED-DEV` | intentional deviations |

## 1. Rules for all predicates

**PRED-GEN-001** (`source_suggested`, semantic, confidence medium). A predicate is a function of its
data and of the values that it reads. The result is pass or fail. The predicate reads each value at
the time of the evaluation. The evaluation changes no value of the world. It does not load a chunk,
and it does not generate a chunk.

**PRED-GEN-002** (`source_suggested`, semantic, confidence medium). Each field that is present in a
predicate adds one test. An absent field adds no test, unless a rule states a default value for it.
The predicate passes when each of its tests passes. The order of the tests has no observer.

**PRED-GEN-003** (`source_suggested`, semantic, confidence medium). No predicate of this document
takes a random value. Two evaluations of one predicate with equal values have equal results.

**PRED-GEN-004** (`design`, semantic). A test that reads an absent value fails. This rule applies to
each expected value. For example, `smokey: false` fails when the host does not have the blocks under
the position. Reason: the vanilla server has each value of section 10, and it fails each test of a
position that is not loaded. A test without its value cannot show that the subject agrees with the
data.

**PRED-GEN-005** (`source_suggested`, semantic, confidence medium). A predicate with no field is an
empty predicate, written `{}`. An empty predicate passes for each subject, with the exceptions of
this table.

| Empty predicate | Fails for |
|-----------------|-----------|
| entity predicate | an absent entity (PRED-ENT-001) |
| block predicate, fluid predicate, light predicate | a position that is not a loaded position (PRED-BLK-001, PRED-BLK-010, PRED-LOC-006) |
| NBT predicate | an absent tag (PRED-NBT-001) |
| mob effects predicate | an entity that is not a living entity (PRED-EFX-001) |
| distance predicate in an entity predicate | an absent origin (PRED-ENT-009) |
| `stepping_on` | an entity whose on-ground flag is false (PRED-ENT-007) |
| `equipment` | an entity that is not a living entity (PRED-ENT-015) |
| each data component predicate but `custom_data` | a component holder without the component (PRED-ITM-007) |
| `type_specific/lightning`, `type_specific/player`, `type_specific/cube_mob`, `type_specific/sheep` | an entity of a different type (section 9.4) |
| `type_specific/raider` | an entity that is not a raider, a raider with a raid and a raid captain (PRED-ENT-033) |

The empty location predicate passes for each position, also for a position that is not a loaded
position. The empty `type_specific/fishing_hook` passes for each entity.

## 2. Number bounds

**PRED-NUM-001** (`source_suggested`, semantic, confidence medium). Integer bounds pass for an
integer `v` when both of these are true:

- `min` is absent, or `min ≤ v`;
- `max` is absent, or `v ≤ max`.

Each bound is inclusive. The bounds `{}` pass for each value. A bare number `n` is the bounds with
`min` and `max` equal to `n` (REG-FLD-646).

**PRED-NUM-002** (`source_suggested`, semantic, confidence medium). Double bounds pass for a value
`v` with the same two conditions, in 64-bit floats. When the subject value is a 32-bit float, the
test converts it to a 64-bit float with no change of the value. For example, the bounds `0.1` fail
for the 32-bit float nearest to 0.1.

**PRED-NUM-003** (`source_suggested`, semantic, confidence medium). Some rules use the squared form
of double bounds. The caller of the squared form supplies a squared quantity `s`. The squared form
passes when both of these are true:

- `min` is absent, or `min × min ≤ s`;
- `max` is absent, or `s ≤ max × max`.

The products are 64-bit floats. The squared form has the same result as PRED-NUM-002 for the square
root of `s`, except for the rounding of the root.

**PRED-NUM-004** (`unresolved`, unspecified). A squared form with a negative bound has no stated
result. The reference squares the bound, so `min: -2.0` acts as `min: 2.0`. A distance and a speed
are never negative, so a data pack has no use for such a bound. A parity test must not compare this
case.

**PRED-NUM-005** (`source_suggested`, semantic, confidence medium). Bounds with `min` larger than
`max` pass for no value. The decoder of integer bounds and of double bounds rejects such bounds, so
a loaded predicate does not hold them (REG-FLD-646).

## 3. The item predicate

### 3.1 The predicate

**PRED-ITM-001** (`source_suggested`, semantic, confidence medium). The subject of an item predicate
(REG-FLD-613) is a stack. The predicate reads the item of the stack, the count and the effective
components. The predicate fails when the stack is an absent value, for example the stack of a slot
that the host does not have (PRED-ENT-044). An empty slot holds the empty stack, which is not an
absent value. A slot that the entity does not have gives no stack, and PRED-ENT-019 skips it.

**PRED-ITM-002** (`source_suggested`, semantic, confidence medium). The fields of an item predicate
add these tests.

| Field | The test passes when |
|-------|----------------------|
| `items` | the item of the stack is in the entry set |
| `count` | `in(count, c)`, where `c` is the count of the stack |
| `components` | PRED-ITM-004 passes for the stack |
| `predicates` | PRED-ITM-005 passes for the stack |

**PRED-ITM-003** (`source_suggested`, semantic, confidence medium). The empty stack is a subject
with the item `minecraft:air`, the count 0 and no component. The predicate `{}` thus passes for the
empty stack. A test `count` with `min: 1` fails for it. A test `items` passes for it only when the
entry set contains `minecraft:air`.

### 3.2 The data component matchers

The fields `components` and `predicates` are the data component matchers (REG-FLD-596). The item
predicate, the block predicate and the entity predicate have them. The subject of each rule of this
section and of section 3.3 is a component holder.

**PRED-ITM-004** (`source_suggested`, semantic, confidence medium). The field `components` is an
exact predicate (REG-FLD-595). It lists component ids with a value for each. The test passes when,
for each listed id, the holder has the component and its value equals the listed value.
`item-stack.md` states the equality of two component values (STK-EQ-005, STK-EQ-006). An empty list
passes for each holder.

**PRED-ITM-005** (`source_suggested`, semantic, confidence medium). The field `predicates` is a map
of data component predicates (REG-FLD-597). The test passes when each entry of the map passes. An
empty map passes for each holder.

**PRED-ITM-006** (`source_suggested`, semantic, confidence medium). An entry of `predicates` whose
key is a component id, and not a predicate type id, passes when the holder has that component. The
value of the component has no effect.

**PRED-ITM-007** (`source_suggested`, semantic, confidence medium). Each data component predicate
reads one component of the holder, as this table shows. The predicate fails when the holder does not
have the component. The predicate `custom_data` is the one exception (PRED-ITM-014).

| Predicate type | Component | Rule |
|----------------|-----------|------|
| `damage` | `minecraft:damage`, and `minecraft:max_damage` | PRED-ITM-010 |
| `enchantments` | `minecraft:enchantments` | PRED-ITM-011 |
| `stored_enchantments` | `minecraft:stored_enchantments` | PRED-ITM-011 |
| `potion_contents` | `minecraft:potion_contents` | PRED-ITM-013 |
| `custom_data` | `minecraft:custom_data` | PRED-ITM-014 |
| `container` | `minecraft:container` | PRED-ITM-015 |
| `bundle_contents` | `minecraft:bundle_contents` | PRED-ITM-015 |
| `firework_explosion` | `minecraft:firework_explosion` | PRED-ITM-016 |
| `fireworks` | `minecraft:fireworks` | PRED-ITM-017 |
| `writable_book_content` | `minecraft:writable_book_content` | PRED-ITM-018 |
| `written_book_content` | `minecraft:written_book_content` | PRED-ITM-019 |
| `attribute_modifiers` | `minecraft:attribute_modifiers` | PRED-ITM-020 |
| `trim` | `minecraft:trim` | PRED-ITM-021 |
| `jukebox_playable` | `minecraft:jukebox_playable` | PRED-ITM-022 |
| `villager/variant` | `minecraft:villager/variant` | PRED-ITM-023 |

For a stack, the component is the effective component of STK-PAT-002. A stack with no patch entry
for the component type thus reads the component of the prototype. In this game version, each
prototype has `minecraft:enchantments` with the empty map `{}`, and
`minecraft:attribute_modifiers` (STK-PRO-004, STK-SHP-015). Each file of
`assets/mcrs/item_definition/` shows both components. Thus these results apply:

- `enchantments` with the empty list passes for each stack that is not empty, when the patch has no
  removal entry for `minecraft:enchantments`. `attribute_modifiers` `{}` acts in the same way.
- A stack with a removal entry for the component fails. The empty stack fails (STK-PAT-003).
- An item that a later game version adds can have a prototype without the component. Its stack
  with no patch entry fails.

### 3.3 The data component predicates

**PRED-ITM-010** (`source_suggested`, semantic, confidence medium). `damage` (REG-FLD-598). Let `d`
be the value of the component `minecraft:damage`. Let `m` be the value of `minecraft:max_damage`, or
0 when the holder does not have it. The predicate passes when `in(durability, m − d)` and
`in(damage, d)` are true.

The test reads the values of the two components with no limit. It does not use the damage value of
STK-DMG-002, which the maximum damage limits. When `d` is larger than `m`, `m − d` is negative. For
example, take a stack with `minecraft:max_damage` 1561 and `minecraft:damage` 2000. It passes
`durability` `{max: -439}` and `damage` `{min: 2000}`, and it fails `durability` `{min: 0}`. The
command `give` can make such a stack.

`m − d` is the exact difference, a 64-bit integer. It does not overflow. The reader of the wire form
does not check the ranges of STK-CMP-004, so a stack from a client can hold any 32-bit values. For
example, take `minecraft:max_damage` 2147483647 and `minecraft:damage` −2147483648. Then `m − d` is
4294967295, and the stack passes `durability` `{min: 0}`. PRED-DEV-004 records the reference.

**PRED-ITM-011** (`source_suggested`, semantic, confidence medium). `enchantments` and
`stored_enchantments` (REG-FLD-599). The value is a list of enchantment predicates. The predicate
passes when each element of the list passes for the component (PRED-ITM-012). An empty list passes
for each holder that has the component.

**PRED-ITM-012** (`source_suggested`, semantic, confidence medium). An enchantment predicate
(REG-FLD-600) has an optional entry set `enchantments` and the bounds `levels`. The level of an
enchantment that the component does not hold is 0. The element passes as this table shows.

| `enchantments` | `levels` | The element passes when |
|----------------|----------|-------------------------|
| present | any value | one enchantment of the entry set has a level `l` in the component with `l ≠ 0` and `in(levels, l)` |
| absent | not `{}` | one enchantment of the component has a level `l` with `in(levels, l)` |
| absent | `{}` | the component holds at least one enchantment |

**PRED-ITM-013** (`source_suggested`, semantic, confidence medium). `potion_contents` (REG-FLD-601).

- `potions`: the test passes when the component has a potion and the potion is in the entry set.
- `effects`: a collection predicate (section 4). The collection is the effects of the potion of
  the component, then the custom effects of the component. PRED-EFX-002 states the test of one
  element.

**PRED-ITM-014** (`source_suggested`, semantic, confidence medium). `custom_data` (REG-FLD-602). The
value is an NBT predicate. The subject tag is the compound of the component
`minecraft:custom_data`. When the holder does not have the component, the subject tag is the empty
compound. The predicate `{}` thus passes for each holder.

The match compares the tags of the compound that the component holds (ARG-BLK-011). The result
depends on these tags, not on the form in which a holder stores them.

**PRED-ITM-015** (`source_suggested`, semantic, confidence medium). `container` and
`bundle_contents` (REG-FLD-603). The field `items` is a collection predicate with item predicate
elements. For `container`, the collection is the stacks of the component that are not empty. For
`bundle_contents`, the collection is the stacks of the bundle.

**PRED-ITM-016** (`source_suggested`, semantic, confidence medium). `firework_explosion`
(REG-FLD-604). Each of the fields `shape`, `has_twinkle` and `has_trail` that is present must equal
the value of the explosion. The same test is the element test of `fireworks`.

**PRED-ITM-017** (`source_suggested`, semantic, confidence medium). `fireworks` (REG-FLD-605). The
field `explosions` is a collection predicate over the explosions of the component (PRED-ITM-016).
The test `flight_duration` passes when `in(flight_duration, f)`, where `f` is the flight duration of
the component.

**PRED-ITM-018** (`source_suggested`, semantic, confidence medium). `writable_book_content`
(REG-FLD-606). The field `pages` is a collection predicate over the pages of the component. An
element is a string. It passes for a page when it equals the unfiltered text of the page.

**PRED-ITM-019** (`source_suggested`, semantic, confidence medium). `written_book_content`
(REG-FLD-607). The fields add these tests.

| Field | The test passes when |
|-------|----------------------|
| `author` | the string equals the author |
| `title` | the string equals the unfiltered title |
| `generation` | `in(generation, g)`, where `g` is the generation of the book |
| `resolved` | the boolean equals the resolved flag |
| `pages` | the collection predicate passes for the pages; an element passes when the text component equals the unfiltered page |

**PRED-ITM-020** (`source_suggested`, semantic, confidence medium). `attribute_modifiers`
(REG-FLD-608). The field `modifiers` is a collection predicate over the entries of the component. An
element passes for an entry when each of its fields that is present passes.

| Field | The test passes when |
|-------|----------------------|
| `attribute` | the attribute of the entry is in the entry set |
| `id` | the id equals the id of the modifier |
| `amount` | `in(amount, a)`, where `a` is the amount of the modifier |
| `operation` | the operation equals the operation of the modifier |
| `slot` | the slot group equals the slot group of the entry |

**PRED-ITM-021** (`source_suggested`, semantic, confidence medium). `trim` (REG-FLD-609). The test
`material` passes when the material of the trim is in the entry set. The test `pattern` passes when
the pattern of the trim is in the entry set.

**PRED-ITM-022** (`source_suggested`, semantic, confidence medium). `jukebox_playable`
(REG-FLD-610). The test `song` passes when the song of the component is in the entry set. Without
`song`, the predicate passes for each holder that has the component.

**PRED-ITM-023** (`source_suggested`, semantic, confidence medium). `villager/variant`
(REG-FLD-611). The predicate passes when the villager type of the component is in the entry set.

## 4. The collection predicate

**PRED-COL-001** (`source_suggested`, semantic, confidence medium). A collection predicate
(REG-FLD-612) tests an ordered collection of elements with an element predicate type. It has the
optional fields `contains`, `count` and `size`. The order of the elements has no effect on the
result. The predicate `{}` passes for each collection, also for the empty collection.

**PRED-COL-002** (`source_suggested`, semantic, confidence medium). `contains` is a list of element
predicates. The test passes when each element predicate passes for at least one element. One element
can satisfy two or more element predicates. An empty list passes. A list that is not empty fails for
the empty collection.

**PRED-COL-003** (`source_suggested`, semantic, confidence medium). `count` is a list of entries.
Each entry has an element predicate `test` and the integer bounds `count`. Let `n` be the number of
elements for which `test` passes. The entry passes when `in(count, n)`. The test passes when each
entry passes. An entry with `max: 0` passes when no element passes `test`.

**PRED-COL-004** (`source_suggested`, semantic, confidence medium). `size` is integer bounds. The
test passes when `in(size, k)`, where `k` is the number of elements of the collection.

## 5. The block predicate, the state properties predicate and the fluid predicate

### 5.1 The block predicate

**PRED-BLK-001** (`source_suggested`, semantic, confidence medium). The subject of a block predicate
(REG-FLD-614) in the world form is a block position of the dimension. The predicate fails when the
position is not a loaded position. This applies to each block predicate, also to `{}`.

**PRED-BLK-002** (`source_suggested`, semantic, confidence medium). The fields of a block predicate
add these tests. `S` is the block state at the position.

| Field | The test passes when |
|-------|----------------------|
| `blocks` | the block of `S` is in the entry set |
| `state` | the state properties predicate passes for `S` (section 5.2) |
| `nbt` | PRED-BLK-003 passes |
| `components`, `predicates` | PRED-BLK-004 passes |

**PRED-BLK-003** (`source_suggested`, semantic, confidence medium). The test `nbt` fails when the
position has no block entity. Otherwise the subject tag of the NBT predicate is the saved data of
the block entity (ARG-BLK-012). The saved data has the keys `id`, `x`, `y`, `z` and `components`.

**PRED-BLK-004** (`source_suggested`, semantic, confidence medium). The data component matchers of a
block predicate test the block entity at the position (section 3.2). The component holder is the
block entity. Its components are the components that the block entity gives to the item of its
block. When at least one of `components` and `predicates` has an entry, the test fails for a
position with no block entity. When both are empty or absent, they add no test.

**PRED-BLK-005** (`source_suggested`, client agreement, confidence medium). The item components
`minecraft:can_place_on` and `minecraft:can_break` hold block predicates (REG-FLD-517). They use the
held-block form. The subject is a block position that the player acts on. The held-block form has
the tests `blocks`, `state` and `nbt` of PRED-BLK-002. It does not have the test of the data
component matchers. The vanilla client computes the same test before it sends the action.

### 5.2 The state properties predicate

**PRED-BLK-006** (`source_suggested`, semantic, confidence medium). The subject of a state
properties predicate (REG-FLD-615) is a block state or a fluid state. The predicate is a list of
entries. Each entry names a property and has an exact form or a range form. The predicate passes
when each entry passes. An entry fails when the state does not have the named property. The
predicate `{}` passes for each state.

**PRED-BLK-007** (`source_suggested`, semantic, confidence medium). An exact entry holds one string.
The entry passes when the string is the name of a value of the property, and the state has that
value. For an integer property, PRED-BLK-009 states the names. For each other property, the names
are those of the block report, for example `true` or `north`. A string that is not a name of a
value of the property fails.

**PRED-BLK-008** (`source_suggested`, semantic, confidence medium). A range entry holds an optional
`min` string and an optional `max` string. Each string that is present must be the name of a value
of the property. Otherwise the entry fails. The values of a property have an order. The entry passes
when the value of the state is not before `min` and not after `max`. For an integer property, the
order is the numeric order. For a boolean property, `false` is before `true`.

**PRED-BLK-009** (`source_suggested`, semantic, confidence medium). This rule states the limits and
the order of a range entry (PRED-BLK-008) for each kind of property.

- An absent `min` gives no lower limit. An absent `max` gives no upper limit. Thus the range entry
  `{}` passes for each state that has the property.
- A `min` or a `max` that is not the name of a value of the property makes the entry fail for each
  state. This applies also when a different property has a value with that name. For example,
  `min: "up"` fails for each state of a furnace, because the `facing` of a furnace has no value
  `up`.
- When `min` is after `max`, the entry fails for each state. The decoder accepts such an entry.

The order of the values of a property depends on its kind:

| Kind of property | Order, from first to last |
|------------------|---------------------------|
| integer | the numeric order |
| boolean | `false`, `true` |
| direction: each property with the name `facing` or `vertical_direction` | `down`, `up`, `north`, `south`, `west`, `east` |
| each other enumerated property | the listed order of the generated block definition |

All directions have the one order of the table. A property with a part of the directions uses the
same order for that part. For example, the four values of the `facing` of a furnace have the order
`north`, `south`, `west`, `east`. Thus `{facing: {min: "north", max: "south"}}` passes for `north`
and for `south` only.

The generated block definitions of this repository, `assets/mcrs/block_definition/<block>.json`,
list the values of each property in `minecraft:block.description.properties`. This listed order is
the order of the block report.

- For an integer property and for each other enumerated property, the listed order is the order
  of this rule.
- For a boolean, the listed order is `true`, `false`. This is the reverse of the order of this rule.
- For a direction, the listed order is not the order of this rule. For example, `dispenser.json`
  lists `north`, `east`, `south`, `west`, `up`, `down`, and `icicle.json` lists `up`, `down`.

In the held-block form (PRED-BLK-005), the vanilla client uses the same order. No predicate of the
vanilla data pack has a range entry for an enumerated property or a boolean property.

The name of a value of an integer property is a decimal integer. It can have a sign and leading
zeros. Thus `7`, `07` and `+7` each name the value 7. This applies to an exact entry (PRED-BLK-007)
and to each limit of a range entry. A name with no value of the property fails, for example `-1`
or `x`. These names also apply:

- `-0` and `+0` name the value 0.
- A decimal integer outside the range `-2147483648` to `2147483647` names no value, for example
  `2147483648`. The entry fails, as for `x`. Leading zeros do not count for this range.

### 5.3 The fluid predicate

**PRED-BLK-010** (`source_suggested`, semantic, confidence medium). The subject of a fluid predicate
(REG-FLD-616) is a block position of the dimension. The predicate fails when the position is not a
loaded position, also for `{}`. `F` is the fluid state at the position. A position with no fluid has
the fluid `minecraft:empty`.

| Field | The test passes when |
|-------|----------------------|
| `fluids` | the fluid of `F` is in the entry set |
| `state` | the state properties predicate passes for `F` |

The fluids `minecraft:water` and `minecraft:flowing_water` are different fluids. The same applies to
lava.

## 6. The light predicate and the location predicate

### 6.1 The location predicate

**PRED-LOC-001** (`source_suggested`, semantic, confidence medium). The subject of a location
predicate (REG-FLD-618) is a position `(x, y, z)` of the dimension, in 64-bit floats. `P` is the
block position of that position. The caller supplies the position. Section 9 states the position
that each entity sub-predicate supplies.

**PRED-LOC-002** (`source_suggested`, semantic, confidence medium). The fields of a location
predicate add these tests.

| Field | The test passes when | Needs a loaded position |
|-------|----------------------|-------------------------|
| `position.x` | `in(position.x, x)` | no |
| `position.y` | `in(position.y, y)` | no |
| `position.z` | `in(position.z, z)` | no |
| `dimension` | the key equals the key of the dimension | no |
| `biomes` | the biome at `P` is in the entry set | yes |
| `structures` | PRED-LOC-003 passes | yes |
| `smokey` | the boolean equals the result of the smoke test (PRED-LOC-004) | yes |
| `light` | the light predicate passes for `P` (PRED-LOC-006) | yes |
| `block` | the block predicate passes for `P` in the world form (PRED-BLK-001) | yes |
| `fluid` | the fluid predicate passes for `P` (PRED-BLK-010) | yes |
| `can_see_sky` | PRED-LOC-005 passes | yes |

A test with "yes" fails when `P` is not a loaded position. This applies to each expected value, for
example to `smokey: false`. A location predicate with only tests with "no" passes or fails with no
read of the world.

**PRED-LOC-003** (`source_suggested`, semantic, confidence medium). The test `structures` passes
when `P` is inside one piece of a structure of the entry set. A position inside the box of the
structure, but outside each piece, fails.

**PRED-LOC-004** (`source_suggested`, semantic, confidence medium). The smoke test reads the blocks
under `P`. A lit campfire is a block of the tag `minecraft:campfires` with the property `lit=true`.
The centre post is the box from `(6/16, 0, 6/16)` to `(10/16, 1, 10/16)` in the block. For `i` from
1 to 5, in this order, `B(i)` is the block state `i` blocks under `P`:

1. If `B(i)` is a lit campfire, the result is true. The test stops.
2. Otherwise, if the collision shape of `B(i)` intersects the centre post, the test stops. The
   result is true when `B(i + 1)` is a lit campfire. Otherwise the result is false.

When no step stops the test, the result is false. A lit campfire under a hay bale thus gives smoke
at each of the 5 positions over the hay bale.

**PRED-LOC-005** (`design`, semantic). The test `can_see_sky` reads the sky light of `P`, without
the darkening of PRED-LOC-007. The position sees the sky when the sky light is 15. The test passes
when the boolean equals this result. The test fails when `P` is not a loaded position. Reason for
the last sentence: PRED-GEN-004. PRED-DEV-001 records the difference from the vanilla server. In a
dimension without sky light, no position sees the sky.

### 6.2 The light predicate

**PRED-LOC-006** (`source_suggested`, semantic, confidence medium). The subject of a light predicate
(REG-FLD-617) is a block position of the dimension. The predicate fails when the position is not a
loaded position, also for `{}`. Otherwise the predicate passes when `in(light, L)`, where `L` is the
raw brightness of the position.

**PRED-LOC-007** (`source_suggested`, semantic, confidence medium). The raw brightness of a block
position is the larger of two integers: the block light, and the sky light minus the sky darkening.
The raw brightness is from 0 to 15. The sky darkening is the integer part of `15 − a`. `a` is the
dimension value of the environment attribute `minecraft:gameplay/sky_light_level` (ENVA-LAYER-002).

- `a` is a value of the dimension, not a value at a position. All block positions of a dimension
  have the same `a` at one time.
- The biome of a position does not change `a`. A biome file cannot hold this attribute
  (REG-FLD-253).
- `a` comes from these layers of ENVA-LAYER-001: the entry of the dimension type, the timelines of
  the dimension type and the weather layer. Thus `a` changes with the time of day and with rain
  and thunder.

Timing (`design`, semantic). A light test uses the value of `a` at the time of the test. A change
of a clock or of the weather earlier in the same tick changes the result. For example, a function
that runs `time set midnight` and then a light test gets the sky darkening of midnight. Reason: a
data pack author expects a test after a change of the time to see the new time. ENVA-LAYER-008
gives the same result for each other read of an attribute after a change of a clock. PRED-DEV-002
records the difference from the vanilla server.

## 7. The distance predicate

**PRED-DST-001** (`source_suggested`, semantic, confidence medium). The subject of a distance
predicate (REG-FLD-621) is two positions, `a` and `b`. The order of the two positions has no effect.
The test computes `xd`, `yd` and `zd`: the difference of each coordinate, rounded to a 32-bit float.

| Field | The test passes when |
|-------|----------------------|
| `x` | `in(x, |xd|)` |
| `y` | `in(y, |yd|)` |
| `z` | `in(z, |zd|)` |
| `horizontal` | the squared form of `horizontal` passes for `xd² + zd²` (PRED-NUM-003) |
| `absolute` | the squared form of `absolute` passes for `xd² + yd² + zd²` |

The sums of squares are 32-bit floats. The distance predicate reads no value of the world.
PRED-ENT-009 states the two positions for the sub-predicate `distance`.

## 8. The NBT predicate, the mob effects predicate and the damage predicates

### 8.1 The NBT predicate

**PRED-NBT-001** (`source_suggested`, semantic, confidence medium). The subject of an NBT predicate
(REG-FLD-623) is a tag. The value of the predicate is a compound `E`. The predicate fails when the
subject tag is absent. Otherwise the predicate passes when `E` matches the subject tag
(ARG-BLK-011). The predicate `{}` passes for each compound.

**PRED-NBT-002** (`source_suggested`, semantic, confidence medium). The caller supplies the subject
tag, as this table shows.

| Caller | Subject tag |
|--------|-------------|
| `custom_data` | the compound of the component, or the empty compound (PRED-ITM-014) |
| block predicate `nbt` | the saved data of the block entity; absent with no block entity (PRED-BLK-003) |
| entity sub-predicate `nbt` | the saved data of the entity (PRED-ENT-013) |

### 8.2 The mob effects predicate

**PRED-EFX-001** (`source_suggested`, semantic, confidence medium). The subject of a mob effects
predicate (REG-FLD-622) in an entity predicate is an entity. The predicate fails when the entity is
not a living entity, also for `{}`. The value is a map from an effect to an entry. The predicate
passes when, for each key, the entity has an active effect of that kind and the entry passes for it.
An entry passes for an effect when each of these is true:

- `in(amplifier, a)`, where `a` is the amplifier of the effect;
- `in(duration, t)`, where `t` is the duration of the effect in ticks, and −1 for an effect with no
  end;
- `ambient` is absent, or it equals the ambient flag of the effect;
- `visible` is absent, or it equals the visible flag of the effect.

An empty map passes for each living entity.

**PRED-EFX-002** (`source_suggested`, semantic, confidence medium). In `potion_contents`, a mob
effects predicate is an element predicate (PRED-ITM-013). The subject is one effect of the
collection. The element passes when the map has a key for the kind of the effect, and the entry of
that key passes for the effect. A map with no such key fails for the effect. An empty map fails for
each effect.

### 8.3 The damage source predicate

**PRED-DMG-001** (`source_suggested`, semantic, confidence medium). The subject of a damage source
predicate (REG-FLD-619) is a damage source: a damage type, an optional direct entity and an optional
causing entity. Each damage source has a damage type. The damage type is never an absent value. The
caller also supplies the dimension and an origin for the entity predicates. The predicate fails when
the damage source is absent, also for `{}`.

**PRED-DMG-002** (`source_suggested`, semantic, confidence medium). The fields of a damage source
predicate add these tests.

| Field | The test passes when |
|-------|----------------------|
| `tags` | for each entry, the boolean `expected` equals "the damage type is in the entry set `id`" |
| `direct_entity` | the entity predicate passes for the direct entity |
| `source_entity` | the entity predicate passes for the causing entity |
| `is_direct` | the boolean equals the direct result of PRED-DMG-003 |

The entry set `id` of `tags` is a tag with `#`, one damage type key, or a list of keys
(REG-FLD-619). A key with no `#` is one damage type, not a tag. An empty list holds no type, so the
entry passes only with `expected: false`. A tag key with no `#` fails the load (REG-FLD-619).

A test `direct_entity` or `source_entity` fails when the damage source does not have that entity,
also for `{}` (PRED-ENT-001). Each of the two entity predicates receives the origin of the caller.

**PRED-DMG-003** (`source_suggested`, semantic, confidence medium). The direct result of a damage
source compares its direct entity and its causing entity. The entities are the entities of the damage
source when the damage occurred. The result has these values:

| Damage source | Example | Direct result |
|---------------|---------|---------------|
| neither entity | the damage of a fall | true |
| both entities, and they are the same entity | a zombie hits a player | true |
| both entities, and they are different entities | an arrow that a skeleton shoots: the arrow is the direct entity and the skeleton is the causing entity | false |
| a direct entity and no causing entity | the explosion of TNT that redstone ignites; a firework rocket that a dispenser shoots | false |
| a causing entity and no direct entity | does not occur in the vanilla server: each damage source with a causing entity also has a direct entity. A host that makes such a source gets this result. | false |

A projectile with no owner can have itself as the causing entity. The source then has the same
entity two times, and the result is true. Examples are an arrow that a dispenser shoots, and a
fireball with no owner. The removal of an entity after the damage does not change the result.

**PRED-DMG-006** (`design`, semantic). An entity of a damage source that the world removed after the
damage stays present for the damage source predicate. `direct_entity` and `source_entity` test it.
For such an entity, the host keeps at least these values for each test in the tick of the damage:
the entity type, and each value of PRED-HST-002 that the entity had at the moment of the removal.
The vanilla server has these values, because the damage source holds the entity with its last
state. Reason: the damage source records the entities of the damage. A removal that occurs later
must not make a test of `is_direct` and a test of `source_entity` disagree.

A host that cannot keep a value of a removed entity gives an absent value for it, and the test of
that value fails (PRED-GEN-004). This is a limit of that host, not a behaviour of the game. A
differential test does not test a removed entity on such a host.

### 8.4 The damage predicate

**PRED-DMG-004** (`source_suggested`, semantic, confidence medium). The subject of a damage
predicate (REG-FLD-620) is one damage event. The caller supplies these values:

- the player of the event;
- the damage source;
- the amount before the reductions, and the amount after the reductions;
- the blocked flag. It is true when a shield blocked the damage.

**PRED-DMG-005** (`source_suggested`, semantic, confidence medium). The fields of a damage predicate
add these tests.

| Field | The test passes when |
|-------|----------------------|
| `dealt` | `in(dealt, d)`, where `d` is the amount before the reductions |
| `taken` | `in(taken, t)`, where `t` is the amount after the reductions |
| `source_entity` | the entity predicate passes for the causing entity of the source |
| `blocked` | the boolean equals the blocked flag |
| `type` | the damage source predicate passes for the source |

The dimension of the entity predicate and of the damage source predicate is the dimension of the
player. Their origin is the position of the player. REG-FLD-620 states the decode of `blocked`.

## 9. The entity predicate

### 9.1 The predicate

**PRED-ENT-001** (`source_suggested`, semantic, confidence medium). The subject of an entity
predicate (REG-FLD-624) is an optional entity. The predicate fails when the entity is absent. This
applies to each entity predicate, also to `{}`. The predicate `{}` passes for each entity that is
present.

**PRED-ENT-002** (`source_suggested`, semantic, confidence medium). An entity predicate is a map
from a sub-predicate type id to a value. The predicate passes when each sub-predicate of the map
passes for the entity. The order of the sub-predicates has no observer.

**PRED-ENT-003** (`source_suggested`, semantic, confidence medium). The caller of an entity
predicate supplies three values: the dimension, an optional origin and the optional entity. Only the
sub-predicate `distance` reads the origin (PRED-ENT-009). Each location test reads the dimension of
the caller, and not a dimension of the entity. A caller must thus supply the dimension of the
entity.

**PRED-ENT-004** (`source_suggested`, semantic, confidence medium). A sub-predicate that tests a
second entity evaluates a nested entity predicate. This table states the values that the nested
predicate receives. The dimension is always the dimension of the caller.

| Place of the nested predicate | Nested entity | Nested origin |
|-------------------------------|---------------|---------------|
| `vehicle` | the vehicle of the entity | the origin of the caller |
| `passenger` | each direct passenger of the entity | the origin of the caller |
| `targeted_entity` | the target of the mob | the origin of the caller |
| `type_specific/lightning`, `entity_struck` | each entity that the bolt hit | the origin of the caller |
| `type_specific/player`, `looking_at` | the entity that the player looks at | the position of the player |
| damage source predicate, `direct_entity` and `source_entity` | PRED-DMG-002 | the origin of the caller |
| damage predicate, `source_entity` | PRED-DMG-005 | the position of the player |

**PRED-ENT-023** (`design`, semantic). An entity predicate holds a nested entity predicate in
`vehicle`, `passenger`, `targeted_entity`, `type_specific/lightning` `entity_struck` and
`type_specific/player` `looking_at`. These statements define the nesting depth of an entity
predicate:

- An entity predicate that holds no nested entity predicate has the nesting depth 1.
- Another entity predicate has the nesting depth 1 plus the largest nesting depth of the entity
  predicates that it holds.

An entity predicate with a nesting depth of more than 32 fails the decode of the value that
contains it. In a data pack file, that is a fault of the load (REG-ERR-001). Reason: the decode and
the match must not exhaust the stack of the thread. The vanilla data pack has the largest nesting
depth 2, in the advancements `adventure/spyglass_at_*`. PRED-DEV-003 records the vanilla server.

**PRED-ENT-024** (`design`, semantic). A nested entity test is one evaluation of a nested entity
predicate for one second entity (PRED-ENT-004). A match is one evaluation of an entity predicate
that is not nested in an entity predicate, with all its nested entity tests. These statements
apply:

- A match has a budget of nested entity tests. The budget is a value of the implementation. It is
  4096 or more.
- A match that needs more nested entity tests than its budget fails. The caller uses this result as
  it uses each other failed match, and gets no error. Thus an `inverted` condition around the match
  passes.
- A match fails by the budget only when it has more than 4096 nested entity tests with no test
  skipped. Thus the budget has no effect on a match with 4096 or fewer of these tests.

The budget gives a limit to the work of a match. It does not give the correct result for a match
that passes the budget. An author of a data pack gets a bound on the work, not a safe result.

An implementation can stop a test at the first result that decides it. Reason: `passenger` tests
each direct passenger, and `vehicle` of a passenger goes back to the same entity. Thus the number of
nested entity tests can grow as a power of the number of passengers. PRED-DEV-003 records the
vanilla server.

### 9.2 Type, position and movement

**PRED-ENT-005** (`source_suggested`, semantic, confidence medium). `entity_type` (REG-FLD-625)
passes when the type of the entity is in the entry set.

**PRED-ENT-006** (`source_suggested`, semantic, confidence medium). `location` (REG-FLD-626) passes
when the location predicate passes for the position of the entity. The position of an entity is the
centre of the bottom face of its bounding box.

**PRED-ENT-007** (`source_suggested`, semantic, confidence medium). `stepping_on` (REG-FLD-626)
fails when the on-ground flag of the entity is false, also for `{}`. Otherwise let `(bx, by, bz)` be
the support position of the entity (PHY-FLG-005). The sub-predicate passes when the location
predicate passes for the position `(bx + 0.5, by + 0.5, bz + 0.5)`. A test `position.y` thus
compares with `by + 0.5`, and not with the feet of the entity.

**PRED-ENT-008** (`source_suggested`, semantic, confidence medium). `movement_affected_by`
(REG-FLD-626) passes when the location predicate passes for the centre of the movement position of
the entity (PHY-FLG-005). The centre is `(bx + 0.5, by + 0.5, bz + 0.5)` for the movement position
`(bx, by, bz)`. This sub-predicate does not read the on-ground flag. The vanilla data pack uses it
for the enchantment `minecraft:soul_speed`.

**PRED-ENT-009** (`source_suggested`, semantic, confidence medium). `distance` (REG-FLD-627) fails
when the origin is absent, also for `{}`. Otherwise the sub-predicate passes when the distance
predicate passes for the origin and the position of the entity (PRED-DST-001). Section 10 states
that each caller documents its origin.

**PRED-ENT-010** (`source_suggested`, semantic, confidence medium). `movement` (REG-FLD-628) reads
the velocity `v = (vx, vy, vz)` of the entity in blocks for each second. `v` is the reported
movement of the entity multiplied by 20. `f` is the fall distance of the entity.

| Field | The test passes when |
|-------|----------------------|
| `x`, `y`, `z` | `in(x, vx)`, `in(y, vy)`, `in(z, vz)`; the values have a sign |
| `speed` | the squared form of `speed` passes for `vx² + vy² + vz²` (PRED-NUM-003) |
| `horizontal_speed` | the squared form of `horizontal_speed` passes for `vx² + vz²` |
| `vertical_speed` | `in(vertical_speed, |vy|)` |
| `fall_distance` | `in(fall_distance, f)` |

**PRED-ENT-011** (`source_suggested`, semantic, confidence medium). The reported movement of an
entity is a vector in blocks for each tick. The first row of this table that applies gives it.

| Entity | Reported movement |
|--------|-------------------|
| a player that rides a vehicle and does not control it | the reported movement of the vehicle |
| each other player | the position change of the last movement packet of the player; zero in a tick with no movement packet |
| an entity that is alive and that a player passenger controls | the reported movement of that player |
| each other entity | the velocity of the entity |

### 9.3 State of the entity

**PRED-ENT-012** (`source_suggested`, semantic, confidence medium). `effects` (REG-FLD-629) passes
when the mob effects predicate passes for the entity (PRED-EFX-001).

**PRED-ENT-013** (`source_suggested`, semantic, confidence medium). `nbt` (REG-FLD-630) passes when
the NBT predicate passes for the saved data of the entity (ARG-ENT-015). For a player, the saved
data also has the key `SelectedItem` when the selected stack is not empty.

**PRED-ENT-014** (`source_suggested`, semantic, confidence medium). `flags` (REG-FLD-631). Each
field that is present must equal the value of this table.

| Field | Value for a living entity | Value for another entity |
|-------|---------------------------|--------------------------|
| `is_on_ground` | the on-ground flag | the on-ground flag |
| `is_on_fire` | true when the entity is burning | the same |
| `is_sneaking` | true when the pose is `crouching` | the same |
| `is_sprinting` | the sprint state | the same |
| `is_swimming` | the swim state | the same |
| `is_in_water` | true when the entity is in water | the same |
| `is_flying` | true when the entity is in elytra flight, or when it is a player whose flying ability is on | false |
| `is_fall_flying` | true when the entity is in elytra flight | no test: each value passes |
| `is_baby` | true when the entity is a baby | no test: each value passes |

The two "no test" cells are the result of the reference. Section 12.3, U2, asks the owner if they
are correct. The test `is_sneaking` reads the pose, and not the shift key state of a player.

**PRED-ENT-015** (`source_suggested`, semantic, confidence medium). `equipment` (REG-FLD-632) fails
when the entity is not a living entity, also for `{}`. Each of the fields `head`, `chest`, `legs`,
`feet`, `body`, `saddle`, `mainhand` and `offhand` is an item predicate. A field that is present
must pass for the stack of that equipment slot. An empty slot gives the empty stack (PRED-ITM-003).

**PRED-ENT-016** (`source_suggested`, semantic, confidence medium). `periodic_tick` (REG-FLD-633)
has a positive integer `n`. The sub-predicate passes when the entity tick count is a multiple of
`n`.

**PRED-ENT-017** (`source_suggested`, semantic, confidence medium). `vehicle`, `passenger` and
`targeted_entity` (REG-FLD-634) hold a nested entity predicate (PRED-ENT-004).

- `vehicle` passes when the nested predicate passes for the vehicle of the entity. It fails when
  the entity has no vehicle.
- `passenger` passes when the nested predicate passes for at least one direct passenger. It fails
  when the entity has no passenger. A passenger of a passenger is not a direct passenger.
- `targeted_entity` passes when the nested predicate passes for the attack target of the entity.
  It fails when the entity is not a mob, and when the mob has no target.

Each of the three fails for an entity without the second entity, also with the value `{}`.

**PRED-ENT-018** (`source_suggested`, semantic, confidence medium). `team` (REG-FLD-635) passes when
the entity is a member of a scoreboard team and the name of the team equals the string. It fails
for an entity with no team.

**PRED-ENT-019** (`source_suggested`, semantic, confidence medium). `slots` (REG-FLD-636) is a map
from a slot range name to an item predicate. An entry passes when at least one slot of the range
that the entity has holds a stack for which the item predicate passes. The entry skips each slot
number of the range that the entity does not have. An entry fails when the entity has no slot of the
range. The sub-predicate passes when each entry passes. PRED-ENT-020 states the slot numbers of each
name.

**PRED-ENT-020** (`source_suggested`, semantic, confidence medium). A slot range name selects these
slot numbers. The host supplies the stack of each slot number that the entity has. The host also
states which slot numbers the entity has. A slot number that the entity does not have is not an empty
stack and is not an absent value. PRED-ENT-019 skips it.

| Name | Slot numbers |
|------|--------------|
| `contents` | 0 |
| `container.<i>`, `i` in `0..53` | `i` |
| `hotbar.<i>`, `i` in `0..8` | `i` |
| `inventory.<i>`, `i` in `0..26` | `9 + i` |
| `enderchest.<i>`, `i` in `0..26` | `200 + i` |
| `mob.inventory.<i>`, `i` in `0..7` | `300 + i` |
| `horse.<i>`, `i` in `0..14` | `500 + i` |
| `player.crafting.<i>`, `i` in `0..3` | `500 + i` |
| `weapon`, `weapon.mainhand` | 98 |
| `weapon.offhand` | 99 |
| `weapon.*` | 98, 99 |
| `armor.feet`, `armor.legs`, `armor.chest`, `armor.head`, `armor.body` | 100, 101, 102, 103, 105 |
| `armor.*` | 100, 101, 102, 103, 105 |
| `saddle` | 106 |
| `horse.chest` | 499 |
| `player.cursor` | 499 |

Each name with an index also has a form with `*` in place of the index, for example `container.*`
and `player.crafting.*`. That form selects each slot number of the row.

PRED-ENT-045 states the slot numbers that each entity type has. These examples show the result:

- An empty equipment slot holds the empty stack. Thus `{"saddle": {}}` passes for a zombie with no
  saddle.
- `container.30` selects no slot of a chest minecart, and the entry fails. `container.*` selects
  the slots 0 to 26 there.
- `horse.0` selects no slot of a zombie, and the entry fails.

**PRED-ENT-045** (`source_suggested`, semantic, confidence medium). An entity has the slot numbers
of each row of this table that applies to its type. A type that no row names has no slot number.

| Entity types | Slot numbers |
|--------------|--------------|
| each living entity | 98, 99, 100 to 103, 105, 106 |
| `player` | 0 to 35; 200 to 226; 499 (the cursor stack); 500 to 503 (the crafting grid) |
| `villager`, `wandering_trader`, `piglin` | 300 to 307 |
| `pillager` | 300 to 304 |
| `donkey`, `mule`, `llama`, `trader_llama` | 499 (a chest stack when the entity has a chest, else the empty stack) |
| `donkey`, `mule` with a chest | 500 to 514 |
| `llama`, `trader_llama` with a chest | 500 to 500 + 3 × strength − 1 |
| `chest_minecart`, each chest boat and chest raft | 0 to 26 |
| `hopper_minecart` | 0 to 4 |
| `item`, `item_frame`, `glow_item_frame`, `item_display` | 0 |
| `arrow`, `spectral_arrow`, `trident` | 0 (the stack that a pickup gives) |
| `fireball`, `small_fireball` | 0 (the item of the projectile) |

A host with an entity type that no row names states the slot numbers of that type. When the host
states no slot number for it, the type has no slot number.

**PRED-ENT-044** (`design`, semantic). A slot that the entity has, but whose stack the host does not
have, is an absent value. The item predicate fails for that slot (PRED-GEN-004, PRED-ITM-001). The
entry of PRED-ENT-019 still passes when another slot of the range passes. Thus a slot with an absent
value and a skipped slot give the same result for the entry. Reason: a host with fewer values than
the vanilla server must not make an entry pass that the vanilla server fails. The rule of "any slot
passes" then stays the same on each host.

**PRED-ENT-021** (`source_suggested`, semantic, confidence medium). `components` (REG-FLD-637) and
`predicates` (REG-FLD-638) are the data component matchers of section 3.2. The component holder is
the entity. `components` passes when PRED-ITM-004 passes for the entity. `predicates` passes when
PRED-ITM-005 passes for it. The host supplies the components of an entity (section 12.3, U5).

**PRED-ENT-022** (`source_suggested`, semantic, confidence medium). `entity_tags` (REG-FLD-639)
tests the scoreboard tags of the entity. The command `/tag` sets these tags.

| Field | The test passes when |
|-------|----------------------|
| `any_of` | the entity has at least one tag of the list; an empty list fails |
| `all_of` | the entity has each tag of the list; an empty list passes |
| `none_of` | the entity has no tag of the list; an empty list passes |

### 9.4 The sub-predicates for one entity type

**PRED-ENT-030** (`source_suggested`, semantic, confidence medium). `type_specific/lightning`
(REG-FLD-640) fails when the entity is not a lightning bolt, also for `{}`. The test
`blocks_set_on_fire` passes when `in(blocks_set_on_fire, n)`. `n` is the number of blocks that the
bolt set on fire. The test `entity_struck` passes when the nested predicate passes for at least one
entity that the bolt hit and that is alive (PRED-ENT-004). It fails when the bolt hit no such
entity.

**PRED-ENT-031** (`source_suggested`, semantic, confidence medium). `type_specific/fishing_hook`
(REG-FLD-641). Without the field `in_open_water`, the sub-predicate passes for each entity, also for
an entity that is not a fishing hook. With the field, the sub-predicate passes when the entity is a
fishing hook and the boolean equals its open water flag. The open water flag is the flag that the
hook computes for the treasure loot of fishing.

**PRED-ENT-032** (`source_suggested`, semantic, confidence medium). `type_specific/cube_mob`
(REG-FLD-643) fails when the type of the entity is not `minecraft:slime`, `minecraft:magma_cube` or
`minecraft:sulfur_cube`, also for `{}`. Otherwise it passes when `in(size, s)`, where `s` is the
size of the entity.

**PRED-ENT-033** (`source_suggested`, semantic, confidence medium). `type_specific/raider`
(REG-FLD-644) fails when the entity is not a raider. A raider is an entity of the type
`minecraft:evoker`, `minecraft:illusioner`, `minecraft:pillager`, `minecraft:ravager`,
`minecraft:vindicator` or `minecraft:witch`. The sub-predicate always compares both fields. An
absent field has the value false.

- `has_raid` must equal this result: the raider is a member of a raid, or a raid exists at the block
  position of the raider.
- `is_captain` must equal this result: the raider is a patrol leader, and its head slot holds the
  ominous banner.

The value `{}` thus passes only for a raider with no raid that is not a captain.

**PRED-ENT-034** (`source_suggested`, semantic, confidence medium). `type_specific/sheep`
(REG-FLD-645) fails when the entity is not a sheep, also for `{}`. The test `sheared` passes when
the boolean equals the sheared flag of the sheep.

### 9.5 The sub-predicate for a player

**PRED-ENT-040** (`source_suggested`, semantic, confidence medium). `type_specific/player`
(REG-FLD-642) fails when the entity is not a player, also for `{}`. Otherwise the fields add these
tests.

| Field | The test passes when |
|-------|----------------------|
| `level` | `in(level, e)`, where `e` is the experience level of the player |
| `food.level` | `in(food.level, n)`, where `n` is the food level |
| `food.saturation` | `in(food.saturation, s)`, where `s` is the saturation |
| `gamemode` | the list holds the game mode of the player |
| `stats` | for each entry, `in(value, n)`, where `n` is the value of that statistic of the player |
| `recipes` | for each entry, the boolean equals "the recipe book of the player has the recipe" |
| `advancements` | PRED-ENT-041 passes |
| `looking_at` | PRED-ENT-042 passes |
| `input` | PRED-ENT-043 passes |

A statistic that the player never changed has the value 0. An absent `gamemode` passes for each game
mode (REG-FLD-642). `level`, `food.level` and the `value` of `stats` are integer bounds
(PRED-NUM-001). `food.saturation` is double bounds, and the saturation is a 32-bit float
(PRED-NUM-002).

**PRED-ENT-041** (`source_suggested`, semantic, confidence medium). `advancements` is a map from an
advancement id to a boolean or to a map of criteria. The test passes when each entry passes. An
entry fails when the server has no advancement with that id.

- A boolean entry passes when the boolean equals "the player completed the advancement".
- A map entry passes when each criterion name of the map passes. A name passes when the boolean
  equals "the player completed that criterion". A name that the advancement does not have fails.

**PRED-ENT-042** (`source_suggested`, semantic, confidence medium). `looking_at` holds a nested
entity predicate. The view segment starts at the eye position of the player. It has the direction of
the view vector and the length 100 blocks. The candidates are the entities that are not the player
and not in spectator mode. The looked-at entity is the candidate whose bounding box the view segment
enters nearest to the eye position. A block does not stop the view segment. The test passes when
each of these is true:

1. a looked-at entity exists;
2. the nested predicate passes for it, with the position of the player as the origin
   (PRED-ENT-004);
3. the player has line of sight to it.

The line of sight of step 3 is a ray from the eye of the player to the eye of the entity. It fails
when the two eyes are more than 128 blocks apart. The collision shapes of the blocks stop the ray. A
fluid does not stop it.

**PRED-ENT-043** (`source_suggested`, semantic, confidence medium). `input` has the optional
booleans `forward`, `backward`, `left`, `right`, `jump`, `sneak` and `sprint`. Each field that is
present must equal the same key of the last input state that the client of the player sent. The
input is the state of the keys. It is not the result of the movement.

## 10. The values that a caller and a host supply

**PRED-HST-001** (`design`, semantic). A caller supplies the subject of the predicate and, for the
predicates of this table, more values. A specification that uses a predicate must state these
values. Reason: a predicate has no access to the context of its caller.

| Predicate | The caller supplies |
|-----------|---------------------|
| item predicate | the stack |
| block predicate, fluid predicate, light predicate | the dimension and the block position |
| location predicate | the dimension and the position |
| distance predicate | the two positions |
| damage source predicate | the dimension, the origin and the damage source |
| damage predicate | the player, the damage source, the two amounts and the blocked flag |
| entity predicate | the dimension, the optional origin and the optional entity |

**PRED-HST-002** (`design`, semantic). The host supplies each value of this table. Each value can be
an absent value, and PRED-GEN-004 then applies. Reason: the predicates are a library, and a user of
the library can have a world with fewer values than the vanilla server.

| Subject | Values | Rules |
|---------|--------|-------|
| dimension | its key; the loaded check of a block position; the value of `minecraft:gameplay/sky_light_level` | PRED-LOC-002, PRED-LOC-007 |
| block position | the block state; the fluid state; the block light; the sky light; the biome; the collision shape of the block state for the centre post | PRED-BLK-002, PRED-BLK-010, PRED-LOC-002, PRED-LOC-004, PRED-LOC-007 |
| block position and a structure entry set | the result "inside a piece" | PRED-LOC-003 |
| block entity | its presence; its saved data; its components | PRED-BLK-003, PRED-BLK-004 |
| registries | the tags of blocks, fluids, items, entity types, damage types and of each other entry set; the properties of each state with their values | all |
| stack | the item; the count; the effective components | PRED-ITM-001 |
| entity | its type; its position; the on-ground flag; the support position; the movement position; the reported movement; the fall distance | PRED-ENT-005 to PRED-ENT-011 |
| entity | the saved data; the burning state; the pose; the sprint state; the swim state; the in-water state; the entity tick count | PRED-ENT-013, PRED-ENT-014, PRED-ENT-016 |
| entity | the vehicle; the direct passengers; the team name; the scoreboard tags; the components; the slot numbers that the entity has (PRED-ENT-045); the stack of a slot number | PRED-ENT-017 to PRED-ENT-022, PRED-ENT-045 |
| living entity | the active effects; the stacks of the 8 equipment slots; the elytra flight state; the baby flag | PRED-EFX-001, PRED-ENT-014, PRED-ENT-015 |
| mob | the attack target | PRED-ENT-017 |
| lightning bolt | the number of blocks set on fire; the hit entities that are alive | PRED-ENT-030 |
| fishing hook | the open water flag | PRED-ENT-031 |
| slime, magma cube, sulfur cube | the size | PRED-ENT-032 |
| raider | the raid result; the captain result | PRED-ENT-033 |
| sheep | the sheared flag | PRED-ENT-034 |
| player | the experience level; the food level; the saturation; the game mode; the flying ability; the value of a statistic | PRED-ENT-014, PRED-ENT-040 |
| player | the recipe book test of a recipe; the state of an advancement and of each criterion | PRED-ENT-040, PRED-ENT-041 |
| player | the eye position; the view vector; the looked-at entity; the line of sight to an entity; the last input state | PRED-ENT-042, PRED-ENT-043 |
| damage source | the direct entity; the causing entity. The damage type is never an absent value. | PRED-DMG-001, PRED-DMG-006 |

**PRED-HST-003** (`design`, semantic). The type of an entity decides which rows of PRED-HST-002
apply to it. A value of a row that does not apply is not an absent value. The rule of the
sub-predicate states the result, for example "fails when the entity is not a sheep". Reason: the
rules of section 9 must give the same result on each host.

## 12. Intentional deviations and unresolved items

### 12.1 Intentional deviations

**PRED-DEV-001.**

- **Rule**: PRED-LOC-005.
- **Reference**: the test `can_see_sky` has no loaded check. For a position that is not a loaded
  position, the vanilla server reads the sky light of a section that its light storage does not
  have. Each other world test of the location predicate fails for such a position.
- **Visible to the client**: no. A player is not near a position that is not loaded.
- **Test**: a differential test does not evaluate `can_see_sky` at a position that is not loaded.

**PRED-DEV-002.**

- **Rule**: PRED-LOC-007, part "Timing".
- **Reference**: the vanilla server computes the sky darkening one time in each tick of the
  dimension, after the clocks advance and after the weather changes. Each light test until the next
  computation uses that value. The tick functions run before this computation. Thus a function that
  changes the time and then tests the light sees the old sky darkening.
- **Visible to the client**: no. Only a data pack function or a command sees the difference, for
  one tick: after `time set` or `weather`, a light test gets the light of the old time or weather.
- **Test**: a differential test does not run a light test after a change of the time or the weather
  in the same tick.

**PRED-DEV-003.**

- **Rule**: PRED-ENT-023, PRED-ENT-024.
- **Reference**: the vanilla server has no limit of its own on the nesting depth. Its JSON reader
  refuses a data pack file whose objects and lists nest deeper than a limit of the reader. A match has no budget. A predicate
  that alternates `passenger` and `vehicle`, on an entity with many passengers, can make one tick
  last many seconds. The watchdog of the server (`max-tick-time` of `server.properties`) can then
  stop the server.
- **Visible to the client**: yes, but only with a data pack that has such a predicate. The author of
  the data pack sees a load fault for a nesting depth of 33 or more. A player sees the result of a
  failed match in place of a long tick.
- **Test**: a differential test does not use a nesting depth of more than 32, or a match with more
  than 4096 nested entity tests.

**PRED-DEV-004.**

- **Rule**: PRED-ITM-010.
- **Reference**: the vanilla server computes `m − d` as a 32-bit integer, and the difference wraps
  on overflow. For `minecraft:max_damage` 2147483647 and `minecraft:damage` −2147483648, `m − d` is
  −1, and the stack fails `durability` `{min: 0}`.
- **Visible to the client**: no. Only a stack that a modified client sends has such values. A
  player in normal play cannot make one.
- **Test**: a differential test does not use a stack whose `m − d` is outside the range of a 32-bit
  integer.

### 12.3 Unresolved

- **Q2**: PRED-GEN-004 is a design rule. The vanilla server has no absent value except a position
  that is not loaded and an absent entity. The owner can select a different result for an absent
  value, for example an error that the caller sees.
- **U2**: the fields `is_baby` and `is_fall_flying` of `flags` for an entity that is not a living
  entity (PRED-ENT-014). The reference skips the test, so `is_baby: true` passes for an item entity.
  Each other field of `flags` gives false there. Question to the owner: must both fields give false
  for such an entity?
- **U3**: the reported movement of a player in a tick with no movement packet (PRED-ENT-011). The
  reference suggests zero. 
- **U4**: the margin of the bounding box of a candidate in PRED-ENT-042. Some entity types can have
  a margin.
- **U5**: the list of the components of each entity type for PRED-ENT-021. The reference suggests
  the custom name, the custom data and components for the variant of some types, for example the
  colour of a sheep.
- **U6**: the squared form with a negative bound (PRED-NUM-004).
