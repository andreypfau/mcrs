# Molang queries of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Language | `molang.md` |

Source of these values: `assets/minecraft/version.json`.

This document states the contract between Molang (`molang.md`) and the host, and the meaning
of each query that the client supplies. A query gives a value of the subject of a run. The
meaning of an entity query is stated with the state that the Java client keeps for an entity.
`client-entities.md` states how the client updates that state. The document does not
prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Subject**, **run**, **load**, **missing**, **consumer**, **host**, **lane**, **milestone**:
  as `molang.md`, section 0.1, defines them.
- **Context of use**: the set of consumers in which a query is valid. This document uses
  three contexts of use:
  - `block`: the permutation conditions of a block definition under
    `assets/mcrs/block_definition/`. The subject is a block state.
  - `entity`: the scripts, animations, animation controllers and render controllers of a
    client entity (`client-entity-definition.md`, `entity-animation.md`). The subject is a
    client entity.
  - `any`: each context of use.
- **Client entity**: an entity that the client tracks, from its add packet to its remove
  packet (`client-entities.md`).
- **Tick**: one step of the client simulation. There are 20 ticks in a second.
- **Frame**: one image that the client renders.
- **Partial tick** `pt`: the part of the current tick that has gone at the time of the frame,
  in the range `[0, 1)`.
- **Age**: the number of ticks that the client entity ran since the add packet. It is `0` in the
  tick of the add packet (`client-entities.md`, CENTS-TIME-002).
- **Entity clock** `C`: `age + pt`, in ticks (`client-entities.md`, section 0.1).
- **Walk position** and **walk speed**: the two values of the walk accumulator of a client
  entity. `client-entities.md` states how each tick changes them. Each has an old value (the
  value at the start of the latest tick) and a new value.
- **Body yaw**, **head yaw**, **head pitch**: the angles of a client entity in degrees, each
  with an old value and a new value (`client-entities.md`). A yaw of `0` faces `+Z`. A yaw of
  `90` faces `−X`. A positive head pitch looks down.
- **Angle interpolation**: `alerp(t, a, b) = a + t · wrap(b − a)`, where `wrap(x)` gives the
  angle equal to `x` in `[-180, 180)`. It follows the shorter arc. `client-entities.md` uses
  the same function with the same name.
- **Rate class**: how often the value of a query can change (MOLQ-HOST-004).
- **Pose input**: how a GPU evaluation of an animation channel receives the value of a query
  (MOLQ-HOST-006).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". `documentation` refers to the public Bedrock documentation
of the query: its name, its arguments and its one-line meaning. The value in terms of the Java
client has the label `source_suggested` and an experiment.

Each experiment that compares a pose uses a pose dump of `tools/vanilla-oracle`,
`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_poses.json`. The dump gives the
transform of each model part of an entity for a set of inputs. Each experiment that compares a
pose uses the pose tolerance of `entity-animation.md`, EANIM-GPU-003.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `MOLQ-HOST` | the contract between Molang and the host |
| `MOLQ-BLOCK` | queries of a block state |
| `MOLQ-TIME` | time queries |
| `MOLQ-WALK` | walk queries |
| `MOLQ-MOVE` | movement and environment queries |
| `MOLQ-ROT` | rotation queries |
| `MOLQ-STATE` | state queries of a client entity |
| `MOLQ-ANIM` | queries that an animation or a controller supplies |
| `MOLQ-PURE` | queries that are functions of their arguments |
| `MOLQ-VIEW` | queries of the camera |
| `MOLQ-KNOWN` | Bedrock queries that the client knows and does not support |
| `MOLQ-DEV` | intentional deviations |

### 0.4 Scope

In scope: the declaration of a query, the rate classes, unknown queries, and the queries that
the tables of sections 2 to 9 list.

Out of scope: queries of items, armour, locators and particles, and the equipment queries
other than those of MOLQ-STATE-002. A later revision adds them. Until then they are unknown queries (MOLQ-HOST-002).
MOLQ-KNOWN-001 lists the Bedrock queries that the client knows by name and does not support.

Out of scope: the expressions of behaviour packs. The client does not compile them. A
behaviour pack runs on the server.

---

## 1. Host contract

### MOLQ-HOST-001: Query declarations

