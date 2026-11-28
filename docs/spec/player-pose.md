# Pose and eye height of a player

Version: see `assets/minecraft/version.json`.

| Field | Value |
|-------|-------|
| Status | `draft` |

This document states who selects the pose of a player, and when. It states how the pose and the
inputs of the pose go to each client. It also states what the eye height of the pose changes, and
how a client draws another player in each pose. The dimensions of each pose and the selection
procedure are in `entity-physics.md`. This document cites them and does not state them again.

The parts of this document are separate. A headless bot, a proxy, a viewer and a plugin each can
use one part with no other part (section 6).

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **clientbound**, **serverbound**, **tick**: as `entity-physics.md`,
  section 0.1, defines them.
- **Pose**, **dimensions**, **box**, **shift key state**, **swim state**, **flight state**,
  **crouch state**: as `entity-physics.md`, section 0.1, defines them.
- **Eye height**: the eye height of the dimensions of the current pose (PHY-BOX-003, PHY-BOX-004).
- **Eye position**: the point `(x, y + e, z)`, where `(x, y, z)` is the position and `e` is the eye
  height.
- **Own player**: the player of a connection, on the server and on its client.
- **Local player**: the own player on its client.
- **Remote player**: a player that a client shows and that is not its local player.
- **Viewer**: a client that tracks a player (`entity-tracking.md`).
- **Selection**: the procedure of PHY-BOX-006, which gives the pose from the inputs.
- **Shared flags**: the entity data entry with index 0. **Living flags**: the entry with index 8.
- **Sleeping position**: the entity data entry with index 14, an optional `BlockPos`.
- **Fall flying**: bit 7 of the shared flags. **Spin attack**: bit 2 of the living flags.
- **Can glide**: the player is not on the ground, is not a passenger, and has no levitation
  effect. Also, an equipment slot holds an item with the component `glider` (`item-stack.md`). The
  component `equippable` of that item names that slot, and the next damage does not break it.
- **Server tick phases**: in one server tick, the server first runs each dimension. A dimension
  ticks its entities, and then sends the tracker updates (TRK-UPD-001). After that, the server
  ticks each own player. The server handles the packets of a client between two ticks. A packet
  that the server sends in an entity tick thus comes before the tracker update of the same tick
  (`pose_e2_8_boat_then_shift`).
- **Frame time `pt`**, **frame value**: as `client-entities.md`, CENTS-FRAME-001, defines them.
  The frame value of a tick value `v` is `lerp(pt, v_prev, v)`.
- Rotations: `Rx(θ)`, `Ry(θ)` and `Rz(θ)` turn by `θ` about the axis X, Y or Z of internal space
  (`entity-geometry.md`), with the sign of CEREN-PLACE-003. Angles are in degrees, except where a
  rule states radians. A product `A · B` applies `B` to a point first.

### 0.2 Provenance and kinds

The labels of `registry.md`, section 0.3, apply. The kinds are those of `README.md`, section
"Semantics, not accidents". Each rule states its label, its kind and its confidence.

The layout of the entity data and the eye height of each pose come from captured artefacts
(section 0.5). The other rules come from the reference implementation. The recordings of
the experiments E2, E7 and E8 confirm the rules that cite them. No experiment has confirmed the
other rules yet. Section 9 gives the experiments.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `POSE-SRV` | the authority of the server, its inputs and its order |
| `POSE-NET` | the entity data of the pose, its receivers and its timing |
| `POSE-EYE` | the eye height and the rules that read it |
| `POSE-CLI` | the pose on the local client and on a remote player |
| `POSE-REN` | how a client draws a remote player in each pose |
| `POSE-API` | the boundaries for a bot, a proxy, a viewer and a plugin |

### 0.4 Scope

In scope: the pose of the entity type `minecraft:player`, on the server and on each client.

These subjects are in other documents:

- the dimensions of each pose and the selection: `entity-physics.md`, PHY-BOX-003 to PHY-BOX-007;
- the crouch state of the local player: PHY-INP-010;
- the camera at the eye height: `client-camera.md`, CCAM-EYE-001 and CCAM-EYE-002;
- the walk and the crouch of the player model: `client-player-render.md`, CPR-POSE-002 and
  CPR-POSE-003;
- the figure in the inventory screen: CPR-FIG-002;
- the figure in the creative screen: `client-creative-inventory.md`, CCI-LAY-007.

These subjects are out of scope:

- the motion of fluids, of elytra flight and of a riptide (`entity-physics.md`, section 12);
- when a bed lets a player sleep, and the search for a stand-up position;
- the cape, the elytra model, the items in the hands and the arm poses of items;
- the poses of other entity types.

### 0.5 Artefacts cited

