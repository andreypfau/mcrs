# Attributes: the instances and the modifiers of a living entity

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

This document states the attributes of a living entity. It states the definition of each
attribute, the base value that each entity type gives, and the data of one instance. It states how
the base value and the modifiers give the value, in exact f64 arithmetic. It states what the server
sends to a client, what a client does with it, and what a save keeps.

The documents that add a modifier or read a value state their own rules. Section 5 and section 9
list them.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **living entity**, **f32**, **f64**: as `entity-physics.md`,
  section 0.1, defines them.
- **Tracking player**, **update**, **spawn bundle**: as `entity-tracking.md` defines them.
- **Attribute**: an entry of the registry `minecraft:attribute`.
- **Definition**: the data of one attribute: its attribute default, its range and its syncable
  flag (ATR-DEF-001).
- **Attribute default**: the default value of the attribute itself.
- **Range**: the minimum and the maximum of the value of an attribute.
- **Syncable**: the flag of an attribute that the server sends to clients.
- **Attribute set**: the attributes that the entities of one entity type have (ATR-BASE-001).
- **Base of the type**: the base value that a new entity of one entity type has for one attribute
  (ATR-BASE-003).
- **Instance**: the data of one attribute on one entity: a base value and a sequence of modifiers.
- **Base value**: the f64 of an instance from which the computation starts.
- **Modifier**: an identifier, an f64 amount, an operation and a kind, permanent or transient
  (ATR-INS-003, ATR-INS-005).
- **Operation**: one of `add_value`, `add_multiplied_base` and `add_multiplied_total`.
- **Sequence**: the modifiers of one instance, in the order of ATR-INS-004.
- **Permanent modifier**: a modifier that a save keeps. A **transient modifier** is a modifier that
  a save does not keep.
- **Value**: the f64 that ATR-VAL-001 computes from an instance. A reader of an attribute reads
  the value.
- **Own player**: the player whose entity has the instance, as an observer of its own packets.
- **Home slot**: the integer from 0 to 31 that ATR-VAL-002 gives to the identifier of a modifier.

### 0.2 Provenance

The labels are those of `docs/spec/registry.md`, section 0.3. Experiments A1, A3, A4 and A5 of
section 13 ran on the game version of this document. The fixture of A1 confirms the base table and
the value rules. The trace of A3 confirms the instance rules, the kinds of the sources and the save
rules. The recordings of A4 and A5 and the other recordings of section 0.5 confirm the sprint
modifier, the creative mode modifiers and the entries to the own player. The other rules come from
the reference implementation and keep the label `source_suggested`. Each rule states its kind and
its confidence.

### 0.3 ID prefix

Each rule has an ID with the prefix `ATR`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `ATR-DEF` | the definitions |
| `ATR-BASE` | the attribute set and the base of an entity type |
| `ATR-INS` | the instance and its modifiers |
| `ATR-VAL` | the value |
| `ATR-SRC` | the sources of modifiers |
| `ATR-NET` | the packets that the server sends |
| `ATR-CLI` | what a client does |
| `ATR-SAV` | the save data |

### 0.4 Scope

This document states:

- the 40 definitions, and the attribute set and the base values of the 94 living entity types;
- the instance, its base value, its modifiers and the changes of them;
- the value, bit for bit;
- the sprint modifier and the creative mode modifiers, and a table of the other sources;
- the content and the time of `update_attributes`, for a tracking player and for the own player;
- what a client does with `update_attributes`, and the instances of a client before that packet;
- the field `attributes` of a saved living entity.

Section 10 lists the subjects that are out of scope.

### 0.5 Artefacts cited

- `crates/mcrs_minecraft_game_registries/src/entity/fixtures/registry_census.bin`: for each
  attribute, in numeric id order, the attribute default, the range and the syncable flag. The file
  `registry_census_capture_procedure.md` beside it states the layout.
- `crates/mcrs_minecraft_entity/tests/fixtures/vanilla/attributes.json`: the output of experiment
  A1. `tools/vanilla-oracle/README.md`, section "Attributes", states its layout. Each f64 is a
  string of 16 hexadecimal digits, the bits of the value.
- `assets/mcrs/reports/registries.json`, registry `minecraft:attribute`: the numeric ids.
- `assets/mcrs/reports/packets.json`: `update_attributes` is play clientbound 134.
- `crates/mcrs_minecraft_physics/tests/fixtures/player/e9_sprint_stone.txt`, `e12_fly.txt` and
  `e10_jump_boost.txt`: recordings of a vanilla client. A line `< attribute` is one entry of an
  `update_attributes` packet for the own player. `tools/mcrs_minecraft_client_recorder/README.md`
  states the format.
- `tools/mcrs_minecraft_probe/traces/attributes.json`: the trace of experiment A3. Each step holds
  the replies of the game to one command. `tools/mcrs_minecraft_probe/README.md` states the
  format. A reply of `/data get entity` holds the save of the entity as text.
- `crates/mcrs_minecraft_entity/tests/fixtures/vanilla/a4_respawn.txt` and
  `a5_slowness_sprint.txt`: the recordings of experiments A4 and A5, in the format of the client
  recorder. A line `> tick` ends one client tick.
- The results TRK-E1 and TRK-E2 of `entity-tracking.md`.

The oracle takes the amounts of the part `value_cases` of `attributes.json` as inputs. Thus a value
case confirms the value rules for that amount. It does not confirm the amount that a source of
section 5 computes.

## 1. The definitions

**ATR-DEF-001** (`black_box_confirmed`, client agreement, confidence: high). The registry
`minecraft:attribute` is a static registry with these 40 entries. The numeric id is the id on the
wire. Each name has the namespace `minecraft`.

| Id | Name | Attribute default | Minimum | Maximum | Syncable |
|----|------|-------------------|---------|---------|----------|
| 0 | `air_drag_modifier` | 1.0 | 0 | 2048 | yes |
| 1 | `armor` | 0 | 0 | 30 | yes |
| 2 | `armor_toughness` | 0 | 0 | 20 | yes |
| 3 | `attack_damage` | 2 | 0 | 2048 | no |
| 4 | `attack_knockback` | 0 | 0 | 5 | no |
| 5 | `attack_speed` | 4 | 0 | 1024 | yes |
| 6 | `below_name_distance` | 10 | 0 | 512 | yes |
| 7 | `block_break_speed` | 1 | 0 | 1024 | yes |
| 8 | `block_interaction_range` | 4.5 | 0 | 64 | yes |
| 9 | `bounciness` | 0 | 0 | 1 | yes |
| 10 | `burning_time` | 1 | 0 | 1024 | yes |
| 11 | `camera_distance` | 4 | 0 | 32 | yes |
| 12 | `explosion_knockback_resistance` | 0 | 0 | 1 | yes |
| 13 | `entity_interaction_range` | 3 | 0 | 64 | yes |
| 14 | `fall_damage_multiplier` | 1 | 0 | 100 | yes |
| 15 | `flying_speed` | 0.4 | 0 | 1024 | yes |
| 16 | `follow_range` | 32 | 0 | 2048 | no |
| 17 | `friction_modifier` | 1 | 0 | 2048 | yes |
| 18 | `gravity` | 0.08 | -1 | 1 | yes |
| 19 | `jump_strength` | 0.41999998688697815 | 0 | 32 | yes |
| 20 | `knockback_resistance` | 0 | -2 | 1 | no |
| 21 | `luck` | 0 | -1024 | 1024 | yes |
| 22 | `max_absorption` | 0 | 0 | 2048 | yes |
| 23 | `max_health` | 20 | 1 | 1024 | yes |
| 24 | `mining_efficiency` | 0 | 0 | 1024 | yes |
| 25 | `movement_efficiency` | 0 | 0 | 1 | yes |
| 26 | `movement_speed` | 0.7 | 0 | 1024 | yes |
| 27 | `name_tag_distance` | 64 | 0 | 512 | yes |
| 28 | `oxygen_bonus` | 0 | 0 | 1024 | yes |
| 29 | `safe_fall_distance` | 3 | -1024 | 1024 | yes |
| 30 | `scale` | 1 | 0.0625 | 16 | yes |
| 31 | `sneaking_speed` | 0.3 | 0 | 1 | yes |
| 32 | `spawn_reinforcements` | 0 | 0 | 1 | no |
| 33 | `step_height` | 0.6 | 0 | 10 | yes |
| 34 | `submerged_mining_speed` | 0.2 | 0 | 20 | yes |
| 35 | `sweeping_damage_ratio` | 0 | 0 | 1 | yes |
| 36 | `tempt_range` | 10 | 0 | 2048 | no |
| 37 | `water_movement_efficiency` | 0 | 0 | 1 | yes |
| 38 | `waypoint_transmit_range` | 0 | 0 | 60000000 | no |
| 39 | `waypoint_receive_range` | 0 | 0 | 60000000 | no |

