# Movement checks: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

This document states how a server validates the movement of a player. It has two parts. Section 2
states what a vanilla server checks. Sections 3 to 10 state the model of this server.

In this model, the server predicts each client tick from the physics model. Then it compares the
result with the position that the client reports. The model is our own design. Server-side
anticheat software inspired it, but this document states only behaviour.

The model covers gravity and collision on land, and creative flight. Section 8 lists the states
that the model does not predict, and the fallback for each.

The checks are in three layers. A network proxy runs the first two layers without a world. The
server runs all three. Section 15 assigns each rule to one layer.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **clientbound**, **serverbound**,
  **VarInt**: as `login.md`, section 0.1, defines them.
- **Teleport**, **accept**, **pending teleport**, **correction**: as `teleport-confirmation.md`,
  section 0.1, defines them.
- **Physics model**: the model that `entity-physics.md` states, with the rule prefix `PHY`. It
  computes one tick of the movement of a player (PHY-ORD-001).
- **Player state**: the data that the physics model needs for one tick of one player. PHY-ORD-007
  lists it as the state `S`.
- **On-ground flag**, **horizontal collision flag**, **shift key state**, **sprint state**,
  **flight state**, **fly permission**: as `entity-physics.md`, section 0.1, defines them.
- **World view**: the blocks, the collision shapes and the world border that the physics model reads
  for one tick.
- **Legit client**: an unmodified vanilla client of the version in the header table. It can have
  any latency, any jitter and any frame rate.
- **Client tick**: one simulation step of the client. The client ends each client tick with one
  `client_tick_end`.
- **Server tick**: one game tick of the server. A server runs 20 server ticks each second at the
  default tick rate.
- **Tick packets**: the serverbound packets that the client sends in one client tick. The last tick
  packet is the `client_tick_end`.
- **Move packet**: one `move_player_pos`, `move_player_pos_rot`, `move_player_rot` or
  `move_player_status_only`.
- **Position packet**: one `move_player_pos` or `move_player_pos_rot`.
- **Silent tick**: a client tick without a position packet.
- **Reported position**: the position in a position packet.
- **Reported flags**: the on-ground bit and the horizontal collision bit of the last move packet.
- **Reference position**: the reported position of the last position packet after the last
  respawn. The client measures a silent tick against this position (MVC-PKT-003).
- **Candidate**: one possible player state of the client after a client tick.
- **Candidate set**: the candidates that the server keeps after a client tick.
- **Accepted state**: the candidate set after the last client tick that the server accepted.
- **Predicted position**: the position of a candidate.
- **Offset**: a distance in blocks between a reported position and a predicted position, after the
  server subtracts the tolerance (MVC-PRED-008).
- **Predicted-state change**: a clientbound packet that changes the player state or the world view.
  Section 1.2 lists these packets.
- **Probe**: a clientbound `ping` that the server sends for latency compensation.
- **Answer**: the serverbound `pong` with the id of a probe.
- **Acknowledged**: a probe is acknowledged for a client tick when its answer arrives before the
  `client_tick_end` of that tick (MVC-TICK-002).
- **Kept candidate**: a candidate that the comparison keeps (MVC-PRED-009, MVC-PRED-010). The
  **first candidate** is the first kept candidate in the order of MVC-PRED-003.
- **Check**: one test of this server. Sections 7 and 8 state the checks: the offset check, the
  on-ground check, the collision check and the tick rate check. They also state the packet check,
  the sprint check and the coarse speed check.
- **Window**: the client ticks in which the client can have a predicted-state change or not have it
  (MVC-LAT-004).
- **Mode**: the class of a client tick. A tick is predicted, coarse, resync or exempt (section 8).
- **Violation**: a client tick, or a packet, that fails a check.
- **Violation level**: a number for each check. A violation adds to it, and it decays.
- **Offset buffer**: a number that sums the offsets of violations of the offset check (MVC-RSP-004).
- **Setback**: a correction that the server sends because of a violation. It moves the client back
  to the accepted state.
- **Box**: the axis-aligned box of the player for its pose (PHY-BOX-001, PHY-BOX-006).
- **Collision shape**: a shape that stops the box (PHY-SHP-001 to PHY-SHP-011).
- **Afloat**: the status of a player that MVC-VAN-013 states.
- **Uncertain block**: a block whose new state is in its window (MVC-LAT-006).
- **Layer**: one of the three parts L0, L1 and L2 of the checks (MVC-LAY-001).
- **Proxy**: a program between the client and the server that forwards the packets of both
  directions. It has no world and no physics model.
- **Report**: a record that a lower layer gives to a higher layer for a failed check (MVC-LAY-005).
- **Try order**: the order in which the server runs the physics step for the candidates of one
  client tick (MVC-PRED-013).
- **Primary candidate**: the first candidate in the try order.
- **Settled shape**: a collision shape that is in the world view of the client in each state that
  the client can have. A shape in a window is not settled (MVC-LAT-006).

### 0.2 Provenance

- `source_suggested`: taken from the reference implementation, with no captured result. Section 14
  attaches an experiment.
- `black_box_confirmed`: shown by a captured result. In this document, only rules of
  `teleport-confirmation.md` have this label, and this document cites them.
- `design`: a rule of this server's own design. It does not state vanilla behaviour, and a vanilla
  server can behave differently. A comparison with a vanilla server cannot confirm it. A conformance
  test in section 14 confirms that a legit client does not fail the rule. The rule states
  behaviour only, as every other rule does.
- `unresolved`: not decided. Section 13 collects these items.

A rule with the label `design` that states a number gives a default value. Section 10 lists each
configurable value. Nobody has run the experiments of this document yet.

### 0.3 ID prefix

Each rule has an ID with the prefix `MVC`. The second part names the section:

| Prefix | Subject |
|--------|---------|
| `MVC-PKT` | the packets |
| `MVC-VAN` | the checks of a vanilla server |
| `MVC-TICK` | the count of client ticks |
| `MVC-PRED` | the prediction |
| `MVC-UNC` | the sources of uncertainty |
| `MVC-LAT` | latency compensation |
| `MVC-CHK` | the checks of this server |
| `MVC-RSP` | the response to a violation |
| `MVC-MODE` | the modes of a client tick |
| `MVC-CFG` | the configurable values |
| `MVC-INV` | invariants |
| `MVC-LAY` | the layers and the cost |

### 0.4 Scope

This document does not state:

- the physics of a player. `entity-physics.md` states it. This document cites its rule IDs;
- the teleport flow. `teleport-confirmation.md` states it;
- the movement of a vehicle;
- the text of a log line. A log line is unspecified.

The physics model does not cover fluids, elytra flight, the riptide enchantment, vehicles, the push
of entities and pistons. Section 8 states the fallback for each of these states.

## 1. Packets

### 1.1 Serverbound packets

Experiment E20 tests each rule of section 1 that names no other experiment.

**MVC-PKT-001** (`source_suggested`, wire, confidence: high). The four move packets have these
fields, in this order. A float is big-endian. `flags` is one unsigned byte.

| Packet | Fields |
|--------|--------|
| `move_player_pos` | `x`, `y`, `z` (three 64-bit floats), `flags` |
| `move_player_pos_rot` | `x`, `y`, `z` (three 64-bit floats), `yaw`, `pitch` (two 32-bit floats), `flags` |
| `move_player_rot` | `yaw`, `pitch` (two 32-bit floats), `flags` |
| `move_player_status_only` | `flags` |

Bit 0x01 of `flags` is the on-ground bit. Bit 0x02 is the horizontal collision bit. The server
ignores the other bits. A legit client sets them to 0. See experiment E1.

**MVC-PKT-002** (`source_suggested`, client agreement, confidence: medium). In each client tick, a
legit client that controls its own player and does not ride sends at most one move packet. It
selects the packet in this order:

1. `move_player_pos_rot`, when the position changed and the rotation changed.
2. `move_player_pos`, when only the position changed.
3. `move_player_rot`, when only the rotation changed.
4. `move_player_status_only`, when only the reported flags changed.
5. No move packet, in other cases.

The rotation changed when the yaw or the pitch is not equal to the last sent value. See experiment
E2.

**MVC-PKT-003** (`source_suggested`, client agreement, confidence: medium). The position changed
when one of these conditions is true:

- The squared distance from the reference position is more than `(2.0E-4)²`.
- 20 client ticks have passed after the last position packet.

The count of client ticks starts again at each position packet. Thus at most 19 silent ticks follow
a position packet. A teleport does not change the reference position. A respawn sets the reference
position to `(0, 0, 0)`. See experiment E3.

**MVC-PKT-004** (`source_suggested`, wire, confidence: high). `player_input` has one byte of flags:

| Bit | Key |
|-----|-----|
| 0x01 | forward |
| 0x02 | backward |
| 0x04 | left |
| 0x08 | right |
| 0x10 | jump |
| 0x20 | shift |
| 0x40 | sprint |

The server ignores bit 0x80. A legit client sends `player_input` only in a client tick in which the
keys are not equal to the last sent keys. A respawn keeps the last sent keys. See experiment E4.

**MVC-PKT-005** (`source_suggested`, wire, confidence: high). `client_tick_end` and `player_loaded`
have no fields.

**MVC-PKT-006** (`source_suggested`, wire, confidence: high). `player_command` has these fields, in
this order: entity id (VarInt), action (VarInt), data (VarInt). The actions are:

| Action | Meaning |
|--------|---------|
| 0 | stop sleep |
| 1 | start sprint |
| 2 | stop sprint |
| 3 | start ride jump |
| 4 | stop ride jump |
| 5 | open inventory |
| 6 | start elytra flight |

Another action does not decode, and the server closes the connection. A legit client sends action 1
or 2 only when its sprint state changes. See experiment E5.

**MVC-PKT-007** (`source_suggested`, wire, confidence: high). The serverbound `player_abilities` has
one byte. Bit 0x02 is the request to fly. The server ignores the other bits.

**MVC-PKT-008** (`source_suggested`, wire, confidence: high). `pong` has one field, the id, as a
big-endian signed 32-bit integer. A legit client sends `pong` with the id of each `ping`, in the
order of the `ping` packets.

**MVC-PKT-009** (`black_box_confirmed`, wire, confidence: high). `accept_teleportation` has the
fields that TP-CONFIRM-PKT-001 states.

**MVC-PKT-010** (`source_suggested`, client agreement, confidence: low). In one client tick, a legit
client sends the tick packets in this order. Each packet is optional, except the last.

1. The packets of the actions of the player, for example `player_action` and `use_item_on`.
2. `player_abilities`, when the flight state changes in the tick.
3. `player_input`.
4. `player_command` with action 1 or 2.
5. One move packet.
6. `client_tick_end`.

The client applies clientbound packets only between two client ticks. Thus it sends a `pong` and an
`accept_teleportation` only between a `client_tick_end` and the first tick packet of the next client
tick. See experiment E6.

**MVC-PKT-011** (`source_suggested`, client agreement, confidence: medium). A legit client sends no
move packet and no `player_input` before it has loaded the world. It sends `client_tick_end` at the
end of each client tick, also before it has loaded the world.

