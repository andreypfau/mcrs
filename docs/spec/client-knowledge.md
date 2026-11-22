# Client knowledge: behavioural specification

Version: see `assets/minecraft/version.json`.

A client moves and acts in the world as it knew the world a moment ago. This document states how
the server learns which of its changes a client had applied when the client sent a packet. The
server sends a `ping` before and after each change, and the order of the answers tells it what
the client knew. A vanilla server does none of this. Each rule of this document that the server
obeys is a choice of mcrs. Section 12 lists each difference that a differential test can see.

`movement-checks.md` already states the probes, the windows and their use for the movement of
the own player (sections 1, 6 and 15 of that document). This document cites those rules. It adds
these subjects:

- the mechanism as four parts that a user can take one at a time (section 1);
- the bounds of the probes that wait for an answer (section 2);
- the full set of tracked changes, beyond the movement of the own player (section 3);
- the knowledge at each serverbound packet, not only at each client tick (section 4);
- the rules for a reader that treats a change as possible or as required (section 5);
- the position of the own player that a reader without the physics model can know (section 6);
- the positions of the other entities that a client can show (section 7);
- the bracket of a teleport, a setback and a respawn (section 8);
- the reaction to a mismatch (section 9);
- what a client of mcrs must do (section 10).

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **clientbound**, **serverbound**:
  as `login.md`, section 0.1, defines them.
- **Teleport**, **accept**, **pending teleport**: as `teleport-confirmation.md`, section 0.1,
  defines them.
- **Probe**, **answer**, **client tick**, **server tick**, **tick packets**, **move packet**,
  **position packet**, **silent tick**, **reference position**, **load**, **legit client**,
  **setback**, **proxy**: as `movement-checks.md`, section 0.1, defines them.
- **Ping**: one clientbound `ping` packet, from any sender. A probe is a ping that the bracket
  protocol sends.
- **Tracked change**: a clientbound packet that is in a class of section 3. A predicted-state
  change (`movement-checks.md`, MVC-PKT-030) is a tracked change.
- **Group**: the tracked changes between one probe and the next probe on one connection
  (MVC-LAT-001).
- **Probe before** and **probe after**: the probes that come directly before and directly after
  the group of a tracked change. Together they are the **bracket** of the change.
- **Knowledge point**: one serverbound packet. The knowledge at a knowledge point is the set of
  probes whose answers arrive before that packet (CK-BRK-004).
- **State of a change**: one of not applied, window and applied, at one knowledge point. The
  table of MVC-LAT-004 defines the three states. This document uses them at each knowledge point.
- **Apply sequence**: one assignment, to each tracked change, of the knowledge point at which the
  client applied it. An apply sequence is **allowed** when it obeys CK-UNC-002.
- **Possible change**: a change in the window state. Some allowed apply sequences contain it, and
  some do not.
- **Required change**: a change in the applied state. Each allowed apply sequence contains it.
- **Unknown value**: a value that the knowledge state cannot give for a knowledge point
  (CK-BRK-009).
- **Knowledge state**: the record of one connection that gives the state of each tracked change at
  each knowledge point.
- **Reader**: a part that reads the knowledge state and compares a serverbound packet with it.
  The movement checks of `movement-checks.md` are one reader. A reach check is another reader.
- **Mismatch**: a report of a reader. It states that no allowed apply sequence gives the packet.
  A violation of `movement-checks.md` is a mismatch of the movement reader.
- **Reaction**: what the server does for a mismatch, for example a setback or a disconnect.
- **Entity view**: for one tracked entity and one client tick, the set of positions that the
  client can give that entity (section 7).
- **Interpolation length** (`N`): the number of client ticks over which the client moves an entity
  to a new position, as CENTS-INTERP-001 of `client-entities.md` defines it.

### 0.2 Provenance

The labels of `registry.md`, section 0.3, apply. The kinds are those of `README.md`, section
"Semantics, not accidents". Each rule states them on its provenance line.

A rule with the label `design` is a choice of mcrs. A vanilla server does no lag compensation, so
a comparison with a vanilla server cannot confirm such a rule. A conformance test with a legit
client tests it (section 14). Some `design` rules take their idea from a third-party open source
anticheat, which `docs/clean-room.md` allows as a source for the author. Such a rule has the note
`third-party` on its provenance line. It is still a design choice of mcrs and not game behaviour.

A rule of the kind client agreement states what the vanilla client does. Each such rule cites the
rule of another document that states the behaviour, or an experiment of section 14.