- `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json`: the output of the
  oracle task `dumpEntityTypes` of `tools/vanilla-oracle`, game version `26.4-snapshot-7`. For
  `minecraft:player`, it gives the width 0.6, the height 1.8 and the eye height 1.62 of the
  pose `standing`. It also gives the index and the serializer of each entity data entry.
- `crates/mcrs_minecraft_entity/tests/fixtures/vanilla/player_dimensions.json`: the output of
  the oracle task `dumpPlayerDimensions` of `tools/vanilla-oracle`, game version
  `26.4-snapshot-7`. For `minecraft:player` and each pose id from 0 to 17, it gives the pose name,
  the width, the height, the eye height and the fixed flag, with the `scale` attribute at 1 and
  at 2 (experiment E1).
- The data pack: the fluid tag `swimmable` and the block tag `beds`.
- The recordings of the experiments E2, E7 and E8, in
  `tools/mcrs_minecraft_client_recorder/recordings/pose/`, game version `26.4-snapshot-7`. A
  vanilla client played each step with the input agent of `tools/vanilla-client`. A recording
  gives each entity data entry of the own player, each teleport and each serverbound input, with
  the client tick and the time at the proxy. In E8, a module of the recorder proxy changed the
  serverbound packets, and the recording gives the packets that the server received. A rule
  cites a recording by its file name without the extension `.txt`.

The recorder writes a pose by the name of its id in POSE-NET-001. In each step, the names agree
with the pose of the step for the ids 0 to 5. The fixture of E1 gives the dimensions of each pose.
No artefact gives the wire encoding of the pose ids 6 to 17.

## 1. Authority of the server

**POSE-SRV-001** (`source_suggested`, semantic, confidence: medium). The server selects the pose of
each player. It uses the selection of PHY-BOX-006 with its own copy of the inputs. No serverbound
packet carries a pose. The server never takes a pose from a client.

**POSE-SRV-002** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The server
sets the inputs of the selection from these sources:

| Input | Source on the server | When it changes |
|-------|----------------------|-----------------|
| shift key state | the shift flag (0x20) of `player_input` (MVC-PKT-004); also POSE-SRV-012 | when the server handles the packet |
| sprint state | `player_command` actions 1 and 2 (MVC-PKT-006) | when the server handles the packet |
| swim state | POSE-SRV-003 | in the player tick |
| fall flying | POSE-SRV-004 | when the server handles the packet, and in the player tick |
| spin attack | POSE-SRV-005 | in the player tick |
| sleep | POSE-SRV-006 | when the bed lets the player sleep, and in the player tick |
| flight state | the flight state that the server keeps for the player | when it changes |
| passenger | POSE-SRV-007 | when the player mounts or dismounts |

A change of the shift key state sets bit 1 of the shared flags at once to the same value. Thus the
own player receives bit 1 in the tracker update of the next tick, and the pose one tick later
(POSE-SRV-009). Recordings: `pose_e2_1_crouch` (shift, standing and crouching),
`pose_e2_1_fly_shift` (the flight state keeps `standing` with bit 1 on), `pose_e2_3_swim`
(sprint), `pose_e2_8_boat_then_shift` and `pose_e2_8_shift_then_boat` (passenger).

**POSE-SRV-012** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The
secondary action flag of `interact` also sets the shift key state, when the server handles the
packet. Thus bit 1 and the pose follow as after `player_input` (POSE-SRV-009). The server ignores
`player_input` until the player has loaded (MVC-PKT-005). Recordings:

- `pose_e8_2_shift_only`: the proxy clears the shift flag of each `player_input`, and the player
  holds `shift`. No bit 1 comes.
- `pose_e8_2_interact_shift`: the same, and the player uses a pig with `shift` held. The client
  sends `interact` with the flag on. Bit 1 comes in the next update, and `crouching` one tick
  later.
- `pose_e8_3_stand`: the proxy sends `player_input` with the shift flag just before
  `player_loaded`. No bit 1 and no `crouching` come. `pose_e8_3_crouch` shows that a later
  `player_input` with the shift flag sets bit 1.

**POSE-SRV-003** (`source_suggested_black_box_confirmed`, semantic, confidence: high). In the base
tick of a player,
the server sets the swim state:

- A player in the swim state keeps it when all are true: the sprint state is on, the player is
  in a fluid with the tag `swimmable`, and the player is not a passenger.
- A player not in the swim state gets it when all are true: the sprint state is on, the eye
  position is in a fluid with the tag `swimmable`, the player is not a passenger, and the fluid
  at the block of the position has the tag `swimmable`.
- In all other cases, the swim state is off.

Bit 4 of the shared flags is the swim state. Recording: `pose_e2_3_swim`. The sprint under water
gives bit 3, then bits 3 and 4 with the pose `swimming` one tick later. When the sprint stops, bit
4 and the pose `swimming` clear in the next updates.

