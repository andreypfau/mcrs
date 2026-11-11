# The confirmation of a teleport: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

This document states how a client confirms a teleport, and what the server does until the client
confirms it. It states only what a client or a server console can see. It does not prescribe an
architecture. The rules come from a vanilla server, from the reference implementation, from the
public documentation of Bedrock Edition, and from the decisions in issue 623. Section 6 gives the
experiments, and section 7 gives the captured results. Results R1 to R17 come from the game version
in the table. Results R18 to R24 come from `26.4-snapshot-7` (protocol `1073742169`).

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

- `black_box_confirmed`: shown by a captured result in section 7.
- `source_suggested`: taken from the reference implementation, and consistent with the captured
  results. The rules that issue 623 adds with this label come from the reference of
  `26.4-snapshot-7`.
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
  send them. Results R20 and R21 send `move_vehicle`.

## 1. Packets

**TP-CONFIRM-PKT-001** (`black_box_confirmed`, wire). The fields of an accept are, in this order:

| Field | Encoding |
|-------|----------|
| `teleport_id` | VarInt |
| `x`, `y`, `z` | three big-endian 64-bit floats |
| `yaw`, `pitch` | two big-endian 32-bit floats |

The server disconnects a client that sends only `teleport_id`, because it cannot decode the
packet. See result R1.

## 2. Identifiers

**TP-CONFIRM-ID-001** (`black_box_confirmed`, semantic). On each connection, the first teleport has
the `teleport_id` 1. See result R2.

**TP-CONFIRM-ID-002** (`black_box_confirmed`, semantic). Each later teleport has the `teleport_id`
of the previous teleport plus 1. A resend and a correction obey this rule too. See results R3, R4
and R16.

## 3. Rules

### 3.1 An accept while a teleport is pending

**TP-CONFIRM-ACC-001** (`black_box_confirmed`, semantic). Only an accept with the newest id confirms
the pending teleport. See results R3, R5 and R6.

**TP-CONFIRM-ACC-002** (`black_box_confirmed`, semantic). When an accept has another `teleport_id`,
the server must not disconnect the client. The server must not change the position of the player,
and the teleport stays pending. This rule applies to:

- an id that the server never sent (result R5);
- the id of an older teleport, when a newer teleport has replaced it (result R3);
- the id of a teleport that a resend has replaced (result R6).

**TP-CONFIRM-ACC-003** (`black_box_confirmed`, semantic). An accept that confirms the pending
teleport sets the position of the player to the position in the accept. The accept does not need to
hold the position of the teleport. In result R7, the accept names a position 5 blocks away, and the
server uses that position. TP-CONFIRM-ACC-009 states the two steps of this change.

**TP-CONFIRM-ACC-004** (`black_box_confirmed`, semantic). The movement checks apply to the position
in an accept. In results R8 and R16, the accept names a position 1000 blocks away. The player stays
at the position of the teleport, and the server sends a correction. Section 5 gives the limit as
unresolved.

**TP-CONFIRM-ACC-007** (`black_box_confirmed`, semantic). A correction is a pending teleport. Until
an accept confirms it, the server does not apply a move. An accept with its id confirms it. See
result R16.

**TP-CONFIRM-ACC-008** (`black_box_confirmed`, semantic). The server sends a correction for each
accept that fails the movement checks. The interval of TP-CONFIRM-RES-003 does not apply. In result
R17, the server sends three corrections in less than 0.5 seconds.

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
`multiplayer.disconnect.invalid_player_movement`. See result R9. This rule rests on one observation.

**TP-CONFIRM-ACC-006** (`black_box_confirmed`, semantic). When no teleport is pending and an accept
has an id that the server never sent, the server does not disconnect the client. See result R10.
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
does not apply a move. See results R3, R5, R6 and R11.

**TP-CONFIRM-MOV-002** (`black_box_confirmed`, semantic). After an accept confirms the teleport, the
server applies the next move. See results R5, R6 and R11.

### 3.4 Resend