### 0.3 ID prefix

Each rule has an ID with the prefix `CK`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `CK-PART` | the parts of the mechanism |
| `CK-BRK` | the bracket protocol |
| `CK-SET` | the tracked changes |
| `CK-KNW` | the knowledge state |
| `CK-UNC` | possible and required changes |
| `CK-SIL` | the unreported small movement |
| `CK-ENT` | the entity view |
| `CK-TP` | teleports, setbacks and respawns |
| `CK-RCT` | the reaction to a mismatch |
| `CK-CLI` | the duties of a client of mcrs |
| `CK-INV` | invariants |
| `CK-DEV` | intentional deviations |

### 0.4 Scope

This document does not state:

- the movement checks, the prediction and the setback. `movement-checks.md` states them;
- the reach check. `reach.md` states it and reads sections 6 and 7;
- how the client moves an entity. `client-entities.md` states it;
- the crates and the types of an implementation. A design note states them.

### 0.5 Parameters

A user sets each parameter through the plugin that adds the knowledge state. Each value of the
table is a default.

| Parameter | Default | Rule |
|-----------|---------|------|
| `max_pending_probes` | 2400 | CK-BRK-008 |
| `max_kept_bytes` | 1 MiB | CK-BRK-009 |
| `entity_settle_ticks` | 20 client ticks | CK-ENT-004 |

`max_ack_delay` and `replay_depth` are parameters of `movement-checks.md`, section 10.

## 1. Parts

### CK-PART-001: Four parts

The mechanism has four parts. Each part has these inputs and outputs:

| Part | Inputs | Outputs |
|------|--------|---------|
| bracket protocol | the clientbound tracked changes, the serverbound answers | the probes, the knowledge at each knowledge point |
| knowledge state | the outputs of the bracket protocol, the tracked changes | the state of each tracked change and each value that a client can have, at each knowledge point |
| reader | the knowledge state, the serverbound packets | mismatches |
| reaction | the mismatches | clientbound packets, a disconnect, or nothing |

A part reads only the outputs of the parts above it in the table. A part never reads the state of
a part below it. The bracket protocol and the knowledge state work with no reader and no
reaction.

Provenance: `design`. Kind: semantic. Confidence: high.

### CK-PART-002: The knowledge state without the checks

A user of the library can run the bracket protocol and the knowledge state alone. Examples are a
proxy, a bot, an anticheat and a minigame. The user can read, for each serverbound packet, the
state of each tracked change and each value that the client can have. A user can add a reader.
The new reader needs no change to the other parts.

Provenance: `design`. Kind: semantic. Confidence: high.

### CK-PART-003: The reaction is separate

A user of the library can replace the reaction to each kind of mismatch. "No reaction" is a valid
reaction. A reaction never changes the knowledge state directly. When a reaction sends a tracked
change, the bracket protocol brackets it as each other tracked change (CK-RCT-003).

Provenance: `design`. Kind: semantic. Confidence: high.

### CK-PART-004: A proxy

A proxy runs the bracket protocol by MVC-LAY-007. It tracks only the classes of section 3 whose
packets it parses. A reader of a class that the proxy does not track does not run at that proxy.

Provenance: `design`. Kind: semantic. Confidence: medium.

## 2. The bracket protocol

### CK-BRK-001: What the protocol guarantees

The connection delivers the packets of each direction in the order of the sender. The vanilla
client obeys these rules:

1. It applies the clientbound packets in the order of arrival, between two client ticks
   (MVC-PKT-010, MVC-LAT-006).
2. It answers each ping with a `pong` of the same id, in the order of the pings (MVC-PKT-008).
3. It sends the answer to a ping after it applies each packet that came before the ping. It
   sends the answer before it applies a packet that came after the ping.

Thus an answer shows that the client applied each packet before its ping, and no packet after
it. The protocol gives no time bound: a client can answer after any delay.

Provenance: `source_suggested`, from MVC-PKT-008, MVC-PKT-010 and MVC-LAT-006. Kind: client
agreement. Confidence: medium. Experiment: K1.

### CK-BRK-002: Probes around each group

MVC-LAT-001 to MVC-LAT-003 apply to each tracked change of section 3, not only to a
predicted-state change. The server must not send a tracked change outside a group. One probe can
be the probe after one group and the probe before the next group.

The server can send the tracked changes of one server tick for one client as one group. Then one
server tick needs at most two probes for that client. The probe after can also be the probe of
MVC-LAT-002.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-BRK-003: Answers and other senders