**POSE-SRV-004** (`source_suggested_black_box_confirmed`, semantic, confidence: high). Fall
flying:

1. When the server handles `player_command` action 6, it sets fall flying when all are true:
   fall flying is off, the player can glide, and the player is not in a liquid. POSE-SRV-013
   states the result when it does not set fall flying.
2. In each player tick, the server clears fall flying when the player cannot glide.

Recordings: `pose_e2_5_elytra` (bit 7, then the pose `fall_flying` one tick later; both clear when
the player lands), `pose_e2_5_elytra_removed_1` (a command removes the elytra in the air; bit 7
and the pose `fall_flying` clear while the player still falls).

**POSE-SRV-013** (`source_suggested_black_box_confirmed`, semantic, confidence: high). When the
server handles `player_command` action 6 and does not set fall flying, it marks bit 7 of the
shared flags as changed, with the value 0. Thus the own player receives the shared flags with bit
7 cleared in the next update, as a correction. The server sends the entry also when its value did
not change. A vanilla client sends action 6 only when its own check lets the player glide
(POSE-CLI-002). Recording: `pose_e8_1_no_elytra` (the proxy sends action 6 after a `jump` press in
the air with no elytra; the shared flags `0x00` come in the next update). In
`pose_e2_5_no_elytra`, with no action 6, no entry of the shared flags comes.

**POSE-SRV-005** (`source_suggested_black_box_confirmed`, semantic, confidence: high). Only the
server sets the spin attack. It sets it when a riptide attack starts. It clears it when the ticks
of the attack end. POSE-SRV-014 gives the other ends of the attack. Recording: `pose_e2_7_riptide`
(bit 2 of the living flags, then the pose `spin_attack` one tick later; 20 ticks later, the
pose `standing` and the living flags 0).

**POSE-SRV-014** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The server
also clears the spin attack when the player hits a living entity, or when the player has a
horizontal collision. When this occurs in the first player tick of the attack, the selection never
sees the spin attack (POSE-SRV-009). The own player then receives bit 2 on and off in two
consecutive updates, and no pose `spin_attack`. Recordings: `pose_e7_1_riptide_wall` (the first
move after the release has a horizontal collision), `pose_e7_1_riptide_mob` (the health of the
mob drops in the same update that clears bit 2).

**POSE-SRV-006** (`source_suggested_black_box_confirmed`, semantic, confidence: high). Sleep:

1. When the player starts to sleep, the server sets the pose `sleeping` at once. It sets the
   sleeping position to the head block of the bed, also when the player uses the foot block.
2. The player moves to the centre of the head block on `x` and `z`. Its `y` is the bed `y` plus
   the top of the bed shape plus 0.125. For a bed with the top at 0.5625, `y` is the bed `y` plus
   0.6875.
3. When the player stops sleeping, the server sets the pose `standing` at once. The player moves
   to a stand-up position, and the sleeping position becomes empty. The own player receives the
   teleport at once. The pose and the empty position come in the next tracker update
   (POSE-NET-004). After action 0, this update is in the same tick as the teleport. After a stop
   in the player tick (step 4, POSE-SRV-016), it is in the next tick.
4. In each tick, the server stops the sleep when no block with the tag `beds` is at the sleeping
   position.
5. `player_command` action 0 stops the sleep (MVC-PKT-006).

Recordings: `pose_e2_6_leave_button` (steps 1, 2, 3 and 5), `pose_e2_6_break_bed` (step 4: the
teleport one tick after the command that removes the bed, then the pose `standing` and the empty
position one tick later). POSE-SRV-015 and POSE-SRV-016 give the other parts of a sleep.

**POSE-SRV-015** (`source_suggested`, semantic, confidence: medium). When the player starts to
sleep, a passenger dismounts first, and the server sets the velocity to zero.

**POSE-SRV-016** (`source_suggested_black_box_confirmed`, semantic, confidence: high). In each
tick, the server also stops the sleep when the bed rule of the bed no longer lets the player sleep
(`environment-attributes.md`, `gameplay/bed_rule`). The stop follows POSE-SRV-006, step 3.
Recording: `pose_e7_2_time_day`. In it, the game rule `players_sleeping_percentage` is 101, so the
night does not pass. A command sets the time to day 20 ticks after the sleep starts. The teleport
comes in the next tick. The pose `standing` and the empty position come one tick later.

**POSE-SRV-007** (`source_suggested_black_box_confirmed`, semantic, confidence: high). When a
player mounts a vehicle, the server sets the pose `standing` at once. A passenger with the shift
key state on dismounts in its ride tick, before the selection of the same server tick
(POSE-SRV-009). Recordings: `pose_e2_8_shift_then_boat` (a player that crouches and mounts
receives `standing` and then `crouching` in two updates), `pose_e2_8_boat_then_shift`.