After the load, a legit client sends `player_input` in each client tick in which the keys change.
It also does this while it rides, and while its camera is another entity. While it rides, it sends
one `move_player_rot` in each client tick, and no other move packet. While its camera is another
entity, it sends no move packet. See experiment E7.

### 1.2 Clientbound packets that change the prediction

**MVC-PKT-020** (`source_suggested`, wire, confidence: high). `player_position` has these fields, in
this order:

| Field | Encoding |
|-------|----------|
| `teleport_id` | VarInt |
| `x`, `y`, `z` | three big-endian 64-bit floats |
| `vx`, `vy`, `vz` | three big-endian 64-bit floats, the velocity |
| `yaw`, `pitch` | two big-endian 32-bit floats |
| `relative` | big-endian 32-bit integer |

Each bit of `relative` makes one value relative to the current value of the client:

| Bit | Value |
|-----|-------|
| 0, 1, 2 | `x`, `y`, `z` |
| 3, 4 | `yaw`, `pitch` |
| 5, 6, 7 | `vx`, `vy`, `vz` |
| 8 | rotate the velocity by the change of the rotation |

The client applies the packet and then sends an accept with its new position. It does not send a
move packet for the teleport.

**MVC-PKT-021** (`source_suggested`, wire, confidence: high). `set_entity_motion` has an entity id
(VarInt) and a velocity in the low-precision vector format of MVC-PKT-022. When the entity id is the
id of the player, the client sets its velocity to the decoded value.

**MVC-PKT-022** (`source_suggested`, wire, confidence: high). The low-precision vector format has
these parts:

1. One unsigned byte `b0`. When `b0` is 0, the vector is `(0, 0, 0)` and the format ends.
2. One unsigned byte `b1`, and one big-endian unsigned 32-bit integer `w`.
3. The 48-bit value `v = (w << 16) | (b1 << 8) | b0`.
4. The scale `s = b0 & 3`. When `b0 & 4` is not 0, a VarInt `e` follows, read as unsigned 32-bit,
   and `s = s | (e << 2)`.
5. Each component is `s × (min(c, 32766) × 2 / 32766 − 1)`, where `c` is 15 bits of `v`.
   Bits 3 to 17 give `x`. Bits 18 to 32 give `y`. Bits 33 to 47 give `z`.

The encoder sets a NaN component to 0 and clamps each component to ±1.7179869183E10. When the
largest absolute component `m` is less than 3.051944088384301E-5, it writes only `b0 = 0`. Otherwise
`s` is the ceiling of `m`, and each `c` is `round((component / s × 0.5 + 0.5) × 32766)`.

**MVC-PKT-023** (`source_suggested`, wire, confidence: high). `explode` has these fields first: the
center (three 64-bit floats), the radius (32-bit float), the block count (32-bit integer), and an
optional player impulse. The optional impulse is a boolean and, when it is true, three 64-bit
floats. Other fields follow, and this document does not use them. The client adds the impulse to its
velocity (PHY-SRV-007).

**MVC-PKT-024** (`source_suggested`, wire, confidence: high). The clientbound `player_abilities` has
one byte of flags and two 32-bit floats. The flags are 0x01 invulnerable, 0x02 flies, 0x04 fly
permission, 0x08 instant build. The floats are the flight speed and the walk speed. The client
replaces all of these values.

**MVC-PKT-025** (`source_suggested`, wire, confidence: high). `update_mob_effect` has these fields,
in this order: entity id (VarInt), effect (VarInt registry id of `minecraft:mob_effect`), amplifier
(VarInt), duration in ticks (VarInt), flags (byte). `remove_mob_effect` has the entity id (VarInt)
and the effect (VarInt).

**MVC-PKT-026** (`source_suggested`, wire, confidence: high). `update_attributes` has the entity id
(VarInt) and a list of at most 128 entries. The list starts with its length as a VarInt. Each entry
has these fields:

1. The attribute (VarInt registry id of `minecraft:attribute`).
2. The base value (64-bit float).
3. A list of modifiers, with its length as a VarInt. Each modifier has an id (identifier string), an
amount (64-bit float) and an operation (VarInt: 0 add, 1 multiply the base, 2 multiply the total).

For each entry, the client sets the base value, removes all modifiers, and adds the modifiers of the
entry. This also replaces a sprint modifier that the client added itself.

**MVC-PKT-027** (`source_suggested`, wire, confidence: high). `block_update` has a block position
(64-bit integer) and a block state id (VarInt). `section_blocks_update` has a section position
(64-bit integer), a count (VarInt), and for each block one VarLong. The VarLong is the state id
shifted left by 12 bits, with the position in the section in the low 12 bits. `block_changed_ack`
has a sequence (VarInt).

**MVC-PKT-028** (`source_suggested`, wire, confidence: high). `ping` has one field, the id, as a
big-endian signed 32-bit integer.

**MVC-PKT-029** (`source_suggested`, wire, confidence: high). `player_rotation` has these fields, in
this order: yaw (32-bit float), yaw is relative (boolean), pitch (32-bit float), pitch is relative
(boolean).

**MVC-PKT-030** (`design`, semantic, confidence: medium). These packets are predicted-state changes
when they apply to the player or to its world view:

- `set_entity_motion`, `explode` with an impulse;
- `player_abilities`, `update_mob_effect`, `remove_mob_effect`, `update_attributes`;
- `block_update`, `section_blocks_update`, `block_changed_ack`;
- `level_chunk_with_light`, `forget_level_chunk`;
- the packets that change the world border;
- `ticking_state` and `ticking_step`.

`player_position` and `respawn` are not in this list. The teleport flow handles them (MVC-LAT-008).
`player_rotation` is not in this list, because the next move packet reports the rotation.

## 2. The checks of a vanilla server

The rules of this section state what a vanilla server does. This server replaces some of them.
Section 12 lists each difference.

**MVC-VAN-001** (`source_suggested`, semantic, confidence: high). The server disconnects the client
with the translation key `multiplayer.disconnect.invalid_player_movement` when a move packet or an
accept has one of these values:

- a NaN in `x`, `y` or `z`;
- a NaN or an infinite value in `yaw` or `pitch`.

An infinite position is not in this list. The server checks an accept only when its id is the
newest id. See experiment E8.

**MVC-VAN-002** (`source_suggested`, semantic, confidence: high). Before other checks, the server
clamps `x` and `z` to the range from −3.0E7 to 3.0E7, and `y` to the range from −2.0E7 to 2.0E7. It
wraps `yaw` and `pitch` to the range from −180 to 180 degrees. It does not clamp `pitch` to ±90. See
experiment E8.

**MVC-VAN-003** (`source_suggested`, semantic, confidence: high). When the server gets a second
position packet before a `client_tick_end`, it disconnects the client with the translation key
`multiplayer.disconnect.invalid_player_movement`. A `client_tick_end` starts the count again. An
accept is not a position packet. The rule applies also while a teleport is pending. See experiment
E9.

**MVC-VAN-004** (`source_suggested`, semantic, confidence: medium). The server ignores a move packet
until the client has loaded. BRK-REQ-005 states when the client has loaded. The server also ignores
a move packet while the player sees the end credits. See experiment E21.

**MVC-VAN-005** (`source_suggested`, semantic, confidence: medium). While a teleport is pending, the
server does not apply the position of a move packet (TP-CONFIRM-MOV-001). It applies the rotation of
the move packet. See experiment E10.

**MVC-VAN-006** (`source_suggested`, semantic, confidence: medium). While the player rides, the
server applies only the rotation of a move packet. See experiment E22.

**MVC-VAN-007** (`source_suggested`, semantic, confidence: medium). While the player sleeps, the
server does not apply a move packet. When the squared distance from the position of the player to
the reported position is more than 1.0, the server sends a correction. See experiment E23.

**MVC-VAN-008** (`source_suggested`, semantic, confidence: medium). The server counts the move
packets and the accepts that it gets in one server tick. Let `n` be this count. When `n` is more
than 5, the server uses `n = 1`. For each move packet, the server computes these values:

- `d²`: the squared distance from the position of the player at the start of the server tick to the
  reported position;
- `v²`: the squared length of the velocity of the copy of the player on the server.

The server does not take this velocity from the client. Only these events change it:

1. At the start of each server tick of the player, the server multiplies the velocity by 0.98
   (PHY-GRV-009).
2. In the same server tick, the server applies the block effects to its copy (PHY-SRV-004). It
   does not change the fall distance there.
3. After each move of MVC-VAN-010, the server multiplies the horizontal velocity by the speed
   factor (PHY-GRV-012).
4. The jump of the copy in MVC-VAN-010 sets the vertical velocity to the jump power (PHY-INP-005).
   A jump with the sprint state on also adds the horizontal boost of PHY-INP-005.
5. An external impulse that the server applies, for example knockback or an explosion, changes the
   velocity (PHY-SRV-007).
6. A correction sets the velocity to zero.

A move of MVC-VAN-010 does not set a collided axis of the velocity to zero. The server does not
simulate the motion of the player (PHY-RST-001).

When `d² − v²` is more than `100 × n`, the server sends a correction to the current position of the
player. The limit is `300 × n` while the player glides with an elytra. See experiment E11.

**MVC-VAN-009** (`source_suggested`, semantic, confidence: medium). MVC-VAN-008 does not apply in
these conditions:

- The player changes the dimension, from the `respawn` packet until the accept.
- The game rule `player_movement_check` is false.
- The player glides with an elytra, and the game rule `elytra_movement_check` is false.
- The tick rate of the server is frozen, and the server does not step.

See experiment E11.

**MVC-VAN-010** (`source_suggested`, semantic, confidence: medium). Next, the server moves the box
of the player from the last accepted position by the reported change (PHY-SRV-003). This move has
the collision and the step-up (PHY-STP-001). It also has the edge back-off with the shift key of the
last `player_input` (PHY-INP-012).

Before this move, the copy of the player on the server can jump (PHY-INP-005). It jumps when its
on-ground flag is true, the packet has no on-ground bit, and the reported change goes up.

Let `r` be the reported position minus the result. The server always ignores the vertical part of
`r`, for each size of it. The move is wrong when the squared horizontal length of `r` is more than
0.0625. Thus a vertical difference alone never makes a move wrong. See experiment E12.

**MVC-VAN-011** (`source_suggested`, semantic, confidence: medium). The server does not count a
wrong move in these conditions:

- The player changes the dimension.
- The player sleeps.
- The player is in creative mode or spectator mode.
- The player is in the grace time after an impulse. The grace time is 40 server ticks after the
  explosion of a wind charge or a smash attack with a mace. It is 10 server ticks after an impulse
  from an enchantment effect. The explosion of another source ends the grace time.

See experiment E12.

**MVC-VAN-012** (`source_suggested`, semantic, confidence: medium). The server refuses the move
packet when one of these conditions is true:

- The move is wrong and counted, and the box at the last accepted position touches no collision
  shape.
- The box at the reported position touches a collision shape that the box at the last accepted
  position does not touch. Both boxes are smaller by 1.0E-5 on each side for this test.

A player in spectator mode passes this rule. For a refusal, the server sends a correction to the
current position of the player with zero velocity and the reported rotation. Otherwise the server
sets the position of the player to the reported position. See experiment E12.

