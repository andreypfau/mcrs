# The whitelist: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

This document states how a server limits which players can join. It states what a client, the
server console and the server files show. It does not prescribe an architecture. Every rule comes
from a vanilla server in offline mode. Section 7 gives the experiments, and section 8 gives the
captured results.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **offline mode**: as `login.md`, section 0.1, defines
  them.
- **Online player**: as `status.md`, section 0.1, defines it.
- **Profile id**: the UUID of the player profile in `login_finished`. In offline mode, it is the
  version 3 UUID of `OfflinePlayer:<name>` (LOGIN-SEQ-001).
- **Whitelist**: the list of profile ids that the server holds in memory.
- **Whitelist file**: the file `whitelist.json` in the directory of the server.
- **Entry**: one object in the whitelist file.
- **Listed player**: a player whose profile id is on the whitelist.
- **Whitelist check**: the check at login that WL-JOIN-002 states.
- **Operator**: a player that `ops.json` names.
- **Enforcement**: the property `enforce-whitelist` with the value `true`.

### 0.2 Provenance and confidence

This document uses the provenance labels of `registry.md`, section 0.3. A rule with the label
`black_box_confirmed` cites the captured result of section 8 that shows it. Result R*n* comes from
experiment E*n* of section 7. Section 6 collects the items that are not decided.

All the results come from one vanilla server on one machine. A rule that rests on one observation
has the confidence medium.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `WL-CFG` | server inputs |
| `WL-FILE` | the whitelist file |
| `WL-JOIN` | the whitelist check at login |
| `WL-CMD` | the command `whitelist` |
| `WL-ENF` | enforcement for online players |
| `WL-DEV` | intentional deviations |

### 0.4 Scope

This document refines the whitelist check of LOGIN-SEQ-002 in `login.md`. It does not state the
ban lists or the player limit, except for their order relative to the whitelist. It does not state
a server in online mode.

## 1. Server inputs

### WL-CFG-001: The property `white-list`

- **Behavior:** the property `white-list` in `server.properties` turns the whitelist check on and
  off.
- **Behavior:** the default `server.properties` of this version has `white-list=true`.

Provenance: `black_box_confirmed` (R1, R3, R4, and `status.md`, section 7.1, for the default).
Kind: semantic. Confidence: high.

### WL-CFG-002: `whitelist on` and `whitelist off`

- **Behavior:** the commands `whitelist on` and `whitelist off` write the new value of
  `white-list` to `server.properties` at once.
- **Behavior:** the value stays after a restart.

Provenance: `black_box_confirmed` (R3, R4). Kind: semantic. Confidence: high for `whitelist on`.
Confidence: medium for `whitelist off`, which no captured result shows.

### WL-CFG-003: The property `enforce-whitelist`

- **Behavior:** the property `enforce-whitelist` selects the behaviour of WL-ENF-001 and
  WL-ENF-002.

Provenance: `black_box_confirmed` (R13, R14). Kind: semantic. Confidence: high.

## 2. The whitelist file

### WL-FILE-001: Format

- **Behavior:** the whitelist file holds a JSON array. Each entry has the string fields `uuid` and
  `name`.

```json
[
  {
    "uuid": "2ab59332-79f8-3b40-9c54-ea9d7be549c8",
    "name": "wa"
  }
]
```

Provenance: `black_box_confirmed` (R2). Kind: wire. Confidence: high.

### WL-FILE-002: Reads of the file

- **Behavior:** the server reads the whitelist file when it starts and when it runs
  `whitelist reload`.
- **Behavior:** the server does not see a change to the file at another time.

Provenance: `black_box_confirmed` (R4, R5). Kind: semantic. Confidence: high.

### WL-FILE-003: An entry without both fields

- **Behavior:** the server ignores an entry that does not have both fields.
- **Behavior:** a reload does not remove that entry from the file.

Provenance: `black_box_confirmed` (R6). Kind: semantic. Confidence: high.

### WL-FILE-004: A file that is not valid JSON

- **Behavior:** when the whitelist file is not valid JSON, a reload makes the whitelist empty.
- **Behavior:** the server keeps the file, logs a warning and replies as for a good reload.

Provenance: `black_box_confirmed` (R7). Kind: semantic. Confidence: medium. The rule rests on one
observation.

