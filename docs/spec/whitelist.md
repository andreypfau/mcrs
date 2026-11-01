# The whitelist: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

This document states how a server limits which players can join. It states what a client, the
server console and the server files show. It does not prescribe an architecture. Every rule comes
from a vanilla server in offline mode. Section 6 gives the experiments, and section 7 gives the
captured results.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **offline mode**: as `login.md`, section 0.1, defines
  them.
- **Whitelist**: the list of player ids that the server holds in memory.
- **Whitelist file**: the file `whitelist.json` in the directory of the server.
- **Entry**: one object in the whitelist file.
- **Listed player**: a player whose profile id is on the whitelist.
- **Operator**: a player that `ops.json` names.
- **Enforcement**: the property `enforce-whitelist` with the value `true`.

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 7.
- `unresolved`: not decided. Section 5 collects these items.

All the results come from one vanilla server on one machine. A rule that rests on one observation
states this.

### 0.3 ID prefix

Each rule has an ID with the prefix `WL`.

### 0.4 Scope

This document refines the whitelist check of LOGIN-SEQ-002 in `login.md`. It does not state the
ban lists or the player limit, except for their order relative to the whitelist. It does not state
a server in online mode.

## 1. Server inputs

**WL-CFG-001** (`black_box_confirmed`, semantic). The property `white-list` in `server.properties`
turns the whitelist check on and off. See results R1 and R3.

**WL-CFG-002** (`black_box_confirmed`, semantic). The commands `whitelist on` and `whitelist off`
change the property in the file at once, and the value stays after a restart. See results R3 and R4.

**WL-CFG-003** (`black_box_confirmed`, semantic). The property `enforce-whitelist` selects the
behaviour of WL-ENF-001 and WL-ENF-002.

## 2. The whitelist file

**WL-FILE-001** (`black_box_confirmed`, wire). The whitelist file holds a JSON array. Each entry has
the string fields `uuid` and `name`. See result R2.

```json
[
  {
    "uuid": "2ab59332-79f8-3b40-9c54-ea9d7be549c8",
    "name": "wa"
  }
]
```

**WL-FILE-002** (`black_box_confirmed`, semantic). The server reads the whitelist file when it
starts and when it runs `whitelist reload`. It does not see a change to the file at another time.
See results R4 and R5.

**WL-FILE-003** (`black_box_confirmed`, semantic). The server ignores an entry that does not have
both fields. It does not remove that entry from the file during a reload. See result R6.

**WL-FILE-004** (`black_box_confirmed`, semantic). When the whitelist file is not valid JSON, a
reload makes the whitelist empty. The server keeps the file, logs a warning and replies as for a
good reload. See result R7. This rule rests on one observation.

## 3. Joining

**WL-JOIN-001** (`black_box_confirmed`, semantic). When the whitelist check is off, any player can
join. See result R1.

**WL-JOIN-002** (`black_box_confirmed`, semantic). When the check is on, the server refuses a player
that is not listed and is not an operator. It sends `login_disconnect` with the translation key
`multiplayer.disconnect.not_whitelisted`. See result R8.

**WL-JOIN-003** (`black_box_confirmed`, semantic). The server compares the profile id only. It does
not compare the name. See result R9.

**WL-JOIN-004** (`black_box_confirmed`, semantic). An operator can join when the operator is not
listed. See result R10. This rule rests on one observation.

**WL-JOIN-005** (`black_box_confirmed`, semantic). The server checks a ban before the whitelist. A
banned player gets the ban reason, also when the player is not listed. See result R11.

**WL-JOIN-006** (`black_box_confirmed`, semantic). The server checks the whitelist before the player
limit. On a full server, a player that is not listed gets `not_whitelisted`, and a listed player
gets `multiplayer.disconnect.server_full`. See result R12.

## 4. Commands and players that are online

### 4.1 Replies

**WL-CMD-001** (`black_box_confirmed`, semantic). The console shows these replies in English. The
translation keys are unresolved (U3).