- **Inputs:** the set of queries that the client supports.
- **Behavior:** each query has a declaration with these parts:

| Part | Meaning |
|------|---------|
| name | the name after `query.`; names compare without case |
| arguments | the number of arguments and the type of each; `none` means that the query has no argument list |
| result | `number`, `string`, `struct` or `entity reference`; a `struct` result lists its member paths, and each member is a number (`molang.md`, MOL-VAL-001) |
| unit | the unit of a number result |
| rate class | MOLQ-HOST-004 |
| context of use | `block`, `entity` or `any` |
| default | the value when the subject has no such data; `missing` means that the result is missing |
| pose input | MOLQ-HOST-006; only for the context of use `entity` |
| milestone | `1`, `2` or `later` (`molang.md`, section 0.1) |

- **Postconditions:** the compiler checks each query reference against its declaration at
  load.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1.

### MOLQ-HOST-002: Unknown and invalid queries

- **Inputs:** a query reference in an expression.
- **Behavior:**
  1. When no declaration has the name, the compiler reports a load warning. The query reads
     as missing.
  2. In the context of use `block`, an unknown query is a load error `unknown_query`
     (`molang.md`, MOL-LOAD-004).
  3. When the context of use of the declaration does not include the consumer, the query
     reads as missing, and the compiler reports a load warning.
  4. When the number of arguments does not match the declaration, the compiler reports a
     load error `arity`.
  5. A query with no arguments can have an empty argument list: `q.is_baby` and `q.is_baby()`
     are equal.
  6. The warning of item 1 occurs one time for each query name and each consumer in one load
     (`molang.md`, MOL-LOAD-005, postcondition 2).
  7. A missing query result reads as `0.0`. `??` selects its right side (`molang.md`,
     MOL-EVAL-005). A member path on a missing result is also missing.
- **Postconditions:** an entity pack that uses queries that the client does not support loads.
  A block definition with an unknown query does not load.
- **Example:** `q.is_spectator ? 0 : 1` gives `1.0` in milestone 1, because the query is
  missing and reads as `0.0`. `(q.is_spectator ?? 1) ? 0 : 1` gives `0.0`.

Provenance: `documentation` (items 1, 4, 5); items 2, 3, 6 and 7: `design`. Kind: semantic.
Confidence: medium. Milestone: 1.

### MOLQ-HOST-003: Arguments

- **Inputs:** a query reference with arguments.
- **Behavior:**
  1. The host evaluates the arguments from left to right, before the query gives its value.
  2. A string argument that is a literal is known at load. A host can resolve it at load, for
     example to the index of a block state property.
  3. When the declaration needs a literal for an argument and the argument is not a literal,
     the compiler reports a load error `static_type`.
- **Postconditions:** the arguments have the side effects that `molang.md`, MOL-EVAL-001,
  states.

Provenance: `documentation`; item 3: `design`. Kind: semantic. Confidence: medium.
Milestone: 1.

### MOLQ-HOST-004: Rate classes

| Rate class | The value can change |
|------------|----------------------|
| `load` | only at a load; the same for each run on one subject |
| `frame` | in each frame; the same for each subject of the frame |
| `entity_tick` | at most one time in each tick, for each subject; a frame reads the value of the latest tick, with no interpolation |
| `entity_frame` | in each frame, for each subject; the value uses `pt` |
| `animation_frame` | in each frame, for each subject and each animation that the subject plays |

- **Postconditions:**
  1. An implementation can read a query one time for each subject and each frame, and use
     that value for each run of the frame.
  2. An expression that reads only queries of the class `load` and constants has one result
     for each subject. An implementation can compute it at load.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1.

### MOLQ-HOST-005: Stable values in one frame

- **Inputs:** two reads of the same query, with the same arguments, on the same subject, in
  the same frame.
- **Behavior:** the two reads give the same value.
- **Postconditions:** the order in which the client runs expressions does not change a query
  value.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1.

### MOLQ-HOST-006: Pose inputs

- **Inputs:** an entity query that an animation channel reads, in an implementation that
  evaluates the channel on a GPU.
- **Behavior:** the column "Pose input" of each entity query table states how a GPU
  evaluation gets the value of the query in a frame:

| Pose input | The CPU writes | The GPU computes |
|------------|----------------|------------------|
| `time` | the clock offset `O = T − age` of the subject, when it changes, and the entity clock at the start of each animation instance | the entity clock `C = T − O + pt`, with `T` the tick count of the client, and the differences of `C`; no pose input slot is used |
| `frame` | nothing for the subject; one value for each frame | the frame value |
| `tick` | the value of the latest tick, when it changes | the written value, with no interpolation |
| `tick_pt` | the value of the latest tick, when it changes | the written value plus `pt` when the written value is not `0` |
| `lerp` | the old value and the new value, when they change | `lerp(pt, old, new)` |
| `alerp` | the old value and the new value, when they change | `alerp(pt, old, new)` |
| `walk` | the walk position `P`, the walk speed `S`, the old walk speed `S0` and the walk scale `k` | the formulas of MOLQ-WALK-001 |
| — | nothing | nothing; the query is not a pose input |

  1. A query with the pose input `—` returns a string, or has a value only on the CPU.
  2. A channel that reads a query with the pose input `—` must not run on a GPU. It runs on
     the CPU.
  3. The values of the kinds `lerp`, `alerp` and `walk` come from the per-tick record of the
     subject: the body yaw, the head yaw, the head pitch, the walk position and the walk speed.
     They use no input slot. The values of the kinds `time` and `frame` use no input slot.
  4. Each `tick` and `tick_pt` value, and each result of a channel that runs on the CPU, takes
     one `f32` input slot of the subject. The design note `docs/molang.md`, section 10.2,
     states the limit of 16 input slots.
- **Postconditions:** a GPU evaluation and a CPU evaluation of the same query give the same
  value within the pose tolerance (`entity-animation.md`, EANIM-GPU-003).

Provenance: `design`. Kind: unspecified for the split between the CPU and the GPU; semantic for
the value. Confidence: medium. Milestone: 1.

---

## 2. Block state

### MOLQ-BLOCK-001: `block_state`

| Part | Value |
|------|-------|
| name | `block_state` |
| arguments | one string literal: the name of a property of the block |
| result | `number` or `string` |
| rate class | `load` |
| context of use | `block` |
| default | none; see item 2 |
| milestone | 1 |

- **Inputs:** the subject block state and the property name `p`.
- **Behavior:**
  1. The result is the value of `p` in the subject block state:
     - a Boolean property gives `1.0` for `true` and `0.0` for `false`;
     - an integer property gives the integer as a number;
     - each other property gives the name of the value as a string.
  2. When the block has no property `p`, the compiler reports a load error `static_type`.
  3. When the expression compares the query with a literal that is not a value of `p`, the
     compiler reports a load error `static_type`.
  4. A Boolean property compares only with `true` and `false`. A comparison with a number,
     for example `q.block_state('powered') == 1`, is a load error `static_type`.
- **Postconditions:**
  1. `q.block_state('powered') == true` is `1.0` for each state with `powered=true`.
  2. `q.block_state('facing') == 'north'` is `1.0` for each state with `facing=north`.
  3. `q.block_state('age') == 3` is `1.0` for each state with `age=3`.
  4. Each condition of a block definition has one result for each block state. The loader
     computes it at load. No condition runs in a tick.
- **Example:**

```json
{ "condition": "q.block_state('facing') == 'north' && q.block_state('open') == true" }
```

Provenance: `documentation` (the query and the comparison with `true`);
`black_box_confirmed` for the forms of use, `assets/mcrs/block_definition/` (53431 uses,
each a comparison with a string, a Boolean or an integer, joined with `&&`); items 2 to 4:
`design`, the error kind of MOL-LOAD-004 for an operand that the declaration rejects. Kind:
semantic. Confidence: high. Milestone: 1.

---

## 3. Time

### MOLQ-TIME-001: Time queries

| Name | Arguments | Result | Unit | Rate class | Context | Default | Pose input |
|------|-----------|--------|------|------------|---------|---------|------------|
| `life_time` | none | number | seconds | `entity_frame` | entity | `0.0` | `time` |
| `delta_time` | none | number | seconds | `entity_frame` | any | `0.0` | `time` |
| `frame_alpha` | none | number | 1 | `frame` | any | `0.0` | `frame` |
| `time_of_day` | none | number | 1 | `frame` | any | `0.0` | `frame` |