Each number of this document is the f64 that its decimal text gives. The attribute default of
`jump_strength` is the f32 value 0.42, widened to f64. The vanilla client has the same table. The
census of section 0.5 holds each value as bits, and it is the data for a test of this table.

**ATR-DEF-002** (`design`, semantic, confidence: high). The definitions and the base table of
ATR-BASE-003 are data that a user of the library can read with no entity and no world. The
computation of ATR-VAL-001 is a function of a definition, a base value and a sequence. Thus a
proxy can compute a value from the entries of a packet, and an item tooltip can read the base of
the type `player`.

## 2. The attribute set and the base of an entity type

**ATR-BASE-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). An entity
type is a living entity type when Appendix A has a row for it. This game version has 94 such
types. A living entity has one instance for each attribute of the attribute set of its type. It has
no instance for another attribute. An entity of another type has no instance.

In the trace of experiment A3, the step `attribute_not_in_set` reads `follow_range` of a player.
The game replies with the error `commands.attribute.failed.no_attribute`.

A source that adds a modifier to an attribute outside the attribute set does nothing
(ATR-SRC-001).

**ATR-BASE-002** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The
attribute set of each living entity type contains these 26 attributes, the common set:

`air_drag_modifier`, `armor`, `armor_toughness`, `attack_knockback`, `below_name_distance`,
`bounciness`, `burning_time`, `camera_distance`, `entity_interaction_range`,
`explosion_knockback_resistance`, `fall_damage_multiplier`, `friction_modifier`, `gravity`,
`jump_strength`, `knockback_resistance`, `max_absorption`, `max_health`, `movement_efficiency`,
`movement_speed`, `name_tag_distance`, `oxygen_bonus`, `safe_fall_distance`, `scale`,
`step_height`, `water_movement_efficiency` and `waypoint_transmit_range`.

The column "More attributes" of Appendix A gives the other attributes of the set of a type.

**ATR-BASE-003** (`source_suggested_black_box_confirmed`, client agreement, confidence: high). A
new instance has the base of the type and no modifier. The base of the type is the value in the
column "Base values" of Appendix A, when the row of the type names the attribute. In each other
case, it is the attribute default. Appendix A is the one table of these values. Other documents
cite it.

The vanilla client has the same table. Thus the client agrees on each instance that the server
does not send. The part `default_attributes` of the fixture `attributes.json` holds each base of
each type as bits. It is the data for a test of Appendix A.

The spawn of an entity can change the base value of an instance or add a modifier. For example, a
`slime`, a `magma_cube` and a `sulfur_cube` set `max_health` by their size. The specification of
the entity type states such a change. It is not a part of the base of the type.

## 3. The instance

**ATR-INS-001** (`source_suggested`, semantic, confidence: high). An instance has a base value and
a sequence of modifiers. The base value is an f64. Each change of this section is a call that a
user of the library can make. A call changes the instance or leaves it as it is. Only a change of
an instance causes a packet (ATR-NET-003).

**ATR-INS-002** (`source_suggested_black_box_confirmed`, semantic, confidence: high). A call that
sets the base value stores the f64 as given. The range does not limit the base value. Only the
value has the range (ATR-VAL-003). When the new base value has the same bits as the old one, the
instance does not change.

For example, the base value 5000 of `max_health` stays 5000, and the value is 1024. A modifier of
`add_value` with the amount -4990 then gives the value 10.

The trace of experiment A3 shows this for a command and for a load. In the steps
`base_above_maximum_base` and `base_above_maximum_value`, the base value of `max_health` is 5000
and the value is 1024. In the steps `pig_base_out_of_range_base` and
`pig_base_out_of_range_value`, the loaded base value 100 of `scale` stays 100, and the value
is 16.

**ATR-INS-003** (`source_suggested`, wire, confidence: high). A modifier has an identifier, an
amount and an operation. The amount is an f64. These are the operations:

| Operation | Id on the wire | Name in a save and in a data pack |
|-----------|----------------|-----------------------------------|
| add to the base value | 0 | `add_value` |
| add a multiple of the sum of step 1 | 1 | `add_multiplied_base` |
| multiply the total | 2 | `add_multiplied_total` |

An instance has at most one modifier for each identifier. This limit does not depend on the
operation. Two instances of one entity can each have a modifier of the same identifier.

**ATR-INS-004** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). The
sequence starts empty.
These calls change it:

| Call | No modifier has the identifier | A modifier has the identifier |
|------|--------------------------------|-------------------------------|
| add | The modifier goes to the end of the sequence. | The instance does not change. The caller gets an error. |
| set | The modifier goes to the end of the sequence. | See below. |
| remove | The instance does not change. The caller learns that the instance had no such modifier. | The modifier leaves the sequence. The order of the others stays. |

A set of a modifier that is equal to the present one does not change the instance. Two modifiers
of one identifier are equal when the amount bits, the operation and the kind are equal. In each
other case, a set removes the present modifier and then puts the new one at the end of the
sequence.

The position at the end has the provenance `design`. No artefact can decide it. The test of the
deviation "The order of the modifiers in one entry" (section 11) compares an entry as a set. The
server computes the value from the sequence that it sends (ATR-NET-002). Thus the position is a
choice of the server. It must be one deterministic position, because the order of the sequence
decides the value when two modifiers have a home slot conflict (ATR-VAL-002). A source cannot keep
the position of a modifier that it replaces, and no rule needs that.

The sources of section 5 use set and remove. A command that adds a modifier uses add, and reports
the error to the player.

The trace of experiment A3 confirms the rows add and remove. The step `modifier_add_again` adds
`test:a` a second time with another amount and operation. The game replies with the error
`commands.attribute.failed.modifier_already_present`, and the step `modifier_value` then reads the
first amount, 1.0. The step `modifier_remove_again` removes a modifier that is not there. The game
replies with the error `commands.attribute.failed.no_modifier`. No experiment shows the row set.

**ATR-INS-005** (`source_suggested_black_box_confirmed`, semantic, confidence: high). Each modifier
is permanent or transient. The caller selects this when it adds or sets the modifier. Both kinds
count in the value, and the server sends both kinds (ATR-NET-002). A save keeps only the permanent
modifiers (ATR-SAV-001). Section 5 states the kind for each source.