The bracket protocol matches each `pong` with the oldest ping without an answer, in the order of
the connection. That order contains each ping of the connection, also a ping that another part or
a plugin sends. The id of a ping does not identify its sender. The bracket protocol gives the
answer to each other ping to its sender. It removes the answers to its own probes from the
packets that other parts read.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium. The third-party source
selects a reserved range of ids for its own probes. mcrs uses the order instead, as MVC-LAY-007
does, so that two senders can use the same ids.

### CK-BRK-004: The knowledge at a serverbound packet

A probe is acknowledged at a knowledge point when its answer arrives before that serverbound
packet. Each serverbound packet is a knowledge point: a move packet, an accept, an attack, a use
of an item and each other packet.

The client answers only between two client ticks (CK-BRK-001). Thus each packet of client tick
`N` has the same knowledge. This knowledge is the knowledge of tick `N` in MVC-TICK-002.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-BRK-005: Before, between and after the answers

For a tracked change and a knowledge point, the state of the change is:

| Answers before the knowledge point | State | What the server knows |
|------------------------------------|-------|-----------------------|
| no answer to the probe before | not applied | The client has not applied the change. |
| the answer to the probe before, no answer to the probe after | window | The client applied the change at one knowledge point after the first answer, or not yet. |
| the answer to the probe after | applied | The client applied the change at one knowledge point in the window, or at this point. |

This is MVC-LAT-004 at each knowledge point.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-BRK-006: An answer out of order or with an unknown id

MVC-LAT-011 applies. A `pong` when no ping waits for an answer is a mismatch, and the server
ignores it. Another `pong` can have the id of a later ping that waits for an answer. That `pong` is a
mismatch, and it acknowledges each older probe at the same time. A `pong` whose id is the id of no ping that
waits for an answer is a mismatch, and the server ignores it.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium.

### CK-BRK-007: An answer that does not come

MVC-LAT-012 applies to each probe: after `max_ack_delay` without an answer, the server
disconnects the client. Until then, each change after the first probe without an answer stays in
the not applied state or in the window state.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium.

### CK-BRK-008: The bound in probes

At most `max_pending_probes` probes of one connection wait for an answer. The default is 2400:
two probes for each server tick during `max_ack_delay` at 20 server ticks each second. When a new
probe passes the bound, the server disconnects the client, with the reason of MVC-LAT-012.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-BRK-009: The bound in bytes

The records of the tracked changes of one connection use at most `max_kept_bytes`. The default is
1 MiB. When a new group passes the bound, the knowledge state does these steps:

1. Drop the records of changes in the applied state that no reader needs (CK-KNW-003).
2. When the bound is still passed, replace the records of the new group by one record. This
   record holds only the chunk sections and the entities that the group changes.

Until the probe after that group is acknowledged, each value of these sections and entities is an
unknown value. After that, the client has the values that the server sent, and the knowledge
state gives them again.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-BRK-010: The play state only

The server sends probes only in the play state. A connection can go back to the configuration
state (`start_configuration`). The knowledge state of the play state ends at the
`configuration_acknowledged` of the client. A legit client answers each earlier ping before it
sends `configuration_acknowledged` (CK-BRK-001). A probe without an answer at that time gets
CK-BRK-006. A new play state starts a new knowledge state with no probes.

A respawn does not end the knowledge state (CK-TP-003).

Provenance: `design`, third-party. Kind: semantic. Confidence: medium. Experiment: K2 for the
order of the client.

## 3. Tracked changes

### CK-SET-001: The classes of tracked changes

These classes of clientbound packets are tracked changes:

| Class | Packets | Applies to |
|-------|---------|------------|
| movement values | the packets of MVC-PKT-030 | the own player and its world view |
| own entity data | `set_entity_data` (MVC-PKT-031): the flags of elytra flight, item use and riptide, the sleep position | the own player |
| game mode | `game_event` with the event 3; `player_info_update` with a game mode for the own player | the own player |
| teleport | `player_position` | the own player (CK-TP-001) |
| respawn | `respawn` | the own player and its world (CK-TP-003) |
| vehicle | `set_passengers` that adds or removes the own player; the position, the entity data and the attributes of the vehicle of the own player | the own player |
| camera | `set_camera` | the own player |
| own equipment | each packet that changes a slot of the own inventory | the own player |
| tags | `update_tags` | the world view |
| teams | `set_player_team` | the own player and the other entities |
| entities | `add_entity`, `remove_entities`, `move_entity_pos`, `move_entity_pos_rot`, `entity_position_sync`, `teleport_entity`, `set_passengers`; the `set_entity_data` and `update_attributes` that change an entity box | each tracked entity (section 7) |