**MVC-VAN-013** (`source_suggested`, semantic, confidence: low). After it accepts a move packet, the
server marks the player as afloat when all of these conditions are true:

1. The previous move of the player on the server found no collision below the box.
2. The player is not in spectator mode, and the server property `allow-flight` is false.
3. The player does not have the fly permission and has no `minecraft:levitation` effect.
4. The player does not glide and does not do a riptide attack.
5. Each block in the box, grown by 0.0625 on each side and by 0.55 down, is air.

The vertical direction of the move has no effect on this status. A refused move does not change it.
See experiment E13.

**MVC-VAN-014** (`source_suggested`, semantic, confidence: low). At each server tick, when the
player is afloat and does not sleep, ride or die, the server adds 1 to a count. Otherwise the count
is 0. When the count is more than `M`, the server disconnects the client with the translation key
`multiplayer.disconnect.flying`. A teleport by the server sets the count to 0. See experiment E13.

The limit is `M = ceil(80 × max(0.08 / g, 1))`. Here `g` is the gravity of the player: 0 with the
`NoGravity` flag, else the value of the attribute `minecraft:gravity`. At the default gravity, `M`
is 80. When `g` is less than 1.0E-5, there is no limit.

**MVC-VAN-015** (`source_suggested`, semantic, confidence: low). A move packet can have the
on-ground bit when the previous move on the server found no collision below the box. Then the server
sends the blocks below the box again, after the move of MVC-VAN-010. These are the blocks at the
four bottom corners of the box, 1.0E-5 below it, with their block entity data. The server does this
at most one time in 200 server ticks. See experiment E14.

**MVC-VAN-016** (`source_suggested`, semantic, confidence: medium). The server takes the on-ground
bit as true for the fall distance and the fall damage. PHY-FALL-003 states this. A move up sets the
fall distance to 0. See experiment E24.

**MVC-VAN-017** (`source_suggested`, semantic, confidence: medium). The server applies
`player_command` actions 1 and 2 without a check. For `player_abilities`, the player flies when the
request is to fly and the player has the fly permission. The server takes the shift key of
`player_input` as the shift key state of the player. See experiment E25.

**MVC-VAN-018** (`source_suggested`, semantic, confidence: medium). For an accept with the newest
id, the server sets the position to the position of the teleport. Then it handles the position of
the accept as a move packet with both flags false. TP-CONFIRM-ACC-003 and TP-CONFIRM-ACC-004 show
the result. See experiment E26.

**MVC-VAN-019** (`source_suggested`, semantic, confidence: medium). When the server property
`rate-limit` is more than 0, the server counts all packets of the connection in each second. It
keeps an average in f32: `a = count + 0.75 × (a − count)`. When `a` is more than the property, the
server disconnects the client with the translation key `disconnect.exceeded_packet_rate`. The
default of the property is 0, which disables the rule. See experiment E15.

## 3. The count of client ticks

**MVC-TICK-001** (`design`, semantic, confidence: high). The server does one prediction step for
each `client_tick_end`. The step uses the tick packets before the `client_tick_end` and after the
previous one. The server does not do a step for another packet.

**MVC-TICK-002** (`design`, semantic, confidence: low). An answer can arrive after `client_tick_end`
number N − 1 and before `client_tick_end` number N. This answer acknowledges its probe for client
tick N and each later tick. The reason is MVC-PKT-010: the client applies the clientbound packets
before the client tick.

**MVC-TICK-003** (`design`, semantic, confidence: medium). The server keeps MVC-VAN-003. More than
one move packet of any kind in one client tick is a violation of the packet check (MVC-CHK-020).

**MVC-TICK-004** (`design`, semantic, confidence: medium). The server counts the silent ticks after
each position packet. When the count reaches 20, the client tick is a violation of the packet check.
These client ticks do not count:

- a client tick before the client has loaded;
- a client tick while a teleport is pending;
- a client tick while the player rides or the camera of the client is another entity.

**MVC-TICK-005** (`design`, semantic, confidence: medium). The tick rate check limits the number of
client ticks to the time that has passed. MVC-CHK-010 states it.

## 4. The prediction

### 4.1 State and step

**MVC-PRED-001** (`design`, semantic, confidence: medium). For each player, the server keeps the
accepted state, the reference position and the reported flags. After the accept of a teleport, the
accepted state has one candidate at the destination of the teleport. MVC-LAT-008 states this
candidate and the test of the accept. The prediction starts from this candidate, and not from the
position in the accept.

**MVC-PRED-002** (`design`, semantic, confidence: medium). For a client tick in predicted mode, the
server does these steps:

1. Apply to each candidate the predicted-state changes that are acknowledged for this tick
   (MVC-LAT-004).
2. Make the inputs of the tick (section 4.2). Some inputs have more than one value.
3. For each candidate and each combination of input values, run one tick of the physics model
   (PHY-ORD-001). Use the world view of the client (MVC-LAT-006). Each result is a new candidate.
4. Compare the new candidates with the tick packets (section 4.3).
5. Keep the candidates that match as the new accepted state (MVC-PRED-009).

**MVC-PRED-003** (`design`, semantic, confidence: medium). The server enumerates the candidates in a
fixed order. The first key is the order of the accepted state. The second key puts the old value of
an input before the new value. The result of the step depends only on the tick packets, the
acknowledged changes and the world view.

**MVC-PRED-004** (`design`, semantic, confidence: medium). The candidate set has at most
`max_candidates` candidates. The default is 64. When a step makes more candidates, the server
classifies the client tick as coarse (MVC-MODE-003). This is not a violation.

### 4.2 Inputs

**MVC-PRED-005** (`design`, semantic, confidence: medium). The server knows these inputs of a client
tick exactly:

- the movement keys, the jump key, the shift key and the sprint key, from the last `player_input`;
- the yaw and the pitch, from the last move packet that has a rotation;
- the sprint state, from the last `player_command` with action 1 or 2;
- the flight state, from the last `player_abilities`;
- each predicted-state change that is outside its window.

The yaw and the pitch of a move packet are the values of the whole client tick. The client turns
only between client ticks, and it reports the rotation at the end of the tick.

The crouch state of a client tick comes from the shift key of the previous client tick
(PHY-INP-010, PHY-ORD-007). The shift key of this tick does not set it. Thus a `player_input` that
changes the shift key in client tick N changes the crouch state in client tick N + 1. The shift key
of this tick applies to the edge back-off (PHY-INP-012).

**MVC-PRED-006** (`design`, semantic, confidence: medium). The server enumerates two values of an
input in these conditions:

| Input | Condition | Values |
|-------|-----------|--------|
| sprint state | The tick has a `player_command` with action 1 or 2. | the old and the new state |
| flight state | The tick has a `player_abilities`. | the old and the new state |
| item use | The use of an item starts or ends in the tick, by the client or by the server. | used and not used |
| each predicted-state change | The tick is in its window. | without and with the change |

The client can stop flight after the physics step of the tick, when it lands (PHY-INP-015). Thus a
change of the flight state in a tick does not show which state the step used. The client changes the
sprint state before the travel step (PHY-ORD-001), but the server enumerates both values also for
the sprint state.

**MVC-PRED-007** (`design`, semantic, confidence: medium). When the physics model forbids the sprint
state for every candidate, the server predicts without sprint. The reported sprint state has no
effect on the step in this case. PHY-INP-008 and PHY-INP-009 state when sprint is possible. For each
candidate, the server evaluates the stop rule of PHY-INP-009 with the flags of the previous tick. A
horizontal collision stops sprint, except a minor collision of less than 8 degrees (PHY-FLG-003).

### 4.3 Comparison

**MVC-PRED-008** (`design`, semantic, confidence: medium). For a position packet, the offset of a
candidate is the distance to the predicted position minus the tolerance of the candidate. The offset
is never less than 0. The tolerance is 0, unless section 5 adds to it. The offset of the client tick
is the smallest offset of all new candidates.

**MVC-PRED-009** (`design`, semantic, confidence: medium). For a client tick with a position packet,
the server keeps each candidate whose offset is less than `offset_threshold`. The default is 0.001
blocks. When no candidate matches, the client tick is a violation of the offset check (MVC-CHK-001).
When the response sends no setback, the server keeps the first candidate with the smallest offset.
Then the server sets the position of each kept candidate to the reported position.

**MVC-PRED-010** (`design`, semantic, confidence: medium). For a silent tick, the server keeps each
candidate whose distance to the reference position is at most `2.0E-4 + offset_threshold`. When no
candidate matches, the client tick is a violation of the offset check. Its offset is the smallest
distance minus 2.0E-4. When the response sends no setback, the server keeps the first candidate with
the smallest distance. A silent tick does not change the position of a candidate.

A legit client in a silent tick is at its predicted position. The silent tick only shows that this
position is near the reference position. A client that hovers in the air must report a position, or
it fails this rule at the first silent tick.

**MVC-PRED-011** (`design`, semantic, confidence: medium). After MVC-PRED-009 or MVC-PRED-010, the
server compares the reported flags with the kept candidates. MVC-CHK-002 and MVC-CHK-021 state the
result. The reported flags do not select candidates.

**MVC-PRED-012** (`design`, semantic, confidence: medium). The server takes all server-side results
of the movement from the accepted state, and never from the tick packets alone. These results are:

- the position of the player;
- the fall distance and the fall damage;
- the effects of the blocks below the player;
- the chunks that the server sends;
- the statistics.

When the accepted state has more than one candidate, the server uses the first candidate.

### 4.4 Fast path

**MVC-PRED-013** (`design`, semantic, confidence: medium). The server runs the physics step for the
candidates of a client tick in this try order:

1. The first candidate of the accepted state, with the primary value of each input. This is the
   primary candidate.
2. The candidates that differ from the primary candidate in one input. Then the candidates that
   differ in two inputs, and so on.
3. The same candidates for each other candidate of the accepted state, in the order of MVC-PRED-003.
4. The candidates of MVC-LAT-007 last.

In each group, the order of MVC-PRED-003 applies. The primary value of each input is:

| Input | Primary value |
|-------|---------------|
| a predicted-state change in its window | with the change |
| a change of the sprint state in the tick | the new state |
| a change of the flight state to on in the tick | the new state |
| a change of the flight state to off in the tick | the old state |
| the start or the end of an item use in the tick | the state after the tick packets |
| the world view with uncertain blocks | the new states of the blocks |

A legit client usually gets a change and both of its probes in one frame. Thus it usually applies
the change in the first tick of the window. A flight stops after the travel step when the player
lands (PHY-INP-015). Thus the old state is the primary value for a flight that stops.

**MVC-PRED-014** (`design`, semantic, confidence: medium). The server runs the physics step for one
candidate at a time, in the try order. It stops at the first candidate that matches the position
(MVC-PRED-009 or MVC-PRED-010) and the reported flags (MVC-PRED-011). This candidate is the new
accepted state. The server does not run the step for the other candidates.

When no candidate matches the position and the flags, the server runs the step for all candidates.
Then MVC-PRED-009 to MVC-PRED-011 apply to the full candidate set.