- **Behavior:**
  1. `life_time` is `C / 20`, with `C` the entity clock (`client-entities.md`, section 0.1).
  2. `delta_time` is the change of `C` since the previous frame of the subject, divided by 20.
     In the first frame of the subject it is `0.0`. Outside an entity it is the time from the
     start of the previous frame to the start of the current frame.
  3. `frame_alpha` is `pt`.
  4. `time_of_day` is `frac(d / 24000 + 0.25)`, where `d` is the day time of the level in
     ticks that the client knows, and `frac(x) = x − floor(x)`. `0.0` is midnight, `0.25` is
     sunrise, `0.5` is noon and `0.75` is sunset.
- **Postconditions:** `life_time` increases by `1/20` in each tick of the subject, and it
  increases continuously between ticks. It does not increase while the subject does not tick.

Provenance: `documentation` (meanings); `source_suggested` (the formula of `life_time` and the
offset of `time_of_day`). Kind: semantic. Confidence: medium. Milestone: 1. Experiment: spawn a
cow on the vanilla server. Record the pose dump of its head bob for 40 ticks. The phase of
`math.cos(q.life_time * K)` in the corresponding animation matches the dump.

---

## 4. Walk

### MOLQ-WALK-001: Walk queries

| Name | Arguments | Result | Unit | Rate class | Context | Default | Pose input |
|------|-----------|--------|------|------------|---------|---------|------------|
| `modified_distance_moved` | none | number | walk units | `entity_frame` | entity | `0.0` | `walk` |
| `modified_move_speed` | none | number | walk units per tick | `entity_frame` | entity | `0.0` | `walk` |

- **Inputs:** the walk position `P` (new value), the walk speed `S` (new value), the old walk
  speed `S0`, the walk scale `k` and `pt`.
- **Behavior:**
  1. The walk scale `k` is `3` when the subject is a baby (MOLQ-STATE-001), and `1` otherwise.
  2. `modified_distance_moved` is `(P − S · (1 − pt)) · k`.
  3. `modified_move_speed` is `min(S0 + pt · (S − S0), 1)`.
- **Postconditions:**
  1. `modified_move_speed` is never greater than `1`. The walk speed can be `1.5` after a hurt
     (`client-entities.md`), but this query limits it.
  2. A leg that swings with `math.cos(q.modified_distance_moved * 38.1704) * 80.2141 *
     q.modified_move_speed` degrees has the swing of the Java client. `38.1704` degrees is
     `0.6662` radians, and `80.2141` degrees is `1.4` radians, each to four decimal places.

Provenance: `documentation` (meanings); `source_suggested` (the formulas). Kind: client
agreement. Confidence: medium. Milestone: 1. Experiment: walk a pig on the vanilla server for
100 ticks. Record the pose dump of its legs. Compute the leg angle from the two queries with
the expression of postcondition 2. The angles agree within the pose tolerance (section 0.2).

---

## 5. Movement and environment

### MOLQ-MOVE-001: Movement queries

| Name | Arguments | Result | Unit | Rate class | Context | Default | Pose input |
|------|-----------|--------|------|------------|---------|---------|------------|
| `ground_speed` | none | number | blocks per second | `entity_tick` | entity | `0.0` | `tick` |
| `vertical_speed` | none | number | blocks per second | `entity_tick` | entity | `0.0` | `tick` |
| `is_moving` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `is_on_ground` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `is_in_water` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |

- **Inputs:** the position of the subject at the start and at the end of the latest tick, and
  the flags of the subject that `client-entities.md` states.
- **Behavior:**
  1. `ground_speed` is `20 · √(dx² + dz²)`, where `dx` and `dz` are the changes of the
     position in the latest tick.
  2. `vertical_speed` is `20 · dy`. A positive value moves up.
  3. `is_moving` is `1.0` when `|dx|`, `|dy|` or `|dz|` is greater than `0.00001` blocks, and
     `0.0` otherwise.
  4. `is_on_ground` is `1.0` when the on-ground flag of the subject is set
     (`client-entities.md`).
  5. `is_in_water` is `1.0` when the in-water flag of the subject is set
     (`client-entities.md`).

