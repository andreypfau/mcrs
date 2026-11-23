# Reach: behavioural specification

Version: see `assets/minecraft/version.json`.

A client attacks, uses and breaks what it saw on its screen. It saw the world a moment ago, from
its own position and with its own view of the other entities. This document states how the server
checks that the target of an action was in reach on the client. It uses the knowledge state of
`client-knowledge.md` for the positions and the blocks that the client had.

A vanilla server checks reach from its own positions, with a wide margin. Section 12 states that
check as the baseline. Each other rule that the server obeys is a choice of mcrs. Section 14 lists
each difference that a differential test can see.

The document has these subjects:

- the check as five parts that a user can take one at a time (section 1);
- the actions that the check covers and their targets (section 2);
- the time of an action in the client tick (section 3);
- the eye, the look and the target as the client had them (sections 4 to 6);
- the range and the comparison (sections 7 and 8);
- the tolerances (section 9);
- the verdict as data (section 10);
- the reaction to a refusal (section 11).

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **clientbound**, **serverbound**:
  as `login.md`, section 0.1, defines them.
- **Client tick**, **tick packets**, **move packet**, **position packet**, **silent tick**,
  **reference position**, **legit client**: as `movement-checks.md`, section 0.1, defines them.
- **Knowledge point**, **knowledge state**, **possible change**, **required change**, **unknown
  value**, **reader**, **entity view**, **`H(k)`**: as `client-knowledge.md`, section 0.1 and
  CK-ENT-003, define them. The reach check is a reader.
- **Movement reader**: the movement checks of `movement-checks.md`.
- **Action**: one serverbound packet of the table of RCH-ACT-001.
- **Action tick**: the client tick in which the client sent the action. RCH-TCK-001 states how the
  server finds it. In this document, `k` is the action tick, and `k − 1` is the client tick before
  it.
- **Pick**: the selection of the target by the client at the start of a client tick (RCH-TCK-001).
- **Eye set** (`E`): the set of eye positions that the client can have at the pick (section 4).
- **Look set** (`L`): the set of unit view directions that the client can have at the pick
  (section 5).
- **Ray**: the half-line from an eye `e` along a look `v`. The point at **ray distance** `t ≥ 0` is
  `e + t × v`.
- **Region**: a set of points that is a union of boxes. A region **grown by** `g` is the union of
  its boxes, each box grown by `g` on each side of each axis.
- **Entry distance** `d(e, v, B)`: the smallest ray distance `t` at which the ray from `e` along
  `v` is in the closed region `B`. It is 0 when `e` is in `B`. It does not exist when the ray does
  not meet `B`.
- **Nearest distance** `n(e, B)`: the distance from `e` to the nearest point of `B`. It is 0 when
  `e` is in `B`.
- **Farthest distance** `f(e, B)`: the distance from `e` to the farthest point of `B`.
- **Entity region**: the region of an entity target that the client can have (RCH-TGT-001).
- **Outline region**: the region of a block target that the client can have (RCH-TGT-005).
- **Block cube**: the box from the block position to the block position plus `(1, 1, 1)`.
- **Cursor point** `P`: for `use_item_on`, the block position plus the cursor of the packet.
- **Entity range** `R_e` and **block range** `R_b`: the values of the attributes
  `entity_interaction_range` and `block_interaction_range` of the player (RCH-RNG-001).
- **Attack range item**: an item stack with the component `attack_range` (`item-stack.md`). Its
  fields are `min_reach`, `max_reach`, `min_creative_reach`, `max_creative_reach`,
  `hitbox_margin` and `mob_factor`.
- **Intake**: the part that keeps the actions of a client tick until the check (RCH-TCK-002).
- **Hit distance**: the distance that orders the candidates of RCH-CMP-007.
- **Verdict**: the result of the check or of the intake for one action (section 10).
- **Reaction**: what the server does for a verdict (section 11).

### 0.2 Provenance

The labels of `registry.md`, section 0.3, apply. The kinds are those of `README.md`, section
"Semantics, not accidents". Each rule states them on its provenance line.

A rule with the label `design` is a choice of mcrs. A vanilla server does no lag compensation, so a
comparison with a vanilla server cannot confirm such a rule. A conformance test with a legit client
tests it, for example the test of RCH-INV-002. Some `design` rules take their idea from a
third-party open source anticheat, which `docs/clean-room.md` allows as a source for the author.
Such a rule has the note `third-party` on its provenance line. It is still a design choice of mcrs
and not game behaviour.

A rule of the kind client agreement states what the vanilla client does. Each such rule cites the
rule of another document that states the behaviour, or an experiment of section 16. The
experiments of section 16 have not run. A rule that names one of them is not confirmed.

### 0.3 ID prefix

Each rule has an ID with the prefix `RCH`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `RCH-PART` | the parts of the check |
| `RCH-ACT` | the actions and their targets |
| `RCH-TCK` | the time of an action |
| `RCH-EYE` | the eye set |
| `RCH-LOOK` | the look set |
| `RCH-TGT` | the target as the client saw it |
| `RCH-RNG` | the range |
| `RCH-CMP` | the comparison |
| `RCH-TOL` | the tolerances |
| `RCH-VRD` | the verdict |
| `RCH-RCT` | the reaction |
| `RCH-VAN` | the vanilla baseline |
| `RCH-INV` | invariants |
| `RCH-DEV` | intentional deviations |

### 0.4 Scope

This document does not state:

- the knowledge state and its probes. `client-knowledge.md` states them;
- the movement checks and the position that they accept. `movement-checks.md` states them;
- what an accepted action does. `player-attack.md`, `block-breaking.md` and `block-placing.md`
  state it;