A client tick without a window and without a change of an input has one candidate. Thus a legit
client costs one physics step in almost each client tick.

**MVC-PRED-015** (`design`, semantic, confidence: medium). The server records the last
`replay_depth` client ticks. The default is 20. For each tick, the record has the start state of the
accepted candidate, the inputs and the acknowledged changes.

When MVC-PRED-014 finds no match in the full candidate set of a tick, the server replays the
records. It starts at the oldest recorded tick in which the fast path did not run a candidate. For
each recorded tick, it makes the full candidate set (MVC-PRED-002) and compares it with the tick
packets of that tick. When each recorded tick has a match, the current tick is not a violation.

Before a tick with an open window leaves the record, the server adds both candidates of that window
to the accepted state. Thus a replay never needs a tick that is older than the record.

**MVC-PRED-016** (`design`, semantic, confidence: low). In one client tick, the number of candidates
is `K × 2^b + s`. `K` is the number of candidates of the accepted state. `b` is the number of
inputs with two values (MVC-PRED-006, MVC-LAT-006). `s` is the number of candidates of MVC-LAT-007.
The client can make `b` large, because it controls the time of its answers.

The cap is `max_steps_per_tick` physics steps in one client tick, replays included. The default is
64. When a tick reaches the cap, the server does one of these steps:

- When more than `max_open_windows` windows are open, send a setback (MVC-RSP-001). The default is
  4. A legit client keeps a window open only between two frames.
- Else, classify the tick as coarse (MVC-MODE-003). This is not a violation.

**MVC-PRED-017** (`design`, semantic, confidence: medium). A replayed tick uses the world view that
the client can have in that tick. It does not use the current world view. For each block change, the
state of the change for that tick (MVC-LAT-004) selects the block state:

| State of the change for the replayed tick | Block state in the replay |
|-------------------------------------------|---------------------------|
| not applied | the old state |
| window | both states, as MVC-LAT-006 states |
| applied | the new state |

The server makes this world view from the current world view and the kept block changes of
MVC-PRED-018. It undoes the kept changes from the newest to the oldest, until each change has the
state of the replayed tick. The same rule applies to the load and the unload of a chunk, and to the
world border.

**MVC-PRED-018** (`design`, semantic, confidence: medium). For each player, the server keeps each
block change that it sends to that player. A kept change has the block position, the old state, the
new state, and the probes before and after the change. The server keeps the change until
`replay_depth` client ticks have passed after the first client tick in which the change is applied.
Then no record of MVC-PRED-015 can need the old state.

The server can drop a kept change when no box of a recorded tick, grown by the near zone of
MVC-MODE-003, can touch the block. The server can share the block states of a change between
players. The probes of a change are different for each player.

## 5. Sources of uncertainty

**MVC-UNC-001** (`design`, semantic, confidence: medium). Each source of uncertainty has one
treatment. The treatment is enumeration (MVC-PRED-006), a tolerance (MVC-PRED-008), or a mode
(section 8).

| Source | Treatment | Rule |
|--------|-----------|------|
| The arithmetic of the server is not equal to the arithmetic of the client. | `offset_threshold` | MVC-PRED-009 |
| A predicted-state change is in its window. | enumeration | MVC-LAT-004 |
| The sprint state or the flight state changes in the tick. | enumeration | MVC-PRED-006 |
| The use of an item starts or ends. | enumeration | MVC-PRED-006 |
| The tick is silent. | exact step and a test of the distance | MVC-PRED-010 |
| A block of the world view is in a window. | enumeration of two world views | MVC-LAT-006 |
| The client moves itself back when its prediction of a block fails. | an extra candidate | MVC-LAT-007 |
| The velocity of `set_entity_motion` loses precision. | exact decoded value | MVC-LAT-009 |
| An entity with a collision shape is near the box. | coarse mode | MVC-MODE-003 |
| A state that the physics model does not cover. | coarse mode | MVC-MODE-003 |
| The world border moves and is near the box. | coarse mode | MVC-MODE-003 |

**MVC-UNC-002** (`design`, semantic, confidence: medium). A tolerance never carries to the next
client tick. A source that the table does not list adds no tolerance.

**MVC-UNC-003** (`design`, semantic, confidence: low). The arithmetic tolerance covers only the
error of the round of each operation. PHY-NUM-001 to PHY-NUM-006 state the precision of each
operation, for example a 32-bit float or a 64-bit float. The server computes with that precision.

## 6. Latency compensation

**MVC-LAT-001** (`design`, semantic, confidence: medium). The server sends a probe before and after
each group of predicted-state changes. A group is the predicted-state changes for one player in one
flush of the connection. When no predicted-state change is between two groups, one probe can be the
probe after the first group and the probe before the second group.

**MVC-LAT-002** (`design`, semantic, confidence: medium). The server also sends one probe at the end
of each server tick, unless it sent a probe after the last packet of the tick. The tick rate check
uses these probes (MVC-CHK-010).

**MVC-LAT-003** (`design`, semantic, confidence: medium). The id of a probe is unique among the
probes without an answer. The server records the time at which it sends each probe.

**MVC-LAT-004** (`design`, semantic, confidence: medium). For a predicted-state change and a client
tick, the change has one of three states:

| State | Condition | Prediction |
|-------|-----------|------------|
| Not applied | The probe before the change is not acknowledged for the tick. | without the change |
| Window | The probe before is acknowledged, and the probe after is not. | without and with the change |
| Applied | The probe after the change is acknowledged for the tick. | with the change |

The client applies the change at the start of one client tick in the window, or at the start of the
first applied tick. A candidate that has not applied the change applies it at the start of the
first applied tick.

**MVC-LAT-005** (`design`, semantic, confidence: medium). A change applies one time to each
candidate. A velocity change and an impulse apply to the candidate state at that moment. A change of
a value replaces the old value: abilities, effects, attributes and blocks. When two changes are in a
window at one time, the client applies them in the order of the server. A candidate never applies a
later change before an earlier change.

**MVC-LAT-006** (`design`, semantic, confidence: medium). Grow the box of a candidate by the
movement of the tick. When that box touches uncertain blocks, the server runs the physics step two
times. The first run uses the old state of each of these blocks. The second run uses the new state
of each. When the box touches more than one uncertain block, and neither world view matches, the
server classifies the tick as coarse.

**MVC-LAT-007** (`source_suggested`, client agreement, confidence: low). When the client breaks or
places a block with a sequence, it changes the block immediately. Then it waits for a
`block_changed_ack` with this sequence or a later sequence. Until then, it keeps each server state
of `block_update` for that position and does not apply it. At the `block_changed_ack`, the client
applies the kept state.

When the box of the client then touches the kept state, the client moves itself to its position at
the time of the request. The client does not move itself when a teleport arrived after the request.
See experiment E16.

The server treats this as a predicted-state change of the block. Its window starts at the request
and ends when the probe after the `block_changed_ack` is acknowledged. In the window, the server
adds one candidate at the position of the client at the time of the request.

**MVC-LAT-008** (`design`, semantic, confidence: medium). A teleport is not in the scheme of this
section. The accept is its acknowledgement. After the accept, the accepted state has one candidate.
Its position is the destination of the teleport, as PHY-SRV-010 computes it. Its velocity is the
velocity of the teleport, as PHY-SRV-010 computes it.

For a relative field, the server computes the destination from each candidate before the teleport.
The accept matches when the distance from its position to one destination is less than
`offset_threshold`. Then the server keeps the candidate of that destination.

When the accept does not match, it is a violation of the offset check. The server does not add the
offset to the offset buffer. It sends a correction to the destination of the first candidate. The
position in the accept never becomes the accepted position. A respawn starts the same way from its
first teleport.

**MVC-LAT-009** (`design`, semantic, confidence: medium). For `set_entity_motion`, the server uses
the velocity that the client decodes from the packet, not the velocity before the encode.
MVC-PKT-022 states the decode.

**MVC-LAT-010** (`design`, semantic, confidence: medium). For `update_attributes`, the server uses
the base value and the modifiers of the packet as the client applies them. This includes the
presence or the absence of a sprint modifier. The client changes the sprint modifier again only when
its sprint state changes.

**MVC-LAT-011** (`design`, semantic, confidence: medium). These answers are violations of the packet
check:

- An answer with an id that is not the id of a probe without an answer. The server ignores it.
- An answer that is not for the oldest probe without an answer. The server takes each older probe
  as acknowledged at the same time.

**MVC-LAT-012** (`design`, semantic, confidence: medium). When a probe has no answer for
`max_ack_delay`, the server disconnects the client. The default is 60 seconds.

**MVC-LAT-013** (`design`, semantic, confidence: medium). The server uses no grace time after an
external impulse (PHY-SRV-007). The window of MVC-LAT-004 covers each `set_entity_motion` and each
`explode` with an impulse. Thus the prediction knows the ticks in which the client can apply the
impulse.

## 7. The checks of this server

### 7.1 Offset

**MVC-CHK-001** (`design`, semantic, confidence: medium). The offset check fails a client tick in
predicted mode when MVC-PRED-009 or MVC-PRED-010 finds no candidate that matches. MVC-RSP-004 states
the response.

For a log line, the server can name the part of the offset that is largest:

| Part | Name |
|------|------|
| horizontal | speed |
| vertical, up, while the best candidate is on the ground and has no jump | step height |
| vertical, while the best candidate is in the air | gravity |
| any, in a silent tick | hover |

These names are for logs only. Each part has the same response.

These are not violations of the offset check:

- an offset less than `offset_threshold`;
- a client tick in coarse mode, resync mode or exempt mode;
- a client tick while a teleport is pending.

### 7.2 On-ground status and fall distance

**MVC-CHK-002** (`design`, semantic, confidence: medium). The on-ground check fails a client tick in
predicted mode when no kept candidate has an on-ground flag equal to the reported on-ground bit. The
server uses the on-ground flag of the first candidate for each server-side result. It never uses the
on-ground bit of the client.

The violation level of this check rises by 1 for each violation. The level decays by 0.01 for each
client tick without a violation. The check sends no setback by default, because the server already
corrects the value.

These are not violations of the on-ground check:

- a client tick in coarse mode, resync mode or exempt mode;
- the first client tick after an accept;
- a client tick in which a kept candidate has an on-ground flag equal to the reported bit.

**MVC-CHK-003** (`design`, semantic, confidence: medium). The server computes the fall distance of
the player from the accepted state with PHY-FALL-001, as the client does. The server does not use
PHY-FALL-003. A client that reports the on-ground bit in the air does not set the fall distance to
0. The fall damage applies at the client tick in which the first kept candidate lands.

**MVC-CHK-004** (`design`, semantic, confidence: medium). A setback keeps the fall distance of the
accepted state. In coarse mode, the server computes the fall distance from the accepted positions.
It takes the on-ground flag from the collision of the box below the accepted position.

### 7.3 Collision

**MVC-CHK-005** (`design`, semantic, confidence: medium). The collision check applies in predicted
mode and in coarse mode. A client tick fails when the new box touches a settled shape that the old
box does not touch. The new box is the box at the new accepted position. The old box is the box at
the previous accepted position.