**POSE-SRV-008** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The server
never gives a player the pose `dying`. The selection continues while the player is dead. The
death fall of a player on a client comes from the death time (CENTS-DEATH-002, CEREN-PLACE-003),
not from the pose. The row `dying` of PHY-BOX-004 does not apply to a player. Recording:
`pose_e2_8_die` (the health 0, and no change of index 6).

**POSE-SRV-009** (`source_suggested_black_box_confirmed`, semantic, confidence: high). In one
server tick, the server does these steps for a player in this order:

1. in the dimension phase, the ride tick of a passenger, with the dismount of POSE-SRV-007;
2. in the dimension phase, after each entity tick of the dimension, the tracker update of the
   player (POSE-NET-004);
3. in the player tick, the base tick, which sets the swim state (POSE-SRV-003);
4. in the player tick, the AI step, which clears fall flying (POSE-SRV-004, step 2) and ends the
   spin attack (POSE-SRV-005);
5. at the end of the player tick, the selection (PHY-BOX-006).

The selection uses the shift key state of the last packet that the server handled before the tick.
Thus a pose that the selection gives reaches the own player one tick after the flags of the same
inputs. Recordings: `pose_e2_8_boat_then_shift` (the dismount teleport, then bit 1 in the same
tick, then `crouching` one tick later), `pose_e2_1_crouch`, `pose_e2_3_swim`.

**POSE-SRV-010** (`source_suggested`, semantic, confidence: medium). When the pose changes, the
entity sets its dimensions, its box (PHY-BOX-001) and its eye height at once. This applies on the
server and on each client, and also to a pose that a packet or a plugin sets. Each later read in
the same tick sees the new values. A change of the pose never moves the player (PHY-BOX-007).

**POSE-SRV-011** (`source_suggested`, semantic, confidence: medium). The local client selects its
own pose (POSE-CLI-001). When it selects a different pose, the server keeps its own pose. Each
check of the server that reads the box or the eye height of the player uses the pose of the server.
The server sends its pose to the own player as a normal entity data entry (POSE-NET-003). It sends
no other correction of the pose.

## 2. Synchronization

**POSE-NET-001** (`source_suggested`, wire, confidence: medium). The entity data serializer 20
(TRK-DATA-001) encodes a pose as a VarInt id:

| Id | Pose | Id | Pose | Id | Pose |
|----|------|----|------|----|------|
| 0 | `standing` | 6 | `long_jumping` | 12 | `sniffing` |
| 1 | `fall_flying` | 7 | `dying` | 13 | `emerging` |
| 2 | `sleeping` | 8 | `croaking` | 14 | `digging` |
| 3 | `swimming` | 9 | `using_tongue` | 15 | `sliding` |
| 4 | `spin_attack` | 10 | `sitting` | 16 | `shooting` |
| 5 | `crouching` | 11 | `roaring` | 17 | `inhaling` |

A decoder reads an id outside 0 to 17 as `standing`.

**POSE-NET-002** (`source_suggested_black_box_confirmed`, wire, confidence: high). The entries of
a player that carry the pose and its inputs are:

| Index | Serializer | Value | Source |
|-------|------------|-------|--------|
| 0 | 0 (byte) | bit 1 (0x02) shift key state; bit 3 (0x08) sprint state; bit 4 (0x10) swim state; bit 7 (0x80) fall flying | POSE-SRV-002 to POSE-SRV-004 |
| 6 | 20 (pose) | the pose | POSE-SRV-001 |
| 8 | 0 (byte) | bit 2 (0x04) spin attack | POSE-SRV-005 |
| 14 | 11 (optional `BlockPos`) | the sleeping position | POSE-SRV-006 |

The index and the serializer of each entry are in the artefact of section 0.5 (CENTS-DATA-002,
TRK-DATA-002). The recordings of E2 show the meaning of each bit and entry: bit 1 in
`pose_e2_1_crouch`, bits 3 and 4 in `pose_e2_3_swim`, bit 7 in `pose_e2_5_elytra`, bit 2 of index 8
in `pose_e2_7_riptide`, and index 14 in `pose_e2_6_leave_button`.

**POSE-NET-003** (`source_suggested`, wire, confidence: high). The server sends a changed entry of
POSE-NET-002 in `set_entity_data` to each viewer, and to the own player (TRK-DATA-005,
MVC-PKT-031). CK-SET-003 of `client-knowledge.md` overrides this for index 6 of the own player. A
viewer that starts to track the player receives the current value of each entry that is not at its
default value (TRK-SPN-001).