A packet of these classes is a tracked change also when the server sends it in a bundle.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium.

### CK-SET-002: Other packets

A clientbound packet that CK-SET-001 does not list is not a tracked change. A user of the library
can add a class of packets. The knowledge state can drop a tracked change that no reader of that
client needs, for example a block change far from the player (MVC-PRED-018). A reader must not
read a change that the knowledge state dropped.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-SET-003: No pose for the own player

The server does not send the pose (index 6 of POSE-NET-002 of `player-pose.md`) of a player to the
client of that player. It removes that entry from each `set_entity_data` for the own player. When
no other entry remains, it does not send the packet. The other entries of the own player stay
tracked changes (CK-SET-001).

The client selects its own pose in its next tick (POSE-CLI-001). A pose from the server thus
changes the eye height of the client for at most one client tick, and the target of that tick can
use it (CBK-TGT-001). Without the entry, the eye height of the client comes only from its own
selection. CK-DEV-006 records the difference from the reference.

Provenance: `design`. Kind: deviation. Confidence: medium.

## 4. The knowledge state

### CK-KNW-001: Applied changes are a prefix

The client applies the tracked changes in the order of the server (MVC-LAT-006). Thus at each
knowledge point, the changes that the client has applied are a prefix of the tracked changes of
the connection. Each change in the applied state is in that prefix. A change in the not applied
state is not.

Provenance: `source_suggested`, from MVC-LAT-006. Kind: client agreement. Confidence: medium.
Experiment: K1.

### CK-KNW-002: The values that a client can have

For a value that tracked changes set, the knowledge state gives the set of values that the client
can have at a knowledge point. Examples are a block state, an attribute, a game mode and a
position of an entity. The set has one value for each prefix that CK-UNC-002 allows. The set has
one value when no change of that value is in the window state.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-KNW-003: How long the knowledge state keeps a record

The knowledge state keeps the record of a tracked change while a reader needs it. A reader needs
a record until each knowledge point that it can still compare has the change in the applied state.
The movement reader needs a record for `replay_depth` client ticks after that (MVC-PRED-018).
Once a change has the applied state, it keeps it at each later knowledge point (MVC-INV-007).

Provenance: `design`. Kind: semantic. Confidence: medium.

## 5. Possible and required changes

### CK-UNC-001: How a reader treats a change

A reader must accept a serverbound packet when one allowed apply sequence gives it. A reader
reports a mismatch only when no allowed apply sequence gives the packet. Thus:

- A reader must accept a packet that shows a possible change, and a packet that does not show it.
- A packet that does not show a required change is a mismatch.
- A packet that shows a change in the not applied state is a mismatch.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-UNC-002: Allowed apply sequences

An apply sequence is allowed when each of these conditions is true:

1. Each change is applied at most once. The client applies it at one knowledge point of its
   window, or at the first knowledge point at which it has the applied state.
2. The applied changes at each knowledge point are a prefix (CK-KNW-001).
3. A change that is applied at one knowledge point stays applied at each later knowledge point.
4. All the packets of one client tick have the same applied changes (CK-BRK-004).

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-UNC-003: Example: a knockback

The server sends a knockback to a player as `set_entity_motion` in a group. The answer to the
probe before arrives before client tick 10. The answer to the probe after arrives before client
tick 13.

| Client tick | State of the knockback | The movement reader accepts |
|-------------|------------------------|-----------------------------|
| 9 | not applied | only a movement without the knockback |
| 10, 11, 12 | window | a movement without the knockback; a movement with it from tick 10, 11 or 12 |
| 13 | applied | a movement with the knockback from tick 10, 11, 12 or 13 |
| 14 | applied | a movement that continues the knockback. A movement that never had it is a mismatch. |

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-UNC-004: A wider set, never a narrower set

A reader can replace the set of values of CK-KNW-002 by a set that contains it, for example a box
that contains each possible position. A reader must not use a set that does not contain each
value of CK-KNW-002. A wider set can hide a cheat. A narrower set gives a mismatch to a legit
client.

Provenance: `design`. Kind: semantic. Confidence: high.

### CK-UNC-005: Unknown values