For this test, the server shrinks both boxes by 1.0E-5 on each side. For a failed tick, the server
refuses the client tick and sends a setback immediately (MVC-RSP-001).

These are not violations of the collision check:

- a shape that is not settled;
- a shape that the box at the previous accepted position touches;
- the box at the destination of a teleport;
- the position to which the client moves itself in MVC-LAT-007;
- a client tick of a player in spectator mode.

### 7.4 Tick rate

**MVC-CHK-010** (`design`, semantic, confidence: medium). The server keeps a balance `B` in time
units for each player. Let `D` be the duration of one client tick: 1000 ms divided by the tick rate
that the client has acknowledged. Let `C` be the send time of the newest acknowledged probe. For
each `client_tick_end`, the server does these steps:

1. Add `D` to `B`.
2. When `B` is more than the current time, the client tick is a violation of the tick rate check.
   Subtract `D` from `B`.
3. Set `B` to the larger of `B` and `C − tick_drift`. The default of `tick_drift` is 120 ms.

At the start, `B` is the time at which the server sends the first probe minus `tick_drift`.

**MVC-CHK-011** (`design`, semantic, confidence: low). A violation of the tick rate check refuses
the client tick. The server does no prediction step for it, and it sends a setback when the level of
the check is more than `tick_rate_setback_level`. The level rises by 1 for each violation, and it
decays by 0.005 for each client tick without a violation. The default of `tick_rate_setback_level`
is 10.

A client can only send a client tick after it has received the probe of `C`. Thus a legit client
cannot have more ticks than the time after `C` allows, plus a part of one tick. A slow drift between
the clocks of the client and the server causes at most a rare violation, and the decay removes it.

**MVC-CHK-012** (`design`, semantic, confidence: medium). The tick rate check does not apply while
the tick rate that the client has acknowledged is frozen. It does not apply to the client ticks of a
`ticking_step`.

### 7.5 Packets

**MVC-CHK-020** (`design`, semantic, confidence: medium). The packet check fails for each of these
packets:

| Packet | Condition | Server action |
|--------|-----------|---------------|
| any serverbound packet | a test of MVC-CHK-023 fails | MVC-CHK-023 |
| a move packet | a second move packet in one client tick | ignore it, unless MVC-VAN-003 disconnects |
| a position packet | a distance to the reference position of at most 2.0E-4, with fewer than 19 silent ticks before it | none |
| `client_tick_end` | the 20th silent tick (MVC-TICK-004) | none |
| `player_input` | a second `player_input` in one client tick | none |
| `player_input` | the same keys as the last `player_input` | none |
| `player_command` | action 1 while the sprint state is on, or action 2 while it is off | none |
| `player_abilities` | a request to fly when the fly permission is off in each state that the client can have (MVC-LAT-004) | ignore the request |
| `pong` | MVC-LAT-011 | MVC-LAT-011 |

The test of the reference position does not apply to the first position packet after a teleport or
a respawn. The level of this check rises by 1 for each violation, and it decays by 0.01 for each
client tick without a violation. The check sends no setback by default.

**MVC-CHK-021** (`design`, semantic, confidence: medium). Compare the reported horizontal collision
bit with the horizontal collision flag of each kept candidate. When no flag is equal to the bit, the
client tick is a violation of the packet check. The server uses the horizontal collision flag of the
first candidate.

**MVC-CHK-023** (`design`, semantic, confidence: high). This check reads one serverbound packet and
no other state. It fails in these conditions:

| Packet | Condition | Response |
|--------|-----------|----------|
| a move packet or an accept | `x`, `y` or `z` is NaN or infinite | disconnect |
| a move packet or an accept | `yaw` or `pitch` is NaN or infinite | disconnect |
| a move packet or an accept | the absolute value of `pitch` is more than 90 | report; the server clamps `pitch` to ±90 |
| a move packet or an accept | the absolute value of `x` or `z` is more than 3.0E7, or the absolute value of `y` is more than 2.0E7 | report; the server clamps as MVC-VAN-002 states |
| a move packet | a bit of `flags` other than 0x01 and 0x02 | report |
| `player_input` | bit 0x80 | report |
| `player_abilities` | a bit other than 0x02 | report |
| `player_command` | an action outside 0 to 6 | disconnect |
| any serverbound packet | bytes after the last field | disconnect |

The size of each packet after its id is fixed, or it has a small range:

| Packet | Size in bytes |
|--------|---------------|
| `move_player_pos` | 25 |
| `move_player_pos_rot` | 33 |
| `move_player_rot` | 9 |
| `move_player_status_only` | 1 |
| `player_input`, `player_abilities` | 1 |
| `client_tick_end`, `player_loaded` | 0 |
| `pong` | 4 |
| `accept_teleportation` | 33 to 37 |
| `player_command` | 3 to 15 |

A packet of another size fails the check, and the response is a disconnect. A legit client clamps
`x` and `z` to ±2.9999999E7 (PHY-ORD-001, step 16), and it clamps `pitch` to ±90.

**MVC-CHK-024** (`design`, semantic, confidence: low). The L1 speed bound uses no world. Let `p` and
`q` be two positions that the client reports one after the other. Let `k` be the number of client
ticks between them. A position comes from a position packet or from an accept. The bound fails when
the distance from `p` to `q` is more than `k × (L + V)`:

- `L` is the larger of `l1_speed_limit` and `100 × s`. `s` is the largest of the values
  `movement_speed` and the flight speed that the client can have (MVC-LAT-004). The default of
  `l1_speed_limit` is 10 blocks.
- After a `player_command` with action 6, `L` is `l1_elytra_limit` until the next move packet with
  the on-ground bit. The default is 17.4 blocks.
- `V` is the largest length of a velocity of `set_entity_motion` or of an impulse of `explode`. The
  bound uses each such velocity that is in its window or that the client applied in the last 20
  client ticks.

The bound does not apply in these conditions:

- The pair has a pending teleport between its positions.
- The player rides.
- The camera of the client is another entity.

On a server, a failure is a report to L2. On a proxy without L2, a failure is a disconnect.

### 7.6 Sprint

**MVC-CHK-022** (`design`, semantic, confidence: medium). When MVC-PRED-007 applies, and the
reported sprint state is on, the client tick is a violation of the sprint check. The prediction does
not use sprint. The level of this check rises by 1 for each violation, and it decays by 0.01 for
each client tick without a violation. The check sends no setback by default. The offset check
catches a speed that comes from a forbidden sprint.

## 8. Modes of a client tick

**MVC-MODE-001** (`design`, semantic, confidence: high). Each client tick has exactly one mode:

| Mode | Checks | Accepted state after the tick |
|------|--------|-------------------------------|
| predicted | all checks | MVC-PRED-009 or MVC-PRED-010 |
| coarse | collision, coarse speed, tick rate, packet | the reported position, with an unknown velocity |
| resync | collision, coarse speed, tick rate, packet | the reported position, with the velocity of MVC-MODE-004 |
| exempt | tick rate, packet | no change |

**MVC-MODE-002** (`design`, semantic, confidence: medium). These states make a client tick exempt:

- The player is in spectator mode.
- The player rides.
- The player sleeps or is dead.
- The client has not loaded, or a teleport is pending.
- The camera of the client is another entity.
- The game rule `player_movement_check` is false.

When the game rule `player_movement_check` is false, the server does not send a setback for a
movement check.

**MVC-MODE-003** (`design`, semantic, confidence: medium). These triggers make a client tick coarse,
unless MVC-MODE-002 makes it exempt. The near zone is the box of a candidate, grown by the movement
of the tick. Then it is grown by 1 block on each side.

| Trigger | Reason |
|---------|--------|
| The near zone touches a fluid, a waterlogged block or a `minecraft:bubble_column` block. | the fluid model and the bubble column |
| A `minecraft:bubble_column` block is at most 2 blocks below the bottom of the box, and at most 1 block from the box horizontally. | a bubble column acts above it |
| A `minecraft:potent_sulfur` block is at most 25 blocks below the bottom of the box, and at most 1 block from the box horizontally. | its geyser acts up to 6 blocks above it for each of at most 4 water blocks |
| The near zone touches a `minecraft:nether_portal`, `minecraft:end_portal` or `minecraft:end_gateway` block. | a portal teleports |
| A `minecraft:moving_piston` block, a piston, or a block that a piston moves is at most 2 blocks from the box. | the push of a piston |
| The near zone touches a shulker box block. | its shape changes while it opens |
| The near zone touches a `minecraft:big_dripleaf` block. | its shape changes when it tilts |
| The near zone touches a block whose shape has a position offset (PHY-SHP-004). | the corpus does not give the offset shape |
| The near zone touches a block whose collision shape has a coordinate that is not a multiple of 1/16. | the corpus rounds such a coordinate |
| An entity that PHY-SHP-009 lists is at most 1 block from the box. | an entity collider that can move |
| An entity that another entity can push is at most 0.5 blocks from the box. | the push between entities |
| The player glides with an elytra, or does a riptide attack. | their own models |
| The near zone touches a chunk that is not loaded on the client, or a chunk whose load or unload is in a window. | PHY-GRV-020 |
| The world border moves, and the box is at most 1 block from it. | the border moves with real time |
| The near zone touches a block whose effect on movement the physics model does not state. Sections 6.4 to 6.7 and PHY-RST-003 of the physics model state the block effects that it covers. | an effect that the model does not cover |
| MVC-PRED-004, MVC-PRED-016 or MVC-LAT-006 selects coarse mode. | too many candidates, or uncertain blocks |

The test of 1/16 applies until the corpus gives exact shape values. A coordinate that is a multiple
of 1/16 is exact.

**MVC-MODE-007** (`design`, semantic, confidence: medium). Each source of PHY-ORD-008 has exactly
one treatment in this document:

| Source in PHY-ORD-008 | Treatment | Rule |
|-----------------------|-----------|------|
| water, lava and each other fluid | coarse | MVC-MODE-003, row 1 |
| `bubble_column` | coarse | MVC-MODE-003, rows 1 and 2 |
| `potent_sulfur` and its geyser | coarse | MVC-MODE-003, row 3 |
| `nether_portal`, `end_portal`, `end_gateway` | coarse near the portal, then exempt until the accept of the teleport | MVC-MODE-003, row 4, MVC-MODE-002, MVC-LAT-008 |
| a piston, a `moving_piston`, and blocks that a piston moves | coarse | MVC-MODE-003, row 5 |
| a shulker box that opens | coarse | MVC-MODE-003, row 6 |
| a shulker that opens | coarse | MVC-MODE-003, row 10 |
| `big_dripleaf` | coarse | MVC-MODE-003, row 7 |
| a block with a position offset, a `moving_piston`, a shulker box | coarse | MVC-MODE-003, rows 5, 6 and 8 |
| a block shape that the corpus rounds | coarse | MVC-MODE-003, row 9 |
| a ridden entity | exempt | MVC-MODE-002 |
| a boat or a minecart near the player | coarse | MVC-MODE-003, rows 10 and 11 |
| elytra flight, riptide | coarse | MVC-MODE-003, row 12 |
| an entity that pushes the player | coarse | MVC-MODE-003, row 11 |
| knockback, explosion, wind charge, fishing rod pull | enumeration in a window | MVC-LAT-004, MVC-LAT-009, MVC-LAT-013 |
| ender pearl, chorus fruit, a teleport command | exempt until the accept, then a new start | MVC-MODE-002, MVC-LAT-008 |
| a `happy_ghast` that moves | coarse | MVC-MODE-003, row 10 |
| a chunk that is not loaded on the client | coarse | MVC-MODE-003, row 13 |