**POSE-NET-004** (`source_suggested`, unspecified, confidence: medium). A pose that the selection
gives in a tick reaches the viewers in the tracker update of the next server tick. A shift key
state that a packet sets between two ticks reaches them in the tracker update of the next tick.
Thus a viewer can receive bit 1 one tick before the pose `crouching`. The order and the tick
offset between a flag and the pose are unspecified, with this bound: each change reaches the
viewers in an update no later than the end of the server tick after the change. A client must
accept a flag and the pose in either order, and in different packets.

**POSE-NET-005** (`source_suggested_black_box_confirmed`, semantic, confidence: high). Bit 1 of
the shared flags is the shift key state, not the pose. A receiver of the entries can see bit 1 on
with a pose other than `crouching`: for example, during flight, as a passenger, or when no room
for the crouch box exists. It can also see bit 1 off with the pose `crouching`, when no room for
the box of `standing` exists. A client selects nothing from bit 1 for a remote player
(POSE-CLI-004). Recordings: `pose_e2_1_fly_shift` (bit 1 on with `standing`),
`pose_e2_2_low_ceiling` (bit 1 off with `crouching` until the player walks out from under the
ceiling).

## 3. Eye height

**POSE-EYE-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The eye
height of a player is the eye height of its current pose, after the scale attribute (PHY-BOX-003,
PHY-BOX-004). It changes with the pose (POSE-SRV-010). A player never has the eye height of the
row `dying` (POSE-SRV-008). The capture of E1 (`player_dimensions.json`) shows the eye height of
each pose at the scale 1 and 2. For example, the eye height of `crouching` is 1.27 at the scale 1
and 2.54 at the scale 2. The eye height of `sleeping` and `dying` does not change with the scale.

**POSE-EYE-002** (`source_suggested`, semantic, confidence: medium). These rules read the eye
height or the eye position of a player. Each reads the pose of its own side: the server reads the
server pose, and a client reads the pose of POSE-CLI-001 or POSE-CLI-004.

| Rule | Side | Observer |
|------|------|----------|
| BRK-RCH-003 | server | the reach check of a block action |
| CBK-TGT-002, CBK-TGT-003 | local client | the block or entity that the crosshair targets |
| CCAM-EYE-001 | local client | the camera height |
| CMD-EX-017, CMD-EX-020, CMD-TP-011 | server | the anchor `eyes` of a command |
| PRJ-LCH-006 | server | the start point of a projectile |
| IEN-SPN-002 | server | the start point of an item that the player drops |
| GOAL-CTL-003, GOAL-QRY-003, GOAL-QRY-005 | server | where a mob looks, and its line of sight to the player |
| ENV-AIR-002 | server | the air supply under a fluid |
| ENV-WAL-001 | server | damage in a wall; a sleeping player takes none |
| CPR-FIG-002 | local client | the figure in the inventory screen |
| CCI-LAY-007 | local client | the figure in the creative screen |

The reach of an entity interaction also starts at the eye position. The specification of the
interaction domain states that check. It is not in the scope of this document.

## 4. The pose on a client

**POSE-CLI-001** (`source_suggested`, client agreement, confidence: medium). The local client
selects the pose of its local player in each tick, by PHY-BOX-006, at step 17 of PHY-ORD-001. A
pose that the server sends for the local player stays until that selection, which replaces it.
The local client takes these inputs:

- the shift key state from the local input (PHY-INP-010), not from bit 1;
- the swim state from its own world, by the rule of POSE-SRV-003;
- fall flying, the spin attack and the sleeping position from the entity data of the server.

**POSE-CLI-002** (`source_suggested_black_box_confirmed`, client agreement, confidence: high). The
local client sends no pose. It sends these packets for the inputs of POSE-SRV-002:

| Change | Packet |
|--------|--------|
| shift key | `player_input` with the shift flag (MVC-PKT-004) |
| sprint starts or stops | `player_command` action 1 or 2 |
| the player starts elytra flight | `player_command` action 6 |
| the player leaves the bed | `player_command` action 0 |

The client sends action 6 in the tick of the `jump` press in the air. It sends it only when the
local player can glide by its own state. Recordings: `pose_e2_1_crouch` (shift), `pose_e2_3_swim`
(sprint), `pose_e2_5_elytra` (action 6), `pose_e2_5_no_elytra` (no packet with no elytra),
`pose_e2_6_leave_button` (action 0).

**POSE-CLI-003** (`source_suggested`, client agreement, confidence: medium). The local client
uses its own pose for these results:

- the box of its moves (PHY-BOX-001);
- the eye height of the target ray (CBK-TGT-002) and of the camera (CCAM-EYE-001);
- the sneak speed while it crouches or crawls (PHY-INP-001, step 5);
- the crouch of its model in a third-person view, which follows the crouch state of PHY-INP-010.