**TP-CONFIRM-RES-001** (`black_box_confirmed`, semantic). The server does not send a resend on a
timer. Without a move, the server does not send a resend. Section 5 gives the other move packets as
unresolved. A correction is not a resend (TP-CONFIRM-ACC-004). In result R12, a client that sends
no move for 25 seconds gets no resend. In result R13, a confirmed connection with a new pending
teleport gets no resend for 10 seconds.

**TP-CONFIRM-RES-002** (`black_box_confirmed`, semantic). When a move arrives while a teleport is
pending, and about 20 ticks or more have passed since the last teleport, the server sends a resend.
In results R4 and R14, the first move after a wait causes a resend at once.

**TP-CONFIRM-RES-003** (`black_box_confirmed`, semantic). While moves continue to arrive, the
interval between resends is about 20 ticks. A client that sends a move each 50 ms gets one resend
each 1.05 seconds. See result R15. Section 5 gives the exact number of ticks as unresolved.

**TP-CONFIRM-RES-004** (`black_box_confirmed`, semantic). A resend holds the position of the pending
teleport, not the position of the move. See results R4, R14 and R15.

**TP-CONFIRM-RES-005** (`black_box_confirmed`, semantic). The server does not disconnect a client
because it does not confirm a teleport. Results R12 and R15 show this for 25 and 12 seconds. A
longer time is unresolved.

**TP-CONFIRM-RES-006** (`source_suggested`, semantic). The server counts the ticks of each
connection. It keeps the tick at which it sent the last teleport, also a resend or a correction.
When no teleport is pending, each move packet also sets this tick. When a move packet arrives
while a teleport is pending, the server sends a resend when more than 20 ticks have passed after
that tick. Thus the shortest interval is 21 ticks.

These packets are move packets for this rule: `move_player_pos`, `move_player_pos_rot`,
`move_player_rot`, `move_player_status_only` and `move_vehicle`. A resend has the position of the
pending teleport, the current rotation of the player, zero velocity and no relative flags.

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
`MoveReceived`.

With one teleport in the queue, the id is the id of the pending teleport. With more teleports in the
queue, the first flags give older ids, and TP-CONFIRM-ACC-002 applies to them as it does for Java
Edition: an older id does not confirm the pending teleport. Thus an accept of an older teleport has
the same result in both editions. Reason: `PlayerAuthInput` carries no teleport id, and the public
protocol documentation names the flag `HandledTeleport` and gives no id field. The transport of
Bedrock Edition keeps the order of the packets, so the n-th flag belongs to the n-th teleport.
Entry U7 records what a Bedrock client does that the documentation does not state.

**TP-CONFIRM-EVT-004** (`design`, semantic). The resend of TP-CONFIRM-RES-006 also serves a client
of Bedrock Edition. A Bedrock client sends `PlayerAuthInput` each client tick, and each one makes
`MoveReceived`. Thus, while a teleport is pending, the server sends a resend each 21 ticks, also to
a Bedrock client that dropped or ignored a `MovePlayer` teleport, for example while it loads the
world. The edge sends the resend as `MovePlayer` with the position mode `Teleport`, and puts its id
in the queue of TP-CONFIRM-EVT-003. Reason: issue 623.

**TP-CONFIRM-EVT-005** (`design`, semantic). Before the edge of Bedrock Edition puts the id of a
resend or a correction in the queue of TP-CONFIRM-EVT-003, it removes all the ids from the queue.
Thus the next `HandledTeleport` flag gives the id of the resend or the correction. Reason: a client
that drops a teleport never sets a flag for it. Without this rule, each flag would give an older id,
and the teleport would stay pending for all time. A resend has the position of the pending teleport,
so a late flag for an older teleport confirms the resend at the correct destination. When the
client reports another position, the movement checks of TP-CONFIRM-ACC-004 apply to it. Issue 623
says that the edge gives each flag the newest id. The queue gives the same result when the client
sets one flag for each teleport, and it does not confirm a newer teleport with the position of an
older one.

### 3.6 Riding

