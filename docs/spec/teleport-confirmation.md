# The confirmation of a teleport: behavioural specification

This document states how a client confirms a teleport, and what the server does until the client
confirms it. It states only what a client or a server console can see. It does not prescribe an
architecture. The rules come from a vanilla server, from the reference implementation, from the
public documentation of Bedrock Edition, and from the decisions in issue 623.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**, **VarInt**,
  **offline mode**: as `login.md`, section 0.1, defines them.
- **Teleport**: one clientbound `player_position` packet.
- **Accept**: one serverbound `accept_teleportation` packet.
- **Move**: one serverbound `move_player_pos` packet.
- **Pending teleport**: a teleport that the server sent and that no accept has confirmed.
- **Newest id**: the `teleport_id` of the teleport that the server sent last on the connection.
- **Resend**: a teleport that a move causes while a teleport is pending. It repeats the position of
  the pending teleport with a new `teleport_id`.
- **Correction**: a teleport that the server sends because an accept fails the movement checks. It
  holds the position of the player.
- **Tick**: one game tick. A vanilla server runs 20 ticks each second.
- **Edge**: the part of mcrs that connects the clients of one edition and turns their packets into
  canonical events.
- **Vehicle**: an entity that carries passengers. A **riding stack** is an entity, its passengers,
  their passengers, and so on.
- **Seat position**: the position of a passenger that its vehicle sets.

### 0.2 Provenance

- `black_box_confirmed`: shown by an experiment against a vanilla server.
- `source_suggested`: taken from the reference implementation, and consistent with the captured
  results.
- `documentation`: publicly documented behaviour, for example the protocol documentation of Bedrock
  Edition.
- `design`: an mcrs choice. The rule states the reason.
- `unresolved`: not decided. Section 5 collects these items.

All the results come from one vanilla server and one scripted client on one machine. Rules that
depend on one observation state this.

### 0.3 ID prefix

Each rule has an ID with the prefix `TP-CONFIRM`.

### 0.4 Scope

This document does not state:

- what causes a teleport, which `command-teleport.md` states for the `teleport` command;
- the movement checks for a move when no teleport is pending;
- `move_player_pos_rot`, `move_player_rot` and `move_player_status_only`. The experiments did not
  send them.

## 1. Packets

**TP-CONFIRM-PKT-001** (`black_box_confirmed`, wire). The fields of an accept are, in this order:

| Field | Encoding |
|-------|----------|
| `teleport_id` | VarInt |
| `x`, `y`, `z` | three big-endian 64-bit floats |
| `yaw`, `pitch` | two big-endian 32-bit floats |

The server disconnects a client that sends only `teleport_id`, because it cannot decode the
packet.

## 2. Identifiers

**TP-CONFIRM-ID-001** (`black_box_confirmed`, semantic). On each connection, the first teleport has
the `teleport_id` 1.

**TP-CONFIRM-ID-002** (`black_box_confirmed`, semantic). Each later teleport has the `teleport_id`
of the previous teleport plus 1. A resend and a correction obey this rule too.

## 3. Rules

### 3.1 An accept while a teleport is pending

**TP-CONFIRM-ACC-001** (`black_box_confirmed`, semantic). Only an accept with the newest id confirms
the pending teleport.

**TP-CONFIRM-ACC-002** (`black_box_confirmed`, semantic). When an accept has another `teleport_id`,
the server must not disconnect the client. The server must not change the position of the player,
and the teleport stays pending. This rule applies to:

- an id that the server never sent;
- the id of an older teleport, when a newer teleport has replaced it;
- the id of a teleport that a resend has replaced.

**TP-CONFIRM-ACC-003** (`black_box_confirmed`, semantic). An accept that confirms the pending
teleport sets the position of the player to the position in the accept. The accept does not need to
hold the position of the teleport. In an experiment, the accept names a position 5 blocks away, and the
server uses that position. TP-CONFIRM-ACC-009 states the two steps of this change.

