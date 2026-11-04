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
- **Behavior:** a server without the file, or with a file without the property, takes that default
  and writes the property to the file when it starts.

Provenance: `black_box_confirmed` (R1, R3, R4, R19, and `status.md`, section 7.1, for the default).
Kind: semantic. Confidence: high.

### WL-CFG-002: `whitelist on` and `whitelist off`

- **Behavior:** the commands `whitelist on` and `whitelist off` write the new value of
  `white-list` to `server.properties` at once.
- **Behavior:** the value stays after a restart.

Provenance: `black_box_confirmed` (R3, R4, R20). Kind: semantic. Confidence: high.

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
- **Behavior:** a reload does not remove that entry from the file. The next write of the file
  removes it (WL-CMD-002).

Provenance: `black_box_confirmed` (R6, R16). Kind: semantic. Confidence: high.

### WL-FILE-004: A file that does not hold a JSON array

- **Behavior:** when the whitelist file is not valid JSON or does not hold a JSON array, a reload
  makes the whitelist empty.
- **Behavior:** the server keeps the file, logs a warning and replies as for a good reload.
- **Behavior:** the same file at the start of the server gives an empty whitelist and a warning.
  The server continues to start.
- **Behavior:** the stop of the server does not write the file, after a start or after a reload.
- **Behavior:** the next write of the file replaces the content (WL-CMD-002).

Provenance: `black_box_confirmed` (R7, R15, R16, R24, R26). Kind: semantic. Confidence: high.

### WL-FILE-005: A missing file

- **Behavior:** when the whitelist file does not exist at the start of the server, the server
  creates it with an empty array and starts with an empty whitelist.
- **Behavior:** when the whitelist file does not exist at a reload, the whitelist does not change.
  The server does not create the file and replies as for a good reload. It disconnects no listed
  player, and a listed player can join.
- **Edge cases:** U9 covers the next write of the file.

Provenance: `black_box_confirmed` (R15, R26). Kind: semantic. Confidence: medium. The rule for a
start rests on one observation.

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

- **Behavior:** each reply is a translatable component with the key in this table. The console
  shows the English text. `<name>` is the name that WL-CMD-004 gives, not the name of the entry.
- **Behavior:** a reply that the table marks as failed has the form
  `{"text": "", "color": "red", "extra": [{"translate": <key>}]}`.
- **Behavior:** the argument of `add.success` and `remove.success` is `<name>`.
  `commands.whitelist.list` has two arguments: the number of entries as an integer, and the names
  joined with `, ` as one string. U8 covers the order of the names.

| Command | State | Key | Reply on the console |
|---------|-------|-----|----------------------|
| `whitelist list` | empty | `commands.whitelist.none` | `There are no whitelisted players` |
| `whitelist list` | one entry | `commands.whitelist.list` | `There are 1 whitelisted player(s): wa` |
| `whitelist add <name>` | new | `commands.whitelist.add.success` | `Added <name> to the whitelist` |
| `whitelist add <name>` | listed, failed | `commands.whitelist.add.failed` | `Player is already whitelisted` |
| `whitelist remove <name>` | listed | `commands.whitelist.remove.success` | `Removed <name> from the whitelist` |
| `whitelist remove <name>` | not listed, failed | `commands.whitelist.remove.failed` | `Player is not whitelisted` |
| `whitelist on` | off | `commands.whitelist.enabled` | `Whitelist is now turned on` |
| `whitelist on` | on, failed | `commands.whitelist.alreadyOn` | `Whitelist is already turned on` |
| `whitelist off` | on | `commands.whitelist.disabled` | `Whitelist is now turned off` |
| `whitelist off` | off, failed | `commands.whitelist.alreadyOff` | `Whitelist is already turned off` |
| `whitelist reload` | any | `commands.whitelist.reloaded` | `Reloaded the whitelist` |

The permission levels of the command are in `command-permissions.md`.

Provenance: `black_box_confirmed` (R2, R3, R6, R13, R16, R17, R18, R20). Kind: wire for the keys and
the form. Confidence: high.

### WL-CMD-002: Add and remove write the file