**MVC-MODE-004** (`design`, semantic, confidence: low). A resync tick is the first client tick with
a position packet after a coarse tick or an exempt tick. A coarse tick or an exempt tick is not a
resync tick. An accept is also a resync point (MVC-LAT-008).

In a resync tick, the server accepts the reported position when the coarse checks pass. It computes
the velocity at the end of the tick from the reported change of position. It uses PHY-RST-001 to
PHY-RST-004 for the collided axes, and the drag of PHY-ORD-004. The next client tick is in predicted
mode.

**MVC-MODE-005** (`design`, semantic, confidence: medium). The coarse speed check is MVC-VAN-008
with `n = 1` and with the velocity of the last accepted state. In coarse mode, the velocity is the
change of position of the last client tick. The limit is `coarse_speed_limit`, 100 by default, or
`coarse_elytra_limit`, 300 by default, while the player glides. When the game rule
`elytra_movement_check` is false, the check does not apply while the player glides. A violation
refuses the client tick and sends a setback.

**MVC-MODE-006** (`design`, semantic, confidence: medium). A state of MVC-MODE-003 that is in a
window counts as present. Thus the server selects coarse mode when the client can be in the state.

## 9. Response

### 9.1 Setback

**MVC-RSP-001** (`design`, semantic, confidence: medium). A setback is a `player_position` with
these values:

- the position of the first candidate of the accepted state, as absolute values;
- the velocity of that candidate, as absolute values;
- `yaw` 0 and `pitch` 0, with both rotation bits of `relative` set. Thus the client keeps its
  rotation.

The setback uses the next `teleport_id` (TP-CONFIRM-ID-002). The velocity keeps a fall in progress,
so a setback does not stop a fall.

**MVC-RSP-002** (`design`, semantic, confidence: medium). The accepted state does not change in a
refused client tick. For a setback, the server uses the accepted state before the refused tick.

**MVC-RSP-003** (`design`, semantic, confidence: medium). While a setback is pending, these rules
apply:

- The server does not apply move packets (TP-CONFIRM-MOV-001).
- The server runs no prediction step and sends no other setback. It sends a resend as
  TP-CONFIRM-RES-002 states.
- The tick rate check continues.

**MVC-RSP-004** (`design`, semantic, confidence: low). The offset buffer starts at 0. For each
client tick in predicted mode, the server does one of these steps:

- For a violation of the offset check, add the offset to the buffer. Then send a setback when the
  buffer is at least `buffer_limit`, or the offset is at least `instant_setback_offset`. Then limit
  the buffer to `buffer_cap`.
- Without a violation, multiply the buffer by `buffer_decay`.

The defaults are `buffer_limit` 1.0, `instant_setback_offset` 0.1, `buffer_cap` 4.0 and
`buffer_decay` 0.999. Thus small offsets do not cause a setback until their sum reaches 1 block.

**MVC-RSP-005** (`design`, semantic, confidence: high). A violation level never falls below 0. A
check whose level has a setback threshold sends a setback when the level is more than the threshold.
Section 10 gives the thresholds.

**MVC-RSP-006** (`design`, semantic, confidence: medium). This rule applies when the server refuses
a client tick with the on-ground bit, and the accepted state has no collision below the box. Then
the server sends the blocks below the reported box again, as MVC-VAN-015 states. The interval is
`support_resend_interval`, 200 server ticks by default.

**MVC-RSP-007** (`design`, semantic, confidence: medium). The server disconnects the client when it
sends more than `kick_setbacks` setbacks in `kick_window` without a client tick in predicted mode
that passes. The default of `kick_setbacks` is 0, which disables this rule. The translation key of
the disconnect is `multiplayer.disconnect.invalid_player_movement`.

**MVC-RSP-008** (`design`, unspecified, confidence: high). The server can log a violation. The
content and the format of a log line are unspecified.

## 10. Configuration

**MVC-CFG-001** (`design`, semantic, confidence: high). Each value in this table is configurable.
The default applies when the operator sets no value.

| Value | Default | Rule |
|-------|---------|------|
| `offset_threshold` | 0.001 blocks | MVC-PRED-009 |
| `instant_setback_offset` | 0.1 blocks | MVC-RSP-004 |
| `buffer_limit` | 1.0 blocks | MVC-RSP-004 |
| `buffer_cap` | 4.0 blocks | MVC-RSP-004 |
| `buffer_decay` | 0.999 for each client tick | MVC-RSP-004 |
| `max_candidates` | 64 | MVC-PRED-004 |
| `max_ack_delay` | 60 s | MVC-LAT-012 |
| `tick_drift` | 120 ms | MVC-CHK-010 |
| `tick_rate_setback_level` | 10 | MVC-CHK-011 |
| on-ground setback level | none | MVC-CHK-002 |
| packet setback level | none | MVC-CHK-020 |
| sprint setback level | none | MVC-CHK-022 |
| collision setback | immediately | MVC-CHK-005 |
| `coarse_speed_limit` | 100 (squared blocks) | MVC-MODE-005 |
| `coarse_elytra_limit` | 300 (squared blocks) | MVC-MODE-005 |
| `support_resend_interval` | 200 server ticks | MVC-RSP-006 |
| `kick_setbacks`, `kick_window` | 0 (off), 60 s | MVC-RSP-007 |
| `replay_depth` | 20 client ticks | MVC-PRED-015 |
| `max_steps_per_tick` | 64 | MVC-PRED-016 |
| `max_open_windows` | 4 | MVC-PRED-016 |
| `l1_speed_limit` | 10 blocks | MVC-CHK-024 |
| `l1_elytra_limit` | 17.4 blocks | MVC-CHK-024 |

The value 2.0E-4 of MVC-PRED-010 and the count 20 of MVC-TICK-004 are constants of the client. They
are not configurable.

## 11. Invariants

**MVC-INV-001** (`design`, semantic, confidence: medium). A legit client in a state that predicted
mode covers never gets a setback, and a movement check never disconnects it. This is true for any
latency, any jitter, any frame rate and any time order of the packets that TCP delivers in order.

**MVC-INV-002** (`design`, semantic, confidence: medium). An accepted position never makes the box
touch a settled shape that the box at the previous accepted position does not touch.

**MVC-INV-003** (`design`, semantic, confidence: medium). For the same serverbound packets in the
same order, the same clientbound packets and the same world, all checks give the same result. The
tick rate check depends also on the arrival time of each packet. No check uses a random value.

**MVC-INV-004** (`design`, semantic, confidence: medium). Each client tick has exactly one mode of
section 8. A state that section 8 does not list does not change the mode.

**MVC-INV-005** (`design`, semantic, confidence: medium). In predicted mode, the sum of the offsets
that the server accepts before a setback is less than `buffer_limit` plus `instant_setback_offset`.
Thus a client gets at most this advantage before a setback.

**MVC-INV-006** (`design`, semantic, confidence: high). The server never does more prediction steps
than the client sends `client_tick_end` packets.

**MVC-INV-007** (`design`, semantic, confidence: medium). When a candidate applies a change for a
client tick, the change stays applied for each later client tick. A candidate never removes a change
that it applied.

**MVC-INV-008** (`design`, semantic, confidence: medium). The server does not disconnect a client
for a packet sequence that a vanilla server accepts, except for the deviations of section 12.

**MVC-INV-009** (`design`, semantic, confidence: medium). L0 and L1 never reject the packet stream
of a legit client. This is true for any latency, any jitter, any frame rate and any time order of
the packets that TCP delivers in order. Thus L0 and L1 never drop a packet of a legit client, and
they never disconnect it. A report from L0 or L1 never brings a violation level of a legit client to
a setback threshold or to a disconnect threshold.

## 12. Intentional deviations

These entries record where this server differs from a vanilla server.

**D1.**

- **Rule**: MVC-CHK-001 and MVC-MODE-005.
- **Reference**: the vanilla server applies MVC-VAN-008 to each move packet.
- **Visible to the client**: no, for a legit client. A client that moves too far gets a setback
  earlier.
- **Test**: a differential test does not compare the setbacks of a stream that a legit client cannot
  send.

**D2.**

- **Rule**: MVC-CHK-001 and MVC-CHK-005.
- **Reference**: the vanilla server applies MVC-VAN-010 to MVC-VAN-012. It always ignores the
  vertical part of the difference (MVC-VAN-010), so a wrong vertical move passes. This server checks
  the vertical part with the same tolerance as the horizontal part.
- **Visible to the client**: no, for a legit client. A client that moves wrongly up or down gets a
  setback.
- **Test**: as D1.

**D3.**

- **Rule**: MVC-PRED-010 and MVC-CHK-001.
- **Reference**: the vanilla server disconnects an afloat client after 80 server ticks (MVC-VAN-013
  and MVC-VAN-014). The direction of the move has no effect. Thus the vanilla server also
  disconnects a client that falls through air for more than 80 server ticks.
- **Visible to the client**: yes. A client that hovers gets setbacks, and this server does not
  disconnect it. This server also does not disconnect a client that falls for a long time.
- **Test**: a differential test of a long fall expects a disconnect from the vanilla server. It
  expects no disconnect from this server.

**D4.**

- **Rule**: MVC-CHK-023, first row.
- **Reference**: the vanilla server clamps an infinite position (MVC-VAN-002).
- **Visible to the client**: no, for a legit client.
- **Test**: a differential test does not send an infinite position.

**D5.**

- **Rule**: MVC-CHK-002 and MVC-CHK-003.
- **Reference**: the vanilla server takes the on-ground bit as true (MVC-VAN-016, PHY-FALL-003).
- **Visible to the client**: no, for a legit client. A client that reports the on-ground bit in the
  air takes fall damage.
- **Test**: as D1.

**D6.**

- **Rule**: MVC-CHK-023, third row.
- **Reference**: the vanilla server keeps a pitch of more than 90 degrees after the wrap.
- **Visible to the client**: no, for a legit client. Other players see the clamped pitch.
- **Test**: as D1.

**D7.**

- **Rule**: MVC-RSP-001.
- **Reference**: a correction of the vanilla server has zero velocity and the reported rotation.
- **Visible to the client**: yes. After a setback, the client keeps the accepted velocity and its
  own rotation.
- **Test**: as D1.

**D8.**

- **Rule**: MVC-LAT-008 and MVC-PRED-001.
- **Reference**: the vanilla server sets the position of the player to the position in the accept
  (TP-CONFIRM-ACC-003). In result R7 of `teleport-confirmation.md`, an accept 5 blocks away moves
  the player.
- **Visible to the client**: no, for a legit client. A client that sends another position in the
  accept gets a correction.
- **Test**: a differential test does not send an accept at a position that is not the destination.

## 13. Unresolved