**POSE-CLI-004** (`source_suggested`, semantic, confidence: medium). A client never selects the pose
of a remote player. The pose of a remote player is the last value of index 6 (CENTS-DATA-001). The
box and the eye height follow at once (POSE-SRV-010). The client still ticks the remote player
for the values of section 5: the swim amount, the fall-fly ticks and the in-water flag
(CENTS-FLAG-002).

**POSE-CLI-005** (`source_suggested`, semantic, confidence: medium). When a client receives a
sleeping position for an entity, and a block with the tag `beds` is at that position, it moves the
entity to the point of POSE-SRV-006, step 2.

## 5. Drawing of a remote player

These rules extend CEREN-PLACE-002 for a player. `C` is the age of the entity plus `pt`. `pitch`
is the frame value of the pitch. `s` is the scale of CEREN-PLACE-002.

**POSE-REN-001** (`source_suggested`, client agreement, confidence: medium). Step 4 of
CEREN-PLACE-002 uses the pose rotation `M = M_base · M_player`. `M_base` is the first case of this
list that applies:

1. the frame death time is more than 0: the death tilt of CEREN-PLACE-003;
2. the spin attack is on: `Rx(−90 − pitch) · Ry(−75 · C)`;
3. the pose is `sleeping`: `Ry(a) · Rz(90) · Ry(270)`, with `a` from POSE-REN-002;
4. the entity is upside down: as CEREN-PLACE-005;
5. else, the identity.

`M_player` is POSE-REN-003. When the pose is `sleeping`, the client skips step 5 of
CEREN-PLACE-002.

**POSE-REN-002** (`source_suggested`, client agreement, confidence: medium). The bed of a sleeping
player is the block with the tag `beds` at its sleeping position. The angle `a` comes from the
property `facing` of that bed:

| `facing` | `a` | `(dx, dz)` |
|----------|-----|------------|
| `south` | 90 | (0, 1) |
| `west` | 0 | (−1, 0) |
| `north` | 270 | (0, −1) |
| `east` | 180 | (1, 0) |

With a bed, the client also moves the model by `(−dx · k, 0, −dz · k)` blocks after step 6 of
CEREN-PLACE-002, with `k = 1.62 · s − 0.1`. With no bed, `a` is the body yaw `b`, and the model
does not move.

**POSE-REN-003** (`source_suggested`, client agreement, confidence: medium). `M_player` is the
first case that applies:

1. Fall flying is on. `M_player = X · Ry(φ)`, where `φ` is in radians (POSE-REN-005). `X` is
   `Rx(g · (−90 − pitch))`, with `g` from POSE-REN-004. When the spin attack is on, `X` is the
   identity.
2. The frame swim amount `w` (POSE-REN-006) is more than 0. Let `t = −90 − pitch` when the
   in-water flag is on (CENTS-FLAG-002), else `t = −90`. `M_player = Rx(w · t) · T`, where `T`
   moves by `(0, −1, 0.3)` blocks when the pose is `swimming`, and else is the identity.
3. Else, the identity.

**POSE-REN-004** (`source_suggested`, client agreement, confidence: medium). The fall-fly ticks
`n` of a player increase by 1 in each client tick while fall flying is on. They are 0 in each
other tick. `g = clamp((n + pt)² / 100, 0, 1)`.

**POSE-REN-005** (`source_suggested`, client agreement, confidence: medium). Let `v` be the frame
value of the velocity, and `L` the look vector of the frame. Let `vh` and `Lh` be their horizontal
parts `(x, z)`. When `|vh|²` and `|Lh|²` are both more than 1.0E-5:

`φ = sign(v.x · L.z − v.z · L.x) · acos(min(1, |vh · Lh| / (|vh| · |Lh|)))`

Else, the client does not apply `Ry(φ)`.

**POSE-REN-006** (`source_suggested`, client agreement, confidence: medium). The swim amount of a
player starts at 0. In each client tick, it moves by 0.09 toward 1 when the pose is `swimming`, and
by 0.09 toward 0 in other poses. It stays in 0 to 1. The frame swim amount `w` is its frame value.

**POSE-REN-007** (`source_suggested`, client agreement, confidence: medium). While fall flying is
on, the head of the model has the rotation −45 about X. The rotations about X of the arms and the
legs from the walk (CPR-POSE-002) are divided by `max(1, (|u|² / 0.2)³)`. Here `u` is the
velocity of the current tick. Angles of model parts are in the file space of
`client-player-render.md`.

**POSE-REN-008** (`source_suggested`, client agreement, confidence: medium). When `w` > 0, the
model parts move toward the swim targets after the walk and the crouch. `lerp(w, a, b)` is
`a + w · (b − a)`. A short lerp goes on the shorter way around the circle.

1. Head, when fall flying is off: X is the short lerp by `w` from its value to −45.
2. Legs: X is `lerp(w, walk, 17.1887 · cos(p / 3))` for the right leg, and
   `lerp(w, walk, −17.1887 · cos(p / 3))` for the left leg. `p` is the walk position of
   CPR-POSE-002.