**TP-CONFIRM-ACC-004** (`black_box_confirmed`, semantic). The movement checks apply to the position
in an accept. In an experiment, the accept names a position 1000 blocks away. The player stays
at the position of the teleport, and the server sends a correction. Section 5 gives the limit as
unresolved.

**TP-CONFIRM-ACC-007** (`black_box_confirmed`, semantic). A correction is a pending teleport. Until
an accept confirms it, the server does not apply a move. An accept with its id confirms it.

**TP-CONFIRM-ACC-008** (`black_box_confirmed`, semantic). The server sends a correction for each
accept that fails the movement checks. The interval of TP-CONFIRM-RES-003 does not apply. In an experiment, the server sends three corrections in less than 0.5 seconds.

**TP-CONFIRM-ACC-009** (`source_suggested`, semantic). An accept that confirms the pending
teleport has two effects, in this order:

1. The server sets the position of the player to the destination of the teleport.
2. The server handles the position, the yaw and the pitch of the accept as a move from the
   destination. The on-ground flag and the horizontal collision flag of this move are false.

The checks of a move apply to step 2 (TP-CONFIRM-ACC-004). Thus an accept 5 blocks away passes,
and an accept 1000 blocks away fails. The yaw and the pitch of the accept set the rotation of the
player, as in a move. A server without the checks of `movement-checks.md` uses this rule. A server
with these checks uses MVC-LAT-008 in place of step 2 (deviation D8 of that document).

### 3.2 An accept when no teleport is pending

**TP-CONFIRM-ACC-005** (`black_box_confirmed`, semantic). When no teleport is pending and an accept
has the newest id, the server disconnects the client with the translation key
`multiplayer.disconnect.invalid_player_movement`. This rule rests on one observation.

**TP-CONFIRM-ACC-006** (`black_box_confirmed`, semantic). When no teleport is pending and an accept
has an id that the server never sent, the server does not disconnect the client.
This rule rests on one observation.

**TP-CONFIRM-ACC-010** (`source_suggested`, semantic). Only an accept with the newest id has an
effect. An accept with any other id, also the id of an older teleport, changes nothing and does not
disconnect the client, whether a teleport is pending or not. This closes U6.

**TP-CONFIRM-ACC-011** (`source_suggested`, semantic). When an accept has the newest id while a
teleport is pending, and its `x`, `y` or `z` is NaN, or its `yaw` or `pitch` is NaN or infinite, the
server disconnects the client with the translation key
`multiplayer.disconnect.invalid_player_movement`. An infinite position does not cause this
disconnect. The movement checks clamp it and then reject it (MVC-VAN-001, MVC-VAN-002).

### 3.3 A move while a teleport is pending

**TP-CONFIRM-MOV-001** (`black_box_confirmed`, semantic). While a teleport is pending, the server
does not apply the position of a move. The server applies the
rotation of a move packet (MVC-VAN-005 in `movement-checks.md`). The experiments did not read the
rotation.

**TP-CONFIRM-MOV-002** (`black_box_confirmed`, semantic). After an accept confirms the teleport, the
server applies the next move.

### 3.4 Resend

**TP-CONFIRM-RES-001** (`black_box_confirmed`, semantic). The server does not send a resend on a
timer. Without a move, the server does not send a resend. Section 5 gives the other move packets as
unresolved. A correction is not a resend (TP-CONFIRM-ACC-004). In an experiment, a client that sends
no move for 25 seconds gets no resend. In an experiment, a confirmed connection with a new pending
teleport gets no resend for 10 seconds.

**TP-CONFIRM-RES-002** (`black_box_confirmed`, semantic). When a move arrives while a teleport is
pending, and about 20 ticks or more have passed since the last teleport, the server sends a resend.
In an experiment, the first move after a wait causes a resend at once.

**TP-CONFIRM-RES-003** (`black_box_confirmed`, semantic). While moves continue to arrive, the
interval between resends is about 20 ticks. A client that sends a move each 50 ms gets one resend
each 1.05 seconds. Section 5 gives the exact number of ticks as unresolved.

**TP-CONFIRM-RES-004** (`black_box_confirmed`, semantic). A resend holds the position of the pending
teleport, not the position of the move.