## 3. Join

### WL-JOIN-001: The whitelist check is off

- **Behavior:** when the whitelist check is off, any player can join.

Provenance: `black_box_confirmed` (R1). Kind: semantic. Confidence: high.

### WL-JOIN-002: Refusal

- **Behavior:** when the whitelist check is on, the server refuses a player that is not listed and
  is not an operator.
- **Behavior:** the server sends `login_disconnect` with the translation key
  `multiplayer.disconnect.not_whitelisted`.

Provenance: `black_box_confirmed` (R8). Kind: semantic. Confidence: high.

### WL-JOIN-003: The profile id decides

- **Behavior:** the server compares the profile id only. It does not compare the name.

Provenance: `black_box_confirmed` (R9). Kind: semantic. Confidence: high.

### WL-JOIN-004: Operators

- **Behavior:** an operator can join when the operator is not listed.

Provenance: `black_box_confirmed` (R10). Kind: semantic. Confidence: medium. The rule rests on one
observation.

### WL-JOIN-005: Bans before the whitelist

- **Behavior:** the server checks a ban before the whitelist.
- **Behavior:** a banned player gets the ban reason, also when the player is not listed.

Provenance: `black_box_confirmed` (R11). Kind: semantic. Confidence: high.

### WL-JOIN-006: The whitelist before the player limit

- **Behavior:** the server checks the whitelist before the player limit.
- **Behavior:** on a full server, a player that is not listed gets
  `multiplayer.disconnect.not_whitelisted`. A listed player gets
  `multiplayer.disconnect.server_full`.

Provenance: `black_box_confirmed` (R12). Kind: semantic. Confidence: high.

## 4. Commands and online players

### WL-CMD-001: Replies

- **Behavior:** the console shows these replies in English. `<name>` is the name of the entry. The
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

The permission levels of the command are in `command-permissions.md`.

Provenance: `black_box_confirmed` (R2, R3, R6, R13). Kind: semantic. Confidence: high for the
replies that section 8 shows. Confidence: medium for `Player is not whitelisted`,
`Whitelist is now turned off` and `Whitelist is already turned off`, which no captured result
shows.

### WL-CMD-002: Add and remove write the file

- **Behavior:** `whitelist add` and `whitelist remove` write the whitelist file at once.

Provenance: `black_box_confirmed` (R2). Kind: semantic. Confidence: high for `whitelist add`.
Confidence: medium for `whitelist remove`, which no captured result shows in the file.

### WL-CMD-003: A new name in offline mode

- **Behavior:** this rule applies in offline mode to a name that the server does not know and that
  no account holds.
- **Behavior:** `whitelist add` stores the name with the same case as in the command. The id of
  the entry is the version 3 UUID of `OfflinePlayer:<name>`.
- **Behavior:** that id is the profile id of the player with that name. Thus the player can join.
- **Edge cases:** U1 covers a name that an account holds. U2 covers the name of a player that
  joined before.

Provenance: `black_box_confirmed` (R2) for the form of the entry. The rule corrects the reference,
and WL-DEV-001 records the reference behaviour. Kind: semantic. Confidence: high.

### WL-ENF-001: Without enforcement

- **Behavior:** without enforcement, a change to the whitelist does not disconnect an online
  player.
- **Behavior:** an online player that is not listed stays until the player leaves. The next join
  of that player fails (WL-JOIN-002).

Provenance: `black_box_confirmed` (R3, R13). Kind: semantic. Confidence: high.

### WL-ENF-002: With enforcement

- **Behavior:** with enforcement, the server disconnects each online player that is not listed. The
  reason is `multiplayer.disconnect.not_whitelisted`.
- **Behavior:** this occurs after `whitelist on`, after `whitelist remove` and after
  `whitelist reload`, with the whitelist check on.
- **Edge cases:** U6 covers `whitelist remove` and `whitelist reload` with the whitelist check off.

Provenance: `black_box_confirmed` (R14). Kind: semantic. Confidence: high.

### WL-ENF-003: Operators and enforcement

- **Behavior:** WL-ENF-002 also disconnects an operator that is not listed, although WL-JOIN-004
  lets that operator join.

Provenance: `black_box_confirmed` (R14). Kind: semantic. Confidence: high.