3. Arms, when the player does not use an item. Let `m = p mod 26`, `q(x) = x² − 65x` and
   `r = q(m) / q(14)`. The targets `(X, Y, Z)` of the right arm are:

| Range | `u` | Right arm target |
|-------|-----|------------------|
| `m` < 14 | | `(0, 180, 180 − 107.1868 · r)` |
| 14 ≤ `m` < 22 | `(m − 14) / 8` | `(90 · u, 180, 72.8132 + 107.1868 · u)` |
| 22 ≤ `m` < 26 | `(m − 22) / 4` | `(90 − 90 · u, 180, 180)` |

   The left arm has the target `(X, 180, 360 − Z)`, with `X` and `Z` of the right arm. The
   right arm uses `lerp` for each axis. The left arm uses the short lerp for each axis.
4. The amount of an arm is `w`. It is 0 for an arm that swings, and for an arm with the arm pose
   of a spear.

**POSE-REN-009** (`source_suggested`, client agreement, confidence: medium). A remote player in the
pose `crouching` has the crouch of CPR-POSE-003. The client reads the pose for it, not bit 1.

## 6. Boundaries for bots, proxies, viewers and plugins

**POSE-API-001** (`design`, semantic, confidence: medium). The rules of this document form four
parts. Each part takes only the values that it names as input:

| Part | Rules | Input | Output |
|------|-------|-------|--------|
| selection | PHY-BOX-006, POSE-SRV, POSE-CLI-001 | the inputs of POSE-SRV-002, the current pose and the blocks around the player | a pose |
| dimensions | PHY-BOX-003, PHY-BOX-004, POSE-SRV-010 | a pose and the scale | the box and the eye height |
| synchronization | POSE-NET | a pose and the flags | entity data entries |
| drawing | POSE-REN | the entries of POSE-NET-002, the pitch, the head yaw, the velocity and the client ticks | a pose rotation and part angles |

Reason: a bot needs the selection without the drawing, and a viewer needs the drawing without the
selection. A plugin replaces the output of the selection and keeps the other parts.

**POSE-API-002** (`design`, semantic, confidence: medium). A headless bot is a local client without
the drawing. It selects its pose by POSE-CLI-001, sends the packets of POSE-CLI-002, and uses its
eye height for its target ray (POSE-CLI-003).

**POSE-API-003** (`design`, semantic, confidence: medium). A proxy or a viewer reads the entries of
POSE-NET-002 with no world simulation. It gets the box and the eye height from the pose by
PHY-BOX-003 and PHY-BOX-004. A viewer that draws counts the swim amount (POSE-REN-006) and the
fall-fly ticks (POSE-REN-004) with its own client ticks. The in-water flag needs the fluids of the
world. A viewer with no fluids uses the off value of the flag.

**POSE-API-004** (`design`, semantic, confidence: medium). The server gives a plugin point after
the selection of each player tick. A plugin can replace the selected pose there, or keep the
previous pose. The result goes through POSE-SRV-010 and POSE-NET like a selected pose. A plugin can
also refuse a start of elytra flight (POSE-SRV-013) and stop or keep a sleep. A vanilla
client limits what each player sees:

- each viewer draws the forced pose;
- a vanilla local client selects its own pose again in its next tick (POSE-CLI-001). Only fall
  flying, the spin attack and the sleeping position change its selection;
- with a forced pose, the box of the server and the box of the local client can differ. The checks
  of the server use the box of the server (POSE-SRV-011);
- a sleep with no bed draws along the body yaw (POSE-REN-002).

## 7. Intentional deviations

POSE-NET-003 sends no index 6 to the own player. CK-DEV-006 of `client-knowledge.md` records this
difference. No other rule of this document is a deviation from the reference.

## 8. Known vanilla quirks

- **Rule**: POSE-REN-005. The rule uses the absolute value of the cosine. Thus a player that
  glides backward has the same lean as a player that glides forward.
- **Observer**: the vanilla client. A player that sees the same remote player in a vanilla client
  and in the mcrs client sees a different lean without the accident.
- **Decision**: #90.
- **Bug reference**: none.

## 9. Experiments

E1, E2, E7 and E8 have run.

- **E1** (oracle, run). The task `dumpPlayerDimensions` of `tools/vanilla-oracle` writes the width,
  the height, the eye height and the fixed flag of `minecraft:player` for each pose, at the scale
  1 and the scale 2. Run it with `cargo run -p mcrs_minecraft_update -- recapture
  player_dimensions`. The result is
  `crates/mcrs_minecraft_entity/tests/fixtures/vanilla/player_dimensions.json`. Each value agrees with PHY-BOX-004 and
  POSE-EYE-001. The pose ids and names agree with POSE-NET-001. The capture does not show the wire
  encoding of POSE-NET-001.