**TP-CONFIRM-RES-005** (`black_box_confirmed`, semantic). The server does not disconnect a client
because it does not confirm a teleport. Experiments show this for 25 and 12 seconds. A
longer time is unresolved.

**TP-CONFIRM-RES-006** (`source_suggested`, semantic). The server counts the ticks of each
connection. It keeps the tick at which it sent the last teleport, also a resend or a correction.
When no teleport is pending, each move packet also sets this tick. When a move packet arrives
while a teleport is pending, the server sends a resend when more than 20 ticks have passed after
that tick. Thus the shortest interval is 21 ticks.

These packets are move packets for this rule: `move_player_pos`, `move_player_pos_rot`,
`move_player_rot`, `move_player_status_only` and `move_vehicle`. A resend has the position of the
pending teleport, the current rotation of the player, zero velocity and no relative flags.

The server ignores a `move_player_*` packet until the client has loaded, and while the player sees
the end credits (MVC-VAN-004 in `movement-checks.md`). This packet then does not cause a resend and
does not set the tick. A
`move_vehicle` packet has the effects of this rule also before the client has loaded.

**TP-CONFIRM-ID-003** (`source_suggested`, semantic). When the next `teleport_id` is the largest
value of a signed 32-bit integer, the server uses 0 in its place. Thus the ids go from
2147483646 to 0.

### 3.5 Editions: the canonical events

**TP-CONFIRM-EVT-001** (`design`, semantic). The rules of sections 3.1 to 3.4 apply to two canonical
events, not to the packets of one edition:

- `TeleportAccepted(id, position, yaw, pitch)`: the client confirms a teleport. It takes the place
  of an accept.
- `MoveReceived`: the client sends a move. It takes the place of each move packet of
  TP-CONFIRM-RES-006.

The edge of each client edition makes these events from its packets. Reason: issue 623 decides that
one rules kernel serves both editions, and that the edge holds the difference.

**TP-CONFIRM-EVT-002** (`design`, semantic). The edge of Java Edition makes `TeleportAccepted` from
each accept, with its `teleport_id`, position, yaw and pitch. It makes `MoveReceived` from each move
packet of TP-CONFIRM-RES-006. Thus the rules of sections 3.1 to 3.4 do not change for Java
Edition.

**TP-CONFIRM-EVT-003** (`design`, semantic). The edge of Bedrock Edition sends a teleport as
`MovePlayer` with the position mode `Teleport` (2). It keeps a queue of the ids of the teleports that
it sent, in the order in which it sent them. When a `PlayerAuthInput` packet has the input flag
`HandledTeleport` (37), the edge removes the first id from the queue and makes
`TeleportAccepted` with that id, and with the position and the rotation of that `PlayerAuthInput`.
When the queue is empty, the edge makes no event. Each `PlayerAuthInput` also makes
`MoveReceived`. When one `PlayerAuthInput` makes both events, the edge makes `TeleportAccepted`
first. Reason: a client of Java Edition sends the accept before the move. In the other order, the
move can cause a resend before the accept, and the client gets a teleport that it does not need.

With one teleport in the queue, the id is the id of the pending teleport. With more teleports in the
queue, the first flags give older ids, and TP-CONFIRM-ACC-002 applies to them as it does for Java
Edition: an older id does not confirm the pending teleport. Thus an accept of an older teleport has
the same result in both editions. Reason: `PlayerAuthInput` carries no teleport id, and the public
protocol documentation names the flag `HandledTeleport` and gives no id field. The transport of
Bedrock Edition keeps the order of the packets, so the n-th flag belongs to the n-th teleport.
Entry U7 records what a Bedrock client does that the documentation does not state.

The position of a player in `MovePlayer` and in `PlayerAuthInput` is 1.62 blocks above the position
of its feet. The edge adds 1.62 to the `y` of a teleport destination before it sends `MovePlayer`.
It subtracts 1.62 from the `y` of a `PlayerAuthInput` before it makes `TeleportAccepted` or
`MoveReceived`. Reason: the canonical events and the rules use the position of the feet. Without
this change, each accept of a Bedrock client is 1.62 blocks too high, and TP-CONFIRM-ACC-003 accepts
that position.