- **U1**: the server sends entity data for the player to its own client. Nobody knows if this
  data changes the sprint state of the client. When it does, a legit client can send the same
  sprint action two times (MVC-CHK-020). Experiment E17.
- **U2**: the client counts down the duration of an effect. Nobody knows if the client
  removes an effect at the end of its duration before `remove_mob_effect`. Experiment E18.
- **U3**: the duration of a client tick after `ticking_state` and during `ticking_step`. MVC-CHK-010
  and MVC-CHK-012 assume that the client ticks at the acknowledged tick rate. Experiment E19.

- **U4**: closed. PHY-SHP-009 lists the entities that MVC-MODE-003 uses.
- **U5**: the defaults of MVC-RSP-004 and MVC-CHK-011 come from the design. Experiments C1 to C6
  must confirm that a legit client never reaches them.
- **U6**: a client that stays in a coarse state, for example near water, gets only the coarse
  checks. A stricter check for these states is not decided.
- **U7**: MVC-PKT-010 states that the client never sends a `pong` inside a client tick. Experiment
  E6 must confirm this for low frame rates.
- **U8**: a client that answers probes and sends no client ticks. A check for too few client ticks
  is not decided.
- **U9**: closed. MVC-LAT-008 computes a destination from each candidate.
- **U10**: whether the server keeps MVC-VAN-013 and MVC-VAN-014 in coarse mode as an extra limit.

## 14. Experiments

### 14.1 Experiments with a vanilla server

Start a vanilla server with `tools/vanilla-server/run.sh serve`. It writes the properties
`online-mode=false`, `network-compression-threshold=-1` and `gamemode=creative`. For a survival
experiment, run the console command `gamemode survival <name>`.

Connect a scripted client that sends `player_loaded` when the play state starts. Answer each
`keep_alive` and `ping`. Send `client_tick_end` each 50 ms. Read a position with the console
command `data get entity <name> Pos`.

For a client agreement rule, use a vanilla client and record the serverbound packets with a proxy.

| Experiment | Steps | Expected result |
|------------|-------|-----------------|
| E1 | Send `move_player_pos` with `flags` 0xFF. Read the position. | The server applies the position. |
| E2 | With a vanilla client, turn without a move, then move without a turn, then stand still. | `move_player_rot`, then `move_player_pos`, then no move packet. |
| E3 | With a vanilla client, stand still for 5 seconds. Count the client ticks between two position packets. Then move by 1.0E-4 in one tick with a command `tp`. | 19 silent ticks between position packets. No position packet for the small move. |
| E4 | With a vanilla client, hold forward for 1 second. Then die and respawn while you hold the key. | One `player_input` when the key goes down. No `player_input` after the respawn. |
| E5 | Send `player_command` with action 7. | The server closes the connection. |
| E6 | With a vanilla client at 5 frames each second, receive `ping` packets from a modified server. Record the positions of `pong` among the tick packets. | Each `pong` is after a `client_tick_end` and before the next tick packets. |
| E7 | With a vanilla client, record the packets during the load screen. | `client_tick_end` packets, no move packet. |
| E8 | Send a move packet with `x` NaN. Separately, send one with `x` +∞. Separately, send one with `pitch` 120. | NaN: disconnect. +∞: no disconnect, a correction. Pitch 120: no disconnect. |
| E9 | Send two `move_player_pos` packets without a `client_tick_end` between them. | Disconnect with `multiplayer.disconnect.invalid_player_movement`. |
| E10 | Do not confirm the first teleport. Send `move_player_pos_rot` with a new yaw. Read the rotation with `data get entity <name> Rotation`. | The rotation changes. The position does not change. |
| E11 | In survival mode, send one move packet 11 blocks away in the air. Separately, send one 9 blocks away. Repeat after `gamerule player_movement_check false`. | 11 blocks: a correction. 9 blocks: no correction. With the game rule false: no correction. |
| E12 | In survival mode, stand next to a wall. Send a move packet 1 block into the wall. Separately, stand in front of a stone block and send a move 1.3 blocks through it to the air behind it. Repeat both in creative mode. | Survival: a correction for each. Creative: a correction for the first move only. |
| E13 | In survival mode, send move packets at a constant height 10 blocks above the ground. Count the server ticks to the disconnect. Repeat with packets that fall 1 block each tick. | A disconnect with `multiplayer.disconnect.flying` after 81 server ticks in both cases. |
| E14 | In survival mode, stand above air and send `move_player_status_only` with the on-ground bit. Repeat 10 times in 5 seconds. | One `block_update` for each block below the box, one time only. |
| E15 | Set `rate-limit=50`. Send 100 `client_tick_end` packets each second. | A disconnect with `disconnect.exceeded_packet_rate` after about 3 to 4 seconds. |
| E16 | With a vanilla client and a server plugin that refuses each placed block, place a block in the cell of the player. | The client moves itself back to its position at the time of the request. |
| E17 | With a vanilla client, sprint. Make the server stop the sprint of the player. Record `player_command`. | Unresolved U1. |
| E18 | With a vanilla client, give an effect for 20 ticks. Delay `remove_mob_effect` by 1 second with a proxy. Measure the speed. | Unresolved U2. |
| E19 | With a vanilla client, run `tick rate 40`. Count the `client_tick_end` packets each second. | Unresolved U3. |
| E20 | Record the packets of a vanilla client on a vanilla server. Make knockback, an explosion, an effect, an attribute change, a block change and a teleport. Decode each packet with the tables of section 1. | Each packet decodes, and no bytes remain. |
| E21 | Join, and do not send `player_loaded`. Send a `move_player_pos` 1 block away each client tick for 2 seconds. Read the position. Wait 2 more seconds, and read it again. | No change in the first 2 seconds. The position changes after 60 server ticks. |
| E22 | Run `ride <name> mount <minecart>`. Send `move_player_pos_rot` 5 blocks away with a new yaw. Read the position and the rotation. | The rotation changes. The position does not change. |
| E23 | Sleep in a bed. Send a move packet 0.5 blocks away, then one 2 blocks away. | 0.5 blocks: no change and no correction. 2 blocks: a correction. |
| E24 | In survival mode, fall 10 blocks with the on-ground bit in each move packet. Read `Health`. Repeat without the bit until the player lands. | With the bit: no damage. Without the bit: fall damage. |
| E25 | In survival mode, send `player_abilities` with the request to fly. Read `abilities.flying`. Repeat in creative mode. | Survival: `0b`. Creative: `1b`. |
| E26 | Report the on-ground bit. Run `tp <name> ~ ~ ~`. Confirm with an accept at the destination. Read `OnGround`. | `0b`. |

### 14.2 Conformance tests for this server

Run this server and connect a vanilla client through a proxy that delays each direction. A test
passes when the server sends no setback and no disconnect.

| Test | Steps |
|------|-------|
| C1 | Walk, sprint, sneak at an edge, jump, sprint-jump and fall 30 blocks. Use delays of 0, 150 and 500 ms, each with a jitter of ±100 ms. |
| C2 | Do C1 at 5, 20 and 144 frames each second. |
| C3 | Fly in creative mode. Land, and start and stop flight with a double jump. |
| C4 | Take knockback from a mob and from a wind charge while the delay is 500 ms. |
| C5 | Break and place blocks under the player while it walks. Use a plugin that refuses one block in two. |
| C6 | Change the speed attribute and the jump boost effect with commands during a jump. |
| C7 | Freeze the delay proxy for 3 seconds, then release all packets at the same time. |

A scripted client tests the checks. Each test expects a setback, unless it states another result.

| Test | Steps |
|------|-------|
| C8 | Report a position 0.2 blocks above the predicted position. Expect a setback immediately. |
| C9 | Report a horizontal position 0.01 blocks ahead in each client tick. Expect a setback before the sum reaches 1.0 blocks. |
| C10 | In the air, report the same position in each client tick. Expect a setback at the first silent tick. |
| C11 | Report the on-ground bit while the player falls. Expect fall damage in the client tick in which the prediction lands, and no setback. |
| C12 | Report a position inside a stone block. Expect a setback immediately. |
| C13 | Send 30 client ticks in 1 second. Expect violations of the tick rate check, and a setback after the level passes 10. |
| C14 | Do not answer one probe that brackets a `set_entity_motion`. Expect a disconnect after `max_ack_delay`. |

## 15. Layers and cost

**MVC-LAY-001** (`design`, semantic, confidence: high). The checks have three layers:

| Layer | Content | World and physics |
|-------|---------|-------------------|
| L0 | a check of one packet, without state | no |
| L1 | a check with the state of one connection | no |
| L2 | a check with the world view and the physics model | yes |

Each rule of this document has exactly one layer (MVC-LAY-006). A layer reads its own state and
the outputs of the lower layers. A layer must not read the state of a higher layer. A lower layer
never waits for a higher layer.

**MVC-LAY-002** (`design`, semantic, confidence: high). L0 has these properties:

- **Inputs**: one packet of either direction, and its size.
- **State**: none.
- **Outputs**: the decoded fields of the packet, and a report for each failed check.
- **Responses**: disconnect, or report to L1. L0 does not drop a packet.

L0 must not drop a tick packet, because L1 counts the tick packets. MVC-CHK-023 states the checks of
L0.

**MVC-LAY-003** (`design`, semantic, confidence: high). L1 has these properties:

- **Inputs**: the outputs of L0 for both directions, in the order of the connection, and the time
  of arrival of each packet.
- **State**: the state of one connection. The state has no blocks, no entities and no physics. It
  has these values:
  - the count of client ticks and the balance of MVC-CHK-010;
  - the probes without an answer, the windows and the pending teleport ids;
  - the reference position, the last keys and the sprint state;
  - the abilities, the effects and the attributes that the server sent.
- **Outputs**: the boundaries of the client ticks, the state of each predicted-state change for each
  client tick (MVC-LAT-004), and the reports.
- **Responses**: drop, disconnect, or report to L2.

L1 drops only an answer to its own probe and a packet that the server ignores. Examples are an
answer with an unknown id and a request to fly without the fly permission. The memory of L1 for one
connection has a limit, because `max_ack_delay` limits the number of probes without an answer.

**MVC-LAY-004** (`design`, semantic, confidence: high). L2 has these properties:

- **Inputs**: the outputs of L1, the world view and the physics model.
- **State**: the accepted state, the record of MVC-PRED-015, the offset buffer and the violation
  levels.
- **Outputs**: the accepted state and the server-side results of MVC-PRED-012.
- **Responses**: refuse a client tick, send a setback, or disconnect.

**MVC-LAY-005** (`design`, semantic, confidence: high). A report has the rule ID, the number of the
client tick and the values that failed the check. A layer gives each report to the next higher
layer. When no higher layer runs, the configured response applies: disconnect, or ignore. A report
does not change the state of the layer that makes it.

**MVC-LAY-006** (`design`, semantic, confidence: medium). This table gives the layer of each rule.
The cost class is one of these:

- **constant**: work with a fixed limit for each packet;
- **per candidate**: one physics step for each candidate that the server runs;
- **per collider**: one query of the collision shapes near the box.

One physics step contains queries of the colliders (PHY-COL-001). The column "Proxy" tells if a
proxy can run the rule without the help of the server.