- **E2** (recorder, done). The recorder writes the event `< data <entity> <index> <value>` for
  each entry of `set_entity_data`. The scenarios `pose_e2_1` to `pose_e2_8` of
  `tools/mcrs_minecraft_client_recorder/scenarios/` give the steps, and section 0.5 gives the
  recordings. The steps are:
  1. stand, then hold `shift` for 20 ticks, then release it; also in creative flight
     (`fly_shift`);
  2. walk with `shift` under a ceiling at the height 1.5, release `shift` under it, then walk out
     (`low_ceiling`);
  3. sprint into water of depth 2, then stop the sprint with a jump (`swim`);
  4. stand, a command puts a trapdoor at the height 1, then walk out (`trapdoor`);
  5. fall with an elytra, and press `jump` in the air (`elytra`); repeat with no elytra
     (`no_elytra`), and with a command that removes the elytra one or two ticks before the jump
     (`elytra_removed_1`, `elytra_removed_2`);
  6. use a bed at night, then press the leave button (`leave_button`); repeat, and remove the bed
     with a command (`break_bed`);
  7. use a trident with riptide in rain (`riptide`);
  8. mount a boat, then hold `shift` (`boat_then_shift`); hold `shift`, then mount a boat
     (`shift_then_boat`); die with `/kill` (`die`).

  Results: the rules that cite the recordings agree with them. The recordings contradicted the
  order of POSE-SRV-009 and of the term "Server tick phases": the tracker update comes after the
  entity ticks of the dimension. Both are corrected. In step 5, the vanilla client sent no action
  6 with no elytra, and the server accepted the start before the removal of the elytra. Thus E2
  did not decide POSE-SRV-013. In step 4, the pose `swimming` came with no bit 4.
- **E7** (recorder, run). The scenarios `pose_e7_1` and `pose_e7_2` give the steps:
  1. do a riptide into a wall (`riptide_wall`), and into an iron golem with no AI
     (`riptide_mob`). Expect the living flags 0 in the next update after the hit or the collision
     (POSE-SRV-014);
  2. sleep in a bed, then a command sets the time to day (`time_day`). Record whether and when
     the pose `standing` comes (POSE-SRV-016).

  Results: in both riptide steps, the attack ended in its first player tick, so the pose
  `spin_attack` never came. In step 2, the sleep stopped in the first tick after the command.
  Both results also showed that a stop in the player tick sends the teleport one tick before the
  pose. POSE-SRV-006, step 3, is corrected.
- **E8** (proxy, serverbound, run). A module of the recorder proxy changes the serverbound
  packets of a vanilla client (`tools/mcrs_minecraft_client_recorder/README.md`, "Proxy-emulated
  modules"). Each step has a scenario, and each scenario names its module:
  1. `pose_e8_1`, module `start_fall_flying`: send `player_command` action 6 when the player has
     no elytra. Expect bit 7 with the value 0 in the next update (POSE-SRV-013);
  2. `pose_e8_2`, module `no_shift_input`: drop the shift flag of `player_input`, and use a pig
     with `shift` held, so that the client sends `interact` with the secondary action flag.
     Expect bit 1 (POSE-SRV-012);
  3. `pose_e8_3`, module `shift_before_loaded`: send `player_input` with the shift flag just
     before `player_loaded`. Expect no bit 1 (POSE-SRV-012).

  Results: each expectation held. In step 3, the recorder writes a recording only when the
  module changed a packet, so `pose_e8_3_stand` shows that the early packet reached the server.

## 10. Unresolved

These checks have not run. Each needs a change to a tool first.

- **E3** (two clients). Run the steps 1, 3 and 5 of E2 with a second client that tracks the first.
  Record the entries of the first player at the second client, with the server tick of each
  packet. Expect each change in the update of the next tick (POSE-NET-003, POSE-NET-004). Record
  whether bit 1 and the pose `crouching` arrive in the same packet.
- **E4** (proxy). Through a proxy, send index 6 of a remote player with each id 0 to 17, then
  with 18 and with 1000. Record the box that the client shows with the hitbox overlay. Expect the
  box of `standing` for 18 and 1000 (POSE-NET-001, POSE-CLI-004).
- **E5** (proxy). Through a proxy, send the pose `swimming` for the own player. Record the next
  move packets and the box of the local player. Expect the box of the selected pose after one
  tick (POSE-CLI-001).
- **E6** (render). With two clients, make the first player crouch, crawl, swim, glide, sleep in a
  bed of each `facing`, and do a riptide. Take a screenshot at the second client from a fixed
  camera in each case. Compare with the mcrs client at the same state (POSE-REN-001 to
  POSE-REN-009).