The trace of experiment A3 shows both kinds. The transient creative mode modifiers count in the
value 5.0 of `block_interaction_range`, and the save does not hold them (step `creative_saved`).
The permanent effect modifiers count in the value of `movement_speed`, and the save holds them
(steps `effects_value` and `effects_saved`).

## 4. The value

**ATR-VAL-001** (`source_suggested_black_box_confirmed`, client agreement, confidence: high). The
value of an instance comes from its base value `b`, its modifiers, and the range of its attribute.
Do these steps in f64:

1. Let `s = b`. For each modifier of the operation `add_value`: `s = s + amount`.
2. Let `r = s`. For each modifier of the operation `add_multiplied_base`: `r = r + (s × amount)`.
3. For each modifier of the operation `add_multiplied_total`: `r = r × (1 + amount)`.
4. The value is the result of ATR-VAL-003 for `r`.

Each `+` and each `×` is one IEEE 754 binary64 operation that rounds to the nearest even value.
The product of step 2 is rounded before the sum. Each step takes its modifiers in the order of
ATR-VAL-002.

The vanilla client computes the same value from the base value and the modifiers that the server
sends. A reader on the server and the same reader on the client must get the same bits. A closed
form, for example `(b + A) × (1 + B) × C` with the sums `A` and `B` and the product `C`, does not
give the same bits.

**ATR-VAL-002** (`source_suggested_black_box_confirmed`, client agreement, confidence: high for at
most 24 modifiers of one operation). The order of the modifiers in one step of ATR-VAL-001 is not
the order of the sequence. It comes from the identifiers.

The home slot of the identifier `namespace:path` is the result of these steps. Each step is
integer arithmetic modulo 2^32:

1. For a text, let `h = 0`. For each character of the text, in order: `h = 31 × h + c`. Here `c` is
   the code of the character. The characters of an identifier are ASCII.
2. `H = 31 × h(namespace) + h(path)`.
3. `m = H × 2654435769`.
4. The home slot is `(m XOR floor(m / 65536)) mod 32`.

To get the order of one step of ATR-VAL-001, do this:

1. Take the modifiers of the operation of that step, in the order of the sequence.
2. Take an empty ring of 32 slots, with the numbers 0 to 31.
3. Put each modifier in the first empty slot at or after its home slot. Slot 0 comes after slot 31.
4. Read the slots from 31 down to 0.

The modifiers of another operation have no effect on this order. The order depends only on the
present sequence, not on the modifiers that the instance had before.

These are home slots for a test:

| Identifier | Home slot |
|------------|-----------|
| `minecraft:sprinting` | 23 |
| `minecraft:effect.speed` | 19 |
| `minecraft:effect.slowness` | 2 |
| `minecraft:powder_snow` | 12 |
| `minecraft:baby` | 2 |
| `minecraft:creative_mode_block_range` | 25 |
| `minecraft:armor.helmet`, `minecraft:armor.chestplate`, `minecraft:armor.leggings`, `minecraft:armor.boots` | 14, 19, 20, 17 |
| `test:v`, `test:w`, `test:x` | 1, 1, 2 |

Thus `movement_speed` takes `minecraft:sprinting`, then `minecraft:effect.speed`, then
`minecraft:effect.slowness`, in each sequence of these three. The sequence `test:v`, `test:w`,
`test:x` gives the order `test:x`, `test:w`, `test:v`. The sequence `test:x`, `test:w`, `test:v`
gives the order `test:v`, `test:x`, `test:w`.

The order changes the value in normal play. The effects `speed` of the amplifier 0 and `slowness`
of the amplifier 2 with the sprint modifier give `0.08579999948889012`, the bits
`3fb5f6fd1fcd35a5`. The order `speed`, `slowness`, sprint gives the bits `3fb5f6fd1fcd35a4`, one
unit lower in the last place.

The observer of this rule is a reader that compares the f64 itself, for example the feedback of
`/attribute <target> minecraft:movement_speed get`. That feedback shows the shortest decimal text
of the value: `0.08579999948889012` for the input above, and `0.08579999948889011` for the other
order.

With more than 24 modifiers of one operation in one instance, the order is of the kind
unspecified. A parity test must not compare the last bits of such a value. Normal play does not
give that many modifiers. The part `order_cases` of the fixture `attributes.json` holds 37 orders
of the game, and the part `value_cases` holds 68 values as bits.

**ATR-VAL-003** (`source_suggested_black_box_confirmed`, client agreement, confidence: high). The
result for `r` is:

- the minimum, when `r` is a NaN;
- the minimum, when `r` is less than the minimum;
- the maximum, when `r` is more than the maximum;
- positive zero, when `r` is negative zero and the minimum is 0;
- `r`, in each other case.

Thus an infinite `r` gives the minimum or the maximum. A NaN base value of `gravity` gives the
value -1. The value of an instance is never a NaN and never infinite.

These are values for a test. The fixture holds them as bits.

| Attribute | Base value | Modifiers (identifier, amount, operation id) | Value |
|-----------|------------|----------------------------------------------|-------|
| `movement_speed` | 0.10000000149011612 | none | 0.10000000149011612 |
| `movement_speed` | 0.10000000149011612 | `minecraft:sprinting` 0.30000001192092896 2 | 0.13000000312924387 |
| `movement_speed` | 0.10000000149011612 | `minecraft:effect.speed` 0.20000000298023224 2 | 0.12000000208616257 |
| `movement_speed` | 0.10000000149011612 | the two modifiers of the rows above | 0.15600000414252285 |
| `movement_speed` | 0.10000000149011612 | `minecraft:effect.slowness` -1.0500000417232513 2 | 0 |
| `movement_speed` | 0.10000000149011612 | `minecraft:powder_snow` -0.05000000074505806 0 | 0.05000000074505806 |
| `movement_speed` | 0.23000000417232513 | `minecraft:baby` 0.5 1 | 0.3450000062584877 |
| `movement_speed` | 0.1 | `test:a` 0.2 0; `test:c` 0.1 1; `test:e` 0.2 1; `test:g` 0.3 2; `test:r` 0.7 2 | 0.8619000000000002 |
| `block_interaction_range` | 4.5 | `minecraft:creative_mode_block_range` 0.5 0 | 5 |
| `max_health` | 20 | `test:a` 2000 0 | 1024 |
| `max_health` | 20 | `test:a` -40 0 | 1 |

## 5. The sources of modifiers

**ATR-SRC-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). A source is
a rule that gives a modifier to an entity while a condition is true. A source sets its modifier
when the condition starts or when the amount changes. It removes the modifier when the condition
ends. A source does nothing when the attribute set of the entity does not have the attribute. These
are the sources:

| Source | Attribute | Identifier | Kind | Rule |
|--------|-----------|------------|------|------|
| the sprint state | `movement_speed` | `minecraft:sprinting` | transient | ATR-SRC-002 |
| creative mode | `block_interaction_range`, `entity_interaction_range` | `minecraft:creative_mode_block_range`, `minecraft:creative_mode_entity_range` | transient | ATR-SRC-003 |
| a status effect | the table of EFX-ATT-002 | `minecraft:effect.<name>` | permanent | EFX-ATT-001 of `status-effects.md` |
| the frozen ticks | `movement_speed` | `minecraft:powder_snow` | transient | ENV-FRZ-005 of `environment-damage.md` |
| an item in a slot of its slot group | the component `minecraft:attribute_modifiers` (STK-SHP-016 of `item-stack.md`) | the identifier of the component entry | transient | no specification yet; issues #121 and #214 |
| a baby mob | `movement_speed` | `minecraft:baby` | transient | the specification of the mob; TRK-E2 shows the modifier |
| the command `/attribute` | each | the identifier of the command line | permanent | no specification yet |