- **Behavior:** `whitelist add` and `whitelist remove` write the whitelist file at once.
- **Behavior:** the write replaces the whole file with the whitelist in memory. Thus it removes the
  entries that WL-FILE-003 ignores, and it replaces a file of WL-FILE-004.
- **Behavior:** a `whitelist remove` that fails does not write the file.

Provenance: `black_box_confirmed` (R2, R16). Kind: semantic. Confidence: high.

### WL-CMD-003: A new name in offline mode

- **Behavior:** this rule applies in offline mode to a name that the server does not know and that
  no account holds.
- **Behavior:** `whitelist add` stores the name with the same case as in the command. The id of
  the entry is the version 3 UUID of `OfflinePlayer:<name>`.
- **Behavior:** that id is the profile id of the player with that name. Thus the player can join.
- **Edge cases:** U1 covers a name that an account holds. WL-CMD-004 covers a player that is
  online.

Provenance: `black_box_confirmed` (R2) for the form of the entry. The rule corrects the reference,
and WL-DEV-001 records the reference behaviour. Kind: semantic. Confidence: high.

### WL-CMD-004: How `whitelist add` and `whitelist remove` find an entry

- **Behavior:** in offline mode, the command turns the name into a profile.
  - A name of a player that is online matches with any case. The profile has the id and the name
    of that player.
  - For a name that WL-CMD-003 covers, the profile is the one in that rule, also when that player
    joined before. WL-DEV-001 and WL-DEV-004 record the vanilla server.
  - U1 covers a name that an account holds.
- **Behavior:** the command compares the id of the profile with the id of each entry. It does not
  compare the name of the entry.
- **Edge cases:**
  - The entry `{<the id of Wf>, "Other"}`: `whitelist remove Wf` removes it, and `whitelist add Wf`
    fails. `whitelist remove Other` fails, and `whitelist add Other` adds a second entry with the
    name `Other`.

Provenance: `black_box_confirmed` (R21). The rule corrects the reference for a player that is not
online, and WL-DEV-004 records the reference behaviour. Kind: semantic. Confidence: high.

### WL-CMD-005: Who receives the replies

- **Behavior:** the replies follow the conditions of CMDP-EXEC-004.
- **Behavior:** a player that runs the command receives a failed reply in each case. The player
  receives a reply of success, also from `whitelist list`, only when the game rule
  `send_command_feedback` is true. The console receives each reply in each case.
- **Behavior:** after a success of `whitelist add`, `whitelist remove`, `whitelist on`,
  `whitelist off` or `whitelist reload` by a player, each other online operator also receives the
  message in the form of CMDP-EXEC-004, with `chat.type.admin`, in gray and italic, when
  `send_command_feedback` is true. A player that is not an operator receives nothing.
- **Behavior:** `whitelist list` and a failed command reach the source only.
- **Behavior:** after such a success by the console, each online operator receives the
  `chat.type.admin` form with the source name `Server`, when `send_command_feedback` is true and
  the property `broadcast-console-to-ops` is true.
- **Behavior:** after such a success by a player, the server log receives the text
  `[<player>: <message>]` when the game rule `log_admin_commands` is true. The value of
  `send_command_feedback` does not change this. `log_admin_commands` does not change what a player
  receives.

Provenance: `black_box_confirmed` (R22, R25). The results in R25 come from one run. Kind: semantic.
Confidence: high.

### WL-CMD-006: The result of the command

- **Behavior:** `execute store result` receives the number of entries from `whitelist list`, 1 from
  another success, and 0 from a failed command.

Provenance: `black_box_confirmed` (R23). Kind: semantic. Confidence: high.

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
- **Behavior:** with the whitelist check off, `whitelist remove` and `whitelist reload` do not
  disconnect a player.

Provenance: `black_box_confirmed` (R14, R18). Kind: semantic. Confidence: high.

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
- **Reference, `whitelist remove`:** a name that the server does not know becomes the id of the
  lower-case name. The vanilla server does not remove the entry `WlUpper` with
  `whitelist remove WlUpper` (R21). This server removes it, since the entry has the id of that name.
- **Test:** a differential test skips `whitelist add` and `whitelist remove` with an upper-case name
  in offline mode.

### WL-DEV-002: The default of the whitelist check