| Rule | Layer | Inputs | Cost | Proxy |
|------|-------|--------|------|-------|
| MVC-PKT-001 | L0 | one serverbound packet | constant | yes |
| MVC-PKT-004 | L0 | one serverbound packet | constant | yes |
| MVC-PKT-005 | L0 | one serverbound packet | constant | yes |
| MVC-PKT-006 | L0 | one serverbound packet | constant | yes |
| MVC-PKT-007 | L0 | one serverbound packet | constant | yes |
| MVC-PKT-008 | L0 | one serverbound packet | constant | yes |
| MVC-PKT-009 | L0 | one serverbound packet | constant | yes |
| MVC-PKT-002 | L1 | the move packets of the connection | constant | yes |
| MVC-PKT-003 | L1 | the move packets of the connection | constant | yes |
| MVC-PKT-010 | L1 | the order of the serverbound packets | constant | yes |
| MVC-PKT-011 | L1 | the order of the serverbound packets | constant | yes |
| MVC-PKT-020 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-021 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-022 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-023 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-024 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-025 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-026 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-027 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-028 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-029 | L0 | one clientbound packet | constant | yes |
| MVC-PKT-030 | L1 | the clientbound packets | constant | yes |
| MVC-VAN-001 | L0 | one move packet or accept | constant | yes |
| MVC-VAN-002 | L0 | one move packet or accept | constant | yes |
| MVC-VAN-003 | L1 | the move packets and `client_tick_end` | constant | yes |
| MVC-VAN-004 | L1 | the load, teleport, ride and sleep state from the packets | constant | yes |
| MVC-VAN-005 | L1 | the load, teleport, ride and sleep state from the packets | constant | yes |
| MVC-VAN-006 | L1 | the load, teleport, ride and sleep state from the packets | constant | yes |
| MVC-VAN-007 | L1 | the load, teleport, ride and sleep state from the packets | constant | yes |
| MVC-VAN-008 | L2 | the velocity of the copy of the player, which reads blocks | per collider | no |
| MVC-VAN-009 | L2 | the velocity of the copy of the player, which reads blocks | per collider | no |
| MVC-VAN-010 | L2 | the world view, the move procedure | per collider | no |
| MVC-VAN-011 | L2 | the world view, the move procedure | per collider | no |
| MVC-VAN-012 | L2 | the world view, the move procedure | per collider | no |
| MVC-VAN-013 | L2 | the world view | per collider | no |
| MVC-VAN-014 | L2 | the world view | per collider | no |
| MVC-VAN-015 | L2 | the world view | per collider | no |
| MVC-VAN-016 | L2 | the world view | per collider | no |
| MVC-VAN-017 | L1 | `player_command`, `player_abilities`, `player_input`, the clientbound abilities | constant | yes |
| MVC-VAN-018 | L2 | the accept, the world view | per collider | no |
| MVC-VAN-019 | L1 | the count of packets in each second | constant | yes |
| MVC-TICK-001 | L1 | the tick packets and the answers | constant | yes |
| MVC-TICK-002 | L1 | the tick packets and the answers | constant | yes |
| MVC-TICK-003 | L1 | the tick packets and the answers | constant | yes |
| MVC-TICK-004 | L1 | the tick packets and the answers | constant | yes |
| MVC-TICK-005 | L1 | the tick packets and the answers | constant | yes |
| MVC-PRED-001 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-002 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-003 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-004 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-005 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-006 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-007 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-008 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-009 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-010 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-011 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-012 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-013 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-014 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-015 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-016 | L2 | the accepted state, the inputs, the world view | per candidate | no |
| MVC-PRED-017 | L2 | the record, the kept block changes | per collider | no |
| MVC-PRED-018 | L2 | the block changes that the server sends | constant | no |
| MVC-UNC-001 | L2 | the candidates | per candidate | no |
| MVC-UNC-002 | L2 | the candidates | per candidate | no |
| MVC-UNC-003 | L2 | the candidates | per candidate | no |
| MVC-LAT-001 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-LAT-002 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-LAT-003 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-LAT-004 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-LAT-005 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-LAT-006 | L2 | the candidates, the world view | per candidate | no |
| MVC-LAT-007 | L2 | the candidates, the world view | per candidate | no |
| MVC-LAT-008 | L2 | the candidates, the world view | per candidate | no |
| MVC-LAT-009 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-LAT-010 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-LAT-011 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-LAT-012 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-LAT-013 | L1 | the clientbound changes, the probes and the answers | constant | yes |
| MVC-CHK-001 | L2 | the candidates | per candidate | no |
| MVC-CHK-002 | L2 | the candidates | per candidate | no |
| MVC-CHK-003 | L2 | the candidates | per candidate | no |
| MVC-CHK-004 | L2 | the candidates | per candidate | no |
| MVC-CHK-005 | L2 | the accepted boxes, the settled shapes | per collider | no |
| MVC-CHK-010 | L1 | the `client_tick_end` packets, the answers, the clock, `ticking_state` | constant | yes |
| MVC-CHK-011 | L1 | the `client_tick_end` packets, the answers, the clock, `ticking_state` | constant | yes |
| MVC-CHK-012 | L1 | the `client_tick_end` packets, the answers, the clock, `ticking_state` | constant | yes |
| MVC-CHK-020 | L1 | the tick packets, the reference position, the clientbound abilities | constant | yes |
| MVC-CHK-021 | L2 | the candidates | per candidate | no |
| MVC-CHK-022 | L2 | the candidates | per candidate | no |
| MVC-CHK-023 | L0 | one serverbound packet | constant | yes |
| MVC-CHK-024 | L1 | the reported positions, the clientbound velocity, abilities, effects and attributes | constant | yes |
| MVC-MODE-001 | L2 | the world view, the entities near the player | per collider | no |
| MVC-MODE-002 | L2 | the world view, the entities near the player | per collider | no |
| MVC-MODE-003 | L2 | the world view, the entities near the player | per collider | no |
| MVC-MODE-004 | L2 | the world view, the entities near the player | per collider | no |
| MVC-MODE-005 | L2 | the world view, the entities near the player | per collider | no |
| MVC-MODE-006 | L2 | the world view, the entities near the player | per collider | no |
| MVC-MODE-007 | L2 | the world view, the entities near the player | per collider | no |
| MVC-RSP-001 | L2 | the accepted state, the violations | constant | no |
| MVC-RSP-002 | L2 | the accepted state, the violations | constant | no |
| MVC-RSP-003 | L2 | the accepted state, the violations | constant | no |
| MVC-RSP-004 | L2 | the accepted state, the violations | constant | no |
| MVC-RSP-005 | L2 | the accepted state, the violations | constant | no |
| MVC-RSP-006 | L2 | the accepted state, the violations | constant | no |
| MVC-RSP-007 | L2 | the accepted state, the violations | constant | no |
| MVC-RSP-008 | L2 | the accepted state, the violations | constant | no |
| MVC-CFG-001 | L2 | the configuration | constant | no |
| MVC-INV-001 | L2 | the candidates, the world view | per candidate | no |
| MVC-INV-002 | L2 | the candidates, the world view | per candidate | no |
| MVC-INV-003 | L2 | the candidates, the world view | per candidate | no |
| MVC-INV-004 | L2 | the candidates, the world view | per candidate | no |
| MVC-INV-005 | L2 | the candidates, the world view | per candidate | no |
| MVC-INV-006 | L2 | the candidates, the world view | per candidate | no |
| MVC-INV-007 | L1 | the packets of the connection | constant | yes |
| MVC-INV-008 | L1 | the packets of the connection | constant | yes |
| MVC-INV-009 | L1 | the packets of the connection | constant | yes |
| MVC-LAY-001 | L2 | the layers | constant | no |
| MVC-LAY-002 | L0 | the layers | constant | yes |
| MVC-LAY-003 | L1 | the layers | constant | yes |
| MVC-LAY-004 | L2 | the layers | constant | no |
| MVC-LAY-005 | L2 | the layers | constant | no |
| MVC-LAY-006 | L1 | the layers | constant | yes |
| MVC-LAY-007 | L1 | the layers | constant | yes |
| MVC-LAY-008 | L2 | the layers | constant | no |
| MVC-LAY-009 | L1 | the handshake and the clientbound packets | constant | yes |

**MVC-LAY-007** (`design`, semantic, confidence: medium). A proxy can run L0 and L1 without the help
of the server. It obeys these rules:

1. It sends its own probes (MVC-LAT-001, MVC-LAT-002) among the packets that it forwards.
2. It matches each answer with the oldest `ping` without an answer, in the order of the connection.
   This order contains the `ping` packets of the server and the probes of the proxy.
3. It removes the answers to its own probes from the stream to the server.
4. It does not change the order of the other packets.

A legit client answers the `ping` packets in their order (MVC-PKT-008). Thus the ids of the server
and of the proxy can be equal without an error.

**MVC-LAY-008** (`design`, semantic, confidence: low). The target load is 10,000 players at 20
client ticks each second. This is 200,000 client ticks each second. The cost of each layer obeys
these limits:

- L0 and L1 have a constant cost for each packet. They do not read the world.
- For a legit client, L2 runs about one physics step for each client tick (MVC-PRED-014).
- L2 runs at most `max_steps_per_tick` physics steps for one client tick (MVC-PRED-016).
- The result of L2 for one player depends only on the packets of that player and the world view.
  Thus the server can run L2 for different players at the same time.

**MVC-LAY-009** (`design`, semantic, confidence: medium). L1 at a proxy needs the numeric ids of the
attributes and the effects. `minecraft:attribute` and `minecraft:mob_effect` are static registries.
Their numeric ids depend only on the protocol version, and the configuration phase does not send
them in `registry_data` (REG-SET-001). Thus the proxy takes the ids from the registry report of the
protocol version, for example `assets/mcrs/reports/registries.json`.

The proxy parses these packets:

| State | Packet | What the proxy reads |
|-------|--------|----------------------|
| handshake | `intention` | the protocol version |
| play | `login`, `respawn` | the entity id of the player; a respawn starts a new reference position |
| play | `update_attributes` | for the entity id of the player, the base value and the modifiers of `minecraft:movement_speed` |
| play | `update_mob_effect`, `remove_mob_effect` | for the entity id of the player, the effect and the amplifier |
| play | `player_abilities` | the flags and the flight speed |
| play | `set_entity_motion` | the velocity, for the entity id of the player |
| play | `explode` | the optional impulse; the frame length gives the end of the packet |
| play | `player_position`, `ping`, `ticking_state`, `ticking_step` | each field |
| play | `set_passengers`, `set_camera` | the ride state and the camera of the player |
| play | `start_configuration` | the end of the play state |

The proxy computes the value of an attribute from its base value `b` and its modifiers:

- `A` is the sum of the amounts of the modifiers with operation 0.
- `B` is the sum of the amounts of the modifiers with operation 1.
- `C` is the product of `1 + amount` for the modifiers with operation 2.

The value is `(b + A) × (1 + B) × C`, clamped to the range of the attribute.

When the proxy has no registry report for the protocol version, it does not run MVC-CHK-024. It runs
the other rules of L0 and L1.