The trace of experiment A3 confirms the kinds of the rows creative mode, status effect, baby mob
and command. A save holds a modifier only when it is permanent (ATR-SAV-001). The steps
`baby_zombie_speed` and `baby_zombie_modifier` show the modifier `minecraft:baby` with the amount
0.5 and the value `0.3450000062584877` on a baby `zombie`. The step `baby_zombie_saved` shows the
save of that zombie: `movement_speed` with the base value `0.23000000417232513` and no modifier.
The step `modifier_saved` shows the modifier of the command in the save. No experiment shows the
kind of the sprint, frozen and item rows.

The death of a player and the respawn drop each modifier and keep each base value (PLD-RST-004 of
`player-death.md`).

**ATR-SRC-002** (`source_suggested_black_box_confirmed`, client agreement, confidence: high). The
sprint state of a living entity gives this modifier to its `movement_speed`:

| Field | Value |
|-------|-------|
| identifier | `minecraft:sprinting` |
| amount | the f32 value 0.3, widened: `0.30000001192092896` |
| operation | `add_multiplied_total` |
| kind | transient |

When the sprint state changes to on, the server sets the modifier. When the sprint state changes
to off, the server removes the modifier. The vanilla client does the same for its local player in
the tick in which the local sprint state changes. It does not wait for a packet. Thus the server
and the client agree on the speed while the sprint state is the same on both.

The recording `e9_sprint_stone.txt` shows the modifier in a packet to the own player after the
sprint starts. It shows a packet with no modifier after the sprint stops.

**ATR-SRC-003** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). While the
game mode of a player is creative, the player has these two modifiers. Each is transient and has
the operation `add_value`.

| Attribute | Identifier | Amount |
|-----------|------------|--------|
| `block_interaction_range` | `minecraft:creative_mode_block_range` | 0.5 |
| `entity_interaction_range` | `minecraft:creative_mode_entity_range` | 2.0 |

The server sets them in the first tick of the player in creative mode. It removes them in the
first tick of the player in another game mode. Thus the value of `block_interaction_range` is 5.0
in creative mode and 4.5 in survival mode. The client gets both modifiers in `update_attributes`.
The recording `e12_fly.txt` shows them for a player in creative mode. The trace of experiment A3
shows the values: 5.0 for both attributes in creative mode (steps `creative_block_range` and
`creative_entity_range`), and 4.5 after the change back to survival mode (step
`survival_block_range`).

## 6. The packets that the server sends

**ATR-NET-001** (`source_suggested`, wire, confidence: high). The packet `update_attributes` is
play clientbound 134. MVC-PKT-026 of `movement-checks.md` states its layout. The attribute of an
entry is the numeric id of ATR-DEF-001. The operation of a modifier is the id of ATR-INS-003. The
server sends the packet only for a living entity.

**ATR-NET-002** (`source_suggested_black_box_confirmed`, wire, confidence: high). The server sends
only syncable attributes. A client keeps the base of the type for each other attribute. An entry for an instance has these values:

- the base value as stored, not the value and not a clamped base value;
- each modifier of the instance, permanent and transient, in the order of the sequence.

One packet has at most one entry for each attribute, and one entry has at most one modifier for
each identifier. The order of the entries in a packet is of the kind unspecified.

The client puts the modifiers in its instance in the order of the entry (ATR-CLI-002). Thus the
server must compute the value from the same sequence that it sends. The recordings of section 0.5
show the base value `0.10000000149011612` with the sprint modifier, not the value.

The recording `a5_slowness_sprint.txt` of experiment A5 shows both kinds in one entry. The player
gets the effect `slowness` first and then sprints. The entry of `movement_speed` holds the base
value `0.10000000149011612`, then the permanent modifier `minecraft:effect.slowness`, then the
transient modifier `minecraft:sprinting`. This is the order of the sequence, not the order of
ATR-VAL-002, which puts the sprint modifier first.

**ATR-NET-003** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). When an
instance of a syncable attribute changes, the server sends its entry in the next update of the
entity (TRK-UPD-001 of `entity-tracking.md`). A change does not cause an update. The update sends
one `update_attributes` with one entry for each instance that changed after the last update.
TRK-UPD-002 states the position of the packet in the update.

These players get the packet:

- each tracking player of the entity;
- the own player, when the entity is a player (MVC-PKT-031 of `movement-checks.md`).

An entry holds the state of the instance at the update. An instance that changed two times
between two updates gives one entry. A change of a non-syncable instance sends nothing.

This rule has no exception for the sprint modifier. The own player gets the entry of
`movement_speed` after its own sprint state changes, although its client has the modifier
already (ATR-SRC-002). In `e9_sprint_stone.txt` the entry comes about 20 ms after the client sends
the sprint input.

The recordings of experiments A4 and A5 show the own player. In `a5_slowness_sprint.txt` the
entry comes 51 ms after the client sends the sprint input, when the sprint starts and when it
stops. That is one server tick: the server reads the input in a tick and sends the entry in the
update of the same tick. In `a4_respawn.txt` a console command sets the base value of
`max_health`, and a second command adds a modifier to `luck`. Each command gives one packet with
one entry, for the instance that it changed. Between these packets, the own player gets no entry
for an instance that did not change.

**ATR-NET-004** (`source_suggested_black_box_confirmed`, client agreement, confidence: medium). A
player that starts to track a living entity gets the instances in the spawn bundle. TRK-DATA-004
of `entity-tracking.md` states which entries the bundle must hold.

**ATR-NET-005** (`source_suggested_black_box_confirmed`, client agreement, confidence: medium). The
client of a player makes a new local player for a `login` packet and for each `respawn` packet
(ATR-CLI-005). After each of these packets, the first update of the player must send entries to
the own player. It sends an entry for each syncable instance of which one of these is true:

- the base value differs from the base of the type;
- the instance has a modifier.

The server can send entries for other syncable instances in that packet.

The recording `a4_respawn.txt` of experiment A4 confirms this for the respawn after a death. Before
the death, the player has the base value 30 of `max_health`, the modifier `test:a` on `luck` and
the effect `speed`. The client applies one packet in the client tick of the teleport of the
respawn. It has these entries: `max_health` with the base value 30, and `luck`, `movement_speed`,
`block_interaction_range` and `entity_interaction_range`, each with the base of the type and no
modifier. No experiment shows the `login` packet of a player whose instances differ from the base
of the type (U5).

## 7. What a client does

**ATR-CLI-001** (`source_suggested`, client agreement, confidence: high). A client gives each new
living entity the instances of ATR-BASE-001, each with the base of the type and no modifier. The
local player has them before the first `update_attributes`. For example, the `movement_speed` of
the local player is `0.10000000149011612` at the join.

**ATR-CLI-002** (`source_suggested`, client agreement, confidence: high). For an
`update_attributes` packet, the client finds the entity of the entity id. For each entry, in
order, the client does these steps on the instance of the attribute:

1. Set the base value to the base value of the entry.
2. Remove each modifier of the instance.
3. Add each modifier of the entry, in the order of the entry.

Step 2 also removes a sprint modifier that the client added itself (ATR-SRC-002). An instance that
has no entry in the packet does not change. The client then reads values by ATR-VAL-001.

**ATR-CLI-003** (`source_suggested`, client agreement, confidence: medium). These packets and
entries have no effect on a client:

| Input | Vanilla client | Client of this project |
|-------|----------------|------------------------|
| the entity id has no entity | ignores the packet | ignores the packet |
| the entity is not a living entity | fails | ignores the packet |
| the attribute of an entry is not in the attribute set of the entity type | ignores the entry, and applies the other entries | the same |
| the numeric id of an attribute is not in the registry | fails to decode the packet | the same |
| two modifiers of one entry have the same identifier | fails | keeps the first one and ignores the others |

The server must not send an input of the rows 2, 4 and 5. ATR-NET-001 and ATR-NET-002 prevent
them.

**ATR-CLI-004** (`source_suggested`, client agreement, confidence: high). The client adds and
removes the sprint modifier of its local player by itself (ATR-SRC-002). The client adds no other
modifier by itself. Each other modifier comes from an entry of a packet.

**ATR-CLI-005** (`source_suggested`, client agreement, confidence: medium). For a `respawn`
packet, the client makes a new local player. The bit `0x01` of the field "data to keep" selects
what the new local player gets (DIM-CHG-004 of `dimension-change.md`).

- When the bit is set, the new local player gets each base value and each modifier of the old one.
- When the bit is clear, it gets each base value of the old one, and no modifier.

For a `login` packet, the new local player has the instances of ATR-CLI-001.

A recording of the packets does not show the instances of the client itself. In `a4_respawn.txt`
the entries of ATR-NET-005 come in the client tick of the respawn. Thus no client tick reads the
instances that the client copies from the old local player for those attributes. An experiment
that reads the values on the client after a respawn decides this rule (section 13, A6).

## 8. The save data

**ATR-SAV-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The save of a
living entity has the list `attributes`. `persistence.md` states the fields of an attribute
compound. The list must have a compound for each instance of which one of these is true:

- the base value differs from the base of the type;
- the instance has a permanent modifier.

The list can have compounds for other instances. A compound holds the name of the attribute as
`id` and the stored base value as `base`. It holds the permanent modifiers as `modifiers`, in the
order of the sequence. It holds no transient modifier. The field `modifiers` can be absent when the
instance has no permanent modifier.

The trace of experiment A3 shows these saves:

| Step | Instance | Compound in the save |
|------|----------|----------------------|
| `base_above_maximum_saved` | `max_health` with the base value 5000 | `base` 5000, no `modifiers` |
| `modifier_saved` | `luck` with the modifier `test:a` of the command | the modifier with `amount` 1.0, `id` `test:a` and `operation` `add_value` |
| `effects_saved` | `movement_speed` with `speed`, then `slowness` | the two effect modifiers, in that order |
| `creative_saved` | `block_interaction_range` in creative mode | `base` 4.5, no `modifiers` |
| `baby_zombie_saved` | `movement_speed` of a baby `zombie` | `base` `0.23000000417232513`, no `modifiers` |

**ATR-SAV-002** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The load of a
living entity reads each compound of `attributes`. For a compound whose attribute is in the
attribute set of the entity type, the load does this:

1. It sets the base value to `base`, as given (ATR-INS-002).
2. It sets each modifier of `modifiers` as a permanent modifier, in the order of the list
   (ATR-INS-004).

An instance with no compound keeps the base of the type and has no modifier. A compound whose
attribute is not in the attribute set has no effect. The load adds no transient modifier. The
sources of section 5 set their transient modifiers again while their conditions are true.

A correct save does not have the compounds of the table below. The load reads each compound and
each modifier alone, so a bad one does not stop the load of the others.

| Input | Result of the load |
|-------|--------------------|
| the `id` of a compound is not a name of ATR-DEF-001 | the compound has no effect |
| `base` is not a number | the base value is 0, the default of `persistence.md` |
| a modifier has an `operation` that is not a name of ATR-INS-003 | that modifier has no effect; the other modifiers of the compound load |
| two modifiers of one compound have the same `id` | the instance has one modifier of that identifier: the last one of the list |

Step 2 is a set, not an add. Thus the last modifier of one identifier replaces the earlier one,
and no error stops the load.

The trace of experiment A3 shows each row of this rule. A `pig` loads `max_health` 30 and ignores
`attack_damage`, which its attribute set does not have (step `pig_with_bases_saved`). The other
steps are `pig_unknown_attribute_saved`, `pig_bad_base_saved`, `pig_bad_operation_saved`,
`pig_good_and_bad_modifier_saved` and `pig_two_modifiers_one_id_saved`. Each of these pigs comes
from `/summon` with the data of the compounds, and `/data get` reads its save.

## 9. Readers

A reader reads the value of ATR-VAL-001, not the base value. A reader that needs an f32 narrows the
value, and its own rule states that. These documents state readers:

| Attribute | Reader |
|-----------|--------|
| `movement_speed` | PHY-INP-006 of `entity-physics.md`; CCAM-FOV-002 of `client-camera.md`; MVC-LAY-009 of `movement-checks.md` |
| `block_interaction_range` | BRK-RCH-001 and BRK-SRV-012 of `block-breaking.md`; CBK-TGT-004 of `client-block-breaking.md` |
| `max_health`, `armor`, `armor_toughness`, `max_absorption`, `safe_fall_distance`, `fall_damage_multiplier` | DMG-ATT-001 and DMG-ATT-002 of `entity-damage.md` |
| `scale`, `step_height`, `gravity`, `jump_strength` and the other movement attributes | `entity-physics.md` |

A rule that acts when a value changes is a rule of the reader. For example, DMG-ATT-001 lowers the
health when `max_health` becomes less than the health.

## 10. Out of scope

| Subject | Reason |
|---------|--------|
| the command `/attribute` and its messages | a command specification must state them |
| the modifiers of items, enchantments and equipment | issues #121 and #214 |
| the changes of a base value at the spawn of a mob | the specification of each mob |
| the respawn that keeps all player data (the return from the End) on the server | no specification states that respawn yet |
| the modifier of the crouch state on `waypoint_transmit_range`, and the two waypoint attributes | waypoints are another domain |
| the text and the colour of an attribute line in an item tooltip | the tooltip is another domain; ATR-DEF-002 gives it the data |
| the reaction of each reader to a changed value | section 9 |
| a data pack that adds an attribute | the registry is static in this game version |

## 11. Intentional deviations

- **Entries for instances that did not change.**
  - **Rule:** ATR-NET-003.
  - **Reference:** it marks an instance as changed when the server first reads it. It then sends an
    entry with the base of the type and no modifier, for example `movement_speed` of a mob.
  - **Visible to the client:** yes, as an entry on the wire. The player sees no difference, because
    the client has the same values.
  - **Test:** a differential test drops an entry of the reference that has the base of the type
    and no modifier, when the last entry of that instance to that client was equal or when no
    entry came before it. It keeps an entry that removes a modifier or restores the base value.
    It also keeps each entry of `movement_speed` to the own player, because such an entry removes
    the local sprint modifier (ATR-CLI-002).
- **Entries for a modifier that a source sets again.**
  - **Rule:** ATR-INS-004.
  - **Reference:** some sources remove a modifier and add an equal one. The reference then sends
    the entry again.
  - **Visible to the client:** yes, as a repeated entry with the same content. The player sees no
    difference.
  - **Test:** a differential test drops an entry of the reference that is equal to the last entry
    of that instance. It keeps each entry of `movement_speed` to the own player (ATR-CLI-002).
- **The order of the modifiers in one entry.**
  - **Rule:** ATR-INS-004 and ATR-NET-002.
  - **Reference:** some sources replace a modifier in its position, and other sources move it to
    the end.
  - **Visible to the client:** yes, as the order on the wire. The value on the client differs only
    when two modifiers of one operation have a home slot conflict. With the amounts of normal play
    the difference is in the last bit. Amounts that cancel can give a larger difference.
  - **Test:** a differential test compares the modifiers of an entry as a set. It does not compare
    the value of an instance in which two modifiers of one operation have a home slot conflict.
