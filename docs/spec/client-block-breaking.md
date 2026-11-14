# Block breaks in the client: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states what the vanilla client does when its player breaks a block. The subjects are
the target of the player, the attack input, the packets that the client sends, the sequence, the
prediction of the client and the effects that the client shows. `block-breaking.md` states what the
server does with these packets. This document uses the terms and the packet facts of that document,
and refers to its rules by their IDs. The document does not prescribe an architecture.

Scope: creative mode first. Section 8 states survival mode at the level of packets and ticks.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**, **VarInt**: as
  `login.md`, section 0.1, defines them.
- **Action**, **start action**, **sequence**, **acknowledgement**, **block cube**, **break**,
  **refusal**, **observer**: as `block-breaking.md`, section 0.1, defines them.
- **Vanilla client**: the client of the game version in the table at the top of this document.
- **Player**: the player that the client controls.
- **Tick**: one tick of the client. At the normal tick rate, a tick is 50 ms.
- **Attack input**: the input that the key binding "Attack/Destroy" controls. The default is the
  left mouse button.
- **Press**: one transition of the attack input from up to down.
- **Hold**: the attack input is down at a tick.
- **Release**: the attack input is up at a tick.
- **Eye position**: the position of the player, with the eye height of CBK-TGT-002 added to `y`.
  For a player that stands, this is the eye position of `block-breaking.md`.
- **Block range**: the value of the attribute `minecraft:block_interaction_range` of the player, as
  the client knows it.
- **Entity range**: the value of the attribute `minecraft:entity_interaction_range` of the player,
  as the client knows it.
- **Outline shape**: the set of boxes that the client draws as the selection box of a block state.
  Each box is in the coordinates of the block position.
- **Target**: the result of the ray of section 1. A target is a block target, an entity target or
  a miss.
- **Block target**: a target with a block position, a face and a hit point.
- **Face**: a side of a block. The wire values are 0 down, 1 up, 2 north, 3 south, 4 west and
  5 east.
- **Creative flag**: the bit 0x08 of the flags of `player_abilities`. Section 2.1 states how the
  client sets it.
- **Main-hand item**: the item in the selected hotbar slot.
- **Operator block**: a command block, a chain command block, a repeating command block, a
  structure block, a jigsaw block, a test block or a test instance block.
- **Local break**: the client changes a block in its own world before the server answers
  (CBK-PRD-001).
- **Prediction**: the steps of one packet that carries a new sequence (CBK-SEQ-002).
- **Record**: the data that the client keeps for one block position after a prediction changed it
  (CBK-PRD-003).
- **Break delay**: a counter of ticks that the client keeps for the hold of the attack input.
- **Miss cooldown**: a counter of ticks after a press on a miss, in which a press has no effect.
- **Break progress**: a number from 0 that the client adds to while the player breaks a block in
  survival mode. At 1 the block breaks.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". Each rule states its label and its kind. All rules in this