**TP-CONFIRM-EVT-004** (`design`, semantic). The resend of TP-CONFIRM-RES-006 also serves a client
of Bedrock Edition. A Bedrock client sends `PlayerAuthInput` each client tick, and each one makes
`MoveReceived`. Thus, while a teleport is pending, the server sends a resend each 21 ticks, also to
a Bedrock client that dropped or ignored a `MovePlayer` teleport. The client-loaded condition of
TP-CONFIRM-RES-006 applies to `MoveReceived` from `PlayerAuthInput`: before the client has loaded,
it does not cause a resend. A Bedrock client has loaded after it sends
`SetLocalPlayerAsInitialised`, as a Java client after it sends `player_loaded`. The edge sends the resend as `MovePlayer` with the position mode `Teleport`, and puts its id
in the queue of TP-CONFIRM-EVT-003. Reason: issue 623.

**TP-CONFIRM-EVT-005** (`design`, semantic). Before the edge of Bedrock Edition puts the id of a
resend or a correction in the queue of TP-CONFIRM-EVT-003, it removes all the ids from the queue.
Thus the next `HandledTeleport` flag gives the id of the resend or the correction. Reason: a client
that drops a teleport never sets a flag for it. Without this rule, each flag would give an older id,
and the teleport would stay pending for all time. A resend has the position of the pending teleport,
so a late flag for an older teleport confirms the resend at the correct destination. Issue 623
says that the edge gives each flag the newest id. The queue gives the same result when the client
sets one flag for each teleport.

A flag for an older teleport can arrive after a resend has emptied the queue. Then the flag confirms
the resend with the position of the older teleport. The movement checks of TP-CONFIRM-ACC-004 apply
to this position. A client of Java Edition can also confirm a teleport with another position that
passes these checks (TP-CONFIRM-ACC-003), so this case gives a client no new capability. A client
that follows the protocol then reports its true position in the next `PlayerAuthInput`, and the
movement checks apply to that move.

### 3.6 Riding

**TP-CONFIRM-RID-001** (`black_box_confirmed`, semantic). Players cannot carry passengers. The
command `ride <entity> mount <player>` fails with the reply "Players can't be ridden". Thus a player
is always the top of a riding stack.

**TP-CONFIRM-RID-002** (`black_box_confirmed`, semantic). A teleport of an entity that rides
another entity, a player included, first ends the riding: the entity leaves its vehicle. The owner
and the observers get `set_passengers` for the vehicle without the entity. The vehicle stays where
it is. Then the teleport moves the entity and its own passengers. For a player, the teleport is a
pending teleport as section 3.1 states.

**TP-CONFIRM-RID-003** (`black_box_confirmed`, semantic). A teleport of an entity that carries
passengers moves all the entities of the stack above it, and they keep riding. Each player in the
stack gets a teleport with the position of its seat, not the position of the command. This teleport
is a pending teleport of that player, and sections 3.1 to 3.4 apply to it:

- the player confirms it with an accept, not the vehicle;
- while it is pending, the server does not apply `move_vehicle` or the position of
  `move_player_pos`, and each of them can cause a resend with the seat position.

**TP-CONFIRM-RID-004** (`black_box_confirmed`, semantic). When a teleport moves a vehicle with a
player to another dimension, the player goes with it and keeps riding. The player gets, in this
order:

1. `set_passengers` for the old vehicle without the player, and a teleport at the old position.
2. `respawn` for the new dimension, and two teleports in the new dimension.
3. `set_passengers` that links the player to the vehicle with a new entity id.
4. `add_entity` for the vehicle with the new entity id, and `set_passengers` again, two times.

The first `set_passengers` of step 3 names an entity that the client does not know yet. This rule rests on one observation.