A reader must not report a mismatch that depends on an unknown value (CK-BRK-009). The movement
reader classifies such a client tick as coarse (MVC-MODE-003).

Provenance: `design`. Kind: semantic. Confidence: medium.

## 6. The unreported small movement

### CK-SIL-001: The own position in a silent tick

In a silent tick, the client does not report its position. MVC-PKT-003 states when the client
sends a position packet. Thus in a silent tick, the position of the client is at most 2.0E-4
blocks from the reference position. This is true for each silent tick after the load, while the
player does not ride and the camera is the player (MVC-PKT-011). The rotation has no such
threshold: the client reports each change of the rotation (MVC-PKT-002).

At most 19 silent ticks follow a position packet. The distance does not add up over these ticks,
because the client measures each tick from the same reference position.

Provenance: `source_suggested`, from MVC-PKT-002, MVC-PKT-003 and MVC-PKT-011. Kind: client
agreement. Confidence: medium. Experiment: E3 of `movement-checks.md`.

### CK-SIL-002: A reader without the physics model

A reader that has no physics model knows the own position in a silent tick only as the ball of
CK-SIL-001. It must take each position of that ball as possible. For example, a reach reader
adds 2.0E-4 blocks to its bound in a silent tick. The movement reader uses MVC-PRED-010 instead of
the ball.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium.

### CK-SIL-003: The own position in a client tick with a position packet

In a client tick with a position packet, the own position of the client is the reported position.
A reader that trusts the movement reader uses the accepted state (MVC-PRED-012) instead.

Provenance: `design`. Kind: semantic. Confidence: medium.

## 7. The entity view

### CK-ENT-001: Tracked entities

A tracked entity is an entity that the server added for the client with `add_entity`. It stays
tracked until its `remove_entities` has the applied state. While `add_entity` or `remove_entities`
is in the window state, the entity can be present or absent on the client.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-ENT-002: The exact entity view

The entity view of a tracked entity at a client tick is a set of positions. Each position is a
position that the client can give the entity at the end of that client tick. Each allowed apply
sequence (CK-UNC-002) gives one position. CENTS-MOVE-002 to CENTS-MOVE-004 and CENTS-INTERP-001
to CENTS-INTERP-004 of `client-entities.md` give that position. Their input is the position
packets of the entity, at the client ticks of the sequence.

The entity moves only in a client tick in which it ticks on the client (`client-entities.md`,
section 0.1). The knowledge state counts the client ticks of the player by `client_tick_end`.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-ENT-003: A box that contains the entity view

A reader can use the box `H(k)` for client tick `k` instead of the exact entity view
(CK-UNC-004). The knowledge state computes `H(k)` in these steps:

1. At the tick in which `add_entity` can first be applied, `H` is the position of `add_entity`.
2. For each later client tick `k`, take the position packets of the entity whose probe before
   is acknowledged for tick `k`. `H(k)` is the smallest box that contains `H(k − 1)` and the end
   position of each of these packets.
3. `H(k)` becomes the end position `Z` of the last position packet when these conditions are
   true:
   - each position packet of the entity has the applied state for tick `k`;
   - the entity ticked in at least `N` client ticks after the first client tick in which the last
     position packet has the applied state.

`H(k)` contains each position of CK-ENT-002. The client moves the entity only along a path
between its position and the end positions that it received (CENTS-INTERP-002). Or it snaps to an
end position (CENTS-INTERP-003). Each point of such a path is in the box. Step 3 is correct
because the path that remains after a start ends in `N` ticks (CENTS-INTERP-001).

The movement reader uses this box for the possible pushers of MVC-UNC-004 of `movement-checks.md`.
That rule and MVC-UNC-005 to MVC-UNC-007 stay in that document.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium. Experiment: K5.

### CK-ENT-004: The interpolation length of each type

Step 3 of CK-ENT-003 uses the interpolation length `N` of the type of the entity, from the table
of CENTS-INTERP-005 of `client-entities.md`. For a type whose `N` that rule does not state, step 3
uses `entity_settle_ticks` (section 0.5). Without step 3, the box only grows. Thus a value larger than
the true `N` gives a wider box, and it is never wrong (CK-UNC-004). The default of 20 is such a
wider bound for each type whose true `N` is 20 or less.

Step 3 does not apply while the tick rate of the client is frozen. It also does not apply while
the entity can be outside the ticking area of the client.