- **Rule:** WL-CFG-001.
- **Reference:** the whitelist check is on when no setting names it (R19).
- **This server:** the whitelist check is off when no setting names it. This server has no console,
  and `ops.json` starts empty, so the reference default refuses every player until an edit of the
  files. The default of `enforce-whitelist` stays `false`.
- **Visible to the client:** yes. On a new server, a player that is not listed joins.
- **Test:** a differential test sets `white-list` on both servers.

### WL-DEV-003: `whitelist on` and `whitelist off` after a restart

- **Rule:** WL-CFG-002.
- **Reference:** the commands write `white-list` to `server.properties`, and the value stays after
  a restart (R3, R4, R20).
- **This server:** the settings come from the environment, and this server has no
  `server.properties`. The commands change the running server only. After a restart, the setting
  applies again.
- **Visible to the client:** yes, after a restart that follows one of the commands.
- **Test:** a differential test does not restart the server after these commands.

### WL-DEV-004: A player that joined before and is not online

- **Rule:** WL-CMD-004.
- **Reference:** the vanilla server also matches the name of a player that joined before, with any
  case. After `WlUp` joins and leaves, `whitelist remove wlup` removes the entry of `WlUp` (R21).
- **This server:** it does not keep the players that joined. In offline mode, the client chooses the
  name, so a record of each name grows without a limit. `whitelist remove wlup` fails for the entry
  of `WlUp` when that player is not online. `whitelist remove WlUp` removes it.
- **Visible to the client:** yes, in the reply and in the whitelist file.
- **Test:** a differential test uses the exact case for a player that is not online.

## 6. Unresolved

- **U1**: `whitelist add` looks for the name at an external service. A name that an account holds
  gets another result: `whitelist add LB` replies with `LB`, and the server refuses the player `Lb`.
  The experiments did not read the stored id, and they did not run without a network.
- **U4**: the behaviour in online mode.
- **U5**: the reply when the console names several players or a selector.
- **U7**: whether WL-ENF-003 is an accident of the reference. The operator that WL-ENF-002
  disconnects can join again at once (WL-JOIN-004).
- **U8**: the order of the names in `commands.whitelist.list`. After `whitelist add wlkeya` and
  `whitelist add wlkeyb`, the list shows `wlkeyb, wlkeya` (R17).
- **U9**: the content of the file after a command writes it, when the file did not exist at the last
  reload. The whitelist still holds the old entries (WL-FILE-005). The experiments did not test this.

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
| E15 | Start the server without the file, then with `not json` in it, then with `{}` in it. List, join, stop, and read the file each time. | R15 |
| E16 | Write entries without both fields and one good entry. Reload, then run `whitelist add` and `whitelist remove`, and read the file after each. Write `not json`, reload, run `whitelist remove` for a name that is not listed, then `whitelist add`, and read the file after each. | R16 |
| E17 | Connect a scripted client as an operator. Send each form of WL-CMD-001 as `chat_command`, and read `system_chat`. | R17 |
| E18 | With `enforce-whitelist=true` and `white-list=false`, keep a listed player and a player that is not listed online. Run `whitelist reload`, then `whitelist remove` for the listed player. | R18 |
| E19 | Remove the lines `white-list` and `enforce-whitelist`, start, join, stop, and read the file. Delete `server.properties`, start, and read the new file. | R19 |
| E20 | With the whitelist check on, run `whitelist off` on the console, read `server.properties`, and restart. | R20 |
| E21 | Use a new name for each step, and wait for the reply. Write entries where the id and the name differ, reload, and run `whitelist remove` and `whitelist add` with each name, in each case. Join as a player with an upper-case letter in the name, and repeat. | R21 |
| E22 | Keep two operators and a player that is not an operator online. One operator runs each form of WL-CMD-001. Then the console runs `whitelist add`. | R22 |
| E23 | Run `execute store result score #x r run <command>` for each form of WL-CMD-001, and read the score. | R23 |
| E24 | Start with `{}` in the file, stop, and read the file. Then start, add a name, write `{}`, and reload. List, remove a name that is not listed, add a name, and read the file after each. Write `{}` again, reload, stop, and read the file. Use a new name for each step, and wait for the reply. | R24 |
| E25 | Keep two operators and a player that is not an operator online. Set `send_command_feedback` to false, and run each form of WL-CMD-001 as an operator and `whitelist add`, `remove` and `list` on the console. Repeat `add`, `remove` and `reload` with `log_admin_commands` false. Restart with `broadcast-console-to-ops=false`, and repeat them on the console and as an operator. Read the replies and the server log. | R25 |
| E26 | With the whitelist check and `enforce-whitelist` on, write two entries and reload. Keep players online, and write `not json` to the file or delete it. Reload and list. Read the file, and join as a listed player and as a new name. Do `not json` three times and the delete four times, with new names for each round. In the rounds with `not json` and in one delete, keep online both listed players and an operator that is not listed. In the other deletes, keep online one listed player. | R26 |

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