**TP-CONFIRM-RID-005** (`design`, semantic). The edge of Bedrock Edition sends `SetActorLink` for
each `set_passengers` of TP-CONFIRM-RID-002 to TP-CONFIRM-RID-004: the link type `None` (0) for a
passenger that leaves, and the link type `Riding` (1) or `Passenger` (2) for a passenger that the
server links again. The player that rides gets the confirmation of TP-CONFIRM-RID-003 as a
`MovePlayer` teleport with the seat position. Reason: issue 623 asks which entity gets the
confirmation and how passengers link again for Bedrock Edition. The public protocol documentation
names `SetActorLink` and its link types. It does not state which link type a server sends for which
seat (U8).

**TP-CONFIRM-RID-006** (`source_suggested`, semantic). When a player that rides an entity confirms a
teleport, step 2 of TP-CONFIRM-ACC-009 takes only the yaw and the pitch of the accept. The position
of the player stays the seat position, and the movement checks do not apply. While a teleport is
pending, `move_vehicle` changes nothing. A `move_vehicle` with NaN in `x`, `y` or `z`, or with NaN
or an infinite value in `yaw` or `pitch`, disconnects the client with the translation key
`multiplayer.disconnect.invalid_vehicle_movement`, also while a teleport is pending.

### 3.7 The collision check

**TP-CONFIRM-COL-001** (`design`, semantic). A teleport request has the option "fail when the
destination collides". Its default value is false. When it is true, the server tests the collision
box of the entity at the destination against the collision shapes of the blocks. When the box
intersects a shape, the teleport does not occur: no position changes, the server sends no packet,
and the request counts as failed. A block with no collision shape, for example a flower or air,
does not make the test fail.

The `teleport` command of Java Edition never sets the option (`command-teleport.md`). The
`teleport` command of Bedrock Edition sets it with the argument `checkForBlocks` true. The collision
shapes are the shapes of the data model of Java Edition. Reason: issue 623. The public
documentation of the Bedrock command states that it teleports the target "only if the target(s)
would not collide with a block", and that a failed check makes the command fail. It does not state
whether the test uses the box of the entity or one block (U9).

## 4. Edition differences

Each entry gives the rule, the behaviour of each edition, the decision of mcrs, and what selects
the behaviour.

**E-DIFF-1.**

- **Rule**: TP-CONFIRM-EVT-003, TP-CONFIRM-EVT-005.
- **Java Edition**: an accept carries the `teleport_id`.
- **Bedrock Edition**: `PlayerAuthInput` carries the flag `HandledTeleport` and no id.
- **Decision**: the edge gives the flags the ids of its queue, in order. A resend or a correction
  empties the queue first (TP-CONFIRM-EVT-005).
- **Selected by**: the client edition of the connection.
- **Visible to the client**: no.
- **Prediction risk**: low, when a Bedrock client sets the flag one time for each teleport (U7).
- **Reference**: issue 623; the public protocol documentation of Bedrock Edition.

**E-DIFF-2.**

- **Rule**: TP-CONFIRM-COL-001.
- **Java Edition**: the `teleport` command has no collision check.
- **Bedrock Edition**: the argument `checkForBlocks` of the `teleport` command.
- **Decision**: one option of the teleport request. Only the Bedrock form of the command sets it.
- **Selected by**: the command dialect of the sender.
- **Visible to the client**: yes. With a collision, the target does not move and the command fails.
- **Prediction risk**: none. The server decides the teleport.
- **Reference**: issue 623; the public documentation of the `teleport` command.

## 5. Unresolved

- **U7**: what a Bedrock client does after a `MovePlayer` teleport: whether it sets `HandledTeleport`
  in one `PlayerAuthInput` for each teleport, and whether the position of that `PlayerAuthInput` is
  the destination. TP-CONFIRM-EVT-003 assumes one flag for each teleport. An experiment on a Bedrock
  client resolves this.
- **U8**: which `SetActorLink` link type a server of Bedrock Edition sends for the driver and for
  the other passengers of a vehicle (TP-CONFIRM-RID-005).
- **U9**: whether the check of `checkForBlocks` in Bedrock Edition uses the collision box of the
  entity or one block (TP-CONFIRM-COL-001).
