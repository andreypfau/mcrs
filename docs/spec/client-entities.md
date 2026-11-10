# Entities of the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

Source of these values: `assets/minecraft/version.json`. The rules state the behaviour of
`26.4-snapshot-7`. The capture artefacts of section 0.2 were first captured from
`26.4-snapshot-4`. Section 0.5 lists what that version does differently, and which rules wait
for a capture from `26.4-snapshot-7`.

This document states how the client keeps the state of an entity that the server sends. It
covers the lifecycle of the entity, its position and rotation, the interpolation of movement,
the body yaw, the walk accumulator, the hurt time and the death time. It also states the values
that the renderer (`client-entity-render.md`) and the Molang queries (`molang-queries.md`) read
in each frame. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Tick**: one step of the client simulation. The client runs 20 ticks each second when the
  tick rate of the server is 20.
- **Frame**: one image that the client draws. Zero, one or more ticks occur between two frames.
- **Partial tick** (`pt`): the part of the current tick interval that has passed when the client
  draws a frame. `pt` is in the range `[0, 1)`.
- **Client entity**: the state that the client keeps for one entity id that the server added.
- **Living entity**: a client entity of a type that has health. Each mob is a living entity.
- **Mob**: a living entity with an AI on the server. The pig, the cow, the sheep, the chicken, the
  zombified piglin and the frog are mobs. A player is not a mob.
- **Yaw**: a horizontal angle in degrees. Yaw `0` faces +Z. Yaw `90` faces −X. The facing vector
  for yaw `y` and pitch `p` is `(−sin y · cos p, −sin p, cos y · cos p)`.
- **Pitch**: a vertical angle in degrees. A positive pitch looks down.
- **Byte angle**: an angle on the wire as one unsigned byte `b`. The angle is `b · 360 / 256`
  degrees.
- **Wrap**: `wrap(a)` gives the angle equal to `a` modulo 360 in the range `[−180, 180)`.
- **Angle interpolation**: `alerp(t, a, b) = a + t · wrap(b − a)`. It follows the shorter arc.
- **Linear interpolation**: `lerp(t, a, b) = a + t · (b − a)`.
- **Current value** and **previous value**: the client keeps two copies of each interpolated
  quantity. The previous value is the copy at the start of the last tick. The renderer
  interpolates from the previous value to the current value with `pt`.
- **Update interval**: the number of ticks between two position updates that the server sends
  for an entity type. The value of each type is in
  `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json`. It is 3 for the pig and
  the frog, and 2 for a player.
- **Entity clock** (`C`): `C = age + pt`, in ticks, with the age of CENTS-TIME-002. The age counts
  only the ticks that the entity ran, so `C` stops when the entity does not tick.
- **Entity box**: the box of an entity with the width `w · g · s` on X and Z and the height
  `h · g · s`. The box is centred on X and Z at the position, and its bottom is at the position.
  `w` and `h` are the width and the height of the entity type (`entity_types.json`). `g` is the
  baby factor that the mob document gives, or 1 for an adult. `s` is the value of the attribute
  `minecraft:scale`, or 1 when the client has no value.
- **Eye height**: `e · g · s`, with `e` the eye height of the entity type (`entity_types.json`), for
  each pose. An eye height that depends on the pose is a later extension.
- **Simulation distance**: the value `simulation_distance` of the login packet, in chunks.
- **Loaded chunk**: a chunk column for which the client received the chunk data, and which the
  client did not unload after that.
- **Ticking area**: the set of loaded chunks. A client entity is in the ticking area when the
  chunk that contains its position is in the set and the entity is not a passenger. A client
  entity ticks when it is in the ticking area and the tick rate of the server does not freeze
  it. A frozen entity stays in the ticking area.
- **Path**: the list of segments that an entity still has to travel in an interpolation. A
  segment has an end point (a position, a yaw and a pitch) and a weight in ticks.
- **Packet names**: this document names a packet by its identifier in
  `assets/mcrs/reports/packets.json`, for example `minecraft:add_entity`. A field name is a
  description of the field, not a name from the code of the game.

### 0.2 Provenance and confidence

Each rule has a line `Milestone`: `1` is the pig, `2` is the frog, and `later` is a rule that
milestones 1 and 2 do not use.

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". A rule that a capture artefact (below) confirms has the
label `source_suggested_black_box_confirmed` and cites the artefact. Each other rule has the
label `source_suggested` and an experiment.

A parity test of this document compares values with a tolerance of `1e-4` degree for angles,
`1e-6` block for positions and `1e-5` for the walk values. An exact match is not required. A
test that compares a bone pose uses the pose tolerance of `entity-animation.md`, EANIM-GPU-003.

The capture harness of `client-frame-capture.md` (CFC) writes two artefacts for each scene of
`tools/scenes/`. They are in `crates/mcrs_minecraft_client/tests/fixtures/vanilla/frames/`:

- `<scene>.packets.json`: each entity packet that the client applied during the scene, in the
  order of application. The key `packet` is the packet identifier. The key `updates_before` is
  the number of entity updates of the client since the start of the scene. The packet applies
  before the next entity update. The other keys are the decoded fields of the packet.
- `<scene>.entity_ticks.json`: one row for each entity update in which at least one entity
  ticked. The key `update` is the number of the update. The key `entities` holds, for each
  entity that ticked, its state after the update.