Provenance: `documentation` (meanings); `source_suggested` (the use of the flags of the Java
client). Kind: semantic. Confidence: medium. Milestone: 2. Experiment: push a frog into water
on the vanilla server. The frog plays its swim animation in the tick in which the in-water
flag of `client-entities.md` becomes set.

---

## 6. Rotation

### MOLQ-ROT-001: Rotation queries

| Name | Arguments | Result | Unit | Rate class | Context | Default | Pose input |
|------|-----------|--------|------|------------|---------|---------|------------|
| `target_x_rotation` | none | number | degrees | `entity_frame` | entity | `0.0` | `lerp` |
| `target_y_rotation` | none | number | degrees | `entity_frame` | entity | `0.0` | `alerp` |
| `body_y_rotation` | none | number | degrees | `entity_frame` | entity | `0.0` | `alerp` |
| `head_x_rotation` | one number: the head index | number | degrees | `entity_frame` | entity | `0.0` | `lerp` |
| `head_y_rotation` | one number: the head index | number | degrees | `entity_frame` | entity | `0.0` | `alerp` |

- **Inputs:** the old and new body yaw, head yaw and head pitch of the subject, and `pt`.
- **Behavior:**
  1. Let `body = alerp(pt, body_old, body_new)` and `head = alerp(pt, head_old, head_new)`.
     A subject that rides another entity is out of the scope of milestones 1 and 2. A later
     revision of `client-entities.md` states the change of `body` for it.
  2. Let `pitch = head_pitch_old + pt · (head_pitch_new − head_pitch_old)`.
  3. `target_x_rotation` is `pitch`.
  4. `target_y_rotation` is `wrap(head − body)`.
  5. `body_y_rotation` is `wrap(body)`.
  6. `head_x_rotation(0)` is `pitch`. `head_y_rotation(0)` is `wrap(head)`.
  7. For a head index other than `0`, the subject has no such head, and the result is `0.0`.
  8. When the subject is upside down (`client-entity-render.md`), `target_x_rotation`,
     `target_y_rotation` and `head_x_rotation` are negated. `body_y_rotation` and
     `head_y_rotation` are not negated.
- **Postconditions:** a head bone with the rotation `[q.target_x_rotation,
  q.target_y_rotation, 0]` looks where the Java client looks.

Provenance: `documentation` (names); `source_suggested` (the meanings). The documentation
states that `target_x_rotation` aims at a target. The vanilla Bedrock pack uses both target
queries for the head of each mob, also with no target. Kind: client agreement. Confidence:
medium. Milestone: 1; a head index other than `0`: later. Experiment: stand
near a pig on the vanilla server and move the player, so that the pig looks at the player.
Record the pose dump of the head. The head angles agree with items 3 and 4 within the pose
tolerance (section 0.2).

---

## 7. State of a client entity

### MOLQ-STATE-001: State queries

| Name | Arguments | Result | Unit | Rate class | Context | Default | Pose input |
|------|-----------|--------|------|------------|---------|---------|------------|
| `is_baby` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `is_alive` | none | number | 1 | `entity_tick` | entity | `1.0` | `tick` |
| `hurt_time` | none | number | ticks | `entity_tick` | entity | `0.0` | `tick` |
| `death_ticks` | none | number | ticks | `entity_frame` | entity | `0.0` | `tick_pt` |
| `variant` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `variant_identifier` | none | string | none | `entity_tick` | entity | `''` | — |
| `model_scale` | none | number | 1 | `entity_tick` | entity | `1.0` | `tick` |
| `is_saddled` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `pose` | none | string | — | `entity_tick` | entity | `'standing'` | — |
| `is_on_fire` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `is_sneaking` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `is_sprinting` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `is_swimming` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `is_invisible` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `is_riding` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |
| `has_rider` | none | number | 1 | `entity_tick` | entity | `0.0` | `tick` |