- **Compounds for instances that did not change.**
  - **Rule:** ATR-SAV-001.
  - **Reference:** it saves each instance that the server read, also with the base of the type.
  - **Visible to the client:** no. A save tool sees more compounds.
  - **Test:** a differential test drops each compound that has the base of the type and no
    modifier.

## 12. Unresolved

- **U1.** ATR-VAL-002 states the order that the game has, and that order comes from a hash of the
  identifier. A different order changes a value by one unit in the last place of the f64. The author
  computed 42 combinations on the base value `0.10000000149011612`:
  - `speed` of the amplifier 0, 1, 2, 3, 4, 5 or 6;
  - `slowness` of the amplifier 0, 1 or 2;
  - the sprint modifier, present or absent.

  The author compared the order of ATR-VAL-002 (sprint, `speed`, `slowness`) with the order
  `speed`, `slowness`, sprint. In 5 combinations the f64 differs: the amplifier pairs (0, 2),
  (2, 0), (3, 1), (6, 1) and (6, 2) of `speed` and `slowness`, each with the sprint modifier. In
  none of them the f32 that the movement reads differs. A second order, `slowness`, sprint, `speed`,
  changes the f64 in 3 of the 42 combinations. An earlier count of 13 is not reproducible from a
  set that has the sprint modifier present and absent. The owner must decide: keep ATR-VAL-002 as it
  is, or make the order unspecified with a tolerance of one unit in the last place.
- **U2.** ATR-CLI-002 removes the local sprint modifier when an entry of `movement_speed` comes.
  An entry that the server made before it got the sprint input removes the modifier while the
  player sprints. The speed on the client is then wrong until the next entry comes. MVC-LAT-010 of
  `movement-checks.md` depends on this behaviour: the server uses the modifiers of each entry as the
  client applies them. The owner must decide if the client of this project keeps its local sprint
  modifier in that case.
- **U5.** Experiments A4 and A5 show the time of an entry to the own player after a change
  (ATR-NET-003) and after a respawn (ATR-NET-005). Two points stay open, because the client
  recorder accepts one client and no client that joins again:
  - the entries of the first update after a `login` packet, for a player whose instances differ
    from the base of the type (ATR-NET-005);
  - the time of an entry for a tracking player, and the entries of the spawn bundle
    (ATR-NET-003, ATR-NET-004).

  Section 13 states the change of the tool that A4 and A5 need.

## 13. Experiments

| Id | Tool and input | State | Rules |
|----|----------------|-------|-------|
| A1 | `tools/vanilla-oracle`, task `dumpAttributes`. Run `cargo run -p mcrs_minecraft_update -- recapture attributes`. It writes the base values of each type, 68 values and 37 orders. | ran on `26.4-snapshot-7` | ATR-BASE-001 to ATR-BASE-003, ATR-INS-002, ATR-VAL-001 to ATR-VAL-003 |
| A2 | The recordings `e9_sprint_stone.txt`, `e12_fly.txt` and `e10_jump_boost.txt` of the client recorder. Read the lines `< attribute`. | ran before this document | ATR-SRC-002, ATR-SRC-003, ATR-NET-002, ATR-NET-003 |
| A3 | `tools/mcrs_minecraft_probe` with the scenario `scenarios/attributes.json`. Record each reply. The steps `baby_zombie` to `baby_zombie_saved` summon a baby `zombie`, read its value, its modifier and its field `attributes`. The trace is `tools/mcrs_minecraft_probe/traces/attributes.json`. | ran on `26.4-snapshot-7`; two runs give the same trace | ATR-BASE-001, ATR-INS-002, ATR-INS-004, ATR-INS-005, ATR-SRC-001, ATR-SRC-003, ATR-SAV-001, ATR-SAV-002, U3, U6 |
| A4 | The client recorder, scenario `a4`. Give the player a base value of 30 for `max_health`, a modifier of `/attribute` on `luck` and the effect `speed` with the infinite duration. Kill the player; the rule `immediate_respawn` respawns it. Record each `update_attributes` for the own player, with its tick. The recording is `crates/mcrs_minecraft_entity/tests/fixtures/vanilla/a4_respawn.txt`. | the respawn ran on `26.4-snapshot-7`; the leave and the join did not run | ATR-NET-003, ATR-NET-005, PLD-RST-004 |
| A5 | The client recorder, scenario `a5`. Give the player the effect `slowness`, then let it sprint for 20 ticks. Record each `update_attributes`, with its tick. The recording is `crates/mcrs_minecraft_entity/tests/fixtures/vanilla/a5_slowness_sprint.txt`. | the own player ran on `26.4-snapshot-7`; the second client did not run | ATR-NET-002, ATR-NET-003 |
| A6 | Not yet possible. Repeat A4 with a leave and a join before the death. Repeat A5 with a second client that tracks the first player. In each client, read the values of the local player after each packet. | not run | ATR-NET-003, ATR-NET-004, ATR-NET-005, ATR-CLI-005, U5 |

A3, A4 and A5 need a vanilla server. `tools/vanilla-server` starts one only when the person that
runs it accepts the EULA of the game. Run A4 and A5 with this command, with `a5` in place of `a4`
for A5:

```sh
MINECRAFT_EULA=true AUTOPLAY=true MAX_FPS=20 tools/mcrs_minecraft_client_recorder/record.sh a4 \
    crates/mcrs_minecraft_entity/tests/fixtures/vanilla
```

A6 needs these changes of the client recorder. They are more than an input file:

- a scenario line that makes the client leave and join again in a step, and a recording of the
  packets from the `login` packet, with their ticks; now the events before the first teleport of
  a step go only into the start state of the recording;
- a second client, and the entries of `update_attributes` for an entity other than the own player,
  with the network id of that entity;
- a line of the recording for the values that the vanilla client reads for its local player.

## 14. Relation to other documents

| Document | Relation |
|----------|----------|
| `entity-tracking.md` | TRK-TYPE-003 and TRK-TYPE-004 repeat rows of Appendix A. TRK-DATA-004 states the entries of the spawn bundle. TRK-UPD-001 and TRK-UPD-002 state the update. |
| `entity-damage.md` | DMG-ATT-002 reads values. Its Appendix A states the immunities of each type and cites Appendix A of this document for the base values. |
| `entity-physics.md` | PHY-INP-006 reads `movement_speed` and cites the value rules. |
| `movement-checks.md` | MVC-PKT-026 states the layout of the packet. MVC-LAY-009 computes a value at a proxy and cites the value rules. |
| `status-effects.md` | EFX-ATT-001 and EFX-ATT-002 are a source. EFX-ATT-003 cites the value rules. |
| `client-camera.md` | CCAM-FOV-002 reads `movement_speed` and cites the value rules. |
| `block-breaking.md` | BRK-RCH-001 and BRK-SRV-012 read `block_interaction_range`. |
| `environment-damage.md` | ENV-FRZ-005 is a source. |
| `player-death.md`, `dimension-change.md` | PLD-RST-004 and DIM-CHG-004 state what a respawn keeps. |
| `persistence.md` | It states the fields of an attribute compound. |

## Appendix A: the attribute set and the base values of each living entity type

Each type has the common set of ATR-BASE-002 and the attributes of the column "More attributes".
The column "Base values" gives each base of the type that differs from the attribute default. Each
other base of the type is the attribute default of ATR-DEF-001. Experiment A1 gives this table.