| Command | State | Reply |
|---------|-------|-------|
| `whitelist list` | empty | `There are no whitelisted players` |
| `whitelist list` | one entry | `There are 1 whitelisted player(s): wa` |
| `whitelist add <name>` | new | `Added <name> to the whitelist` |
| `whitelist add <name>` | listed | `Player is already whitelisted` |
| `whitelist remove <name>` | listed | `Removed <name> from the whitelist` |
| `whitelist remove <name>` | not listed | `Player is not whitelisted` |
| `whitelist on` | off | `Whitelist is now turned on` |
| `whitelist on` | on | `Whitelist is already turned on` |
| `whitelist off` | on | `Whitelist is now turned off` |
| `whitelist off` | off | `Whitelist is already turned off` |
| `whitelist reload` | any | `Reloaded the whitelist` |

The levels of the command are in `command-permissions.md`.

### 4.2 Add and remove

**WL-CMD-002** (`black_box_confirmed`, semantic). `whitelist add` and `whitelist remove` change the
whitelist file at once. See result R2.

**WL-CMD-003** (`black_box_confirmed`, semantic). In offline mode, for a name that the server does
not know and that no account holds, `whitelist add` stores the name in lower case with the id of
that lower-case name. The id of `wa` is the version 3 UUID of `OfflinePlayer:wa`. A player `Wa` has
another id, so the server refuses that player. See results R2 and R13. Section 5 gives the
alternatives as U1 and U2.

### 4.3 Enforcement

**WL-ENF-001** (`black_box_confirmed`, semantic). Without enforcement, a change to the whitelist does
not disconnect a player that is online. The player stays until the player leaves, and the next join
fails. See results R3 and R14.

**WL-ENF-002** (`black_box_confirmed`, semantic). With enforcement, the server disconnects each
online player that is not listed, with `multiplayer.disconnect.not_whitelisted`. This occurs after
`whitelist on`, after `whitelist remove` and after `whitelist reload`. See result R15.

**WL-ENF-003** (`black_box_confirmed`, semantic). WL-ENF-002 also disconnects an operator that is not
listed, although WL-JOIN-004 lets that operator join. See result R15.

## 5. Unresolved

- **U1**: `whitelist add` looks for the name at an external service. A name that an account holds
  gets another result: `whitelist add LB` replies with `LB`, and the player `Lb` is refused. The
  experiments did not read the stored id, and they did not run without a network.
- **U2**: whether WL-CMD-003 is an accident of the reference. It makes `whitelist add` fail for a
  name with an upper-case letter in offline mode, until that player has joined once.
- **U3**: the translation keys of the replies in WL-CMD-001.
- **U4**: the behaviour in online mode.
- **U5**: the reply when the console names several players or a selector.

## 6. Experiments

Use a vanilla server of the game version in the table at the top of this document, with
`online-mode=false`. Connect a scripted client for each join. Reset the lists between groups, and
restart the server after each change to `server.properties`. Use long lower-case names that no
account holds, except where a step names a player.

| Experiment | Steps | Result |
|------------|-------|--------|
| E1 | Keep `white-list=false`. Join as a player that is not listed. | R1 |
| E2 | Run `whitelist add Wa`, then read the file. Run `whitelist add Wa` again. | R2 |
| E3 | Join as `WOff`. Run `whitelist on`, and read `server.properties`. Keep `WOff` online for 1.5 seconds. | R3 |
| E4 | Stop the server with the whitelist on, restart it, run `whitelist list`, and join. | R4 |
| E5 | Write an entry to the file. Join as that player. Run `whitelist reload`. Join again. | R5 |
| E6 | Write an entry with only `name`, reload, list, and join. Do the same with only `uuid`. | R6 |
| E7 | Write `not json` to the file. Reload, list, join and read the file. | R7 |
| E8 | With the check on, join as a listed player and as a player that is not listed. | R8 |
| E9 | Write an entry with the correct id and another name, and an entry with another id and the correct name. Reload, and join as each. | R9 |
| E10 | Make a player an operator. Turn the check on without a list entry. Join. | R10 |
| E11 | Ban a listed player and a player that is not listed. Join as each. | R11 |
| E12 | Set `max-players=1` with one listed player online. Join as a listed player and as a player that is not listed. | R12 |
| E13 | Run `whitelist add WiCase`. Join as `wicase` and as `WiCase`. | R13 |
| E14 | With `enforce-whitelist=false`, run `whitelist remove` for an online player. Join again after it leaves. | R14 |
| E15 | With `enforce-whitelist=true`, run `whitelist on`, `whitelist remove` and `whitelist reload` with players online. Include an operator that is not listed. | R15 |