- the breaking time (issue #474);
- the stab of an item with the component `piercing_weapon`. The client sends no target for it, and
  the server selects the targets itself (issue #121);
- the other actions with a range test on a vanilla server. These are the pick of a block or an
  entity into the hotbar, and the teleport of a spectator to an entity;
- the client selection of a target. `client-block-breaking.md`, section 1, states it for blocks.
  RCH-TCK-001 and section 6 state the parts that the check needs;
- the crates and the types of an implementation. A design note states them.

### 0.5 Parameters

A user sets each parameter through the plugin that adds the check. Each value of the table is a
default.

| Parameter | Default | Rule |
|-----------|---------|------|
| `box_tolerance` | 5.0E-4 blocks | RCH-TOL-001 |
| `distance_tolerance` | 5.0E-4 blocks | RCH-TOL-001 |
| `point_tolerance` | 1.0E-4 blocks | RCH-TOL-002 |
| `previous_look` | off | RCH-LOOK-004 |
| `max_actions_per_tick` | 10 | RCH-TCK-003 |

## 1. Parts

### RCH-PART-001: Five parts

The reach check has five parts. Each part has these inputs and outputs:

| Part | Inputs | Outputs |
|------|--------|---------|
| knowledge state | as `client-knowledge.md`, CK-PART-001 | the values that the client can have at each knowledge point |
| intake | the serverbound packets of one client | each action at the end of its action tick (RCH-TCK-002), or a verdict with the reason `over_limit` (RCH-TCK-003) |
| check | one action, the eye set, the look set, the target region, the range, the tolerances | one verdict |
| verdict | the output of the check or of the intake | data for the reaction and for other readers |
| reaction | one verdict | clientbound packets, the effect of the action, or nothing |

The check reads only its inputs. It reads no state of the server, of the connection, of the
intake or of the reaction. Only the intake counts the actions of a client tick. The verdict has
no side effect.

Provenance: `design`. Kind: semantic. Confidence: high.

### RCH-PART-002: The check alone

A user of the library can call the check with its own inputs, without the knowledge state and
without a server. Examples are a proxy, a replay tool and a test. Sections 4 to 7 state how the
server gets the inputs from the knowledge state. A user can give other inputs, for example the
exact positions of a recording. The same inputs always give the same verdict.

Provenance: `design`. Kind: semantic. Confidence: high.

### RCH-PART-003: The reaction is separate

A user of the library can read each verdict without a reaction. A user can replace the reaction to
each action and each reason. "No reaction" is a valid reaction. A reaction never changes the
knowledge state directly. When a reaction sends a tracked change, the bracket protocol brackets it
as each other tracked change (CK-RCT-003 of `client-knowledge.md`).

Provenance: `design`. Kind: semantic. Confidence: high.

### RCH-PART-004: The check replaces the reach tests of the action rules

When the check runs for an action, the server does not use the reach test of the rule of that
action. These tests are ATK-PKT-003 step 3 of `player-attack.md`, BRK-RCH-001 of
`block-breaking.md`, PLC-RCH-001 of `block-placing.md` and RCH-VAN-002. The server uses the other
tests of these rules, in their order. When the check does not run, the server uses the baseline of
section 12.

Provenance: `design`. Kind: semantic. Confidence: high.

## 2. Actions and targets

### RCH-ACT-001: The actions

The check covers these actions:

| Action | Packet | Target in the packet | Comparison |
|--------|--------|----------------------|------------|
| attack an entity | `attack` (ATK-PKT-001) | the network id of the entity | RCH-CMP-001, RCH-CMP-002 |
| interact with an entity | `interact` | the network id of the entity, the hand, the location relative to the entity, the secondary action | RCH-CMP-003 |
| start a dig | `player_action`, action 0 | the block position, the face | RCH-CMP-004 |
| change the face of a dig | `player_action`, action 1 | the block position, the face | RCH-CMP-004 |
| finish a dig | `player_action`, action 3 | the block position, the face | RCH-CMP-004 |
| place a block or use a block | `use_item_on` (PLC-REQ-001) | the block position, the face, the cursor, the inside flag | RCH-CMP-005 |

The location of `interact` has the low-precision vector format of MVC-PKT-022 of
`movement-checks.md`. It is the hit point minus the position of the entity on the client.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

### RCH-ACT-002: Each action has one verdict

Each action of the table of RCH-ACT-001 gets one verdict. The check gives it, or the intake gives
it with the reason `over_limit` (RCH-TCK-003). The verdicts of one client keep the order of the
actions.

Provenance: `design`. Kind: semantic. Confidence: high.

### RCH-ACT-003: An abort is not checked

The check does not check `player_action` action 2 (abort). The vanilla client sends an abort for
the old block when it changes to another block, and when it stops while it has no target. Thus an
abort can come at any distance. An abort out of reach still ends the dig (BRK-SRV-016 of
`block-breaking.md`).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: R5.

### RCH-ACT-004: The open of a container

A client opens a container with `use_item_on` on a block, or with `interact` on an entity. The
check of that action is the check of the open. The check does not add a test for the open. While
the menu is open, RCH-VAN-006 applies without a change.

Provenance: `design`. Kind: semantic. Confidence: high.

## 3. The time of an action

### RCH-TCK-001: The order of a client tick

The vanilla client does these steps in each client tick, in this order:

1. It applies the clientbound packets that arrived.
2. It does the pick. The pick uses the position of the player at the end of the previous client
   tick, with each teleport of step 1. It uses the current rotation and the eye height of the
   current pose.
3. It handles the inputs. It sends the actions of this tick, each with the target of step 2.
4. It ticks the player. This step selects the pose and moves the player.
5. It sends the move packet, when MVC-PKT-002 of `movement-checks.md` selects one, and then
   `client_tick_end`.

Thus each action of a client tick comes before the move packet of that tick (MVC-PKT-010). The
server finds the action tick of an action by the `client_tick_end` that follows it. The client
turns the player only in a frame, and no frame comes between step 2 and step 5. Thus the rotation
of step 2 is the rotation that the client reports in step 5. The target of a frame does not drive
an action (CBK-TGT-001 of `client-block-breaking.md`).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: R1.

### RCH-TCK-002: The check waits for the end of the action tick

The intake keeps each action until the first move packet or `client_tick_end` after the action.
The check of the action runs then, and the reaction applies the verdict. Thus an accepted action
takes effect after the move packet of its client tick. The verdicts of one client tick keep the
order of their actions.

Provenance: `design`. Kind: semantic. Confidence: high.

### RCH-TCK-003: A bound on the actions of one tick

The intake keeps at most `max_actions_per_tick` actions of one client tick. When one more action
of the same client tick arrives, the intake does these steps:

1. It gives a verdict with the reason `over_limit` to each action of that client tick that it
   keeps, in the order of the actions.
2. It removes these actions. The check does not run for them.
3. It gives a verdict with the reason `over_limit` to the new action, and to each later action of
   the same client tick when it arrives.

The actions of an earlier client tick got their verdicts no later than the `client_tick_end` of
that tick.
Thus each verdict keeps the order of the actions (RCH-TCK-002), and at most `max_actions_per_tick`
actions wait. A client that sends more actions in one client tick gets a refusal for each action
of that tick.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium.

## 4. The eye

### RCH-EYE-001: The base position

The base position is the position of the player at the pick of the action tick `k`. RCH-TCK-001
step 2 states it. The client takes the eye of the pick from its own position at the end of client
tick `k − 1`, and not from a position that it reported. The server takes the base position from the
first source of this list that applies:

1. Client tick `k − 1` has a position packet, and the movement reader runs. The base position is
   the position that the movement reader accepted for the end of that client tick (MVC-PRED-009 and
   MVC-PRED-012 of `movement-checks.md`).
2. Client tick `k − 1` has a position packet, and the movement reader does not run. The base
   position is the position of that packet (CK-SIL-003 of `client-knowledge.md`).
3. Client tick `k − 1` is a silent tick. The base position is each point at most 2.0E-4 blocks
   from the reference position (CK-SIL-001, CK-SIL-002). This source applies also when the movement
   reader runs.

In source 3, the check does not use the first candidate of MVC-PRED-012. MVC-PRED-010 keeps a
candidate up to `2.0E-4 + offset_threshold` from the reference position. The position of the client
is at most 2.0E-4 from it. Thus the first candidate can be 1.2E-3 from the position of the client,
which is more than `box_tolerance`.

A teleport can come after client tick `k − 1` and before the action. When the client accepts it,
the base position is the position that the teleport gives (`teleport-confirmation.md`). The move
packet of client tick `k` comes after the action. Its position is not the base position.

Example: the reference position is `(0, 64, 0)`. In the silent tick `k − 1`, the client moves to
`(0, 64, 1.5E-4)`. The first candidate of the movement reader is `(0, 64, −1.1E-3)`, and
MVC-PRED-010 keeps it. The player stands, looks along `+z` and attacks a zombie whose box starts at
`z = 3.0`. `R_e` is 3.0. The client measures 2.99985 to the box, which is less than `R_e`, so it
sends `attack` (CBK-TGT-010). In RCH-CMP-001, the region `B` starts at `z = 2.9995`.

| Eye set | Smallest entry distance into `B` | Bound `R_e + distance_tolerance` | Result |
|---------|----------------------------------|----------------------------------|--------|
| the first candidate only | 3.0006 | 3.0005 | refused, `out_of_range` |
| source 3 | 2.9993 | 3.0005 | passes |

Provenance: `design`, third-party. Kind: semantic. Confidence: medium. Experiment: R1.

### RCH-EYE-002: The eye heights

The pose of step 2 of RCH-TCK-001 is the pose that the client selected in client tick `k − 1`. The
set of possible poses comes from the first source of this list that applies:

1. The movement reader runs. The set holds the pose that it accepted for client tick `k − 1`.
2. Otherwise, the set holds the pose `standing`, the pose `crouching` and the poses with the eye
   height 0.4.

The eye height of each pose is the value of CBK-TGT-002 of `client-block-breaking.md`. The
attribute `scale` multiplies it. The check uses each value of `scale` that the client can have
(CK-KNW-002 of `client-knowledge.md`). The client sends no action in the pose `sleeping`.

The eye set `E` holds the base position plus `(0, h, 0)`, for each base position and each eye
height `h`.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium. Experiment: R2.

### RCH-EYE-003: The eye of a passenger

While the player rides, its position is its seat position on the vehicle
(`teleport-confirmation.md`, section 0.1). The base position is the seat position at the end of client tick
`k − 1`:

- For a vehicle that the client controls, the position of the vehicle is the position of the last
  `move_vehicle` before the action.
- For another vehicle, the position of the vehicle is each position of the box that contains
  `H(k − 1)` and `H(k)` of the vehicle.

The seat offset and its turn by the yaw of the vehicle are as CK-ENT-008 of `client-knowledge.md`
states. A wider set is allowed, as CK-ENT-008 step 3 states. RCH-EYE-002 gives the eye heights.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: R7.

### RCH-EYE-004: An eye that the check cannot know

The eye is an unknown value (CK-UNC-005 of `client-knowledge.md`) in each of these cases:

- the camera of the client is another entity;
- the knowledge state has no record of a value of this section (CK-BRK-009).

RCH-VRD-004 then applies.

Provenance: `design`. Kind: semantic. Confidence: medium.

## 5. The look

### RCH-LOOK-001: The rotation

The rotation of the pick is the rotation of the move packet of the action tick, when that packet
has a rotation. Otherwise, it is the last rotation that the client reported before the action.
The client reports each change of the rotation (MVC-PKT-002 of `movement-checks.md`).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: R1.

### RCH-LOOK-002: The look of a passenger

While the player rides, a vehicle can turn the yaw of the player in step 4 of RCH-TCK-001. The
reported yaw then is not the yaw of the pick. Thus while the player rides, the look set holds each
yaw, with the pitch of RCH-LOOK-001.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: R7.

### RCH-LOOK-003: The direction

The look `v` of a yaw `y` and a pitch `p`, both in degrees, is
`(sin(−y) × cos(p), −sin(p), cos(−y) × cos(p))`. The client computes it in binary32, with the
sine table and the angle conversion of ARG-COORD-007 of `argument-coordinates.md`. The check
computes it in the same way.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: R4.

### RCH-LOOK-004: The previous rotation

When `previous_look` is on, the look set also holds the last rotation that the client reported
before client tick `k`. The default is off, because RCH-TCK-001 gives one rotation.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium.

## 6. The target as the client saw it

### RCH-TGT-001: The entity region

Let `X` be the box that contains `H(k − 1)` and `H(k)` of the target entity. The entity region is
the union of the entity boxes of CK-ENT-005 of `client-knowledge.md` at each position of `X`. A packet of
step 1 of RCH-TCK-001 can move an entity at once (CENTS-INTERP-003 of `client-entities.md`). The
box of `H(k)` contains that position. For a passenger, CK-ENT-008 gives the entity view. The
check can use a wider region (CK-UNC-004).

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: R3.

### RCH-TGT-002: The pick radius

The client grows the box of each entity by its pick radius before the test. The pick radius is
1.0 for an entity of the tag `#minecraft:redirectable_projectile`. It is 0 for each other entity.
The check grows the entity region by the same radius.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### RCH-TGT-003: An entity position that the check cannot know

The position of the target is an unknown value in each of these cases:

- The client moves the entity with its own step (CK-ENT-004 of `client-knowledge.md`). The
  projectiles are examples.
- The target is a part of an entity with parts. The client computes the position of each part in
  its own step.
- The knowledge state has no record of the entity view (CK-BRK-009).

RCH-VRD-004 then applies.

Provenance: `design`. Kind: semantic. Confidence: medium.

### RCH-TGT-004: An entity in the view of the client

The target entity is in the view of the client when it is a tracked entity at the knowledge point
of the action (CK-ENT-001 of `client-knowledge.md`). An entity whose `add_entity` or
`remove_entities` is in the window state is in the view. Otherwise, the check refuses the action
with the reason `not_in_view`. When the network id names no entity of the server in the dimension
of the player, the check refuses the action with the reason `unknown_target`.

Provenance: `design`. Kind: semantic. Confidence: medium.

### RCH-TGT-005: The outline region

Let `Q` be the position of a block target. The outline region is the union of the outline shapes
of each block state that the client can have at `Q` (CK-KNW-002). The client tests the outline shape, and
not the block cube (CBK-TGT-005 of `client-block-breaking.md`). The outline shape can depend on
the item that the player holds (CBK-TGT-006). The check uses the shape for each item that the
client can hold in each hand. A fluid has no outline shape (CBK-TGT-006).

The client predicts the result of its own action at a block. While the acknowledgement of an
earlier action of the same client is not applied, the states also include these states:

- the state that the server set for that action, for each position that the action changed;
- a block cube at each position that the action could change, when the server refused that action.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: R4.

### RCH-TGT-006: The second shape

Some blocks have a second shape inside the outline shape (CBK-TGT-008 of
`client-block-breaking.md`). The face of a hit can come from that shape. RCH-CMP-005 uses that
shape for the face only. The second shape never changes the cursor point or the block position.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### RCH-TGT-007: A block in the view of the client

The block at `Q` is in the view of the client when an allowed apply sequence has the chunk of `Q`
loaded on the client at the knowledge point. Otherwise, the check refuses the action with the
reason `not_in_view`. When `Q` is outside the build height, or outside the chunks that the server
has loaded, the check refuses the action with the reason `unknown_target`.

Provenance: `design`. Kind: semantic. Confidence: medium.

## 7. The range

### RCH-RNG-001: The attribute ranges

`R_e` is the value of the attribute `entity_interaction_range`, and `R_b` is the value of the
attribute `block_interaction_range`. The check uses the largest value that the client can have at
the knowledge point. The client takes these values from `update_attributes` (CBK-TGT-004 of
`client-block-breaking.md`).

The edition policy (issue #655) gives the base values. The base values of the Java Edition are 3.0
and 4.5 (`attributes.md`). In creative mode, the server adds the two modifiers of ATR-SRC-003 of
`attributes.md`. The check adds no range for creative mode itself.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### RCH-RNG-002: The item of the pick and the item of the attack

The pick uses the active item: the item in use, or else the item in the main hand. When that item
is an attack range item, the pick uses its values. The pick comes before a change of the selected
slot in the same client tick (ATK-PKT-001 of `player-attack.md`). Before the client sends `attack`, it tests the distance with
the attack range item of the main hand after that change. The check uses each item that the
client can have at each of these two times.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: R6.

### RCH-RNG-003: The values of an attack range item

For an attack range item, the reach values are `min = min_reach` and `max = max_reach`. In creative
mode, they are `min_creative_reach` and `max_creative_reach`. The margin is `hitbox_margin`. When
the client can be in both game modes, the check uses both sets of values. The component does not
change `R_e`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: R6.

### RCH-RNG-004: The speed term

With an attack range item, the pick segment of the client is longer by the speed term
`w(v) = max(0, u · v)`. `u` is the velocity of the player at the pick. The check takes `u` from the
movement reader. Without the movement reader, `u` is an unknown value, and RCH-VRD-004 applies.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: R6.

## 8. The comparison

### RCH-CMP-001: An attack without an attack range item

This rule applies to `attack` when no item of RCH-RNG-002 is an attack range item. Let `B` be the
entity region, grown by the pick radius and by `box_tolerance`. The attack passes when an eye `e`
in `E` and a look `v` in `L` give `d(e, v, B) < R_e + distance_tolerance`.

When no pair `(e, v)` gives an entry distance, the reason is `no_line`. When the smallest entry
distance is too large, the reason is `out_of_range`.

The client hit point is in its own box of the target, and that box is in `B`. The client sends the
attack only when the distance to that point is less than `R_e` (CBK-TGT-010). Thus a legit attack
passes.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: R3.

### RCH-CMP-002: An attack with an attack range item

This rule applies to `attack` when an item of RCH-RNG-002 is an attack range item. Let `B` be the
entity region, and `m` the margin. The attack passes when an eye `e` in `E` and a look `v` in `L`
obey each of these conditions:

1. Grow `B` by the pick radius, `m` and `box_tolerance`. The ray from `e` along `v` meets it at a
   ray distance from `min` to `max + w(v)`.
2. `n(e, B) ≤ max + m + distance_tolerance`.
3. `f(e, B) ≥ min − m − distance_tolerance`.

The client tests the segment of condition 1. When the hit is only in the margin, the client moves
the hit point onto the box of the target. Before it sends `attack`, the client tests that the
distance from the eye to that point is from `min − m` to `max + m`. Conditions 2 and 3 contain each
point of that box.

When no pair `(e, v)` obeys condition 1 with an unbounded ray distance, the reason is `no_line`.
Otherwise, the reason is `out_of_range`.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: R6.

### RCH-CMP-003: An interaction with an entity

Let `X` be the box that contains `H(k − 1)` and `H(k)` of the target, and `ℓ` the decoded location
of the packet. Let `ε` be the encoding error of MVC-PKT-022: `s / 32766` on each axis, with `s` the
scale of the vector, or 3.06E-5 when the vector is `(0, 0, 0)`. The interaction passes when each of
these conditions is true:

1. `ℓ` is in the entity box of the target relative to its position, grown by the pick radius, `ε`
   and `box_tolerance`. The check uses each size that the client can have.
2. An eye `e` in `E` and a look `v` in `L` give a ray that comes within `ε + point_tolerance` of the
   box `X + ℓ`. The ray distance of that point is less than `R + distance_tolerance`.

`R` is `R_e`. When the item of the pick is an attack range item, `R` is the larger of `R_e` and
`max + m + w(v)`.

The hit point of the client is its position of the entity plus `ℓ`. That position is in `X`, and
the hit point is on the ray. When condition 1 fails, or no ray comes within the distance of
condition 2, the reason is `no_line`. When only the ray distance is too large, the reason is
`out_of_range`.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: R3.

### RCH-CMP-004: A dig action

This rule applies to `player_action` actions 0, 1 and 3. Let `O` be the outline region at the
block position of the packet, grown by `box_tolerance`. The action passes when an eye `e` in `E`
and a look `v` in `L` give `d(e, v, O) < R_b + distance_tolerance`.

The client sends these actions only for the block of the pick of the same client tick:

- action 0 when it starts on the block, and every 5 client ticks in creative mode while it has a
  block target;
- action 1 when it continues on the same block with another face;
- action 3 when the progress reaches 1.

The check does not test the face. When `O` is empty, or no ray meets it, the reason is `no_line`.
When the smallest entry distance is too large, the reason is `out_of_range`.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: R5.

### RCH-CMP-005: A placement or a use of a block

This rule applies to `use_item_on`. Let `Q` be the block position, `f` the face and `P` the cursor
point of the packet. Let `U` be the set of the boxes of the outline region at `Q`, not grown. Let
`O` be the outline region, grown by `box_tolerance`. Let `D` be `max(R_b, R_e)`, the length of the
ray of the client (CBK-TGT-003). The action passes when an eye `e` in `E` and a look `v` in `L`
obey the conditions of the inside flag.

When the inside flag is false, these conditions apply. In them, `p` is `point_tolerance`, and a box
goes from the corner `lo` to the corner `hi`. `x_a` is the coordinate of a point `x` on the axis
`a`.

1. Ray test. Let `t = ((P − e) · v) / (v · v)`. Then `0 ≤ t < R_b + distance_tolerance`, and
   `|P − (e + t × v)| ≤ p`.
2. Surface test. One box of `U` obeys both of these conditions:
   - on each axis `a`, `lo_a − p ≤ P_a ≤ hi_a + p`;
   - on one axis `a` or more, `|P_a − lo_a| ≤ p` or `|P_a − hi_a| ≤ p`.
3. Face test. Take the boxes of `U` and the boxes of the second shape of RCH-TGT-006, not grown.
   Let `a` be the axis of face `f`. Let `σ` be +1 for the faces up, south and east, and −1 for the
   faces down, north and west. Let `c` be `hi_a` of a box when `σ` is +1, and `lo_a` otherwise. One
   of these boxes obeys `σ × (e_a − c) ≥ −box_tolerance`.

The surface test and the face test use the boxes of `U`, not the boxes of `O`. The boxes of an
outline shape are exact data, and the client puts its hit point on the surface of one of them. A box
grown by `box_tolerance` moves its surface 5.0E-4 blocks away from the hit point, which is more than
`point_tolerance`.

When the inside flag is true:

1. `e` is in `O`.
2. `|P − e| ≤ 0.001 × D + point_tolerance`.
3. `f` is the face whose normal points against the axis of the largest absolute component of `v`
   (CBK-TGT-007).

When `U` is empty, or no pair `(e, v)` obeys the conditions without the bound on `t`, the reason
is `no_line`. Otherwise, the reason is `out_of_range`.

A legit placement passes. The client computes its hit point `H` in binary64, as the first point of
its ray in the outline shape (CBK-TGT-005). `H` is on the surface of one box of the shape, and
`|H − e| < R_b` (CBK-TGT-010). The client sends `H − Q` as three binary32 values (PLC-REQ-001).
When the eye is in the shape, the hit point is the eye plus `0.001 × D × v`. The arithmetic of the
other case is this:

- A binary32 value with an absolute value less than 2 has a rounding error of at most 2⁻²⁴, about
  6.0E-8. Thus each axis of `P` is within 6.0E-8 of the same axis of `H`, and `|P − H|` is at
  most 1.1E-7.
- The surface test holds, because `H` is on the surface of a box and `p` is 1.0E-4.
- The distance from `P` to the ray is at most `|P − H|`, which is less than `p`.
- The look of RCH-LOOK-003 has a length that differs from 1 by less than 1.0E-6. Thus `t` is less
  than `R_b × (1 + 1.0E-6) + 1.1E-7`. For each `R_b` up to 64, that is less than
  `R_b + distance_tolerance`.
- The ray goes into the box through face `f` (CBK-TGT-007, CBK-TGT-008). Thus `e` is on the outer
  side of the plane of that face, and `σ × (e_a − c) ≥ 0`.

Example: the block at `Q = (0, 64, 0)` is a full block. Thus `U` holds the box from `(0, 64, 0)` to
`(1, 65, 1)`. The eye is `e = (0.5, 64.5, −2.0)`, with the yaw 0 and the pitch 0, so
`v = (0, 0, 1)`. The ray meets the north face at `H = (0.5, 64.5, 0.0)`, at the distance 2.0. The
packet has the face north and the cursor `(0.5, 0.5, 0.0)`. Thus `P` is `(0.5, 64.5, 0.0)`.

| Test | Values | Result |
|------|--------|--------|
| ray | `t = 2.0`, `2.0 < 4.5 + 0.0005`; distance from the ray 0 | passes |
| surface | `0 − p ≤ 0.5 ≤ 1 + p`, `64 − p ≤ 64.5 ≤ 65 + p`, `0 − p ≤ 0.0 ≤ 1 + p`; `|P_z − lo_z| = 0` | passes |
| face | axis `z`, `σ = −1`, `c = 0`; `−1 × (−2.0 − 0) = 2.0 ≥ −0.0005` | passes |

The same test against the boxes of `O` puts `P` 0.0005 from the nearest surface, at `z = −0.0005`.
That is more than `p`, so a surface test with `O` refuses this placement. With the eye at
`x = 0.3`, the cursor `x` is the binary32 value 0.300000011920929. `P` is then 1.2E-8 from the ray
and from `H`, and each test passes.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium. Experiment: R4.

### RCH-CMP-006: No test of occlusion

The check does not test whether a block or another entity is between the eye and the target. The
client selects the nearest hit (CBK-TGT-009), but the check does not repeat that selection.

Provenance: `design`. Kind: semantic. Confidence: medium.

### RCH-CMP-007: The first entity on the rays of the client

A rule can select a target on the rays of the client, for example ATK-TGT-006 of
`player-attack.md`. The candidates are the entities that obey each of these conditions:

1. The rule allows the entity.
2. The entity is in the view of the client (RCH-TGT-004).
3. The entity passes the comparison of an attack. This comparison is RCH-CMP-001 when no item of
   RCH-RNG-002 is an attack range item, and RCH-CMP-002 when one is.

The hit distance of a candidate is the smallest value of this list, over each pair `(e, v)` that
passes that comparison:

- for RCH-CMP-001, the entry distance `d(e, v, B)`, with `B` of RCH-CMP-001;
- for RCH-CMP-002, the smallest ray distance `t` from `min` to `max + w(v)` at which the ray from
  `e` along `v` is in the region of its condition 1. The rule uses each set of values of
  RCH-RNG-003 that passes.

The first entity is the candidate with the smallest hit distance. When two candidates have the same
hit distance, the first entity is the one with the smaller network id. When a value of sections 4
to 7 is an unknown value, the rule uses the value of the server in its place (RCH-VRD-004). This
value is the eye or the look of the player, or the box of the entity, on the server.

Provenance: `design`. Kind: semantic. Confidence: medium.

## 9. Tolerances

### RCH-TOL-001: The box and distance tolerances

`box_tolerance` grows each target region, except in the surface test of RCH-CMP-005.
`distance_tolerance` adds to each bound of a distance.
These tolerances cover the rounding difference between the client and the check. They do not cover
latency. The eye set and the target region cover latency.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium.

### RCH-TOL-002: The point tolerance

`point_tolerance` is the largest distance of the cursor point of `use_item_on` from the ray. It is
also the largest distance of that point from the surface of a box of `U` (RCH-CMP-005). It covers the
binary32 rounding of the cursor in the packet.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: R4.

### RCH-TOL-003: The tolerances are parameters

A user can set each tolerance of section 0.5 to each value of 0 or more. The check adds no other
tolerance, margin or allowance to the bounds of section 8.

Provenance: `design`. Kind: semantic. Confidence: high.

## 10. The verdict

### RCH-VRD-001: The fields of a verdict

A verdict has these fields:

| Field | Value |
|-------|-------|
| action | the packet, and its place among the actions of the client tick |
| knowledge point | the knowledge point of the action (CK-BRK-004) |
| result | `accepted` or `refused` |
| reason | for `refused`, one reason of RCH-VRD-002 |
| basis | `client_view`, or `baseline` when RCH-VRD-004 applies; none for the reason `over_limit` |
| distance | the measured distance of RCH-VRD-003, when the check measured one |
| bound | the bound with which the check compared the distance, when the check ran |

A verdict is data. It is not a mismatch of `client-knowledge.md` until a reaction reports it.

Provenance: `design`. Kind: semantic. Confidence: high.

### RCH-VRD-002: The reasons

The intake gives the reason `over_limit` (RCH-TCK-003), and the check does not run for that
action. The check never gives `over_limit`. For each other action, the check does the tests of rows
2 to 5 in their order, and the first test that fails gives the reason:

| Order | Reason | Part | Test |
|-------|--------|------|------|
| 1 | `over_limit` | intake | RCH-TCK-003 |
| 2 | `unknown_target` | check | the server has no such target (RCH-TGT-004, RCH-TGT-007) |
| 3 | `not_in_view` | check | the client cannot have the target (RCH-TGT-004, RCH-TGT-007) |
| 4 | `no_line` | check | no ray from the eye set along the look set meets the target as section 8 states |
| 5 | `out_of_range` | check | each such ray meets the target too far away |

Provenance: `design`. Kind: semantic. Confidence: high.

### RCH-VRD-003: The measured distance

The measured distance of an accepted action, or of the reason `out_of_range`, is the smallest
distance that section 8 compares with its bound. For the reason `no_line`, it is the smallest
nearest distance `n(e, B)` from the eye set to the target region. A refusal with an earlier
reason has no measured distance.

Provenance: `design`. Kind: semantic. Confidence: high.

### RCH-VRD-004: The baseline for an unknown value

A value of sections 4 to 7 can be an unknown value. The check then uses the baseline of section 12
with the positions of the server. The verdict then has the basis `baseline`. The check never
refuses an action only because a value is unknown (CK-UNC-005).

Provenance: `design`. Kind: semantic. Confidence: medium.

## 11. The reaction

### RCH-RCT-001: The default reaction

The default reaction to an accepted action applies the action by its own rules, at the time of
RCH-TCK-002. The default reaction to a refused action is this:

| Action | Effect on the server | Packets to the client |
|--------|----------------------|-----------------------|
| `attack` | none | none |
| `interact` | none | none |
| `player_action` 0, 1 or 3 | none | `block_update` for the block position, then the acknowledgement of the sequence |
| `use_item_on` | none | `block_update` for `Q` and for the position next to face `f`, `container_set_slot` for the slot of the hand, then the acknowledgement of the sequence |

A refused attack does no hit, plays no sound and does not reset the attack counter. A refused
open of a container opens no menu. The server sends no `open_screen` for it.

Provenance: `design`. Kind: semantic. Confidence: medium.

### RCH-RCT-002: The client after a refusal

The acknowledgement alone makes the vanilla client restore each block that it predicted for the
sequence. The `block_update` packets give it the state of the server. The `container_set_slot`
packet gives it the stack of the server, when the client changed that stack in its prediction.

The vanilla client applies steps 3 and 4 of ATK-KB-002 of `player-attack.md` to itself when it
attacks (ATK-KB-003). After a refused attack, the server applies neither step. Thus the velocity
and the sprint state of the attacker on the client can differ from the copy on the server.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: R9.

### RCH-RCT-003: No disconnect by default

The default reaction does not disconnect the client and does not count the refusals. A user
reaction can count them, for example to disconnect after many refusals in a time.

Provenance: `design`. Kind: semantic. Confidence: high.

## 12. The vanilla baseline

The vanilla server checks reach in the rules of this section. It uses its own position of the
player and the eye height of its own pose of the player. It uses its own positions of the
targets.

### RCH-VAN-001: An attack

Let `d` be the nearest distance from the eye to the box of the target. The vanilla server accepts
the reach of an attack when `min − m − 3.0 ≤ d ≤ max + m + 3.0`. The values come from the attack
range item of the main hand. Without such an item, `min` is 0, `max` is `R_e` and `m` is 0. The
server computes `d` with a square root. Both bounds are inclusive. The tests of ATK-PKT-002 and of
ATK-PKT-003 steps 1 and 2 come first.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: R8.

### RCH-VAN-002: An interaction with an entity

Let `d` be the nearest distance from the eye to the box of the target. The vanilla server accepts
the reach of `interact` when `d²` is less than `(R_e + 3.0)²`. It does not test the location of the
packet. The entity must exist, and its block position must be inside the world border.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: R8.

### RCH-VAN-003: A dig action

Let `d` be the nearest distance from the eye to the block cube. The vanilla server refuses the
actions 0 to 3 of `player_action` when `d` is `R_b + 1.0` or more. BRK-RCH-001, BRK-RCH-002 and
BRK-SRV-012 of `block-breaking.md` state this test. It sends one acknowledgement for each action (BRK-ACK-001). An abort out of
reach still ends the dig (BRK-SRV-016).

Provenance: `source_suggested_black_box_confirmed`, from BRK-RCH-001. Kind: semantic. Confidence:
medium.

### RCH-VAN-004: A placement or a use of a block

The vanilla server tests `use_item_on` in this order:

1. The nearest distance from the eye to the block cube of `Q` is less than `R_b + 1.0`
   (PLC-RCH-001 of `block-placing.md`). With the base values, the limit is 5.5 in survival mode and
   6.0 in creative mode.
2. The cursor point is less than 1.0000001 from the center of the block cube on each axis
   (PLC-CUR-001).

When a test fails, the placer gets only the acknowledgement. When both tests pass, the server goes
on with the rules of `block-placing.md`. It then sends `block_update` for `Q` and for the position
next to the face.

Provenance: `source_suggested_black_box_confirmed`, from PLC-RCH-001 and PLC-CUR-001. The trace
`tools/mcrs_minecraft_probe/traces/reach_placement.json` shows the limit of test 1 in both game modes.
Kind: semantic. Confidence: high.

### RCH-VAN-005: What the vanilla server does not test

The vanilla server does not test the look, a ray, the outline shape or the face. It does not use
the position of a target on the client or the eye of the pick. It does not wait for the move packet of the action
tick.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### RCH-VAN-006: An open menu

The vanilla server keeps the menu of a block open while the block cube is within `R_b + 4.0` of the
eye. For a menu of an entity, it keeps it open while the box of the entity is
within `R_e + 4.0` of the eye. Otherwise, it closes the menu. The server of mcrs keeps this rule,
with its own positions.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: R8.

## 13. Invariants

### RCH-INV-001: The bound of an attack on a player

An accepted attack on a player with the basis `client_view` has an eye `e` in `E` that obeys one
of these bounds:

- without an attack range item, `n(e, B) < R_e + distance_tolerance + √3 × box_tolerance`, with
  `B` the entity region;
- with an attack range item, `n(e, B) ≤ max + m + distance_tolerance`.

With the defaults, the first bound is less than `R_e + 0.0014`. No tolerance of an edition
(section 15) applies to an attack on a player. A player is not a type of RCH-TGT-003. Thus the
basis `baseline` applies to an attack on a player only in the cases of RCH-EYE-004 and RCH-RNG-004,
and in the case of CK-BRK-009.

Provenance: `design`. Kind: semantic. Confidence: high.

### RCH-INV-002: Latency gives no refusal

A legit client gets no refusal from the check because of its latency. This is true for any latency
below `max_ack_delay` of `movement-checks.md`, any jitter and any frame rate.

Conformance test: record a vanilla client in the scenes of R1, R3 and R5 (section 16), with 0, 150,
300 and 1000 ms of delay in each direction. Give each recording to a server with the check. The
check gives no refusal.

Provenance: `design`. Kind: semantic. Confidence: medium.

### RCH-INV-003: The same inputs give the same verdict

The same inputs of the check (RCH-PART-001) give the same verdict. The time of arrival of a packet
changes no verdict of the check.

Provenance: `design`. Kind: semantic. Confidence: high.

## 14. Intentional deviations

A vanilla server checks reach from its own positions. Each entry of this section is a place where a
server with these rules does something that a vanilla server does not. A differential test consults
this list.

| Entry | Rules | Test |
|-------|-------|------|
| RCH-DEV-001 | RCH-CMP-001 to RCH-CMP-005 | no comparison of an action between the two bounds |
| RCH-DEV-002 | RCH-EYE-001, RCH-TGT-001 | no comparison of an action at the edge of reach while a target moves |
| RCH-DEV-003 | RCH-CMP-001 to RCH-CMP-005 | no comparison of an action that only the ray test refuses |
| RCH-DEV-004 | RCH-TCK-002 | compare the packets of one client tick as a set |
| RCH-DEV-005 | RCH-RCT-001 | no comparison of a refused block action |
| RCH-DEV-006 | RCH-TCK-003 | no stream with more than `max_actions_per_tick` actions in a client tick |

### RCH-DEV-001: The bound without a margin

- **Rule**: RCH-CMP-001 to RCH-CMP-005.
- **Reference**: the vanilla server adds 3.0 to the entity range and 1.0 to the block range
  (RCH-VAN-001 to RCH-VAN-004).
- **Reason**: the margin of the vanilla server covers latency. The check covers latency with the
  eye set and the target region, so it needs no margin.
- **Visible to the client**: no, for a legit client. A client that acts beyond its range gets a
  refusal.
- **Bug reference**: none.
- **Test**: a differential test does not send an action whose target is beyond the range of the
  client and within the vanilla bound.

### RCH-DEV-002: The positions of the client

- **Rule**: RCH-EYE-001, RCH-TGT-001.
- **Reference**: the vanilla server uses its current position of the player and of the target.
- **Reason**: the client acts on the positions that it had. The positions of the server are later.
- **Visible to the client**: yes, in rare cases. A vanilla server can refuse an attack on a target
  that moved away after the pick. The check accepts it.
- **Bug reference**: none.
- **Test**: a differential test does not compare an action at the edge of reach while the player
  or the target moves.

### RCH-DEV-003: The ray, the outline and the cursor point

- **Rule**: RCH-CMP-001 to RCH-CMP-005.
- **Reference**: the vanilla server tests only the nearest distance to a box (RCH-VAN-005).
- **Reason**: a client that looks away from a target cannot pick it.
- **Visible to the client**: no, for a legit client.
- **Bug reference**: none.
- **Test**: a differential test does not send an action whose look does not meet the target.

### RCH-DEV-004: The time of the effect

- **Rule**: RCH-TCK-002.
- **Reference**: the vanilla server applies an action when the packet arrives, before the move
  packet of the same client tick.
- **Reason**: the check needs the rotation of the move packet.
- **Visible to the client**: yes, on the wire. The packets that an action causes can come after the
  packets that the move packet causes. The player sees no difference.
- **Bug reference**: none.
- **Test**: a differential test compares the clientbound packets of one client tick without their
  order.

### RCH-DEV-005: The packets of a refused block action

- **Rule**: RCH-RCT-001.
- **Reference**: the vanilla server sends only the acknowledgement for an action out of reach
  (BRK-ACK-001, PLC-RCH-001).
- **Reason**: `block_update` and `container_set_slot` give the client the state of the server, also
  for a prediction that the acknowledgement does not restore.
- **Visible to the client**: yes, on the wire. The player sees no difference.
- **Bug reference**: none.
- **Test**: a differential test does not compare the packets of a refused block action.

### RCH-DEV-006: Too many actions in one tick

- **Rule**: RCH-TCK-003.
- **Reference**: the vanilla server handles each action.
- **Reason**: the memory of one connection must have a bound. The verdicts must keep the order of
  the actions.
- **Visible to the client**: no, for a legit client that sends at most `max_actions_per_tick`
  actions in a client tick. A client that sends more gets a refusal for each action of that tick.
- **Bug reference**: none.
- **Test**: a differential test does not send more than `max_actions_per_tick` actions in one client
  tick.

## 15. Edition parameters

This section states parameters, not rules. A Bedrock client joins through its own edge
(`teleport-confirmation.md`, section 3.5). The parts of section 1 stay the same. The edge gives
them these values:

| Parameter | Java Edition | Bedrock Edition |
|-----------|--------------|-----------------|
| base values of `R_e` and `R_b` | the primary edition of the edition policy (issue #655) | the same |
| range of the client | the attributes that the server sends | the own range of the client |
| tolerance of the range | the tolerances of section 9 | the tolerances of section 9 plus a prediction tolerance of the edge, never for an attack on a player |
| outline region | the outline shapes of the Java Edition | the outline table of the Bedrock Edition, or the union of both tables |
| entity box | the Java Edition dimensions | the Java Edition dimensions, which the edge sends in the entity metadata |
| block interaction near an entity | as section 8 | an entity can block a block interaction at a wider range (MCPE-172630) |
| range of the use of a bed | the Java Edition range | the same, because it contains the Bedrock Edition range |
| reaction to a refusal | section 11 | the edge reverts the refused action on the client |

The Bedrock Edition outlines differ for some blocks. Stairs, a hopper and scaffolding are a full
block. Flowers, torches and signs have other sizes. A fence and a pane at a corner do not follow
the direction.

## 16. Unresolved

These experiments have not run. Each rule that names one of them is not confirmed until the
experiment runs. Each experiment uses a vanilla client and the proxy of
`tools/mcrs_minecraft_client_recorder`, except R8, which uses `tools/vanilla-server`. Each
experiment records the target of the pick in each client tick with the capture of
`client-entities.md`, section 0.2.

| Experiment | Steps | Expected result | Rules |
|------------|-------|-----------------|-------|
| R1 | Walk, turn fast and attack a standing zombie at about 2.9 blocks in many client ticks. Do the same just after a `tp`. Record the serverbound packets and the pick. | Each `attack` comes before the move packet of its client tick. The eye of the pick is the position at the end of the previous client tick plus the eye height, or the position of the teleport. The look of the pick is the rotation of the move packet of the same client tick. | RCH-TCK-001, RCH-EYE-001, RCH-LOOK-001 |
| R2 | Press the sneak key and attack in the same client tick. Release it and attack in the same client tick. | The eye height of the pick is the eye height of the pose of the previous client tick. | RCH-EYE-002 |
| R3 | Attack and interact with a pig that walks, with 0, 150 and 300 ms of delay in each direction. | The hit point of each pick is in the region of RCH-TGT-001. The location of `interact` is the hit point minus the position of the pig on the client. | RCH-TGT-001, RCH-CMP-001, RCH-CMP-003 |
| R4 | Place blocks on a slab, a fence, a composter and a piston head, from outside and from inside the outline. | The cursor point is within 1.1E-7 of the ray of RCH-LOOK-003 and of the surface of a box of the outline shape. The inside flag and the face obey RCH-CMP-005. | RCH-LOOK-003, RCH-TGT-005, RCH-CMP-005, RCH-TOL-002 |
| R5 | Break a block, change the face, change to another block, and look away during a dig. Do the same in creative mode. | Actions 0, 1 and 3 come only for the block of the pick of the same client tick. Action 2 comes for the old block at any distance. | RCH-ACT-003, RCH-CMP-004 |
| R6 | Give an item the component `attack_range` with `min_reach` 2.0 and `max_reach` 4.5. Attack at 1.0, 3.5 and 5.0 blocks while the player walks. Change the selected slot in the tick of an attack. | The client sends `attack` only when the conditions of RCH-CMP-002 are true for its eye and look. The pick uses the item before the slot change. | RCH-RNG-002, RCH-RNG-003, RCH-RNG-004, RCH-CMP-002 |
| R7 | Ride a boat that turns, and attack a zombie and use a chest. | The eye of the pick is in the set of RCH-EYE-003. The yaw of the pick differs from the reported yaw by the turn of the boat in that client tick. | RCH-EYE-003, RCH-LOOK-002 |
| R8 | On the vanilla server, attack, interact, dig and place at distances around the bounds of section 12. Open a chest and walk away. | The vanilla server accepts and refuses at the bounds of RCH-VAN-001 to RCH-VAN-004. It closes the menu at the bound of RCH-VAN-006. | RCH-VAN-001, RCH-VAN-002, RCH-VAN-006 |
| R9 | Send a sprint attack and a placement that a server refuses with the reaction of RCH-RCT-001. | The client restores the block and the stack. Its velocity after the attack has steps 3 and 4 of ATK-KB-002. | RCH-RCT-002 |