**TP-CONFIRM-RID-001** (`black_box_confirmed`, semantic). Players cannot carry passengers. The
command `ride <entity> mount <player>` fails with the reply "Players can't be ridden". Thus a player
is always the top of a riding stack. See result R22.

**TP-CONFIRM-RID-002** (`black_box_confirmed`, semantic). A teleport of an entity that rides
another entity, a player included, first ends the riding: the entity leaves its vehicle. The owner
and the observers get `set_passengers` for the vehicle without the entity. The vehicle stays where
it is. Then the teleport moves the entity and its own passengers. For a player, the teleport is a
pending teleport as section 3.1 states. See results R18, R21 and R23 (cases player and middle).

**TP-CONFIRM-RID-003** (`black_box_confirmed`, semantic). A teleport of an entity that carries
passengers moves all the entities of the stack above it, and they keep riding. Each player in the
stack gets a teleport with the position of its seat, not the position of the command. This teleport
is a pending teleport of that player, and sections 3.1 to 3.4 apply to it:

- the player confirms it with an accept, not the vehicle;
- while it is pending, the server does not apply `move_vehicle` and `move_player_pos`, and each of
  them can cause a resend with the seat position.

See results R19, R20 and R23 (case root).

**TP-CONFIRM-RID-004** (`black_box_confirmed`, semantic). When a teleport moves a vehicle with a
player to another dimension, the player goes with it and keeps riding. The player gets `respawn`
for the new dimension, then teleports, then `add_entity` for the vehicle with a new entity id, and
`set_passengers` that links the player to the vehicle again. In result R24 the player gets three
teleports: one at the old position before `respawn`, and two in the new dimension. This rule rests
on one observation.

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

- **U1**: closed by TP-CONFIRM-RES-006. The interval is more than 20 ticks. When the client sends
  a move each second, the interval between resends is 1 second or 2 seconds (result R14).
- **U2**: closed. The limit of TP-CONFIRM-ACC-004 is the coarse speed check of MVC-VAN-008 in
  `movement-checks.md`, which step 2 of TP-CONFIRM-ACC-009 applies: from the reference.
- **U3**: closed by TP-CONFIRM-ACC-009, from the reference. The yaw and the pitch of an accept set
  the rotation of the player. The experiments did not read the rotation.
- **U4**: closed by TP-CONFIRM-ID-003, from the reference.
- **U5**: closed by TP-CONFIRM-RES-006, from the reference. Each move packet causes a resend. The
  experiments send only `move_player_pos`.
- **U6**: closed by TP-CONFIRM-ACC-010, from the reference.
- **U7**: what a Bedrock client does after a `MovePlayer` teleport: whether it sets `HandledTeleport`
  in one `PlayerAuthInput` for each teleport, and whether the position of that `PlayerAuthInput` is
  the destination. TP-CONFIRM-EVT-003 assumes one flag for each teleport. An experiment on a Bedrock
  client resolves this.
- **U8**: which `SetActorLink` link type a server of Bedrock Edition sends for the driver and for
  the other passengers of a vehicle (TP-CONFIRM-RID-005).
- **U9**: whether the check of `checkForBlocks` in Bedrock Edition uses the collision box of the
  entity or one block (TP-CONFIRM-COL-001).

## 6. Experiments

For E1 to E17, use a vanilla server of the game version in the table at the top of this document.
For E18 to E24, use `26.4-snapshot-7` (protocol `1073742169`). Set these
properties: `online-mode=false`, `network-compression-threshold=-1`, `gamemode=creative`. Connect a
scripted client that sends `player_loaded` when the play state starts. Answer each `keep_alive` and
`ping`, and send `client_tick_end` each 50 ms. Read a position with the console command
`data get entity <name> Pos`.