| Key of a packet | Content |
|-----------------|---------|
| `entity` | the network id of the entity |
| `delta` | the wire delta: `form` (`single` or `stepped`), `units` (`[dx, dy, dz]`), or `steps` (each with `units` and `ticks`) and `start_immediately` |
| `path` | the decoded path: `form`, and `position` or `steps` (each with `position` and `ticks`) and `start_immediately` |
| `yaw`, `pitch`, `head_yaw` | angles in degrees, after the decode of the byte angle |
| `on_ground` | the on-ground flag |
| `values` | entity data: `index` and `value`, or `kind` `other` for a value that is not a number, a Boolean, a string or a registry entry |
| `slots` | equipment: `slot`, `item` and `count` |
| `event`, `damage_type`, `cause`, `direct_cause` | entity event and damage fields |
| `tick_rate`, `frozen`, `steps` | the tick rate packets (CENTS-TIME-003) |

| Key of an entity row | Content |
|----------------------|---------|
| `age` | the age (CENTS-TIME-002) |
| `position`, `previous_position` | the current and the previous position |
| `pitch`, `body_yaw`, `previous_body_yaw`, `head_yaw`, `previous_head_yaw` | angles in degrees |
| `walk_position`, `walk_speed` | the walk accumulator (CENTS-WALK-001) |
| `hurt_time`, `death_time` | CENTS-HURT-001 and CENTS-DEATH-002 |
| `on_ground` | the on-ground flag |
| `path_before_update`, `path_after_update` | `true` when the path is not empty before and after the update |