document have the label `source_suggested`, except those of section 10.5, which have the label
`design`. Nobody has run the experiments of section 12.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CBK-TGT` | the target |
| `CBK-INP` | the attack input in each game mode |
| `CBK-CRE` | creative mode |
| `CBK-PKT` | the packets that the client sends |
| `CBK-SEQ` | the sequence |
| `CBK-PRD` | the prediction and the reconciliation |
| `CBK-EFF` | the effects that the client shows |
| `CBK-SRV` | survival mode |
| `CBK-DST` | the crack stages of other players |
| `CBK-LGT` | the light in the client world |

### 0.4 Packets

| State, direction | Name | Id |
|------------------|------|----|
| PLAY, serverbound | `player_action` | 41 |
| PLAY, serverbound | `punch` | 46 |
| PLAY, serverbound | `spectator_action` | 63 |
| PLAY, serverbound | `set_carried_item` | 54 |
| PLAY, clientbound | `block_changed_ack` | 4 |
| PLAY, clientbound | `block_destruction` | 5 |
| PLAY, clientbound | `block_update` | 8 |
| PLAY, clientbound | `section_blocks_update` | 86 |
| PLAY, clientbound | `level_event` | 47 |
| PLAY, clientbound | `swing_animation` | 123 |
| PLAY, clientbound | `player_position` | 73 |
| PLAY, clientbound | `player_abilities` | 65 |
| PLAY, clientbound | `update_attributes` | 134 |
| PLAY, clientbound | `light_update` | 49 |
| PLAY, clientbound | `level_chunk_with_light` | 46 |

Source of the ids: `assets/mcrs/reports/packets.json`.

## 1. The target

**CBK-TGT-001** (`source_suggested`, semantic). The client calculates the target one time in each
tick, before it handles the attack input. It uses the eye position and the view direction of the
player at that time. The view direction comes from the yaw and the pitch of the player. The client
also calculates a target in each frame for the selection box, but the attack input uses only the
target of the tick.

**CBK-TGT-002** (`source_suggested`, semantic). The eye height depends on the pose of the player.

| Pose | Eye height |
|------|------------|
| Stand | 1.62 |
| Crouch (sneak when the player does not fly) | 1.27 |
| Swim, crawl, glide with elytra, spin attack with a trident | 0.4 |
| Sleep | 0.2 |

The value of the attribute `minecraft:scale` multiplies the eye height.

**CBK-TGT-003** (`source_suggested`, semantic). The ray starts at the eye position and goes along
the view direction. Its length is the larger of the block range and the entity range.

**CBK-TGT-004** (`source_suggested`, semantic). The client does not add a range for creative mode
itself. It uses the attribute values that the server sends in `update_attributes`. The base
values of the block range and the entity range of a player are the attribute defaults of
ATR-DEF-001 of `attributes.md` (ATR-BASE-003). In creative mode, the vanilla server adds the two
modifiers of ATR-SRC-003 of that document. Thus a vanilla client in creative mode on a vanilla
server has a block range of 5.0, when the server sends no other base value and no other modifier. On a server that sends no `update_attributes`, the block range
stays 4.5.

**CBK-TGT-005** (`source_suggested`, semantic). The block test visits the block positions that the
ray crosses, in order from the eye position. The first position is the position that holds the eye
position. At each position, the client tests the ray against the outline shape of the block state
there. The first position where the ray crosses the outline shape gives the block hit. The hit point
is the first point of the ray in the outline shape.

**CBK-TGT-006** (`source_suggested`, semantic). The block test ignores fluids. These blocks have an
empty outline shape, and the ray goes through them:

- air, cave air and void air;
- water and lava, at each level;
- a bubble column and a moving piston;
- `minecraft:light`, when the player does not hold the item `minecraft:light`.

For a waterlogged block, the ray tests only the outline shape of the block. The water in it has no
effect on the target.

**CBK-TGT-007** (`source_suggested`, semantic). The face of a block hit is the side of the outline
box through which the ray goes in. When the eye position is inside the outline shape, the hit point
is the eye position. The face is then the side that points against the main axis of the view
direction. For example, a view direction that points mostly down gives the face 1 (up).

**CBK-TGT-008** (`source_suggested`, semantic). A composter, a cauldron of each type, a hopper and
scaffolding have a second shape inside the outline shape. When the ray crosses the second shape
before the outline shape, the face comes from the second shape. The block position and the hit
point do not change.

**CBK-TGT-009** (`source_suggested`, semantic). The client also tests the entities along the ray.
An entity hit that is nearer to the eye position than the block hit gives an entity target. This
document does not state the entity test.

**CBK-TGT-010** (`source_suggested`, semantic). The target is a miss in each of these cases:

- The block test and the entity test find nothing.
- The hit is a block hit, and the distance from the eye position to the hit point is equal to or
  more than the block range.
- The hit is an entity hit, and the distance from the eye position to the hit point is equal to or
  more than the entity range.

The limit tests the hit point on the outline shape, and not the block cube. Thus the client range
is smaller than the server limit of BRK-RCH-001. The server limit is the block range plus 1.0, to the
nearest point of the block cube.

## 2. The attack input

### 2.1 The game mode

**CBK-INP-001** (`source_suggested`, semantic). The client knows the game mode of the player from
`login`, `respawn` and the `game_event` that changes the game mode. When the game mode changes, the
client sets the creative flag: on in creative mode, off in the other modes. A `player_abilities`
packet also sets the creative flag. The rules of section 3 apply when the creative flag is on. The
game mode alone selects the rules for adventure mode, spectator mode and the miss cooldown.

### 2.2 When the client handles the input

**CBK-INP-002** (`source_suggested`, semantic). In each tick, the client does these steps in this
order:

1. If the selected hotbar slot changed, send `set_carried_item`.
2. Calculate the target (CBK-TGT-001).
3. Handle each press since the last tick, in order (CBK-INP-004).
4. Handle the hold (CBK-INP-006).
5. If the miss cooldown is more than 0, decrease it by 1.
6. Move the player, and send the movement packets.

Thus the `player_action` packet of a tick leaves the client before the movement packet of the same
tick. The client does steps 3 to 5 only when no screen and no overlay is open.

**CBK-INP-003** (`source_suggested`, semantic). While the player uses an item, for example to eat or
to draw a bow, the client discards each press. The hold has no effect.

### 2.3 A press

**CBK-INP-004** (`source_suggested`, semantic). For each press, the client does these steps:

1. If the miss cooldown is more than 0, stop.
2. If the hands of the player are busy, for example when the player paddles a boat, stop.
3. In spectator mode, send `spectator_action` and stop (CBK-INP-008).
4. If the main-hand item is not enabled in the world, stop.
5. If the main-hand item has the component `minecraft:minimum_attack_charge`, and the attack charge
   is below its value, stop.
6. If the main-hand item has the component `minecraft:piercing_weapon`, do a stab (CBK-INP-005) and
   stop.
7. If the target is an entity target, attack the entity. This document does not state the attack.
8. If the target is a block target, and the block at the position is not air, start the break.
   Section 3 states it for the creative flag, and section 8 for survival mode.
9. If the target is a miss, or a block target at a position that holds air, start the miss
   cooldown of CBK-INP-007.
10. Swing the arm of the main hand, and send `punch`.

The press "breaks at once" when the block at the target position is air after step 8. The client
does not handle the hold in a tick with a press that breaks at once.

**CBK-INP-005** (`source_suggested`, wire). A stab sends `player_action` with the action 8, the
position `(0, 0, 0)`, the face 0 and the sequence 0. A stab sends no `punch`. Thus an item with the
component `minecraft:piercing_weapon`, for example a spear, never breaks a block in the vanilla
client.

**CBK-INP-006** (`source_suggested`, semantic). The hold applies in a tick when all of these
conditions are true:

- No screen is open, and the window holds the mouse.
- The attack input is down.
- No press in this tick broke at once.

When the hold applies, the miss cooldown is 0, the player does not use an item, and the main-hand
item has no component `minecraft:piercing_weapon`:

1. If the target is a block target at a position that is not air, continue the break. Section 3
   and section 8 state it. When the continuation returns "swing", swing the arm and send `punch`.
2. If the target is a block target at a position that holds air, do nothing.
3. If the target is an entity target or a miss, stop the break (CBK-SRV-006).

When the hold does not apply, the client sets the miss cooldown to 0. It then stops the break
(CBK-SRV-006), if the miss cooldown and the item allow it.

**CBK-INP-007** (`source_suggested`, semantic). In survival mode and in adventure mode, a press on a
miss sets the miss cooldown to 10 ticks. In creative mode, a press on a miss does not set it. When a
screen opens, the client sets the miss cooldown to 10000. Thus after a screen closes, the attack
input has no effect until the player releases it.

### 2.4 Adventure mode and spectator mode

**CBK-INP-008** (`source_suggested`, wire). In spectator mode, a press sends `spectator_action`. The
packet has one field: an optional VarInt, a Boolean and then the VarInt when the Boolean is true.
On an entity target, the field holds the id of the entity. On another target, the field is empty. A
press in spectator mode sends no `player_action` and no `punch`, and the client does not swing the
arm. The hold sends nothing.

**CBK-INP-009** (`source_suggested`, semantic). In adventure mode, the client does not start a break
and sends no `player_action`. A press on a block still swings the arm and sends `punch`. One
exception applies: the main-hand item has the component `minecraft:can_break`, and its predicate
accepts the block. The client then breaks the block as in survival mode (section 8).

**CBK-INP-010** (`source_suggested`, semantic). The client does not start a break at a position
outside the world border. It sends no `player_action` for that position. A press still swings the
arm and sends `punch`.

## 3. Creative mode

The rules of this section apply when the creative flag is on.

**CBK-CRE-001** (`source_suggested`, semantic). A press on a block target starts the break at once.
The client does these steps in this order:

1. Make a prediction with a new sequence (CBK-SEQ-002). In the prediction, try the local break of
   CBK-PRD-001.
2. Send `player_action` with the start action, the target position, the target face and the new
   sequence.
3. Set the break delay to 5.
4. Swing the arm of the main hand, and send `punch` (CBK-INP-004, step 10).

The client sends the start action also when the local break does not occur, for example with a
sword in the main hand.

**CBK-CRE-002** (`source_suggested`, semantic). When the hold applies and the target is a block
target at a position that is not air, the client continues the break:

1. If the break delay is more than 0, decrease it by 1. Swing the arm and send `punch`. Stop.
2. If the target position is outside the world border, send nothing and stop.
3. Set the break delay to 5.
4. Make a prediction with a new sequence, and try the local break.
5. Send `player_action` with the start action, the target position, the target face and the new
   sequence.
6. Swing the arm and send `punch`.

**CBK-CRE-003** (`source_suggested`, semantic). The break delay counts only the ticks in which the
hold applies to a block target that is not air. A tick with a miss, an entity target or a released
input does not change it. A new target does not reset it. Thus the player can break a row of
different blocks at the same rate as one block.

**CBK-CRE-004** (`source_suggested`, semantic). While the player holds the attack input on blocks,
the interval between two start actions is 6 ticks (300 ms). This is the case when the target
position holds air after the local break. When the position does not hold air after the press, the
hold applies in the same tick, and the first interval is 5 ticks. Examples are a refused local
break, and a waterlogged block that becomes water.

| Tick | Target holds air after the press | Target does not hold air after the press |
|------|-----------------------------------|-------------------------------------------|
| T (press) | `player_action` 0, `punch` | `player_action` 0, `punch`, `punch` |
| T+1 to T+4 | `punch` | `punch` |
| T+5 | `punch` | `player_action` 0, `punch` |
| T+6 | `player_action` 0, `punch` | `punch` |

The table assumes that the hold applies to a block in each tick after the press.

**CBK-CRE-005** (`source_suggested`, semantic). In creative mode, the client sends only the start
action for a block. It does not send the actions 1, 2 or 3 for a break that starts in creative mode.

**CBK-CRE-006** (`source_suggested`, semantic). An attack on an entity in creative mode sets the
break delay to 5.

**CBK-CRE-007** (`source_suggested`, semantic). Two presses in one tick on the same block act as
follows. The first press breaks the block at once. The second press uses the same target, but the
position now holds air. Thus the second press only swings the arm and sends `punch`.

## 4. Packets

**CBK-PKT-001** (`source_suggested`, wire). The client sends `player_action` with the fields that
`block-breaking.md` states for the request:

| Field | Type | Value |
|-------|------|-------|
| action | VarInt | see CBK-PKT-002 |
| position | long | `x` in the bits 38 to 63, `z` in the bits 12 to 37, `y` in the bits 0 to 11, each in two's complement |
| face | byte | 0 to 5 (section 0.1) |
| sequence | VarInt | see section 5 |

**CBK-PKT-002** (`source_suggested`, semantic). The client uses these actions for blocks:

| Action | Purpose | When the client sends it | Sequence |
|--------|---------|--------------------------|----------|
| 0 | start | a press on a block (CBK-CRE-001, CBK-SRV-001); a creative hold (CBK-CRE-002) | new (CBK-SEQ-002) |
| 1 | face change | a survival hold on the same block with a new face (CBK-SRV-004) | 0 |
| 2 | abort | a survival break stops before the end (CBK-SRV-006), or moves to a new block (CBK-SRV-002) | 0 |
| 3 | finish | the survival break progress reaches 1 (CBK-SRV-005) | new |

**CBK-PKT-003** (`source_suggested`, wire). `punch` has no fields. It stands for one swing of the
arm of the main hand. The vanilla server answers it with `swing_animation` to the observers, and
not to the player. The client swings its own arm without a packet from the server.

**CBK-PKT-004** (`source_suggested`, semantic). In a tick, the `punch` of a press or a hold follows
the `player_action` of the same step. A `set_carried_item` for a new hotbar slot comes before the
`player_action` of the tick (CBK-INP-002).

**CBK-PKT-005** (`source_suggested`, semantic). The server ignores `player_action` until the client
has loaded, as `block-breaking.md` states. The client sends `player_loaded` for this. This document does not state
when the client sends it.

## 5. The sequence

**CBK-SEQ-001** (`source_suggested`, semantic). The client keeps one sequence counter for each
world. A new world starts with the counter at 0. The client makes a new world when it joins the
server, and when `respawn` names a dimension that is not the current dimension. A `respawn` into the
same dimension keeps the counter.

**CBK-SEQ-002** (`source_suggested`, semantic). Each prediction adds 1 to the counter first, and
then uses the new value. Thus the first sequence in a world is 1. These packets make a prediction:

- `player_action` with the action 0 or 3;
- `use_item_on`;
- `use_item`.

The three packets share the counter. The client sends the value of the counter in the sequence
field of the packet.

**CBK-SEQ-003** (`source_suggested`, wire). The other `player_action` packets carry the sequence 0.
These are the actions 1, 2, 4, 5, 6, 7 and 8.

**CBK-SEQ-004** (`source_suggested`, semantic). The counter is a signed 32-bit integer. After
2147483647, the next value is -2147483648. A vanilla server closes the connection on a negative
sequence (BRK-SEQ-005). Section 11 records this.

## 6. Prediction and reconciliation

### 6.1 The local break

**CBK-PRD-001** (`source_suggested`, semantic). The local break occurs only when all of these
conditions are true:

- The game mode is not spectator. In adventure mode, the main-hand item has a
  `minecraft:can_break` component that accepts the block.
- The main-hand item allows the break. With the creative flag on, an item refuses the break when its
  `minecraft:tool` component has `can_destroy_blocks_in_creative` set to false. The debug stick
  always refuses the break. These are the items of BRK-REF-002.
- The block is not an operator block, or the player can use operator blocks. The player can
  use them when the creative flag is on and the operator level is 2 or more. The client knows the
  operator level from the `entity_event` with the status 24 to 28 for the player. The status 24
  gives level 0, and the status 28 gives level 4.
- The block at the position is not air.

**CBK-PRD-002** (`source_suggested`, semantic). In the local break, the client does these steps in
this order:

1. Show the break effects of the old block state (CBK-EFF-001).
2. Set the block at the position to the block of its fluid. A waterlogged block, kelp, seagrass and
   tall seagrass become a water source. Each other block becomes air.
3. Update the shapes of the neighbour blocks (CBK-PRD-004).

**CBK-PRD-003** (`source_suggested`, semantic). The client keeps a record for each block position
that changes during a prediction. A change is a new block state that is not equal to the old state.
A record holds these data:

- the sequence of the prediction;
- the server state: the block state before the first change;
- the position of the player at the first change.

When a record for the position already exists, the client sets only its sequence to the new
sequence. The server state and the player position stay. The client changes no record outside a
prediction.

**CBK-PRD-004** (`source_suggested`, semantic). A local break updates the shapes of the six neighbour
blocks. When a neighbour gets a new state that is not air, the client sets it, and keeps a record
for it. Examples are a fence or a pane that loses a connection. When a neighbour must become air,
the client does not change it. Thus these blocks stay in the client world until the server sends
their new state:

- the other part of a door, a bed or a tall plant;
- a block that needs the broken block as support, for example a torch or a sign.

**CBK-PRD-005** (`source_suggested`, semantic). The client does not predict drops, falls of blocks
or the extinction of fire. For fire, the local break changes the fire to air, as for each other
block.

**CBK-PRD-006** (`source_suggested`, semantic). A change at a position outside the height limits of
the world, or in a chunk that the client does not hold, does not occur. It makes no record.

### 6.2 Block updates from the server

**CBK-PRD-007** (`source_suggested`, semantic). For each block in `block_update` and in
`section_blocks_update`, the client does one of two things:

- A record exists for the position: set the server state of the record to the new state. The block
  in the world does not change.
- No record exists: set the block in the world to the new state. The client tells the neighbours
  about the change, but it does not update their shapes.

Thus the block of a local break stays as the client predicted it until the acknowledgement. The
chunk packet `level_chunk_with_light` does not change a record.

### 6.3 The acknowledgement

**CBK-PRD-008** (`source_suggested`, semantic). When the client receives an acknowledgement with the
sequence `s`, it settles each record with a sequence of `s` or less. For each such record:

1. Remove the record.
2. If the block in the world is not equal to the server state of the record, set the block to the
   server state. The client tells the neighbours about the change, but it does not update their
   shapes.
3. Do the check of CBK-PRD-009.

A record with a sequence more than `s` stays. An acknowledgement with a sequence that the client
has not sent settles each record up to that value.

**CBK-PRD-009** (`source_suggested`, semantic). After step 2 of CBK-PRD-008, the client compares
`s` with the counter value at the last `player_position` packet. If `s` is more than that value,
and the player collides with the new block, the client moves the player to the player position of
the record. Before the first `player_position`, the counter value of the last teleport is -1.

**CBK-PRD-010** (`source_suggested`, semantic). These are the results for the answers of the vanilla
server:

| Server answer | Result in the client |
|---------------|----------------------|
| The new block state, then the acknowledgement (BRK-BRK-001) | The block keeps the state of the server. |
| A refusal with `block_update` (BRK-REF-001) | The acknowledgement restores the old block. |
| A refusal with only the acknowledgement (BRK-ACK-001, BRK-SPN-002) | The acknowledgement restores the server state of the record. This is the old block. |
| The other part of a door, a bed or a tall plant (BRK-PRT-001) | That part has no record, so its new state applies at once. |
| A water source for a waterlogged block (BRK-ACK-003) | The block keeps the water source. |

**CBK-PRD-011** (`source_suggested`, semantic). A record stays when the client drops the chunk of its
position. When the client gets the chunk again before the acknowledgement, the acknowledgement can
write the old server state into the new chunk. A record ends when the world ends (CBK-SEQ-001).

**CBK-PRD-012** (`source_suggested`, semantic). The acknowledgement of a request that made no record
changes nothing. Examples are a refused local break with a sword, and the actions 1 and 2, which
carry the sequence 0.

## 7. Effects

**CBK-EFF-001** (`source_suggested`, semantic). The local break of CBK-PRD-002 shows the effect of
`level_event` with the type 2001 and the state id of the old block. The client shows it at the
target position, without a packet from the server. The effect of a type 2001 is the same for a local
break and for a `level_event` from the server:

- The break sound of the sound type of the block, with the volume `(v + 1) / 2` and the pitch
  `p × 0.8`. `v` and `p` are the volume and the pitch of the sound type. Air plays no sound.
- The break particles of the block state.

**CBK-EFF-002** (`source_suggested`, semantic). The vanilla server sends no type 2001 to the breaker
for a break of one block (BRK-BRK-004). The client effect of CBK-EFF-001 is the only effect. For a
block with two parts, the server sends the type 2001 at the main part to the breaker (BRK-PRT-002).
When the player hits the main part, the client thus shows the effect two times at that position.
For fire, the server also sends the type 1009 (BRK-BRK-009).

**CBK-EFF-003** (`source_suggested`, semantic). The client shows the break of a block by another
player only from `level_event` with the type 2001 (BRK-BRK-003). It shows the arm swing of another
player only from `swing_animation`.

**CBK-EFF-004** (`source_suggested`, semantic). The start of a break in survival mode plays the hit
sound of the sound type of the block, with the volume `(v + 1) / 8` and the pitch `p × 0.5`. It also
shows one small block particle at the target face. A block with no visible model, or with no
terrain particles, shows no particle.

## 8. Survival mode

The rules of this section apply when the creative flag is off, in survival mode, and in adventure
mode with an accepted `minecraft:can_break` component.

**CBK-SRV-001** (`source_suggested`, semantic). The client keeps one break at a time. A break has a
block position, a face, the main-hand item at its start and the break progress. A press on a block
target, or a hold on a block target that is not the current break, starts a break:

1. If a break exists, and its position or its item is different, send `player_action` with the
   action 2. The position is that of the old break, and the face is the new face.
2. Make a prediction with a new sequence. In the prediction:
   - With a break progress of 0 and a block that is not air, the block reacts to a hit. This document
     does not state the hit reactions.
   - If the break progress of one tick (CBK-SRV-003) is 1 or more, do the local break. No break
     remains.
   - Otherwise, the break starts with the break progress 0. Show the effect of CBK-EFF-004.
3. Send `player_action` with the action 0, the position, the face and the new sequence.
4. Set the break delay to 0.

A break is the same as the current break when the position is equal, and the main-hand item has the
same item and the same components.

**CBK-SRV-002** (`source_suggested`, semantic). When the hold applies to a block target that is not
air, the client continues the break:

1. If the break delay is more than 0, decrease it by 1, and return "swing".
2. If the target is not the current break, start a break (CBK-SRV-001). Return "swing" when the
   start occurs.
3. Add the break progress of one tick to the break progress.
4. If the break progress is 1 or more, finish the break (CBK-SRV-005).
5. Otherwise, if the face is different, send the face change (CBK-SRV-004).
6. Show the crack stage of the break (CBK-SRV-007), and return "swing".

When a survival press does not break at once, the hold applies in the tick of the press. Thus the
first tick adds the progress of one tick.

The client keeps the position and the item of the last break after the break ends. A hold on that
position with that item continues the old break, and does not start a new break. Section 11, item
CU6, records this.

**CBK-SRV-003** (`source_suggested`, semantic). The break progress of one tick is `S / H / F`:

- `H` is the hardness of the block. A hardness of -1 gives a break progress of 0.
- `F` is 30 when the player can harvest the block, and 100 when it cannot. The player can harvest a
  block that does not need a tool, or with a main-hand item that is a correct tool for it.
- `S` is the break speed of the player.

The break speed `S` starts with the speed of the main-hand item for the block. When this speed is
more than 1, add the attribute `minecraft:mining_efficiency`. Then apply in this order:

1. Haste or Conduit Power with the amplifier `a`: multiply by `1 + (a + 1) × 0.2`. With both
   effects, `a` is the larger amplifier.
2. Mining Fatigue with the amplifier `a`: multiply by `0.3^(a + 1)`.
3. Multiply by the attribute `minecraft:block_break_speed`.
4. When the eyes are in water: multiply by the attribute `minecraft:submerged_mining_speed`.
5. When the player is not on the ground: divide by 5.

The client adds the progress in 32-bit floating point. Section "Not in this document" lists the
inputs that this document does not state. The vanilla server uses the same rate, and experiments on
it confirm each factor (`block-breaking.md`, BRK-PRG-002). The server accepts a finish from a
progress of 0.7 (BRK-SRV-008). Thus one more tick of the client, from the sum in 32-bit floating
point, does not change the result of a finish.

**CBK-SRV-004** (`source_suggested`, wire). A face change sends `player_action` with the action 1,
the position of the break, the new face and the sequence 0. It makes no prediction.

**CBK-SRV-005** (`source_suggested`, semantic). To finish a break, the client does these steps:

1. Make a prediction with a new sequence, and do the local break in it.
2. Send `player_action` with the action 3, the position, the face and the new sequence.
3. End the break. Set the break progress to 0, and the break delay to 5.

**CBK-SRV-006** (`source_suggested`, wire). To stop a break, the client sends `player_action` with
the action 2, the position of the break, the face 0 and the sequence 0. It removes its own crack
stage and ends the break. Without a break, the stop does nothing. The client stops the break in these
cases:

- The player releases the attack input.
- The target becomes a miss or an entity target while the player holds the input.
- The client receives `player_position`.

**CBK-SRV-007** (`source_suggested`, semantic). The client shows its own crack stage without a packet
from the server. The stage is `floor(progress × 10)` when the break progress is more than 0. With a
break progress of 0, the client shows no crack. The client uses the entity id of the player for its
crack, with the rules of section 9.

**CBK-SRV-008** (`source_suggested`, semantic). While the break continues, the vanilla server sends a
`level_event` to each player near the block in each tick, the breaker too (`block-breaking.md`,
BRK-SRV-004). The type is 2020 when
the number of ticks since the start is a multiple of 4, and otherwise 2019. The data is the face.
The type 2020 plays the hit sound of CBK-EFF-004 and shows a particle. The type 2019 only shows a
particle. The client plays the hit sound during the break only from these packets.

## 9. The crack stages of other players

**CBK-DST-001** (`source_suggested`, wire). `block_destruction` has these fields, in this order: the
entity id (VarInt), the position (long, as CBK-PKT-001) and the progress (unsigned byte).

**CBK-DST-002** (`source_suggested`, semantic). The client keeps at most one crack for each entity id.

- A progress from 0 to 9 sets the crack of the entity id to that stage. When the position is
  different, the crack moves to the new position.
- Each other progress removes the crack of the entity id. The vanilla server uses 255 for an abort
  and -1 as a byte, which is also 255.

**CBK-DST-003** (`source_suggested`, semantic). At one position, the client shows the crack with the
highest stage. When two cracks have the same stage, the client shows the crack of the higher entity
id. The client shows a crack only when the distance from the camera to the centre of the block is 32
or less.

**CBK-DST-004** (`source_suggested`, semantic). The client removes a crack that did not change for
more than 400 ticks of world time. It does this check when the world time is a multiple of 20.

## 10. Light in the client world

The client keeps sky light and block light for each section that it holds. It calculates light
itself after a block change, and it also receives light from the server. In this section, a
**frame** is one image that the client draws. A frame can follow zero, one or more ticks.

### 10.1 When the client calculates light

**CBK-LGT-001** (`source_suggested`, semantic). The client calculates light again after each block
change in its own world. These changes all count:

- a local break and its neighbour changes (CBK-PRD-002, CBK-PRD-004);
- a block from `block_update` or `section_blocks_update` that applies at once (CBK-PRD-007);
- a block that an acknowledgement restores (CBK-PRD-008).

The block data of `level_chunk_with_light` is not such a change. The light of a new chunk comes
only from the packet (CBK-LGT-007).

**CBK-LGT-002** (`source_suggested`, semantic). A block change starts a light check only when the
old state and the new state differ in one of these properties:

- the light that the block absorbs;
- the light that the block emits;
- the use of the block shape for the light occlusion. A check starts when one of the two states
  uses its shape, for example a slab or stairs.

A check covers sky light and block light. For sky light, the client also updates the height in the
column from which the sky light falls straight down. A dimension with no sky light has only block
light.

**CBK-LGT-003** (`source_suggested`, semantic). The client does not calculate light at the time of
the block change. It collects the checks, and calculates all of them one time in each frame. The
step comes after the ticks of the frame, and before the client prepares the image. Thus the block
change and its light are in the same frame. The step has no limit: it completes each check that it
collected, and the full propagation of each check.

**CBK-LGT-004** (`source_suggested`, semantic). The propagation rules of the client are the rules of
the server. A block gets the larger of its own emission and the light of each face neighbour,
minus the absorption. Light does not pass between two faces whose shapes occlude each other. This
document does not state the rules further.

### 10.2 The extent

**CBK-LGT-005** (`source_suggested`, semantic). The propagation crosses the borders of sections and
of columns. It changes light only in sections for which the client holds light data. It does not
enter a column that the client does not hold, or a section with no light data. A light source in a
column counts only after the client has the light of that column (CBK-LGT-007).

**CBK-LGT-006** (`source_suggested`, semantic). When the light of a block changes, the client marks
for a new mesh each section that holds the block or one of its 26 neighbours. This is a cube of up
to 3 × 3 × 3 sections.

### 10.3 Light from the server

**CBK-LGT-007** (`source_suggested`, semantic). The client does not apply the light of
`light_update` or of `level_chunk_with_light` when it receives the packet. It adds the light to a
queue. The block data of `level_chunk_with_light` applies at once. In each frame, before the
calculation of CBK-LGT-003, the client takes a number of entries from the queue:

| Entries in the queue | Entries that the client takes |
|----------------------|-------------------------------|
| fewer than 1000 | the larger of 10 and the number divided by 10, rounded down |
| 1000 or more | all |

One entry is the light of one packet.

**CBK-LGT-008** (`source_suggested`, semantic). For each section that an entry names in its masks,
the server light replaces the light of the client. The replacement is the full section of one
layer: sky light or block light. A section in the empty mask gets light 0 in each block. A section
that no mask names keeps the light of the client. The entry also enables the light of the column.

**CBK-LGT-009** (`source_suggested`, semantic). In a frame, the client applies the replacements of
CBK-LGT-008 after the calculation of CBK-LGT-003. The client does not calculate light again after a
replacement. Thus the server light wins, also when it is older than the local light. The old light
stays until the next check or the next packet for that section.

**CBK-LGT-010** (`source_suggested`, semantic). Each section that an entry names gets a new mesh,
together with its 26 neighbour sections.

### 10.4 The mesh

**CBK-LGT-011** (`source_suggested`, semantic). The client reads the blocks and the light for a new
mesh in the same frame, after the light calculation. Thus a mesh shows the new block and the new light
together. A block change marks for a new mesh each section that holds the block or one of its 26
neighbours.

**CBK-LGT-012** (`source_suggested`, semantic). The video option `options.prioritizeChunkUpdates`
selects when a mesh is ready. Each value has a translation key:

| Translation key | Result |
|-----------------|--------|
| `options.prioritizeChunkUpdates.none` (default) | Each mesh builds in the background, and shows in a later frame. |
| `options.prioritizeChunkUpdates.byPlayer` | A section that a local change of the player marked builds in the same frame. A local break is such a change. The other sections build in the background. |
| `options.prioritizeChunkUpdates.nearby` | As `byPlayer`. A section whose centre is nearer than the square root of 768 to the camera also builds in the same frame. |

A section that a `block_update`, a `section_blocks_update`, an acknowledgement or only a light change
marked is not a section of the player. A section that a local change and a light change both marked
stays a section of the player. With the value `byPlayer`, a light change outside the sections of
CBK-LGT-011 can thus show one or more frames after the block. With the default value, two sections
can show their new meshes in different frames.

### 10.5 Columns that arrive without light

A program can embed the client as a viewer of saved worlds. It puts columns into the client world
with no server in between, so nobody sends light. This section states an option for that case. The
rules of this section are the choice of mcrs. The reference has no such option and always gets its
light from the server.

**CBK-LGT-013** (`design`, semantic). The option is off by default. With the option off, the client
calculates only what CBK-LGT-001 to CBK-LGT-011 state, and a column that arrives without light stays
dark until light arrives for it. Reason: a client that talks to a server must behave as the
reference does.

**CBK-LGT-014** (`design`, semantic). With the option on, a column that the client takes into its
world can count as a column without light. CBK-LGT-015 states when. In the step of CBK-LGT-003, in
the frame in which the column arrives, the client calculates the light of this column from its
blocks, with the rules of CBK-LGT-004. A column that counts as having light is not calculated. The
client does not calculate a column that another column replaces before the step, or that the server
takes back before the step. A replacement column keeps the light that it came with. A light update
of the server in the same frame wins, as CBK-LGT-009 states.

**CBK-LGT-015** (`design`, semantic). The option has two forms. In the form for block light, the
client calculates only block light, and it leaves sky light as the program set it. A column counts
as without light when each of its block light values is 0, whatever its sky light is. In the form
for sky light and block light, the client calculates both layers. A column counts as without light
when it has light data of neither layer. A dimension with no sky light has no sky light in either
form (CBK-LGT-002). Reason: a viewer often has its own sky light, for example full sky light
everywhere, and a calculation of it would replace that light.

**CBK-LGT-016** (`design`, semantic). The calculation of CBK-LGT-014 crosses the borders of columns
(CBK-LGT-005). Light from the new column changes the light of a column that the client already
holds. Light of a held column enters the new column in the same way. A
section whose light does not change keeps its light and is not marked for a new mesh. A column that
the client does not hold is not entered. When the new column replaces a column at the same place,
the light that the old column gave to the columns around it goes away.

**CBK-LGT-017** (`design`, semantic). The client calculates all columns without light that arrived
in a frame together, in the step of that frame, and it calculates each column one time. This holds
also when thousands of columns arrive in a few frames. A column that arrives later does not start
the calculation of the columns before it. Its light enters those columns only as CBK-LGT-016
states, and only the sections whose light changes get a new mesh. When no more columns arrive, the
client changes no light, and the meshes of the world become complete.

## 11. Unresolved

- **CU1**: the sequence after 2147483647 (CBK-SEQ-004). The vanilla client sends a negative sequence,
  and the vanilla server closes the connection. A player cannot reach this value in normal use. The
  mcrs client can select a behaviour, but a parity test must not compare it.
- **CU2**: a record that outlives its chunk (CBK-PRD-011). The old server state can overwrite a
  newer chunk. This can be a defect of the reference. No experiment shows it yet.
- **CU3**: the entity test of the ray, and the effect of the component `minecraft:attack_range` on
  the target. CBK-TGT-009 states only the order of a block hit and an entity hit.
- **CU6**: a survival hold on a block that comes back after a finish. The server can refuse the
  finish and send the old block again. The vanilla client then adds progress to the old break, and
  later sends the action 3 with no action 0 before it. This can be a defect of the reference.
  `block-breaking.md` states that such a finish gets only the acknowledgement (BRK-SRV-010), and
  entry D2 records that the reference can break the block.

- **CU7**: the light of a new chunk. Its blocks apply at once, and its light applies from the
  queue of CBK-LGT-007, in the same frame or later. The client can show the new blocks with no light
  for some frames. No experiment shows how long.
- **CU8**: a `light_update` that the server sent before a local break, and that the client applies
  after it. CBK-LGT-009 states that the older light then stays. No experiment shows this with a
  vanilla server, which sends the light of a break after the block change.

## 12. Experiments

Use a vanilla client and a vanilla server of the game version in the table at the top of this
document, with `online-mode=false` and `level-type=minecraft:flat`. Put a proxy between the client
and the server. The proxy logs each packet with the time, and it can hold or change a packet.

| Experiment | Steps | Rule |
|------------|-------|------|
| X1 | In creative mode, press the attack input one time on stone. | CBK-CRE-001, CBK-PKT-004, CBK-SEQ-002 |
| X2 | In creative mode, hold the attack input for 40 ticks on a wall of stone. Then repeat with a diamond sword. | CBK-CRE-002, CBK-CRE-004 |
| X3 | In creative mode, hold the attack input. Turn from a wall of stone to the sky for 10 ticks, and back. | CBK-CRE-003 |
| X4 | Join, break two blocks, change the dimension, and break one block. Read the sequences. | CBK-SEQ-001, CBK-SEQ-002 |
| X5 | Let the proxy hold the acknowledgement for 2 s. Let it send `block_update` with stone at the broken position. Look at the block before and after the acknowledgement. | CBK-PRD-007, CBK-PRD-008 |
| X6 | Break two blocks with the sequences `n` and `n + 1`. Let the proxy drop all answers, and send one acknowledgement with `n`. | CBK-PRD-008 |
| X7 | Break the upper part of a door. Let the proxy drop the answers of the server for 2 s. Look at the lower part. | CBK-PRD-004 |
| X8 | In creative mode, let the proxy drop `update_attributes`. Find the largest distance at which a press sends `player_action`. Repeat crouched. | CBK-TGT-002, CBK-TGT-004, CBK-TGT-010 |
| X9 | In survival mode, break stone with a wooden pickaxe. Change the face once. Then release before the end on a second block. | CBK-SRV-001 to CBK-SRV-006 |
| X10 | Let the proxy send `block_destruction` with the progress 3, then 3 at another position, then 10 and 255, for one entity id. | CBK-DST-002 |
| X11 | In adventure mode and in spectator mode, press the attack input on stone. | CBK-INP-008, CBK-INP-009 |
| X12 | Hold a spear and press the attack input on stone. | CBK-INP-005 |
| X13 | Break a waterlogged slab in creative mode, and hold the attack input. | CBK-PRD-002, CBK-CRE-004 |
| X14 | In a closed cave, break one block of the roof below the open sky. Let the proxy drop each `light_update`. Look at the light of the hole and of the floor. | CBK-LGT-001, CBK-LGT-003, CBK-LGT-005 |
| X15 | Put a torch on a wall in a dark room. Break the torch, and then the wall block. Let the proxy drop each `light_update`. | CBK-LGT-001, CBK-LGT-002 |
| X16 | Break a block at a section border and at a column border. Read the light of the debug screen on both sides. | CBK-LGT-005, CBK-LGT-006 |
| X17 | Break a block that a torch lights. Let the proxy hold the `light_update` for 2 s, and then send it with the light of before the break. | CBK-LGT-008, CBK-LGT-009 |
| X18 | Let the proxy send 50 `light_update` packets in one burst. Count the frames until the last one shows. | CBK-LGT-007 |
| X19 | Record the screen at a high frame rate while you break a block next to a torch, with each value of `options.prioritizeChunkUpdates`. Compare the frame of the block with the frame of the light. | CBK-LGT-011, CBK-LGT-012 |

## Not in this document

- The outline shape of each block state. The ray needs it (CBK-TGT-005).
- The propagation rules of light (CBK-LGT-004), and the light properties of each block state. No
  document under `docs/spec/` states them yet.
- The entity test of the ray, and the attack on an entity.
- The hit reactions of blocks at the start of a survival break, for example a note block.
- The speed of an item for a block, the correct tool for a block, and the effects of enchantments
  on the break speed.
- The particles, the sounds and the crack textures as graphics and audio.
- The animation of the arm swing.
- The time when the client sends `player_loaded`.
- Block placement with `use_item_on`, and the use of an item with `use_item`. They share the
  sequence of section 5.