- **Inputs:** the entity data and the client state of the subject (`client-entities.md`).
- **Behavior:**
  1. `is_baby` is `1.0` when the baby flag of the entity data of the subject is set.
  2. `is_alive` is `0.0` from the tick in which the health of the subject becomes `0` on the
     client. It is `1.0` before that tick.
  3. `hurt_time` is the hurt time of the subject (`client-entities.md`, CENTS-HURT-001): `10` in
     the tick of a hurt, then one less in each tick of the subject, down to `0`.
  4. `death_ticks` is the frame death time (`client-entities.md`, CENTS-DEATH-002): `0.0` while
     the death time `t` is `0`, otherwise `t + pt`.
  5. The variant queries read the variant entry of the subject:
     - `variant` is the numeric variant id, as a number: the value that the entity data
       gives, which is the position of the entry in the variant registry that the server
       synced, from `0`. For a type with no variant, it is `0.0`.
     - `variant_identifier` is the identifier of the same entry, for example
       `'minecraft:cold'`. For a type with no variant, it is `''`. When the numeric id is not
       in the variant registry that the server synced, it is `''`.
     - The order of a synced registry can change between servers. A pack for a mob with a
       registry variant selects the variant with `variant_identifier`, `variant_asset` or
       `variant_model`, not with `variant`.
  6. `model_scale` is the value of the scale attribute of the subject.
  7. `is_saddled` is `1.0` when the saddle equipment slot of the subject holds an item.
  8. `pose` is the name of the pose in the entity data, in lower case, for example
     `'standing'`, `'croaking'`, `'long_jumping'` or `'using_tongue'`.
  9. `is_on_fire`, `is_sneaking`, `is_sprinting`, `is_swimming` and `is_invisible` are the
     bits 0, 1, 3, 4 and 5 of the shared flags byte of the entity data.
  10. `is_riding` is `1.0` when the subject has a vehicle. `has_rider` is `1.0` when the
      subject has one or more passengers.
- **Postconditions:** each state query reads data that the server sends. No state query
  computes physics.

Provenance: `documentation` (names, except `pose` and `variant_identifier`; `variant` is a
number in the Bedrock packs, which do arithmetic on it); `design` for `variant_identifier`,
because the Java registries identify a variant by a resource location, and for the numeric
value of `variant`, which is the only number that the server sends; `source_suggested` (the source of each
value in the Java client). `pose` is an mcrs addition with no Bedrock counterpart
(MOLQ-DEV-001). Kind: semantic. Confidence: medium. Milestone: 1. Experiment: make a frog
croak on the vanilla server. Capture the set entity data packet. The pose field changes to
the croaking pose, and `q.pose == 'croaking'` is `1.0` in the same tick.

### MOLQ-STATE-002: Assets from registry data

| Name | Arguments | Result | Unit | Rate class | Context | Default | Pose input |
|------|-----------|--------|------|------------|---------|---------|------------|
| `variant_asset` | none | string | none | `entity_tick` | entity | `''` | — |
| `variant_model` | none | string | none | `entity_tick` | entity | `''` | — |
| `equipment_asset` | slot name: string | string | none | `entity_tick` | entity | `''` | — |
| `equipment_texture` | slot name: string, layer type: string | string | none | `entity_tick` | entity | `''` | — |

- **Inputs:** the variant entry of the subject in the synced variant registry of its entity
  type, the equipment of the subject, and the synced item components of each equipped item.
- **Behavior:**
  1. `variant_asset` is the asset identifier of the variant entry, for example
     `'minecraft:entity/pig/pig_cold'`.
  2. When `is_baby` is `1.0` and the variant entry has a baby asset identifier,
     `variant_asset` is the baby asset identifier.
  3. `variant_model` is the model type of the variant entry, in lower case, for example
     `'cold'`. When the entry has no model type, the value is the default model type of
     the registry, for example `'normal'`.
  4. `equipment_asset('<slot>')` is the asset identifier of the equippable component of the
     item in the slot `<slot>`, for example `'minecraft:saddle'`.
  5. The slot names are `mainhand`, `offhand`, `head`, `chest`, `legs`, `feet`, `body` and
     `saddle`.
  6. `equipment_asset` is `''` when the slot is empty, or when the item has no equippable
     component, or when the component has no asset identifier.
  7. `equipment_texture('<slot>', '<layer type>')` reads the equipment asset file of
     `equipment_asset('<slot>')`, `assets/<ns>/equipment/<path>.json`. It takes the first entry
     of the list `layers.<layer type>`. The value is the texture identifier `<tns>:entity/equipment/<layer type>/<tpath>`,
     where `<tns>:<tpath>` is the `texture` field of that entry.
  8. `equipment_texture` is `''` when `equipment_asset` is `''`, when the asset file does not
     exist, or when the file has no entry for the layer type.
  9. An unknown slot name is a load error `static_type` of the expression (`molang.md`,
     MOL-LOAD-004).
  10. For an entity type with no variant registry, `variant_asset` and `variant_model` are
     `''`. When the numeric variant id is not in the variant registry that the server synced,
     `variant_asset` and `variant_model` are `''`.