| Experiment | Steps | Results |
|------------|-------|---------|
| E1 | Send an accept that holds only `teleport_id`. | R1 |
| E2 | Join, and read the first `teleport_id`. | R2 |
| E3 | Confirm the first teleport. Send `tp` two times without an accept. Send an accept with the older id. Send a move, then read the position. Send an accept with the newest id. Send a move, then read the position. | R3 |
| E4 | Do not send an accept. Wait 1.5 seconds. Send a move. | R4 |
| E5 | Send an accept with the id plus 1000 and a position 5 blocks away. Read the position. Send a move, and read the position. Send an accept with the correct id. Send a move, and read the position. | R5 |
| E6 | Continue from E4. Send an accept with the replaced id. Send a move, and read the position. Send an accept with the newest id. Send a move, and read the position. | R6 |
| E7 | Send an accept with the correct id and a position 5 blocks away. Read the position. | R7 |
| E8 | Send an accept with the correct id and a position 1000 blocks away. Read the position. | R8 |
| E9 | Confirm the first teleport. Send the same accept again. | R9 |
| E10 | Confirm the first teleport. Send an accept with the id plus 1000. | R10 |
| E11 | Read the position. Send a move. Read the position. Confirm. Send the move again. Read the position. | R11 |
| E12 | Do not send an accept or a move for 25 seconds. | R12 |
| E13 | Confirm the first teleport. Send `tp`. Do not send an accept or a move for 10 seconds. | R13 |
| E14 | Do not send an accept. Send a move each second for 25 seconds. Separately, wait 5 seconds, then send one move. | R14 |
| E15 | Do not send an accept. Send a move each 50 ms for 12 seconds. | R15 |
| E16 | Send an accept with the correct id and a position 1000 blocks away. Read the position. Send a move, and read the position. Send an accept with the new id at the position of the teleport. Send a move, and read the position. | R16 |
| E17 | Send an accept with the correct id and a position 1000 blocks away. Each 0.2 seconds, do this again with the new id, two more times. | R17 |
| E18 | Summon `oak_boat` at the player and run `ride <player> mount` on it. Run `tp <player>` 10 blocks away. Read the packets, the position of the player, `RootVehicle` and the position of the boat. | R18 |
| E19 | Do E18, but run `tp` on the boat. | R19 |
| E20 | Do E19 and send no accept. After 1.5 seconds, send `move_vehicle` 1 block away. After 1.5 seconds more, send a move. Read the packets and the positions. | R20 |
| E21 | Do E18 and send no accept. After 1.5 seconds, send `move_vehicle` at the old position of the boat. | R21 |
| E22 | Summon a pig and run `ride <pig> mount <player>`. | R22 |
| E23 | Summon a minecart and a saddled pig. Put the pig on the minecart and the player on the pig. Run `tp` 10 blocks away on the minecart, on the pig and on the player, each time with a new stack. Read the packets of the player and of an observer, the positions and the passengers. | R23 |
| E24 | Do E19 with `execute in minecraft:the_nether run tp` on the boat. | R24 |

## 7. Captured results

Times are in seconds from the start of the probe. Each server console reply is shortened to the
position.

**R1.** `Internal Exception: ... Failed to decode packet 'serverbound/minecraft:accept_teleportation'`
is the disconnect reason, and it comes at once.

**R2.** Every connection in these results gets `player_position id=1` first.

**R3.**

```text
19.037 send accept 1
19.540 console: tp E3Old 10 100 10
19.584 player_position id=2 pos=(10.500,100.000,10.500)
19.842 console: tp E3Old 20 100 20
19.880 player_position id=3 pos=(20.500,100.000,20.500)
20.144 send accept 2 at (20.5,100.0,20.5)
21.653 no disconnect
21.653 send move (21,100,21)
21.677 player_position id=4 pos=(20.500,100.000,20.500)
21.955 position [20.5, 100.0, 20.5]
21.955 send accept 4 at (20.5,100.0,20.5)
22.459 send move (21,100,21)
22.761 position [21.0, 100.0, 21.0]
```

**R4.**

```text
35.354 player_position id=1 pos=(-39.500,74.000,-0.500)
36.863 send move (-38.5,74.0,-0.5)
36.899 player_position id=2 pos=(-39.500,74.000,-0.500)
```

**R5.**