The box of CK-ENT-003 contains the view only of a type that the client moves by the rules of
section 4 of `client-entities.md` alone. The client also moves some types with their own step,
for example the simulated entities of section 16 of that document and the projectiles. For such a
type, each position in the entity view is an unknown value (CK-UNC-005).

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-ENT-005: The entity box

The box of an entity in the entity view is the entity box of `client-entities.md`, section 0.1,
at each position of the view. The width, the height, the scale, the baby factor and the pose are
values of CK-KNW-002. The reader uses each value that the client can have.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-ENT-006: A frame between two client ticks

The client draws a frame between two client ticks, at a position between the previous value and
the current value (CENTS-TIME-001). A reader that compares a result of a frame uses the box that
contains `H(k − 1)` and `H(k)`.

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-ENT-007: The records of the entity view

For each pair of a client and a tracked entity, the knowledge state keeps one box and the position
packets in the window state. The server sends the same position packets to each client that tracks
the entity. Thus the clients can share the records of these packets (MVC-UNC-007).

Provenance: `design`. Kind: semantic. Confidence: medium.

### CK-ENT-008: The entity view of a passenger

While a tracked entity rides a tracked vehicle on the client, the client puts it at its seat
position (`teleport-confirmation.md`, section 0.1). The client does not move it by its own position
packets. The entity view of the passenger is then the view of the vehicle plus the seat offset:

1. The seat offset is the seat position minus the position of the vehicle. It depends on the type,
   the scale and the state of the vehicle, the type of the passenger and the index of the
   passenger in `set_passengers`. Examples are the passenger attachment points of ANIM-TYPE-002 of
   `mob-animals.md`.
2. The client turns the horizontal part of the seat offset with the yaw of the vehicle. Thus the
   view of the passenger contains the seat offset at each yaw that the client can give the vehicle.
3. A reader can use a wider set (CK-UNC-004). An example is `H(k)` of the vehicle, grown on `x` and
   `z` by the horizontal length of the seat offset, and moved on `y` by its vertical part.

While the `set_passengers` that adds or removes the passenger is in the window state, the entity
view of the passenger contains its view as a passenger and its view without the vehicle.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: K7.

## 8. Teleports, setbacks and respawns

### CK-TP-001: The bracket of a teleport

A teleport is a tracked change with a bracket, like each other tracked change. A legit client
sends the accept when it applies the teleport (MVC-PKT-020). Thus a legit client sends the accept
after the answer to the probe before, and before the answer to the probe after.

The accept is the acknowledgement of the teleport (MVC-LAT-008, `teleport-confirmation.md`). The
bracket only checks the order of the accept.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium. Experiment: K3.

### CK-TP-002: A teleport that the client skips

These cases are a mismatch of the teleport:

- The answer to the probe after a teleport arrives, and the accept of that teleport did not
  arrive before it.
- The accept of a teleport arrives before the answer to the probe before that teleport.

The default reaction to the first case is a new setback (MVC-RSP-001) with the next `teleport_id`.
The reaction ignores the accept of the second case.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium.

### CK-TP-003: A respawn and a change of dimension

A `respawn` is a tracked change. It starts a load (MVC-PKT-012). While it is in the window
state, a reader that reads the world uses the old world and the new world. When it has the
applied state, the knowledge state drops the records of the old world: blocks, chunks and
entities. The first teleport after the respawn obeys CK-TP-001.

Provenance: `design`. Kind: semantic. Confidence: medium.

## 9. The reaction to a mismatch

### CK-RCT-001: A mismatch

A mismatch has the reader, the serverbound packet, the knowledge at that packet and the values that
no allowed apply sequence gives. A mismatch does not change the knowledge state.

Provenance: `design`. Kind: semantic. Confidence: high.

### CK-RCT-002: The default reaction

The default reaction to a mismatch of the movement reader is section 9 of `movement-checks.md`.
The default reaction to a mismatch of CK-BRK-006 is the packet check of MVC-CHK-020. Another
reader states its default reaction. A user can replace each default (CK-PART-003).

Provenance: `design`. Kind: semantic. Confidence: high.

### CK-RCT-003: A setback is a tracked change

A setback is a teleport. Thus it gets a bracket (CK-TP-001), whatever reaction sends it. While a
setback is pending, MVC-RSP-003 applies to the movement reader.

Provenance: `design`, third-party. Kind: semantic. Confidence: medium.

## 10. A client of mcrs

The client of mcrs, and a bot on it, must answer as a vanilla client does. Otherwise the server
cannot know what the client applied.

### CK-CLI-001: Answer each ping in order