- **Postconditions:** the result is a resource identifier with its namespace, or `''`. A
  render controller turns it into a texture (`client-entity-definition.md`, CENT-RC-003).
  A data pack that adds a variant entry or an equippable item gets its texture with no
  change to the client entity files.

Provenance: `design` (names and the string form; Bedrock has no such queries);
`source_suggested` for the fields of the variant entry and of the equippable component.
Only the first layer entry is read. A layer entry with a dye colour or a second layer entry is
a later extension. Kind: semantic. Confidence: medium. Milestone: 1. Experiment: add a pig
variant with the asset identifier `example:entity/pig/blue` in a data pack on the vanilla server. Spawn a pig with
this variant. The variant entry in the synced registry carries the asset identifier, and
`q.variant_asset == 'example:entity/pig/blue'` is `1.0`.

---

## 8. Animation

### MOLQ-ANIM-001: Queries that an animation supplies

| Name | Arguments | Result | Unit | Rate class | Context | Default | Pose input |
|------|-----------|--------|------|------------|---------|---------|------------|
| `anim_time` | none | number | seconds | `animation_frame` | entity | `0.0` | `time` |
| `key_frame_lerp_time` | none | number | 1 | `animation_frame` | entity | `0.0` | `time` |
| `all_animations_finished` | none | number | 1 | `animation_frame` | entity | `0.0` | — |
| `any_animation_finished` | none | number | 1 | `animation_frame` | entity | `0.0` | — |

- **Behavior:** `entity-animation.md` states the value of each query. Outside an animation or
  an animation controller state, each query gives its default.
- **Postconditions:** these queries have a value only while a consumer runs an expression for
  one animation or one controller state.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: 1.

---

## 9. Functions of arguments

### MOLQ-PURE-001: Pure queries

| Name | Arguments | Result | Rate class | Context |
|------|-----------|--------|------------|---------|
| `in_range` | `v`, `lo`, `hi` | number | that of the arguments | any |
| `approx_eq` | `v`, then one or more numbers | number | that of the arguments | any |
| `all` | `v`, then two or more numbers | number | that of the arguments | any |
| `any` | `v`, then two or more numbers | number | that of the arguments | any |

- **Behavior:**
  1. `in_range(v, lo, hi)` is `1.0` when `lo ≤ v ≤ hi`, and `0.0` otherwise.
  2. `approx_eq(v, a1, …)` is `1.0` when `|v − ai| ≤ 0.0000001` for each `ai`, and `0.0`
     otherwise.
  3. `all(v, a1, …)` is `1.0` when `ai == v` for each `ai`, and `0.0` otherwise.
  4. `any(v, a1, …)` is `1.0` when `ai == v` for one or more `ai`, and `0.0` otherwise.
- **Postconditions:** these queries do not read the subject. A compiler can treat them as
  functions.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: 2; the queries are
pure and the compiler treats them as functions.

---

## 10. Camera

### MOLQ-VIEW-001: `distance_from_camera`

| Part | Value |
|------|-------|
| name | `distance_from_camera` |
| arguments | none |
| result | number |
| unit | blocks |
| rate class | `entity_frame` |
| context of use | entity |
| default | `0.0` |
| milestone | later |

- **Behavior:** the result is the distance from the camera position of the frame to the
  interpolated position of the subject (`client-entities.md`).
- **Postconditions:** the result is never negative.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: later.

---

## 11. Known queries that the client does not support

### MOLQ-KNOWN-001: Known Bedrock queries

- **Inputs:** a query name that the Bedrock documentation defines and that sections 2 to 10 do
  not declare.