```text
 9.222 player_position id=1 pos=(-37.500,80.000,0.500)
 9.474 send accept 1001 at (-32.5,80.0,0.5)
10.029 position [-37.5, 80.0, 0.5]
10.029 send move (-36.5,80.0,0.5)
10.331 position [-37.5, 80.0, 0.5]
10.331 send accept 1 at (-37.5,80.0,0.5)
10.583 send move (-36.5,80.0,0.5)
10.885 position [-36.5, 80.0, 0.5]
10.885 no disconnect
```

**R6.**

```text
37.115 send accept 1 at (-39.5,74.0,-0.5)
37.367 send move (-38.5,74.0,-0.5)
37.669 position [-39.5, 74.0, -0.5]
37.669 send accept 2 at (-39.5,74.0,-0.5)
37.922 send move (-38.5,74.0,-0.5)
38.224 position [-38.5, 74.0, -0.5]
```

**R7.**

```text
35.732 player_position id=1 pos=(-38.500,80.000,-4.500)
36.035 position [-38.5, 80.0, -4.5]
36.035 send accept 1 at (-33.5,80.0,-4.5)
36.588 position [-33.5, 80.0, -4.5]
```

**R8.**

```text
39.554 player_position id=1 pos=(-37.500,83.000,3.500)
39.806 send accept 1 at (962.5,83.0,3.5)
39.849 player_position id=2 pos=(-37.500,83.000,3.500)
40.862 position [-37.5, 83.0, 3.5]
```

**R9.**

```text
11.888 player_position id=1
12.140 send accept 1
13.148 no disconnect
13.148 send accept 1
13.179 disconnect, translation key multiplayer.disconnect.invalid_player_movement
```

**R10.**

```text
12.157 player_position id=1
12.409 send accept 1
12.912 send accept 1001
13.919 no disconnect
```

**R11.**

```text
15.834 player_position id=1 pos=(-38.500,81.000,-7.500)
16.188 position [-38.5, 81.0, -7.5]
16.188 send move (-37.5,81.0,-7.5)
16.742 position [-38.5, 81.0, -7.5]
16.742 send accept 1
17.044 position [-38.5, 81.0, -7.5]
17.044 send move (-37.5,81.0,-7.5)
17.598 position [-37.5, 81.0, -7.5]
```

**R12.**

```text
37.836 player_position id=1
62.989 no other teleport, no disconnect
```

**R13.**

```text
24.085 send accept 1
24.382 player_position id=2 pos=(30.500,100.000,30.500)
34.394 no other teleport, no disconnect
```

**R14.** A move each second:

```text
64.286 id=1   65.328 id=2   67.327 id=3   69.327 id=4   71.327 id=5   72.378 id=6   74.427 id=7
76.478 id=8   78.477 id=9   80.478 id=10  82.478 id=11  83.529 id=12  85.527 id=13  87.529 id=14
```

One move after a wait of 5 seconds:

```text
28.555 player_position id=1 pos=(-28.500,63.000,2.500)
33.588 send move (-27.5,63.0,2.5)
33.600 player_position id=2 pos=(-28.500,63.000,2.500)
```

**R15.** A move each 50 ms, all at the position of the first teleport:

```text
15.205 id=1   16.249 id=2   17.300 id=3   18.350 id=4   19.400 id=5   20.449 id=6
21.499 id=7   22.549 id=8   23.599 id=9   24.649 id=10  25.699 id=11  26.749 id=12
27.280 no disconnect
```

**R16.**

```text
 7.724 player_position id=1 pos=(-40.500,78.000,9.500)
 7.977 send accept 1 at (959.5,78.0,9.5)
 8.020 player_position id=2 pos=(-40.500,78.000,9.500)
 8.280 position [-40.5, 78.0, 9.5]
 8.280 send move (-39.5,78.0,9.5)
 8.633 position [-40.5, 78.0, 9.5]
 8.633 send accept 2 at (-40.5,78.0,9.5)
 8.884 send move (-39.5,78.0,9.5)
 9.186 position [-39.5, 78.0, 9.5]
```

**R17.**