The client answers each ping with a `pong` of the same id. It answers the pings in the order in
which they arrive. It sends no `pong` without a ping.

Provenance: `source_suggested`, from MVC-PKT-008. Kind: client agreement. Confidence: medium.
Experiment: K6.

### CK-CLI-002: Answer after the changes before the ping

The client sends the answer to a ping after it applies each packet that arrived before the ping.
It sends the answer before it applies a packet that arrived after the ping. It sends the answer
between two client ticks, never between the tick packets of one client tick (MVC-PKT-010).

Provenance: `source_suggested`, from MVC-PKT-010 and MVC-LAT-006. Kind: client agreement.
Confidence: medium. Experiment: K6.

### CK-CLI-003: Accept a teleport when it applies it

The client sends the accept of a teleport when it applies the teleport, before it answers a later
ping (MVC-PKT-020).

Provenance: `source_suggested`, from MVC-PKT-020. Kind: client agreement. Confidence: medium.
Experiment: K6.

### CK-CLI-004: Answer in the configuration state

The client answers a ping in the configuration state with the same rules. It answers each ping
that arrived before `start_configuration` before it sends `configuration_acknowledged`.

Provenance: `documentation`, the protocol pages of minecraft.wiki list `ping` and `pong` in the
configuration state and the play state. Kind: client agreement. Confidence: low. The order of
the second sentence has no source yet. Experiment:
K2.

## 11. Invariants

### CK-INV-001: Latency gives no mismatch

A legit client never gets a mismatch because of its latency. This is true for any latency, any
jitter and any frame rate. Two examples are the break of the block below a player that moves, and
a knockback of a player. Neither gives a mismatch at any latency below `max_ack_delay`.

Provenance: `design`. Kind: semantic. Confidence: medium. Experiment: K4.

### CK-INV-002: The same packets give the same knowledge

Take the same clientbound and serverbound packets in the same order. Then the knowledge state
gives the same state of each change at each knowledge point. The arrival time
changes only CK-BRK-007.

Provenance: `design`. Kind: semantic. Confidence: high.

## 12. Intentional deviations

A vanilla server does no lag compensation. Each entry of this section is a place where a server
with these rules sends or does something that a vanilla server does not. A differential test
consults this list.

| Entry | Rules | Test |
|-------|-------|------|
| CK-DEV-001 | CK-BRK-002, CK-TP-001 | ignore `ping` and `pong` |
| CK-DEV-002 | CK-BRK-007 | D9 of `movement-checks.md` |
| CK-DEV-003 | CK-BRK-008 | no comparison of a stream with no answers |
| CK-DEV-004 | CK-BRK-006 | no comparison of a stream with wrong answers |
| CK-DEV-005 | CK-TP-002 | no comparison of a stream that skips a teleport |
| CK-DEV-006 | CK-SET-003 | ignore index 6 in `set_entity_data` for the own player |

### CK-DEV-001: Probes in the play state

- **Rule**: CK-BRK-002, CK-TP-001.
- **Reference**: the vanilla server sends no probes and does no lag compensation. It ignores each
  `pong` (`movement-checks.md`, D9).
- **Reason**: the answers are the only way to know what the client applied.
- **Visible to the client**: yes, on the wire: one `ping` before and after each group. The player
  sees nothing.
- **Bug reference**: none.
- **Test**: a differential test removes each `ping` and each `pong` from both streams before it
  compares them.

### CK-DEV-002: A probe without an answer

- **Rule**: CK-BRK-007. D9 of `movement-checks.md` is the entry.
- **Reference**, **Reason**, **Visible to the client**, **Bug reference**, **Test**: as D9.

### CK-DEV-003: Too many probes without an answer

- **Rule**: CK-BRK-008.
- **Reference**: the vanilla server keeps no probes.
- **Reason**: the memory of one connection must have a bound in entries.
- **Visible to the client**: no, for a legit client. A legit client that answers no probe for
  `max_ack_delay` also answers no `keep_alive`, and a vanilla server disconnects it earlier.
- **Bug reference**: none.
- **Test**: a differential test does not send a stream that answers `keep_alive` but not `ping`.

### CK-DEV-004: Answers out of order

- **Rule**: CK-BRK-006.
- **Reference**: the vanilla server ignores each `pong`.
- **Reason**: an answer out of order can hide the time at which the client applied a change.
- **Visible to the client**: no, for a legit client, because it answers in order (CK-BRK-001).
  A client that answers out of order gets violations of the packet check.