## 7. Captured results

Each console reply is shortened to its text.

**R1.** With the check off, `WOff` joins, and it reaches the play state.

**R2.**

```text
whitelist add Wa -> Added wa to the whitelist
whitelist.json   -> [ { "uuid": "2ab59332-79f8-3b40-9c54-ea9d7be549c8", "name": "wa" } ]
whitelist add Wa -> Player is already whitelisted
whitelist list   -> There are 1 whitelisted player(s): wa
```

**R3.**

```text
whitelist on -> Whitelist is now turned on
server.properties white-list=true
whitelist on -> Whitelist is already turned on
WOff still online after 1.5 seconds
```

**R4.**

```text
stop, with the whitelist on and an entry for wn
server.properties white-list=true, whitelist.json keeps the entry
after the restart: whitelist list -> There are 1 whitelisted player(s): wn
Wp refused with multiplayer.disconnect.not_whitelisted
```

**R5.**

```text
whitelist.json edited to name wlnoreload
wlnoreload refused with multiplayer.disconnect.not_whitelisted
wlbanlist joins, from the list that the server read before
whitelist reload -> Reloaded the whitelist
wlnoreload joins
```

**R6.**

```text
whitelist.json [{"name": "Wh"}]
whitelist reload -> Reloaded the whitelist
whitelist.json unchanged
whitelist list -> There are no whitelisted players
Wh refused

whitelist.json [{"uuid": "65337305-98aa-3838-9ef5-1133463431c5"}], the id of Wj
whitelist reload -> Reloaded the whitelist
whitelist list -> There are no whitelisted players
Wj refused
```

**R7.**

```text
whitelist.json: not json
whitelist reload -> a warning in the server log, then Reloaded the whitelist
whitelist list -> There are no whitelisted players
wlnoreload refused, and this player was on the list before
whitelist.json still holds: not json
```

**R8.**

```text
wllisted joins
wlfullc gets login_disconnect {"translate":"multiplayer.disconnect.not_whitelisted"}
```

**R9.**

```text
whitelist.json: [{"uuid": <the id of Wf>, "name": "Other"},
                 {"uuid": "00000000-0000-0000-0000-000000000001", "name": "Wg"}]
Wf joins
Wg refused
whitelist.json unchanged after the reload and the joins
```

**R10.**

```text
op wlopper -> Made wlopper a server operator
whitelist on, with no entry for wlopper
wlopper joins
```

**R11.**

```text
ban wlbanlist, an entry on the whitelist
wlbanlist gets multiplayer.disconnect.banned.reason
ban wlbanonly, no entry on the whitelist
wlbanonly gets multiplayer.disconnect.banned.reason
```

**R12.**

```text
max-players=1, wlfulla online
wlfullb, listed: multiplayer.disconnect.server_full
wlfullc, not listed: multiplayer.disconnect.not_whitelisted
```

**R13.**

```text
whitelist add WiCase -> Added wicase to the whitelist
wicase joins
WiCase refused with multiplayer.disconnect.not_whitelisted
```

**R14.**

```text
enforce-whitelist=false, wllisted online
whitelist remove wllisted -> Removed wllisted from the whitelist
wllisted still online after 1.5 seconds
wllisted leaves and joins again: refused
```

**R15.**

```text
enforce-whitelist=true
Ea and Eo not listed, Eb listed, Eo is an operator
whitelist on -> Ea and Eo disconnected with multiplayer.disconnect.not_whitelisted, Eb stays
whitelist remove Eb -> Eb disconnected with the same key
wlreload listed and online, whitelist.json edited to []
still online after 1 second
whitelist reload -> wlreload disconnected with the same key
```