```text
10.680 send accept 1 at (971.5,63.0,-7.5)
10.719 player_position id=2 pos=(-28.500,63.000,-7.500)
10.881 send accept 2 at (971.5,63.0,-7.5)
10.920 player_position id=3 pos=(-28.500,63.000,-7.500)
11.082 send accept 3 at (971.5,63.0,-7.5)
11.119 player_position id=4 pos=(-28.500,63.000,-7.500)
```

**R18.** Teleport of a player that rides a boat. The boat is at (0.5, -60, 0.5).

```text
tp <player> 10.5 -60 10.5
P: set_passengers boat []; player_position id=5 pos=(10.5, -60.0, 10.5); set_passengers boat []
W: set_passengers boat []
after: player at [10.5, -60.0, 10.5]; RootVehicle: none; boat at [0.5, -60.0, 0.5]
```

**R19.** Teleport of a boat that carries the player.

```text
tp <boat> 10.5 -60 10.5
P: player_position id=8 pos=(10.5, -60.41, 10.5); teleport_entity boat pos=(10.5, -60.0, 10.5); entity_position_sync boat
W: entity_position_sync boat
after: player at [10.5, -60.4125, 10.5]; RootVehicle: oak_boat; boat at [10.5, -60.0, 10.5]
```

**R20.** R19 with no accept.

```text
tp <boat> 10.5 -60 10.5      -> player_position id=12 pos=(10.5, -60.41, 10.5); teleport_entity boat
+1.5 s move_vehicle (11.5, -60, 10.5)   -> player_position id=13 pos=(10.5, -60.41, 10.5); boat stays at [10.5, -60.0, 10.5]
+1.5 s move_player_pos (11.5, -60, 10.5) -> player_position id=14 pos=(10.5, -60.41, 10.5); boat stays; player at [10.5, -60.4125, 10.5]
no disconnect
```

**R21.** R18 with no accept.

```text
tp <player> 10.5 -60 10.5    -> set_passengers boat []; player_position id=18 pos=(10.5, -60.0, 10.5); RootVehicle: none
+1.5 s move_vehicle (1.5, -60, 0.5)     -> player_position id=19 pos=(10.5, -60.0, 10.5); boat at [0.5, -60.0, 0.5]; player at [10.5, -60.0, 10.5]
```

**R22.**

```text
ride <pig> mount <player>    -> reply "Players can't be ridden"
```

**R23.** A stack: minecart at (0.5, -60, 0.5), pig on the minecart, player on the pig.

```text
root (tp minecart):  P: player_position pos=(10.5, -59.54, 10.5); teleport_entity minecart
                     after: minecart [10.5, -60.0, 10.5]; pig [10.5, -59.8125, 10.5] on the minecart;
                            player [10.5, -59.54375, 10.5]; RootVehicle: minecart
middle (tp pig):     P: player_position pos=(10.5, -59.73, 10.5); teleport_entity pig; set_passengers minecart []
                     W: set_passengers minecart []
                     after: minecart [0.5, -60.0, 0.5] with no passengers; pig [10.5, -60.0, 10.5];
                            player [10.5, -59.73125, 10.5]; RootVehicle: pig
player (tp player):  P: set_passengers pig []; player_position pos=(10.5, -60.0, 10.5)
                     W: set_passengers pig []
                     after: minecart and pig stay; player [10.5, -60.0, 10.5]; RootVehicle: none
```

**R24.** A boat with the player, to the Nether.

```text
execute in minecraft:the_nether run tp <boat> 0.5 100 0.5
P: set_passengers boat []; player_position id=34 pos=(0.5, -60.0, 1.77); respawn the_nether;
   remove_entities; player_position id=35 pos=(0.5, 100.0, 1.77); player_position id=36 pos=(0.5, 99.59, 0.5);
   set_passengers new_boat [player]; add_entity new_boat (oak_boat) pos=(0.5, 100.0, 0.5);
   set_passengers new_boat [player] (two more times)
after: Dimension the_nether; RootVehicle: oak_boat
```