- **Behavior:**
  1. The query is an unknown query (MOLQ-HOST-002). It reads as missing, and the compiler
     reports one load warning for the name.
  2. A member path on a query of the table that has a `struct` result parses. The value is
     missing, as item 1 states. The compiler does not report the load error `static_type`
     for the path.
  3. The compiler does not report the load error `arity` for a query of this section. A
     later revision that supports the query states its arguments.
  4. The table lists the queries that the vanilla Bedrock pack uses most, and each query with
     a `struct` result. Each other query name of `docs/molang-corpus.md`, section 4.1, is
     also known and not supported. Items 1 to 3 apply to it.

| Name | Arguments (documentation) | Result | Meaning (documentation) |
|------|---------------------------|--------|-------------------------|
| `get_equipped_item_name` | optional hand slot, optional item kind | string | the name of the item in the slot; `''` when the slot is empty; deprecated |
| `is_spectator` | none | number | `1.0` when the entity is a spectator |
| `get_root_locator_offset` | locator name, axis | number | one axis of the offset of a locator of the root model |
| `is_sitting` | none | number | `1.0` when the entity sits |
| `item_is_charged` | optional hand slot | number | `1.0` when the item in the slot is charged |
| `owner_identifier` | none | string | the identifier of the root entity; deprecated |
| `is_item_equipped` | optional hand slot | number | `1.0` when the slot holds an item |
| `property` | property name | number or string | the value of an entity property; `0.0` when the entity has no such property |
| `is_sleeping` | none | number | `1.0` when the entity sleeps |
| `bone_origin` | bone name | struct `x`, `y`, `z` | the pivot of the bone in the geometry file |
| `bone_rotation` | bone name | struct `x`, `y`, `z` | the rest rotation of the bone in the geometry file, in degrees |
| `bone_aabb` | bone name | struct `min.x`, `min.y`, `min.z`, `max.x`, `max.y`, `max.z` | the axis-aligned box of the bone |
| `bone_orientation_trs` | bone name | struct `t.x` … `s.z` (members `t`, `r`, `s`, each with `x`, `y`, `z`) | the translation, the rotation and the scale of the bone |
| `spellcolor` | none | struct `r`, `g`, `b`, `a` | the spell colour of the entity, each member from `0.0` to `1.0` |
| `surface_particle_color` | none | struct `r`, `g`, `b`, `a` | the particle colour of the block under the entity |
| `surface_particle_texture_coordinate` | none | struct `u`, `v` | the particle texture coordinate of the block under the entity |

- **Postconditions:**
  1. A vanilla Bedrock resource pack loads with no load error from its queries.
  2. The pig and the frog (milestones 1 and 2) use no query of this section.

Provenance: `documentation` (bedrock.dev, `/docs/stable/Molang`, the query list; the names
and the frequency: `docs/molang-corpus.md`, section 4); items 1 to 3: `design`. Kind:
semantic. Confidence: medium. Milestone: 1 for the behaviour of items 1 to 3; later for the
value of each query.

---

## 12. Intentional deviations

### MOLQ-DEV-001: The `pose` query

- **Rule:** MOLQ-STATE-001, item 8.
- **Reference:** Bedrock has no `query.pose`. Bedrock packs read state through other queries
  and through entity properties.
- **Visible to the client:** no. Only mcrs packs use the query.
- **Test:** a differential test against Bedrock does not use `query.pose`.

### MOLQ-DEV-002: The value of `variant` for a registry variant

- **Rule:** MOLQ-STATE-001, item 5.
- **Reference:** in Bedrock, `query.variant` is the integer of the variant component of the
  entity, which the behaviour pack fixes. For a mob with a synced variant registry, mcrs gives
  the position of the entry in the registry that the server sent. That number can change
  between servers. `variant_identifier` is an mcrs addition with no Bedrock counterpart.
- **Visible to the client:** no, when a pack selects a registry variant with
  `variant_identifier`, `variant_asset` or `variant_model`.
- **Test:** the pig and frog packs of mcrs do not read `query.variant`.

---

## 13. Open questions

- `head_y_rotation(n)`: the documentation does not state if the value is relative to the
  body. MOLQ-ROT-001 states the absolute head yaw.
- `hurt_time`: the documentation does not state the unit. MOLQ-STATE-001 states ticks.
- Queries of the Bedrock pack that the client does not support yet read as missing
  (MOLQ-KNOWN-001). A pack that tests `q.is_spectator` with no `??` shows the state of a
  player that is not a spectator.