- **Bug reference**: none.
- **Test**: a differential test does not send a `pong` out of order.

### CK-DEV-005: A skipped teleport

- **Rule**: CK-TP-002.
- **Reference**: the vanilla server matches an accept only by its `teleport_id`
  (`teleport-confirmation.md`). It does not test the order of the accept and a `pong`.
- **Reason**: a client that does not apply a teleport keeps a position that the server does not
  know.
- **Visible to the client**: no, for a legit client. A client that skips a teleport gets a new
  setback.
- **Bug reference**: none.
- **Test**: a differential test does not send a stream that skips an accept.

### CK-DEV-006: No pose for the own player

- **Rule**: CK-SET-003.
- **Reference**: the vanilla server sends each change of the pose of a player to the client of that
  player (POSE-NET-003 of `player-pose.md`).
- **Reason**: the client replaces that pose in its next tick (POSE-CLI-001). Until then, the pose
  from the server changes the eye height of the target of the client. The server then cannot know
  the eye of an action of that tick from the movement packets alone.
- **Visible to the client**: no, in normal play. The client selects the same pose in its next tick.
  Its camera eye height follows the pose with a smooth filter (CCAM-EYE-001 of `client-camera.md`),
  so a pose of one tick shows no visible step.
- **Bug reference**: none.
- **Test**: a differential test removes index 6 from each `set_entity_data` for the own player in
  both streams before it compares them. It drops a packet that has no other entry.

## 13. Edition parameters

This section states parameters, not rules. A Bedrock client joins through its own edge
(`teleport-confirmation.md`, section 3.5). The parts of section 1 stay the same. The edge gives
them these values:

| Parameter | Java Edition | Bedrock Edition |
|-----------|--------------|-----------------|
| knowledge point | one serverbound packet; the packets of one client tick share it | one input frame of the client |
| probe and answer | `ping` and `pong` | the latency probe of the edition |
| duration of a client tick | 1000 ms divided by the tick rate that the client acknowledged | 50 ms, also after a change of the tick rate |
| what the client applied | the prefix at the client tick | the prefix at the input frame |

A Bedrock client sends one input frame in each of its own ticks, at a fixed 20 each second. It
cannot follow a change of the tick rate. Thus the edge records the applied changes for each input
frame, not for each server tick.

## 14. Experiments

No experiment of this section has run. Each experiment uses a vanilla client and the proxy of `tools/mcrs_minecraft_client_recorder`.
The proxy needs one change: it must insert a `ping` into the clientbound stream and record the
place of each `pong` among the serverbound packets.

| Experiment | Steps | Expected result | Rules |
|------------|-------|-----------------|-------|
| K1 | Insert a `ping` before and after a `block_update` that removes the block below the player, and before and after a `set_entity_motion` for the player. Add 300 ms of delay in both directions. Record the serverbound packets. | Each `pong` is in the order of the pings, between two client ticks. The first move packet after the first `pong` and before the second `pong` shows the change or not. Each move packet after the second `pong` shows it. | CK-BRK-001, CK-KNW-001 |
| K2 | Insert a `ping` directly before `start_configuration`. Record the serverbound packets. | The `pong` comes before `configuration_acknowledged`. | CK-BRK-010, CK-CLI-004 |
| K3 | Run `tp <name> ~ ~10 ~`. Insert a `ping` before and after the `player_position`. | The accept comes after the first `pong` and before the second `pong`. | CK-TP-001 |
| K4 | Run the scenes of K1 and a hit by a zombie with 0, 150, 300 and 1000 ms of delay in each direction. Feed the recordings to a server with the movement reader. | No mismatch. | CK-INV-001 |
| K5 | Lead a pig and a player that walk. Delay each `pong` by 300 ms. Record the positions of the client entities with the capture of `client-entities.md`, section 0.2. | Each position of each client tick is in `H(k)` of CK-ENT-003. | CK-ENT-003 |
| K6 | Run K1 and K3 with the client of mcrs in place of the vanilla client. | The same order of `pong`, accept and tick packets as the vanilla client. | CK-CLI-001 to CK-CLI-003 |
| K7 | Seat a player in a boat and a second player in a minecart. Let a third player watch both while the vehicles move and turn. Record the positions of the client entities with the capture of `client-entities.md`, section 0.2. | Each position of each passenger is in the set of CK-ENT-008, step 3. A position packet of a passenger does not move it. | CK-ENT-008 |

## 15. Unresolved

No item is open.