## 5. Intentional deviations

### WL-DEV-001: The case of a new name in offline mode

- **Rule:** WL-CMD-003.
- **Reference:** the vanilla server stores the name in lower case, with the id of the lower-case
  name. `whitelist add Wa` stores `wa` with the version 3 UUID of `OfflinePlayer:wa` (R2). After
  `whitelist add WiCase`, the player `wicase` joins. The server refuses the player `WiCase` with
  `multiplayer.disconnect.not_whitelisted`.
- **Visible to the client:** yes. The vanilla server refuses a player with an upper-case letter in
  the name. This server lets that player join. The console reply names `wa` on the vanilla server
  and `Wa` on this server.
- **Test:** a differential test skips `whitelist add` with an upper-case name in offline mode.

## 6. Unresolved

- **U1**: `whitelist add` looks for the name at an external service. A name that an account holds
  gets another result: `whitelist add LB` replies with `LB`, and the server refuses the player `Lb`.
  The experiments did not read the stored id, and they did not run without a network.
- **U2**: the result of `whitelist add` for the name of a player that joined the server before. No
  experiment tested it.
- **U3**: the translation keys of the replies in WL-CMD-001.
- **U4**: the behaviour in online mode.
- **U5**: the reply when the console names several players or a selector.
- **U6**: whether WL-ENF-002 applies to `whitelist remove` and `whitelist reload` when the
  whitelist check is off.
- **U7**: whether WL-ENF-003 is an accident of the reference. The operator that WL-ENF-002
  disconnects can join again at once (WL-JOIN-004).

## 7. Experiments

Use a vanilla server of the game version in the table at the top of this document, with
`online-mode=false`. Connect a scripted client for each join. Reset the lists between groups, and
restart the server after each change to `server.properties`. Use long lower-case names that no
account holds, except where a step names a player.

| Experiment | Steps | Result |
|------------|-------|--------|
| E1 | Set `white-list=false`. Join as a player that is not listed. | R1 |
| E2 | Run `whitelist add Wa`, then read the file. Run `whitelist add Wa` again. | R2 |
| E3 | Join as `WOff`. Run `whitelist on`, and read `server.properties`. Keep `WOff` online for 1.5 seconds. | R3 |
| E4 | Stop the server with the whitelist check on, restart it, run `whitelist list`, and join. | R4 |
| E5 | Write an entry to the file. Join as that player. Run `whitelist reload`. Join again. | R5 |
| E6 | Write an entry with only `name`, reload, list, and join. Do the same with only `uuid`. | R6 |
| E7 | Write `not json` to the file. Reload, list, join and read the file. | R7 |
| E8 | With the whitelist check on, join as a listed player and as a player that is not listed. | R8 |
| E9 | Write an entry with the correct id and another name, and an entry with another id and the correct name. Reload, and join as each. | R9 |
| E10 | Make a player an operator. Turn the whitelist check on without a list entry. Join. | R10 |
| E11 | With the whitelist check on, ban a listed player and a player that is not listed. Join as each. | R11 |
| E12 | With the whitelist check on, set `max-players=1` with one listed player online. Join as a listed player and as a player that is not listed. | R12 |
| E13 | With `enforce-whitelist=false`, run `whitelist remove` for an online player. Join again after it leaves. | R13 |
| E14 | With `enforce-whitelist=true`, run `whitelist on`, `whitelist remove` and `whitelist reload` with players online. Include an operator that is not listed. | R14 |

## 8. Captured results

Each console reply is shortened to its text.

**R1.** With the whitelist check off, `WOff` joins, and it reaches the play state.

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
enforce-whitelist=false, wllisted online
whitelist remove wllisted -> Removed wllisted from the whitelist
wllisted still online after 1.5 seconds
wllisted leaves and joins again: refused
```

**R14.**

```text
enforce-whitelist=true
Ea and Eo not listed, Eb listed, Eo is an operator
whitelist on -> Ea and Eo disconnected with multiplayer.disconnect.not_whitelisted, Eb stays
whitelist remove Eb -> Eb disconnected with the same key
wlreload listed and online, whitelist.json edited to []
still online after 1 second
whitelist reload -> wlreload disconnected with the same key
```