The key `start_immediately` is present only in a capture from `26.4-snapshot-7`. Three runs of
`pig_walk`, `pig_hurt_death` and `pig_idle` from `26.4-snapshot-4` gave the same packets and the
same entity rows. Only the value `updates_before` can change between two runs, and only for a packet
that arrives while no entity ticks. A replay test therefore applies each packet before the
first row with an `update` larger than its `updates_before`.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CENTS-TIME` | tick, partial tick and age |
| `CENTS-LIFE` | add and remove |
| `CENTS-MOVE` | position and rotation packets |
| `CENTS-INTERP` | interpolation of position and rotation |
| `CENTS-HEAD` | head yaw |
| `CENTS-TICK` | the order of operations in one tick |
| `CENTS-BODY` | body yaw |
| `CENTS-WALK` | the walk accumulator |
| `CENTS-HURT` | hurt time |
| `CENTS-DEATH` | death time and removal |
| `CENTS-DATA` | entity data |
| `CENTS-EQUIP` | equipment |
| `CENTS-FLAG` | on ground and in water |
| `CENTS-FRAME` | the values of one frame |
| `CENTS-DEV` | intentional deviations |

### 0.4 Scope

In scope: living entities that are not the local player, and that do not ride and are not
ridden. The rules for the local player are in other documents.

Out of scope: riding and passengers, sleeping, the drawing of items in hands and of armour,
particles and sounds of entity events, entities that are not living entities, and the movement
of an entity by the client physics. The equipment that the client keeps (CENTS-EQUIP-001) is in
scope.

### 0.5 Differences in 26.4-snapshot-4

The first capture artefacts come from `26.4-snapshot-4`. That version is different in three
behaviours of this document:

1. A stepped delta and a stepped path have no Boolean `start immediately` (CENTS-MOVE-005).
2. Thus no start empties the path before the reference point (CENTS-INTERP-001, item 4).
3. The movement along the path uses a speed that catches up with the remaining weight, not an
   even spread of the remaining weight over `N` ticks (CENTS-INTERP-002).

The replay of `pig_walk.packets.json` with the rules of `26.4-snapshot-4` gave each position of
the pig within `3e-8` block. That result confirms the other rules of section 4, which both
versions share. The rules CENTS-MOVE-005, CENTS-INTERP-001 and CENTS-INTERP-002 have the label
`source_suggested` until a capture from `26.4-snapshot-7` replaces the artefacts.

---

## 1. Time

### CENTS-TIME-001: Tick and partial tick

- **Inputs:** the tick rate of the server; the time of the frame.
- **Behavior:**
  1. The client applies all packets that arrived before a tick, in arrival order, before that
     tick.
  2. The client then ticks each client entity one time (CENTS-TICK-001).
  3. When the client draws a frame, `pt` is the part of the current tick interval that has
     passed.
  4. When the tick rate of the server freezes the world (CENTS-TIME-003), the frame uses
     `pt` = 1 for each entity that is not a player. Thus the frame shows the current values of
     that entity. A step of the tick rate does not change this rule.
- **Postconditions:** each value that a frame reads is a function of the previous value, the
  current value and `pt` (CENTS-FRAME-001). Item 4 agrees with `client-frame-capture.md`,
  CFC-RUN-003.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: spawn a pig with
the tag `NoAI`. Teleport it 1 block east with `/tp`. Record frames at 240 Hz. The pig moves
over 3 ticks, and its frame position changes in each frame.
Milestone: 1.

### CENTS-TIME-002: Age

- **Inputs:** a client entity.
- **Behavior:**
  1. The age is 0 when the client adds the entity.
  2. The age increases by 1 in each tick of the entity (CENTS-TICK-001, step 3).
- **Postconditions:** the frame age is the entity clock `C`. The query `life_time` is `C / 20`,
  in seconds. The query `delta_time` is the change of `C` since the previous frame of the entity,
  divided by 20.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: spawn a bat.
The wing animation of a new bat starts from the same phase each time.
Milestone: 1.

### CENTS-TIME-003: Tick rate packets

- **Inputs:** the packets `minecraft:ticking_state` and `minecraft:ticking_step`.
- **Behavior:**
  1. `minecraft:ticking_state` is an `f32` tick rate, then a Boolean `frozen`.
  2. `minecraft:ticking_step` is a `VarInt` count of steps.
  3. While the world is frozen, an entity that is not a player does not tick.
  4. A count of steps lets the client run that number of ticks for all entities while the world
     is frozen. A new count replaces the count that remains.
  5. The tick speed `s` of CENTS-INTERP-002 is the tick rate divided by 20 when the tick rate
     is more than 20. Otherwise, `s` is 1.
- **Postconditions:** in `pig_walk`, each step of 1 tick gives one row of
  `pig_walk.entity_ticks.json`.

Provenance: `source_suggested_black_box_confirmed`, artefacts `pig_walk.packets.json` and
`pig_walk.entity_ticks.json` (section 0.2). Kind: wire for items 1 and 2, semantic for the rest.
Confidence: high.
Milestone: 1.

---

## 2. Lifecycle

### CENTS-LIFE-001: Add an entity

- **Inputs:** the packet `minecraft:add_entity` with these fields: the entity id, the UUID, the
  entity type, the position, the velocity, the pitch, the yaw, the head yaw and the data value.
- **Behavior:**
  1. When the numeric id of the entity type is not in the entity type registry, the client
     ignores the packet.
  2. The client makes a client entity with the entity id and the UUID.
  3. The position is the position of the packet. The X and Z values are clamped to
     `[−3·10^7, 3·10^7]`.
  4. The yaw is the yaw of the packet. The pitch is the pitch of the packet, clamped to
     `[−90, 90]`.
  5. For a living entity, the head yaw and the body yaw are both the head yaw of the packet.
  6. Each previous value is equal to its current value.
  7. The delta base (CENTS-MOVE-001) is the position of the packet.
  8. The entity data has the default value of each index (CENTS-DATA-001).
  9. Each equipment slot is empty (CENTS-EQUIP-001).
- **Postconditions:** the first frame shows the entity at its position with no interpolation.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: summon a pig
with `Rotation:[90f,0f]` while you look at a different direction. The pig appears with its body
and its head to the west.
Milestone: 1.

### CENTS-LIFE-002: Remove entities

- **Inputs:** the packet `minecraft:remove_entities` with a list of entity ids.
- **Behavior:** for each id of the list, the client removes the client entity with that id and
  all its state, its equipment included. The client ignores an id that has no client entity.
- **Postconditions:** the next frame does not show the removed entities.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: kill a pig.
The pig disappears in the frame after the remove packet.
Milestone: 1.

---

## 3. Position and rotation packets

### CENTS-MOVE-001: Delta base and delta decoding

- **Inputs:** the delta base `B` of the entity; a delta `(dx, dy, dz)` of three `i16` values.
- **Behavior:**
  1. When `dx`, `dy` and `dz` are all 0, the decoded position is `B`.
  2. Otherwise, for each axis, a value of 0 keeps the axis of `B`.
  3. A value `d` that is not 0 gives `(round(B · 4096) + d) / 4096` for that axis. The
     function `round(x)` is `floor(x + 0.5)`. Thus a value halfway between two integers goes to
     the larger integer: `round(−2.5)` is `−2`.
- **Postconditions:** a stepped delta decodes each step from the decoded position of the
  step before it. The first step decodes from `B`.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: capture a move packet
of a pig. Decode it from the last position sync. The result matches the next position sync.
Milestone: 1.

### CENTS-MOVE-005: Wire form of a delta and of a path

- **Inputs:** the bytes of a move packet or of a position sync.
- **Behavior:**
  1. A move packet with a delta has a `VarInt` properties field. Bit 0 is the on-ground flag.
     The bits from bit 1 are the step count `n`.
  2. When `n` is 0, the delta is linear: three `i16` values `dx`, `dy`, `dz` follow.
  3. When `n` is more than 0, the delta is stepped. A Boolean `start immediately` follows, then
     `n` steps. Each step is a `VarInt` tick count and three `i16` values.
  4. A path of a position sync starts with a `VarInt` kind: 0 is linear, 1 is stepped.
  5. A linear path is three `f64` values: the end position.
  6. A stepped path is a `VarInt` count, then that number of steps, then a Boolean
     `start immediately`. Each step is three `f64` values of a position and a `VarInt` tick
     count.
  7. The end position of a stepped path is the position of its last step. A stepped path of a
     position sync with a count of 0 has no end position: the packet is malformed, and the
     client handles it as each malformed packet (it disconnects).
  8. A server sends a stepped form for an entity that records its positions between two
     updates. Each step is one recorded position. The tick count of a step is the number of
     server ticks since the step before it, or since the previous update for the first step. A
     mob that moves smoothly gives one step with the tick count `N` in each update.
  9. A server sets `start immediately` to `true` when, since the previous update of the entity,
     the server gave the entity a sudden change of movement. Examples are knockback, a jump and
     a push. Otherwise the server sets it to `false`.
- **Postconditions:** a decoder reads the Boolean of a stepped delta before the steps, and the
  Boolean of a stepped path after the steps.

Provenance: `source_suggested`. Kind: wire. Confidence: high. Experiment: capture the packets of
`pig_walk` from `26.4-snapshot-7` (section 0.2). Each move of the pig is a stepped delta with
one step of 3 ticks and `start_immediately` `false`. Hit the pig: the next stepped delta has
`start_immediately` `true`.
Milestone: 1.

### CENTS-MOVE-002: Relative move packets

- **Inputs:** `minecraft:move_entity_pos`, `minecraft:move_entity_pos_rot` or
  `minecraft:move_entity_rot` for an entity id with a client entity.
- **Behavior:**
  1. A packet with a delta decodes a path from the delta base (CENTS-MOVE-001). A linear
     delta gives one position. A stepped delta gives a list of steps, each with a position
     and a tick count.
  2. The delta base becomes the end position of the path.
  3. A packet with a rotation gives a target yaw and a target pitch, each as a byte angle.
  4. The client starts an interpolation (CENTS-INTERP-001) with the path and the rotation.
     A packet without a delta gives no path. A packet without a rotation gives no rotation.
  5. The on-ground flag becomes the on-ground flag of the packet.
- **Postconditions:** the entity reaches the end of the path through the interpolation.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: lead a pig with
a carrot on a stick. Capture the packets. Each move packet starts a new interpolation.
Milestone: 1.

### CENTS-MOVE-003: Position sync

- **Inputs:** `minecraft:entity_position_sync` with a path, a rotation and an on-ground flag.
- **Behavior:**
  1. The delta base becomes the end position of the path.
  2. Let `D` be the distance from the current position to the end position.
  3. When the entity is not in the ticking area (section 0.1), or `D` > 64 blocks, the client
     snaps (CENTS-INTERP-003) to the end position and the rotation of the packet. A frozen tick
     rate does not cause a snap: a frozen entity in the ticking area starts an interpolation,
     and the interpolation moves when the entity ticks again.
  4. Otherwise, the client starts an interpolation with the path and the rotation.
  5. The on-ground flag becomes the flag of the packet.
- **Postconditions:** a position sync corrects the drift of the delta base.

Provenance: `source_suggested`; item 3 `source_suggested_black_box_confirmed` (in the
`pig_walk` capture of `26.4-snapshot-7`, a position sync arrives while the world is frozen, and
the pig keeps its position and its path). Kind: semantic. Confidence: high. Experiment: teleport
a pig by 100 blocks with `/tp`. The pig appears at the new place in the next frame, with no
movement between the two places.
Milestone: 1.

### CENTS-MOVE-004: Teleport

- **Inputs:** the packet `minecraft:teleport_entity`: a position, a velocity, a yaw, a pitch, a
  set of relative axes and an on-ground flag.
- **Behavior:**
  1. The client computes the absolute values. A relative axis adds its value to the current value.
  2. When the distance from the current position to the new position is more than 64 blocks,
     the client snaps to the new values.
  3. Otherwise, the client starts an interpolation with the new position and rotation.
  4. The on-ground flag becomes the flag of the packet.
- **Postconditions:** the teleport packet does not change the delta base.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: teleport a pig
by 2 blocks. The pig moves over 3 ticks.
Milestone: 1.

---

## 4. Interpolation

The client does not move a non-local entity with its velocity. It moves the entity only along
the interpolation.

### CENTS-INTERP-001: The path after a start

- **Inputs:** a start of an interpolation (CENTS-MOVE-002 to CENTS-MOVE-004): an optional path
  of the packet with its Boolean `start immediately`, an optional target yaw `y` and target
  pitch `p`; the interpolation length `N` of the entity type; the path of the entity, which can
  be empty; the end point `Z` of the last start. Before the first start, `Z` is the position
  `(0, 0, 0)` with yaw 0 and pitch 0. The interpolation is active while the remaining time `T`
  is more than 0.
- **Behavior:**
  1. The reference point is `Z` when the interpolation is active. Otherwise, it is the current
     position, yaw and pitch of the entity. A missing packet path is a linear path to the
     position of the reference point. A missing rotation is the yaw and the pitch of the
     reference point.
  2. When `N` is 0, the client snaps (CENTS-INTERP-003) to the end of the packet path, `y` and
     `p`, and the path becomes empty with `T` = 0 and `W` = 0. `Z` keeps its value.
  3. When the interpolation is active, and `Z` is equal to the end position of the packet
     path, `y` and `p`, the start changes nothing.
  4. When the packet path is stepped and its `start immediately` is `true`, the path becomes
     empty, with `T` = 0 and the total weight `W` = 0. `Z` keeps its value.
  5. When the interpolation is not active, the point `A` becomes the current position, yaw and
     pitch of the entity.
  6. The start adds segments to the end of the path, and adds the weight of each segment to
     `W`:
     - when the end position of the packet path is equal to the position of `Z`, one segment
       to that position, `y` and `p`, with the weight `N`;
     - otherwise, for a linear packet path, one segment to its end position, `y` and `p`, with
       the weight `N`;
     - otherwise, for a stepped packet path, one segment for each step, with the weight of its
       tick count. When `y` and `p` are equal to the yaw and the pitch of `Z`, each segment has
       `y` and `p`. Otherwise, the segment of step `i` has the yaw
       `alerp(aᵢ, yaw of Z, y)` and the pitch `lerp(aᵢ, pitch of Z, p)`, with `aᵢ = Sᵢ / S`.
       `Sᵢ` is the sum of the tick counts of the steps up to and including step `i`. `S` is the
       sum of all tick counts. When `S` is 0, `aᵢ` is 1.
  7. `Z` becomes the end position of the packet path, `y` and `p`. `T` becomes `N`.
- **Postconditions:** the end point of the path is `Z`. Each start gives the path that remains
  `N` ticks, whatever the weight that remains. Note that `Z` keeps its value when the path
  becomes empty. Item 6 compares with that old value.

The interpolation length `N` is the update interval of the entity type for a pig and a frog.
`T` and `W` are `f32` values. The value `aᵢ = 1` for `S` = 0 is a `design` choice: the
reference divides 0 by 0 there. No vanilla server sends such a path.

Provenance: `source_suggested` (section 0.5). Kind: semantic. Confidence: high. Experiment:
replay `pig_walk.packets.json` of a capture from `26.4-snapshot-7` with this rule and
CENTS-INTERP-002. Each position of `pig_walk.entity_ticks.json` matches within `1e-6` block. The
first packet of the pig is a move with a zero delta. It adds a segment of 3 ticks that does not
move the pig.
Milestone: 1.

### CENTS-INTERP-002: Movement along the path

- **Inputs:** the point `A`, the path, the remaining time `T` and the total weight `W` of the
  interpolation; the tick speed `s` (CENTS-TIME-003). `T`, `W`, `d` and `a` are `f32` values.
- **Behavior:** in each tick of the entity:
  1. When the interpolation is not active, the path becomes empty and `W` becomes 0. The tick
     does not do the steps that follow.
  2. The client applies CENTS-INTERP-004.
  3. Let `d = s · W / T`. `W` becomes `W − d`. `T` becomes `T − s`.
  4. While a segment remains, let `L` be the weight of the first segment and `E` its end point:
     - when `d` is less than `L`: let `a = d / L`. When `a` is more than 0, the position of `A`
       becomes `lerp(a, position of A, position of E)`, the yaw of `A` becomes
       `alerp(a, yaw of A, yaw of E)`, and the pitch of `A` becomes
       `lerp(a, pitch of A, pitch of E)`. The weight of the segment becomes `L − d`. Stop.
     - otherwise: `d` becomes `d − L`, `A` becomes `E`, and the client removes the segment.
  5. When no segment remains after step 4, the path becomes empty, with `T` = 0 and `W` = 0.
  6. The position, the yaw and the pitch of the entity become those of `A`.
- **Postconditions:** with no other start, and `s` = 1, a linear move from the position `P₀` to
  `P₁` gives the position `lerp(k / N, P₀, P₁)` at the end of tick `k`, for `k` = 1 to `N`. A
  start during an active interpolation adds its segments after the segments that remain. The
  next `N` ticks then spread all the remaining weight evenly.

Provenance: `source_suggested` (section 0.5). Kind: semantic. Confidence: high. Experiment: as
for CENTS-INTERP-001.
Milestone: 1.

### CENTS-INTERP-003: Snap

- **Inputs:** a position, a yaw and a pitch.
- **Behavior:**
  1. The current position, yaw and pitch become the new values.
  2. The previous position, yaw and pitch become the same values.
  3. A snap of CENTS-MOVE-003 or CENTS-MOVE-004 does not change the path. When the entity
     ticks with an active path, CENTS-INTERP-004 moves the rest of the path by the change.
     Only the snap of CENTS-INTERP-001, item 2, and a start with `start immediately` (item 4)
     empty the path.
- **Postconditions:** the next frame shows the entity at the new position with no
  interpolation between frames.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as for
CENTS-MOVE-003.
Milestone: 1.

### CENTS-INTERP-004: Movement from other causes

- **Inputs:** an active interpolation; a change `Δ` of the position of the entity, and a change
  of its yaw and pitch, from a cause other than the interpolation. The client measures each
  change from the position, the yaw and the pitch that the entity had at the end of the last
  start or step of the interpolation.
- **Behavior:**
  1. When `|Δ|²` > `1e-5`, and the bounding box at `Z` moved by `Δ` has no collision, `Z`, all
     end points of the path that remain, and the point `A` move by `Δ`.
  2. The yaw and the pitch of `Z`, of all end points of the path that remain, and of `A` change
     by the change of the yaw and the pitch. This item has no threshold and no collision test.
- **Postconditions:** a snap or a teleport during an interpolation moves the rest of the path
  with the entity.

Provenance: `source_suggested`. Kind: semantic. Confidence: low. Experiment: during an
interpolation of a pig, send a teleport of the same pig by 1 block up. The pig ends the
interpolation 1 block higher than the target of the earlier packet.
Milestone: 1 (CENTS-INTERP-003, item 3, needs it). CENTS-DEV-001 applies to the collision test.

---

## 5. Head yaw

### CENTS-HEAD-001: Head yaw packet

- **Inputs:** `minecraft:rotate_head` with a head yaw `h` as a byte angle, for a living entity;
  the head yaw `H₀` of the entity when the packet arrives.
- **Behavior:**
  1. At the end of tick `k` after the packet, for `k` = 1, 2 and 3, the head yaw is
     `alerp(k / 3, H₀, h)`, when no other cause changes the head yaw.
  2. A new packet before the end restarts the rule from the head yaw at that time.
  3. The head yaw changes in step 7 of CENTS-TICK-001.
- **Postconditions:** the head yaw reaches `h` after 3 ticks.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: stand near a pig
that looks at you. Walk around the pig. Record the head yaw. Each change takes 3 ticks.
Milestone: 1.

---

## 6. The tick of an entity

### CENTS-TICK-001: Order of operations in one tick

- **Inputs:** a living entity that ticks (section 0.1, "Ticking area").
- **Behavior:** in each tick, the client does these steps in this order:
  1. The previous position, yaw and pitch become the current values.
  2. The client does one step of the interpolation (CENTS-INTERP-002). The step occurs in each
     tick, also when the interpolation is not active: then item 1 of CENTS-INTERP-002 empties
     the path. A path can keep a segment after `T` reaches 0, because `T`, `W` and `d` are
     `f32` values.
  3. The age increases by 1.
  4. When the hurt time is more than 0, it decreases by 1 (CENTS-HURT-001).
  5. When the entity is dying, the death time changes (CENTS-DEATH-002).
  6. The previous head yaw and the previous body yaw become the current values.
  7. The head yaw does one step (CENTS-HEAD-001).
  8. The walk accumulator updates (CENTS-WALK-001).
  9. The body yaw updates (CENTS-BODY-001 for a mob, CENTS-BODY-002 for another living entity).
  10. Each previous angle changes by a multiple of 360 so that the current value minus the
      previous value is in `[−180, 180)`.
- **Postconditions:** the movement of step 2 is the input of steps 8 and 9.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: compare the walk
speed of a pig in a frame capture with a model that uses this order. A different order changes
the leg phase by one tick.
Milestone: 1.

---

## 7. Body yaw

### CENTS-BODY-001: Body yaw of a mob

- **Inputs:** a mob; its yaw, head yaw and body yaw after step 7 of CENTS-TICK-001; the movement
  `(dx, dz)` of step 2 of CENTS-TICK-001; the head limit `L`. `L` is 75 degrees. A mob
  document can give another `L`.
- **Behavior:** "`a` stays within `m` of `t`" means: when `wrap(t − a)` is in `[−m, m]`, `a`
  does not change; otherwise `a` becomes the angle at the distance `m` from `t`, on the side of
  `a`. A turn tick is a tick in which item 1 or item 2 applies. The turn head yaw is the head
  yaw after the last turn tick. It is 0 when the client adds the mob.
  1. In a tick with `dx² + dz²` > `2.5e-7`, the body yaw becomes the yaw. Then the head yaw stays
     within `L` of the body yaw.
  2. In a tick with no such movement, when `|head yaw − turn head yaw|` > 15, with no wrap, the
     body yaw stays within `L` of the head yaw.
  3. In tick `n` after the last turn tick (`n` = 1, 2, …), with no movement and no item 2:
     - for `n` ≤ 10, the body yaw does not change;
     - for `n` > 10, the body yaw stays within `L · (1 − f)` of the head yaw, with
       `f = min(1, (n − 10) / 10)`.
  4. Items 2 and 3 do not apply when the mob carries a mob as a passenger.
- **Postconditions:** a mob that stands still with a stable head turns its body to its head
  between tick 11 and tick 20 after the last turn tick. From tick 20, the body yaw is equal
  to the head yaw.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: give a pig the
tag `NoAI`. Turn its head with a head yaw packet from a test server, then stop. The body
follows the head between tick 11 and tick 20. No capture scene turns a head yet; a new scene
can add the case to the artefacts of section 0.2.
Milestone: 1.

### CENTS-BODY-002: Body yaw of another living entity

- **Inputs:** a living entity that is not a mob; its yaw, body yaw and movement `(dx, dz)`.
- **Behavior:**
  1. Let the target be the body yaw.
  2. When `dx² + dz²` > `0.0025`, let `w = atan2(dz, dx)` in degrees `− 90`. When
     `|wrap(yaw) − w|` is more than 95 and less than 265, the target is `w − 180`. Otherwise, it
     is `w`.
  3. When the entity swings an arm, the target is the yaw.
  4. The body yaw increases by `0.3 · wrap(target − body yaw)`.
  5. Let `h = wrap(yaw − body yaw)`. When `|h|` > 50, the body yaw increases by
     `h − sign(h) · 50`.
- **Postconditions:** the body follows the direction of movement, and stays in 50 degrees of
  the yaw.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: watch another
player that walks backward. The body faces the yaw, not the direction of movement.
Milestone: later.

---

## 8. Walk accumulator

### CENTS-WALK-001: Update of the walk accumulator

- **Inputs:** a living entity; the movement `(dx, dy, dz)` of step 2 of CENTS-TICK-001; the
  speed `v`, the previous speed `v₀`, the position `x` and the scale `k` of the accumulator.
  `v`, `v₀` and `x` are 0 when the client adds the entity.
- **Behavior:**
  1. When the entity is a passenger, or it is not alive, `v₀`, `v` and `x` become 0. The update
     stops.
  2. Let `d = sqrt(dx² + dz²)`. An entity type that moves in all directions in the air uses
     `sqrt(dx² + dy² + dz²)`. The mob document states the type.
  3. Let the target be `min(4 · d, 1)`. A mob document can give another target.
  4. `v₀` becomes `v`. Then `v` becomes `v + (target − v) · 0.4`.
  5. `x` becomes `x + v`.
  6. `k` becomes 3 when the entity is a baby. Otherwise, `k` becomes 1.
- **Postconditions:** `v` approaches the target with the factor 0.4 in each tick.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: lead a pig in a
straight line at a constant speed. The leg swing reaches a constant amplitude after about 10
ticks.
Milestone: 1.

### CENTS-WALK-002: Walk values in a frame

- **Inputs:** `v`, `v₀`, `x` and `k` of CENTS-WALK-001; `pt`.
- **Behavior:**
  1. When the entity is a passenger, or it is not alive, both values are 0.
  2. Otherwise, the walk position is `(x − v · (1 − pt)) · k`.
  3. The walk speed is `min(lerp(pt, v₀, v), 1)`.
- **Postconditions:** the query `modified_distance_moved` is the walk position. The query
  `modified_move_speed` is the walk speed. The walk speed is at most 1, also after a hurt
  (CENTS-HURT-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: hit a pig that
stands still. Its legs swing for a few ticks.
Milestone: 1.

---

## 9. Hurt time

### CENTS-HURT-001: Hurt time

- **Inputs:** a living entity; its hurt time. The hurt time is 0 when the client adds it.
- **Behavior:**
  1. The packet `minecraft:damage_event` sets the hurt time to 10, and sets the walk speed `v`
     to 1.5. It does not change `v₀`.
  2. The packet `minecraft:hurt_animation` sets the hurt time to 10. It does not change the walk
     accumulator.
  3. In each tick, a hurt time more than 0 decreases by 1 (CENTS-TICK-001, step 4).
- **Postconditions:** the hurt time is `max(0, 10 − (age − A))`, with `A` the age at the last
  packet of items 1 and 2. This is a difference of entity clocks. The query `hurt_time` is the hurt time. The red overlay
  (`client-entity-render.md`, CEREN-SURF-004) shows while the hurt time is more than 0.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: hit a pig. The
red tint lasts 10 ticks.
Milestone: 1.

### CENTS-HURT-002: Fields of the damage and hurt packets

- **Inputs:** the bytes of the packets. The client does not decode these packets yet.
- **Behavior:**
  1. `minecraft:damage_event` has these fields in this order: the entity id (`VarInt`); the
     damage type (`VarInt`, a numeric id of the registry `damage_type`); the cause entity id
     (`VarInt`, the id plus 1, where 0 means none); the direct entity id (`VarInt`, the same
     encoding); an optional source position (a Boolean, then three `f64` values when it is
     true).
  2. `minecraft:hurt_animation` has these fields: the entity id (`VarInt`); the yaw of the hurt
     direction (`f32`, in degrees).
  3. The client uses only the entity id for the rules of this document.
- **Postconditions:** the server sends the damage event to each client that tracks the entity.
  The server sends the hurt animation only to a player about itself.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: capture the packets
when a player hits a pig. Decode them with these fields. The packet length matches.
Milestone: 1.

---

## 10. Death

### CENTS-DEATH-001: Dying

- **Inputs:** a living entity; its health (CENTS-DATA-002, index 9).
- **Behavior:**
  1. The entity is dying when its health is 0 or less.
  2. The entity event `3` sets the health of the client entity to 0.
  3. The entity is alive when it is not removed and its health is more than 0.
- **Postconditions:** the query `is_alive` is 1 when the entity is alive. Otherwise, it is 0.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: kill a pig. The
pig turns red and falls on its side.
Milestone: 1.

### CENTS-DEATH-002: Death time

- **Inputs:** a dying entity; its death time, which is 0 when the client adds it.
- **Behavior:** in each tick, when the entity is in the simulation distance, the death time
  increases by 1. The simulation distance uses the chessboard distance in chunks from the chunk
  of the local player.
- **Postconditions:** the frame death time is `death time + pt` when the death time is more than
  0. Otherwise, it is 0. The query `death_ticks` is the frame death time. While the entity stays
  in the simulation distance, the frame death time is `C − A`, with `A` the age when the entity
  became dying. The frame death time is 0 while `C − A` is less than 1.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: kill a pig.
Record the tilt of the body in each frame. The tilt follows CEREN-PLACE-003 from the first tick.
Milestone: 1.

### CENTS-DEATH-003: End of the death

- **Inputs:** the entity event `60`; the remove packet.
- **Behavior:**
  1. The server removes a dying entity when its death time reaches 20. Before that, it sends the
     entity event `60`.
  2. The entity event `60` makes 20 poof particles at the entity. The particles are out of
     scope.
  3. The remove packet removes the entity (CENTS-LIFE-002).
- **Postconditions:** the client does not remove a dying entity without the remove packet.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: kill a pig with
the server at tick rate 20. The pig disappears 20 ticks after its health becomes 0.
Milestone: 1.

---

## 11. Entity data

### CENTS-DATA-001: Storage of entity data

- **Inputs:** `minecraft:set_entity_data` with an entity id and a list of entries. Each entry
  has an index, a type and a value.
- **Behavior:**
  1. The client stores the value of each entry at its index for the client entity.
  2. An index that the packet does not contain keeps its value.
  3. Each index has a default value before the first packet. The default values are in
     CENTS-DATA-002 and in the mob documents.
- **Postconditions:** a renderer reads the last value of each index.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: name a pig with
a name tag. Then make it invisible with an effect. The name stays.
Milestone: 1.

### CENTS-DATA-002: Indices that each living entity has

- **Inputs:** the entity data of a living entity.
- **Behavior:** the renderer and the queries read these indices:

| Index | Type | Default | Meaning |
|-------|------|---------|---------|
| 0 | byte | 0 | shared flags: bit 0 on fire; bit 1 crouching; bit 3 sprinting; bit 4 swimming; bit 5 invisible; bit 6 glowing; bit 7 fall flying |
| 2 | optional text | absent | custom name |
| 3 | Boolean | false | custom name always visible |
| 6 | pose | standing | pose |
| 7 | `VarInt` | 0 | ticks frozen |
| 8 | byte | 0 | living flags |
| 9 | `f32` | 1.0 | health |
| 15 | byte | 0 | mob flags: bit 0 no AI; bit 1 left handed; bit 2 aggressive. Only a mob has it |
| 16 | Boolean | false | baby. Only an ageable mob has it |
| 17 | Boolean | false | age locked. Only an ageable mob has it |

- **Postconditions:** the mob documents give the indices from 18 for each type.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: capture the set
entity data packet of a baby pig with a custom name. The indices 2, 3 and 16 have these types.
Milestone: 1.

### CENTS-DATA-003: Values derived from entity data

- **Inputs:** the entity data of CENTS-DATA-002; the entity type.
- **Behavior:**
  1. The entity is on fire when bit 0 of index 0 is set and the type is not fire immune. The
     mob document states when a type is fire immune. The zombified piglin is fire immune
     (`entity_types.json`). The pig, the cow, the sheep, the chicken and the frog are not fire
     immune.
  2. The entity is invisible when bit 5 of index 0 is set.
  3. The entity glows when bit 6 of index 0 is set.
  4. The entity is fully frozen when index 7 is 140 or more.
  5. The entity is a baby when index 16 is true.
  6. The scale is the value of the attribute `minecraft:scale` from the attributes packet. It
     is 1 before the first attributes packet. Let `b` be the base value of the packet. Let
     `b′` be `b` plus the amount of each modifier with the operation `add_value`. The value is
     `b′ + b′ · (sum of the amounts with the operation add_multiplied_base)`, then multiplied by
     `1 + amount` for each modifier with the operation `add_multiplied_total`. The client then
     clamps the value to `[0.0625, 16]`. A NaN value becomes 0.0625.
- **Postconditions:** the queries `is_on_fire`, `is_invisible`, `is_baby`, `is_sneaking`,
  `is_sprinting`, `is_swimming`, `pose` and `model_scale` read these values.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: set a pig on
fire with a fire aspect sword. The client shows the fire while bit 0 is set.
Milestone: 1.

---

## 12. Equipment

### CENTS-EQUIP-001: Equipment of an entity

- **Inputs:** the packet `minecraft:set_equipment`: an entity id and a list of entries. Each
  entry is a slot byte and an item stack. The bits 0 to 6 of the slot byte are the slot id. Bit
  7 is set when another entry follows.
- **Behavior:**
  1. The slot ids are:

| Id | Slot |
|----|------|
| 0 | `mainhand` |
| 1 | `offhand` |
| 2 | `feet` |
| 3 | `legs` |
| 4 | `chest` |
| 5 | `head` |
| 6 | `body` |
| 7 | `saddle` |

  2. For a living entity, the client stores the item stack of each entry in its slot. An empty
     item stack makes the slot empty. A slot that the packet does not contain keeps its stack.
  3. Each slot is empty when the client adds the entity. The client deletes the equipment with
     the entity (CENTS-LIFE-002).
  4. The equippable asset of a stack is the value `asset_id` of its `minecraft:equippable`
     component. The client takes the component from the component patch of the stack when the
     patch contains it. When the patch removes the component, the stack has no equippable
     asset. Otherwise, the client takes the default component of the item, from the item
     definitions (`assets/mcrs/item_definition/`).
  5. A stack with no `minecraft:equippable` component, or a component with no `asset_id`, has
     no equippable asset.
- **Postconditions:** the queries `equipment_asset` and `equipment_texture` of
  `molang-queries.md` read the equippable asset of a slot. The query `is_saddled` is 1 when the
  slot `saddle` is not empty.

Provenance: `source_suggested`. Kind: wire for 1; semantic for the others. Confidence: medium.
Experiment: put a saddle on a pig. Capture the packet `minecraft:set_equipment`. The slot byte
is 7. The asset is `minecraft:saddle`. Remove the saddle with shears. The next packet has an
empty stack for the slot 7.
Milestone: 1.

---

## 13. Flags from the world

### CENTS-FLAG-001: On ground

- **Inputs:** the on-ground flag of a move packet, a position sync or a teleport.
- **Behavior:** the on-ground flag becomes the value of the last packet. It is false when the
  client adds the entity.
- **Postconditions:** the query `is_on_ground` is the on-ground flag.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: push a frog
from a block. The flag changes when the frog lands.
Milestone: 2.

### CENTS-FLAG-002: In water

- **Inputs:** the position and the bounding box of the entity; the fluids of the client world.
- **Behavior:** in each tick, the client computes the in-water flag from its own world. The flag
  is true when the bounding box, made smaller by 0.001 on each side, touches water below the
  surface of that water.
- **Postconditions:** the query `is_in_water` is the in-water flag. The server does not send
  this flag.

Provenance: `source_suggested`. Kind: semantic. Confidence: low. Experiment: put a frog in water
of depth 1. Record the flag with a test mod. Compare it with the top of the water.
Milestone: 2.

---

## 14. Values of one frame

### CENTS-FRAME-001: Values that the renderer and the queries read

- **Inputs:** the current values and the previous values of the entity; `pt`.
- **Behavior:** in each frame, the client computes these values one time:

| Value | Formula | Query |
|-------|---------|-------|
| position | `lerp(pt, previous position, position)` | — |
| body yaw | `wrap(alerp(pt, previous body yaw, body yaw))` | `body_y_rotation` |
| head yaw | `wrap(alerp(pt, previous head yaw, head yaw))` | `head_y_rotation` |
| relative head yaw | `wrap(head yaw − body yaw)`, with the frame values | `target_y_rotation` |
| pitch | `lerp(pt, previous pitch, pitch)` | `target_x_rotation`, `head_x_rotation` |
| entity clock | `C = age + pt`, in ticks | `life_time` (`C / 20`), `delta_time` |
| walk position and speed | CENTS-WALK-002 | `modified_distance_moved`, `modified_move_speed` |
| death time | CENTS-DEATH-002 | `death_ticks` |
| hurt time | the current hurt time | `hurt_time` |
| partial tick | `pt` | `frame_alpha` |
| eye position | the frame position plus `(0, eye height, 0)` | — |
| in-water flag | the flag of the last tick (CENTS-FLAG-002) | `is_in_water` |
| upside down | the custom name of index 2 (`client-entity-render.md`, CEREN-PLACE-005) | — (it negates the relative head yaw and the pitch) |
| vehicle and riders | `0.0` in milestones 1 and 2; later, the passenger list of `minecraft:set_passengers` | `is_riding`, `has_rider` |

  The eye height is the term of section 0.1. The renderer and the queries read these values
  from one record of the frame. A value that a later milestone supplies has its default in that
  record until then.
- **Postconditions:** each query reads the same value in all expressions of one frame.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: record a pig at
240 Hz while it turns. The body and the head turn in each frame, not only in each tick.
Milestone: 1.

### CENTS-FRAME-002: Ground speed and movement

- **Inputs:** the movement `(dx, dy, dz)` of step 2 of CENTS-TICK-001.
- **Behavior:**
  1. The ground speed is `sqrt(dx² + dz²) · 20`, in blocks per second.
  2. The vertical speed is `dy · 20`, in blocks per second.
  3. The entity is moving when the position after the latest tick differs from the position
     after the tick before it by more than `1e-5` block on at least one axis.
- **Postconditions:** the queries `ground_speed`, `vertical_speed` and `is_moving` read these
  values.

Provenance: `source_suggested`. Kind: semantic. Confidence: low. Experiment: lead a pig at a
known speed. Compare the ground speed with the distance per second.
Milestone: 2.

---

## 15. Intentional deviations

### CENTS-DEV-001: Collision test of a moved path

CENTS-INTERP-004, item 1, moves the path only when the moved entity box has no collision. A
client that has no collision shapes of blocks yet can treat the test as passed. The difference
is visible only when a snap during a path puts the entity into a block.

Provenance: `design`. Reason: the block collision shapes are not part of milestones 1 and 2.
Milestone: 1.