| Type | More attributes | Base values |
|------|-----------------|-------------|
| `allay` | `attack_damage`, `flying_speed`, `follow_range` | `flying_speed` 0.10000000149011612, `follow_range` 16, `movement_speed` 0.10000000149011612 |
| `armadillo` | `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 12, `movement_speed` 0.14 |
| `armor_stand` | none | `step_height` 0 |
| `axolotl` | `attack_damage`, `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 14, `movement_speed` 1, `step_height` 1 |
| `bat` | `follow_range` | `follow_range` 16, `max_health` 6 |
| `bee` | `attack_damage`, `flying_speed`, `follow_range`, `tempt_range` | `flying_speed` 0.6000000238418579, `follow_range` 16, `max_health` 10, `movement_speed` 0.30000001192092896 |
| `blaze` | `attack_damage`, `follow_range` | `attack_damage` 6, `follow_range` 48, `movement_speed` 0.23000000417232513 |
| `bogged` | `attack_damage`, `follow_range` | `follow_range` 16, `max_health` 16, `movement_speed` 0.25 |
| `breeze` | `attack_damage`, `follow_range` | `attack_damage` 3, `follow_range` 24, `max_health` 30, `movement_speed` 0.6299999952316284 |
| `camel` | `follow_range`, `tempt_range` | `fall_damage_multiplier` 0.5, `follow_range` 16, `max_health` 32, `movement_speed` 0.09000000357627869, `safe_fall_distance` 6, `step_height` 1.5 |
| `camel_husk` | `follow_range`, `tempt_range` | `fall_damage_multiplier` 0.5, `follow_range` 16, `max_health` 32, `movement_speed` 0.09000000357627869, `safe_fall_distance` 6, `step_height` 1.5 |
| `cat` | `attack_damage`, `follow_range`, `tempt_range` | `attack_damage` 3, `follow_range` 16, `max_health` 10, `movement_speed` 0.30000001192092896 |
| `cave_spider` | `attack_damage`, `follow_range` | `follow_range` 16, `max_health` 12, `movement_speed` 0.30000001192092896 |
| `chicken` | `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 4, `movement_speed` 0.25 |
| `cod` | `follow_range` | `follow_range` 16, `max_health` 3 |
| `copper_golem` | `follow_range` | `follow_range` 16, `max_health` 12, `movement_speed` 0.20000000298023224, `step_height` 1 |
| `cow` | `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 10, `movement_speed` 0.20000000298023224 |
| `creaking` | `attack_damage`, `follow_range` | `attack_damage` 3, `max_health` 1, `movement_speed` 0.4000000059604645, `step_height` 1.0625 |
| `creeper` | `attack_damage`, `follow_range` | `follow_range` 16, `movement_speed` 0.25 |
| `dolphin` | `attack_damage`, `follow_range` | `attack_damage` 3, `follow_range` 16, `max_health` 10, `movement_speed` 1.2000000476837158 |
| `donkey` | `follow_range`, `tempt_range` | `fall_damage_multiplier` 0.5, `follow_range` 16, `jump_strength` 0.5, `max_health` 53, `movement_speed` 0.17499999701976776, `safe_fall_distance` 6, `step_height` 1 |
| `drowned` | `attack_damage`, `follow_range`, `spawn_reinforcements` | `armor` 2, `attack_damage` 3, `follow_range` 35, `movement_speed` 0.23000000417232513, `step_height` 1 |
| `elder_guardian` | `attack_damage`, `follow_range` | `attack_damage` 8, `follow_range` 16, `max_health` 80, `movement_speed` 0.30000001192092896 |
| `ender_dragon` | `follow_range` | `camera_distance` 16, `follow_range` 16, `max_health` 200 |
| `enderman` | `attack_damage`, `follow_range` | `attack_damage` 7, `follow_range` 64, `max_health` 40, `movement_speed` 0.30000001192092896, `step_height` 1 |
| `endermite` | `attack_damage`, `follow_range` | `follow_range` 16, `max_health` 8, `movement_speed` 0.25 |
| `evoker` | `attack_damage`, `follow_range` | `follow_range` 12, `max_health` 24, `movement_speed` 0.5 |
| `fox` | `attack_damage`, `follow_range`, `tempt_range` | `max_health` 10, `movement_speed` 0.30000001192092896, `safe_fall_distance` 5 |
| `frog` | `attack_damage`, `follow_range`, `tempt_range` | `attack_damage` 10, `follow_range` 16, `max_health` 10, `movement_speed` 1, `step_height` 1 |
| `frostbite` | `attack_damage`, `follow_range`, `spawn_reinforcements` | `armor` 2, `attack_damage` 3, `follow_range` 35, `max_health` 24, `movement_speed` 0.20999999344348907 |
| `ghast` | `flying_speed`, `follow_range` | `camera_distance` 8, `flying_speed` 0.06, `follow_range` 100, `max_health` 10 |
| `giant` | `attack_damage`, `follow_range` | `attack_damage` 50, `camera_distance` 16, `follow_range` 16, `max_health` 100, `movement_speed` 0.5 |
| `glow_squid` | `follow_range` | `follow_range` 16, `max_health` 10 |
| `goat` | `attack_damage`, `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 10, `movement_speed` 0.20000000298023224 |
| `guardian` | `attack_damage`, `follow_range` | `attack_damage` 6, `follow_range` 16, `max_health` 30, `movement_speed` 0.5 |
| `happy_ghast` | `flying_speed`, `follow_range`, `tempt_range` | `camera_distance` 8, `flying_speed` 0.05, `follow_range` 16, `movement_speed` 0.05, `tempt_range` 16 |
| `hoglin` | `attack_damage`, `follow_range` | `attack_damage` 6, `attack_knockback` 1, `follow_range` 16, `knockback_resistance` 0.6000000238418579, `max_health` 40, `movement_speed` 0.30000001192092896 |
| `horse` | `follow_range`, `tempt_range` | `fall_damage_multiplier` 0.5, `follow_range` 16, `jump_strength` 0.7, `max_health` 53, `movement_speed` 0.22499999403953552, `safe_fall_distance` 6, `step_height` 1 |
| `husk` | `attack_damage`, `follow_range`, `spawn_reinforcements` | `armor` 2, `attack_damage` 3, `follow_range` 35, `movement_speed` 0.23000000417232513 |
| `illusioner` | `attack_damage`, `follow_range` | `follow_range` 18, `max_health` 32, `movement_speed` 0.5 |
| `iron_golem` | `attack_damage`, `follow_range` | `attack_damage` 15, `follow_range` 16, `knockback_resistance` 1, `max_health` 100, `movement_speed` 0.25, `step_height` 1 |
| `llama` | `follow_range`, `tempt_range` | `fall_damage_multiplier` 0.5, `follow_range` 16, `jump_strength` 0.5, `max_health` 53, `movement_speed` 0.17499999701976776, `safe_fall_distance` 6, `step_height` 1 |
| `magma_cube` | `attack_damage`, `follow_range` | `follow_range` 16, `movement_speed` 0.20000000298023224 |
| `mannequin` | none | none |
| `mooshroom` | `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 10, `movement_speed` 0.20000000298023224 |
| `mule` | `follow_range`, `tempt_range` | `fall_damage_multiplier` 0.5, `follow_range` 16, `jump_strength` 0.5, `max_health` 53, `movement_speed` 0.17499999701976776, `safe_fall_distance` 6, `step_height` 1 |
| `nautilus` | `attack_damage`, `follow_range`, `tempt_range` | `attack_damage` 3, `follow_range` 16, `knockback_resistance` 0.30000001192092896, `max_health` 15, `movement_speed` 1 |
| `ocelot` | `attack_damage`, `follow_range`, `tempt_range` | `attack_damage` 3, `follow_range` 16, `max_health` 10, `movement_speed` 0.30000001192092896 |
| `panda` | `attack_damage`, `follow_range`, `tempt_range` | `attack_damage` 6, `follow_range` 16, `movement_speed` 0.15000000596046448 |
| `parched` | `attack_damage`, `follow_range` | `follow_range` 16, `max_health` 16, `movement_speed` 0.25 |
| `parrot` | `attack_damage`, `flying_speed`, `follow_range`, `tempt_range` | `attack_damage` 3, `flying_speed` 0.4000000059604645, `follow_range` 16, `max_health` 6, `movement_speed` 0.20000000298023224 |
| `phantom` | `attack_damage`, `follow_range` | `follow_range` 16 |
| `pig` | `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 10, `movement_speed` 0.25 |
| `piglin` | `attack_damage`, `follow_range` | `attack_damage` 5, `follow_range` 16, `max_health` 16, `movement_speed` 0.3499999940395355 |
| `piglin_brute` | `attack_damage`, `follow_range` | `attack_damage` 7, `follow_range` 12, `max_health` 50, `movement_speed` 0.3499999940395355 |
| `pillager` | `attack_damage`, `follow_range` | `attack_damage` 5, `max_health` 24, `movement_speed` 0.3499999940395355 |
| `player` | `attack_damage`, `attack_speed`, `block_break_speed`, `block_interaction_range`, `luck`, `mining_efficiency`, `sneaking_speed`, `submerged_mining_speed`, `sweeping_damage_ratio`, `waypoint_receive_range` | `attack_damage` 1, `movement_speed` 0.10000000149011612, `waypoint_transmit_range` 60000000, `waypoint_receive_range` 60000000 |
| `polar_bear` | `attack_damage`, `follow_range`, `tempt_range` | `attack_damage` 6, `follow_range` 20, `max_health` 30, `movement_speed` 0.25 |
| `pufferfish` | `follow_range` | `follow_range` 16, `max_health` 3 |
| `rabbit` | `attack_damage`, `follow_range`, `tempt_range` | `attack_damage` 3, `follow_range` 16, `max_health` 3, `movement_speed` 0.30000001192092896 |
| `ravager` | `attack_damage`, `follow_range` | `attack_damage` 12, `attack_knockback` 1.5, `knockback_resistance` 0.75, `max_health` 100, `movement_speed` 0.3, `step_height` 1 |
| `salmon` | `follow_range` | `follow_range` 16, `max_health` 3 |
| `sheep` | `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 8, `movement_speed` 0.23000000417232513 |
| `shulker` | `follow_range` | `follow_range` 16, `max_health` 30 |
| `silverfish` | `attack_damage`, `follow_range` | `attack_damage` 1, `follow_range` 16, `max_health` 8, `movement_speed` 0.25 |
| `skeleton` | `attack_damage`, `follow_range` | `follow_range` 16, `movement_speed` 0.25 |
| `skeleton_horse` | `follow_range`, `tempt_range` | `fall_damage_multiplier` 0.5, `follow_range` 16, `jump_strength` 0.7, `max_health` 15, `movement_speed` 0.20000000298023224, `safe_fall_distance` 6, `step_height` 1 |
| `slime` | `attack_damage`, `follow_range` | `follow_range` 16 |
| `sniffer` | `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 14, `movement_speed` 0.10000000149011612 |
| `snow_golem` | `follow_range` | `follow_range` 16, `max_health` 4, `movement_speed` 0.20000000298023224 |
| `spider` | `attack_damage`, `follow_range` | `follow_range` 16, `max_health` 16, `movement_speed` 0.30000001192092896 |
| `squid` | `follow_range` | `follow_range` 16, `max_health` 10 |
| `stray` | `attack_damage`, `follow_range` | `follow_range` 16, `movement_speed` 0.25 |
| `strider` | `follow_range`, `tempt_range` | `follow_range` 16, `movement_speed` 0.17499999701976776 |
| `sulfur_cube` | `follow_range`, `tempt_range` | `follow_range` 16, `tempt_range` 8 |
| `tadpole` | `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 6, `movement_speed` 1 |
| `trader_llama` | `follow_range`, `tempt_range` | `fall_damage_multiplier` 0.5, `follow_range` 16, `jump_strength` 0.5, `max_health` 53, `movement_speed` 0.17499999701976776, `safe_fall_distance` 6, `step_height` 1 |
| `tropical_fish` | `follow_range` | `follow_range` 16, `max_health` 3 |
| `turtle` | `follow_range`, `tempt_range` | `follow_range` 16, `max_health` 30, `movement_speed` 0.25, `step_height` 1 |
| `vex` | `attack_damage`, `follow_range` | `attack_damage` 4, `follow_range` 16, `max_health` 14 |
| `villager` | `follow_range` | `follow_range` 16, `movement_speed` 0.5 |
| `vindicator` | `attack_damage`, `follow_range` | `attack_damage` 5, `follow_range` 12, `max_health` 24, `movement_speed` 0.3499999940395355 |
| `wandering_trader` | `follow_range` | `follow_range` 16 |
| `warden` | `attack_damage`, `follow_range` | `attack_damage` 30, `attack_knockback` 1.5, `follow_range` 24, `knockback_resistance` 1, `max_health` 500, `movement_speed` 0.30000001192092896 |
| `witch` | `attack_damage`, `follow_range` | `follow_range` 16, `max_health` 26, `movement_speed` 0.25 |
| `wither` | `attack_damage`, `flying_speed`, `follow_range` | `armor` 4, `flying_speed` 0.6000000238418579, `follow_range` 40, `max_health` 300, `movement_speed` 0.6000000238418579 |
| `wither_skeleton` | `attack_damage`, `follow_range` | `follow_range` 16, `movement_speed` 0.25 |
| `wolf` | `attack_damage`, `follow_range`, `tempt_range` | `attack_damage` 4, `follow_range` 16, `max_health` 8, `movement_speed` 0.30000001192092896 |
| `zoglin` | `attack_damage`, `follow_range` | `attack_damage` 6, `attack_knockback` 1, `follow_range` 16, `knockback_resistance` 0.6000000238418579, `max_health` 40, `movement_speed` 0.30000001192092896 |
| `zombie` | `attack_damage`, `follow_range`, `spawn_reinforcements` | `armor` 2, `attack_damage` 3, `follow_range` 35, `movement_speed` 0.23000000417232513 |
| `zombie_horse` | `follow_range`, `tempt_range` | `fall_damage_multiplier` 0.5, `follow_range` 16, `jump_strength` 0.7, `max_health` 25, `movement_speed` 0.22499999403953552, `safe_fall_distance` 6, `step_height` 1 |
| `zombie_nautilus` | `attack_damage`, `follow_range`, `tempt_range` | `attack_damage` 3, `follow_range` 16, `knockback_resistance` 0.30000001192092896, `max_health` 15, `movement_speed` 1.100000023841858 |
| `zombie_villager` | `attack_damage`, `follow_range`, `spawn_reinforcements` | `armor` 2, `attack_damage` 3, `follow_range` 35, `movement_speed` 0.23000000417232513 |
| `zombified_piglin` | `attack_damage`, `follow_range`, `spawn_reinforcements` | `armor` 2, `attack_damage` 5, `follow_range` 35, `movement_speed` 0.23000000417232513 |