**R15.**

```text
no file at the start: the server starts, whitelist.json holds []
  whitelist list -> There are no whitelisted players, and a player is refused
whitelist.json: not json, at the start: a warning in the server log, the server starts
  whitelist list -> There are no whitelisted players, and a player is refused
  after stop, whitelist.json still holds: not json
whitelist.json: {}, at the start: a warning in the server log, the server starts
  whitelist list -> There are no whitelisted players
```

**R16.**

```text
whitelist.json: [{"name": "wlonlyname"}, {"uuid": <an id>}, {"uuid": <the id of wlkept>, "name": "wlkept"}]
whitelist reload, whitelist list -> There are 1 whitelisted player(s): wlkept
whitelist add wlnewone -> whitelist.json holds wlkept and wlnewone, with an indent of two spaces
whitelist remove wlkept -> Removed wlkept from the whitelist, whitelist.json holds wlnewone

whitelist.json: not json, whitelist reload
whitelist remove wlabsent -> Player is not whitelisted, whitelist.json still holds: not json
whitelist add wlafterbad -> whitelist.json holds wlafterbad
```

**R17.** Each line shows the text component of one `system_chat` that the operator reads.

```text
whitelist list          -> {"translate": "commands.whitelist.none"}
whitelist add wlkeya    -> {"translate": "commands.whitelist.add.success", "with": ["wlkeya"]}
whitelist add wlkeya    -> {"text": "", "color": "red", "extra": [{"translate": "commands.whitelist.add.failed"}]}
whitelist list          -> {"translate": "commands.whitelist.list", "with": [1, "wlkeya"]}
whitelist add wlkeyb, whitelist list -> the names are "wlkeyb, wlkeya"
whitelist remove wlkeyb -> {"translate": "commands.whitelist.remove.success", "with": ["wlkeyb"]}
whitelist remove wlkeyb -> the red form, "commands.whitelist.remove.failed"
whitelist off           -> the red form, "commands.whitelist.alreadyOff"
whitelist on            -> {"translate": "commands.whitelist.enabled"}
whitelist on            -> the red form, "commands.whitelist.alreadyOn"
whitelist off           -> {"translate": "commands.whitelist.disabled"}
whitelist reload        -> {"translate": "commands.whitelist.reloaded"}
```

**R18.**

```text
enforce-whitelist=true, white-list=false, wlusix listed, wlunlisted not listed, both online
whitelist reload -> both still online after 1.5 seconds
whitelist remove wlusix -> both still online after 1.5 seconds
```

**R19.**

```text
without the lines: a player is refused, then server.properties has white-list=true and
  enforce-whitelist=false
without the file: the new server.properties has white-list=true and enforce-whitelist=false
```

**R20.**

```text
whitelist off -> Whitelist is now turned off
server.properties white-list=false, also after the restart
```

**R21.** `<n>` is a name that the server does not know, and `id(<n>)` is the id of that name.

