# The confirmation of a teleport: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

This document states how a client confirms a teleport, and what the server does until the client
confirms it. It states only what a client or a server console can see. It does not prescribe an
architecture. Every rule comes from a vanilla server. Section 5 gives the experiments, and
section 6 gives the captured results.

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

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 6.
- `source_suggested`: taken from the reference implementation, and consistent with the captured
  results.
- `unresolved`: not decided. Section 4 collects these items.

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
at the position of the teleport, and the server sends a correction. Section 4 gives the limit as
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

### 3.3 A move while a teleport is pending

**TP-CONFIRM-MOV-001** (`black_box_confirmed`, semantic). While a teleport is pending, the server
does not apply a move. See results R3, R5, R6 and R11.

**TP-CONFIRM-MOV-002** (`black_box_confirmed`, semantic). After an accept confirms the teleport, the
server applies the next move. See results R5, R6 and R11.

### 3.4 Resend

**TP-CONFIRM-RES-001** (`black_box_confirmed`, semantic). The server does not send a resend on a
timer. Without a move, the server does not send a resend. Section 4 gives the other move packets as
unresolved. A correction is not a resend (TP-CONFIRM-ACC-004). In result R12, a client that sends
no move for 25 seconds gets no resend. In result R13, a confirmed connection with a new pending
teleport gets no resend for 10 seconds.

**TP-CONFIRM-RES-002** (`black_box_confirmed`, semantic). When a move arrives while a teleport is
pending, and about 20 ticks or more have passed since the last teleport, the server sends a resend.
In results R4 and R14, the first move after a wait causes a resend at once.

**TP-CONFIRM-RES-003** (`black_box_confirmed`, semantic). While moves continue to arrive, the
interval between resends is about 20 ticks. A client that sends a move each 50 ms gets one resend
each 1.05 seconds. See result R15. Section 4 gives the exact number of ticks as unresolved.

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

## 4. Unresolved

- **U1**: closed by TP-CONFIRM-RES-006. The interval is more than 20 ticks. When the client sends
  a move each second, the interval between resends is 1 second or 2 seconds (result R14).
- **U2**: the limit on the distance in TP-CONFIRM-ACC-004. 5 blocks pass and 1000 blocks fail.
- **U3**: closed by TP-CONFIRM-ACC-009, from the reference. The yaw and the pitch of an accept set
  the rotation of the player. The experiments did not read the rotation.
- **U4**: closed by TP-CONFIRM-ID-003, from the reference.
- **U5**: closed by TP-CONFIRM-RES-006, from the reference. Each move packet causes a resend. The
  experiments send only `move_player_pos`.
- **U6**: the result of an accept with the id of an older teleport when no teleport is pending.
  TP-CONFIRM-ACC-005 and TP-CONFIRM-ACC-006 state only the newest id and an id that the server never
  sent.

## 5. Experiments

Use a vanilla server of the game version in the table at the top of this document. Set these
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

## 6. Captured results

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