```text
{id(<n>), "wlsomeother"}:  whitelist remove <n> -> Removed <n> from the whitelist, the file holds []
{id(<n>), "WLNAME"}:       whitelist remove <n> -> Removed <n> from the whitelist, the file holds []
{<another id>, <n>}:       whitelist remove <n> -> Player is not whitelisted, the file is unchanged
{id(<n>), <n>}:            whitelist remove <N in upper case> -> Removed <n> from the whitelist
{id(Wl..), "Wl.."}:        whitelist remove Wl.. -> Player is not whitelisted, the file is unchanged
{id(<n>), "wlsomeother"}:  whitelist add <n> -> Player is already whitelisted, the file is unchanged
{<another id>, <n>}:       whitelist add <n> -> Added <n> to the whitelist, the file has both entries

after Wl.. joins:
{id(Wl..), "wlsomeother"}: whitelist remove Wl.. -> Removed Wl.. from the whitelist
{id(Wl..), "Wl.."}:        whitelist remove wl.. -> Removed Wl.. from the whitelist
[]:                        whitelist add wl.. -> Added Wl.. to the whitelist, {id(Wl..), "Wl.."}
```

**R22.** `a` and `b` are operators, and `c` is not.

```text
a: whitelist add wlaud
  a <- {"translate": "commands.whitelist.add.success", "with": ["wlaud"]}
  b <- {"translate": "chat.type.admin", "color": "gray", "italic": true,
        "with": [<the display name of a>, {"translate": "commands.whitelist.add.success", "with": ["wlaud"]}]}
the same two messages for remove.success, enabled, disabled and reloaded
only a receives: none, list, add.failed, remove.failed, alreadyOn, alreadyOff
c receives none of them
console: whitelist add wlbyconsole -> a and b <- chat.type.admin, with "Server" and add.success
console: whitelist list -> a and b receive nothing
```

**R23.**

```text
list, empty -> 0        list, one entry -> 1        list, two entries -> 2
add, new -> 1           add, listed -> 0
remove, listed -> 1     remove, not listed -> 0
on, off -> 1            on, on -> 0
off, on -> 1            off, off -> 0
reload -> 1
```

**R24.** `<n>` is a new name for each step.

```text
whitelist.json: {}, at the start, then stop -> whitelist.json still holds: {}

whitelist add <n>
whitelist.json: {}, whitelist reload -> a warning in the server log, Reloaded the whitelist
whitelist list -> There are no whitelisted players, whitelist.json still holds: {}
whitelist remove <n> -> Player is not whitelisted, whitelist.json still holds: {}
whitelist add <n> -> Added <n> to the whitelist, whitelist.json holds that entry
whitelist.json: {}, whitelist reload, whitelist list -> There are no whitelisted players
after stop, whitelist.json still holds: {}
```

**R25.** `a` and `b` are operators, and `c` is not. `c` received no reply in any step. The names in
the first two blocks were held by accounts, so the replies show the case of the account.

```text
send_command_feedback=false, log_admin_commands=true
a: list, add, remove, on, off, reload, each with success -> a and b receive nothing
a: add, remove, on, off, each with failure -> a receives the red reply
a: add -> the server log: [a: Added <n> to the whitelist]
console: add, remove, list -> the console receives the reply, a and b receive nothing

send_command_feedback=true, log_admin_commands=false
a: add, remove, reload -> a receives the reply, b receives chat.type.admin, the server log has no line
console: add, remove -> the console receives the reply, a and b receive chat.type.admin with "Server"

send_command_feedback=true, log_admin_commands=true, broadcast-console-to-ops=false
console: add, remove, reload -> the console receives the reply, a and b receive nothing
a: add, remove -> a receives the reply, b receives chat.type.admin
```

**R26.** `enforce-whitelist=true`, with the whitelist check on.

```text
whitelist.json: not json, whitelist reload -> a warning in the server log, Reloaded the whitelist
both listed players and the operator disconnected with multiplayer.disconnect.not_whitelisted
whitelist list -> There are no whitelisted players, whitelist.json still holds: not json
a listed player and a new name refused with multiplayer.disconnect.not_whitelisted
the same in each of 3 rounds

whitelist.json deleted, whitelist reload -> Reloaded the whitelist
whitelist list -> There are 2 whitelisted player(s), with the same names
each listed player that is online stays online, whitelist.json does not exist
a new name refused with multiplayer.disconnect.not_whitelisted
the same in each of 4 rounds
in the 3 rounds with one listed player online, the other listed player joins
in the round with an operator that is not listed, the operator disconnected with the same key
```
