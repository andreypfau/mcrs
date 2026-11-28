# The whitelist: behavioural specification

This document states how a server limits which players can join. It states what a client, the
server console and the server files show. It does not prescribe an architecture. Every rule comes
from a vanilla server in offline mode.

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
`black_box_confirmed` is shown by an experiment against a vanilla server. Section 6 collects the items that are not decided.

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

Provenance: `black_box_confirmed` (and `status.md` for the default).
Kind: semantic. Confidence: high.

### WL-CFG-002: `whitelist on` and `whitelist off`

- **Behavior:** the commands `whitelist on` and `whitelist off` write the new value of
  `white-list` to `server.properties` at once.
- **Behavior:** the value stays after a restart.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-CFG-003: The property `enforce-whitelist`

- **Behavior:** the property `enforce-whitelist` selects the behaviour of WL-ENF-001 and
  WL-ENF-002.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

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

Provenance: `black_box_confirmed`. Kind: wire. Confidence: high.

### WL-FILE-002: Reads of the file

- **Behavior:** the server reads the whitelist file when it starts and when it runs
  `whitelist reload`.
- **Behavior:** the server does not see a change to the file at another time.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-FILE-003: An entry without both fields

- **Behavior:** the server ignores an entry that does not have both fields.
- **Behavior:** a reload does not remove that entry from the file. The next write of the file
  removes it (WL-CMD-002).

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-FILE-004: A file that does not hold a JSON array

- **Behavior:** when the whitelist file is not valid JSON or does not hold a JSON array, a reload
  makes the whitelist empty.
- **Behavior:** the server keeps the file, logs a warning and replies as for a good reload.
- **Behavior:** the same file at the start of the server gives an empty whitelist and a warning.
  The server continues to start.
- **Behavior:** the stop of the server does not write the file, after a start or after a reload.
- **Behavior:** the next write of the file replaces the content (WL-CMD-002).

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-FILE-005: A missing file

- **Behavior:** when the whitelist file does not exist at the start of the server, the server
  creates it with an empty array and starts with an empty whitelist.
- **Behavior:** when the whitelist file does not exist at a reload, the whitelist does not change.
  The server does not create the file and replies as for a good reload. It disconnects no listed
  player, and a listed player can join.
- **Edge cases:** U9 covers the next write of the file.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: medium. The rule for a
start rests on one observation.

## 3. Join

### WL-JOIN-001: The whitelist check is off

- **Behavior:** when the whitelist check is off, any player can join.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-JOIN-002: Refusal

- **Behavior:** when the whitelist check is on, the server refuses a player that is not listed and
  is not an operator.
- **Behavior:** the server sends `login_disconnect` with the translation key
  `multiplayer.disconnect.not_whitelisted`.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-JOIN-003: The profile id decides

- **Behavior:** the server compares the profile id only. It does not compare the name.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-JOIN-004: Operators

- **Behavior:** an operator can join when the operator is not listed.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: medium. The rule rests on one
observation.

### WL-JOIN-005: Bans before the whitelist

- **Behavior:** the server checks a ban before the whitelist.
- **Behavior:** a banned player gets the ban reason, also when the player is not listed.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-JOIN-006: The whitelist before the player limit

- **Behavior:** the server checks the whitelist before the player limit.
- **Behavior:** on a full server, a player that is not listed gets
  `multiplayer.disconnect.not_whitelisted`. A listed player gets
  `multiplayer.disconnect.server_full`.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

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

Provenance: `black_box_confirmed`. Kind: wire for the keys and
the form. Confidence: high.

### WL-CMD-002: Add and remove write the file

- **Behavior:** `whitelist add` and `whitelist remove` write the whitelist file at once.
- **Behavior:** the write replaces the whole file with the whitelist in memory. Thus it removes the
  entries that WL-FILE-003 ignores, and it replaces a file of WL-FILE-004.
- **Behavior:** a `whitelist remove` that fails does not write the file.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-CMD-003: A name of a player that is not online

- **Behavior:** this rule applies in offline mode to a name of a player that is not online and that
  no account holds.
- **Behavior:** `whitelist add` stores the name with the same case as in the command. The id of
  the entry is the version 3 UUID of `OfflinePlayer:<name>`.
- **Behavior:** that id is the profile id of the player with that name. Thus the player can join.
- **Edge cases:** U1 covers a name that an account holds. WL-CMD-004 covers a player that is
  online.

Provenance: `black_box_confirmed` for the form of the entry. The rule corrects the reference,
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

Provenance: `black_box_confirmed`. The rule corrects the reference for a player that is not
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

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high for the replies.
Confidence: medium for the conditions, which rest on one run.

### WL-CMD-006: The result of the command

- **Behavior:** `execute store result` receives the number of entries from `whitelist list`, 1 from
  another success, and 0 from a failed command.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-ENF-001: Without enforcement

- **Behavior:** without enforcement, a change to the whitelist does not disconnect an online
  player.
- **Behavior:** an online player that is not listed stays until the player leaves. The next join
  of that player fails (WL-JOIN-002).

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-ENF-002: With enforcement

- **Behavior:** with enforcement, the server disconnects each online player that is not listed. The
  reason is `multiplayer.disconnect.not_whitelisted`.
- **Behavior:** this occurs after `whitelist on`, after `whitelist remove` and after
  `whitelist reload`, with the whitelist check on.
- **Behavior:** with the whitelist check off, `whitelist remove` and `whitelist reload` do not
  disconnect a player.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

### WL-ENF-003: Operators and enforcement

- **Behavior:** WL-ENF-002 also disconnects an operator that is not listed, although WL-JOIN-004
  lets that operator join.

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

## 5. Intentional deviations

### WL-DEV-001: The case of a new name in offline mode

- **Rule:** WL-CMD-003.
- **Reference:** the vanilla server stores the name in lower case, with the id of the lower-case
  name. `whitelist add Wa` stores `wa` with the version 3 UUID of `OfflinePlayer:wa`. After
  `whitelist add WiCase`, the player `wicase` joins. The server refuses the player `WiCase` with
  `multiplayer.disconnect.not_whitelisted`.
- **Visible to the client:** yes. The vanilla server refuses a player with an upper-case letter in
  the name. This server lets that player join. The console reply names `wa` on the vanilla server
  and `Wa` on this server.
- **Reference, `whitelist remove`:** a name that the server does not know becomes the id of the
  lower-case name. The vanilla server does not remove the entry `WlUpper` with
  `whitelist remove WlUpper`. This server removes it, since the entry has the id of that name.
- **Test:** a differential test skips `whitelist add` and `whitelist remove` with an upper-case name
  in offline mode.

### WL-DEV-002: The default of the whitelist check

- **Rule:** WL-CFG-001.
- **Reference:** the whitelist check is on when no setting names it.
- **This server:** the whitelist check is off when no setting names it. This server has no console,
  and `ops.json` starts empty, so the reference default refuses every player until an edit of the
  files. The default of `enforce-whitelist` stays `false`.
- **Visible to the client:** yes. On a new server, a player that is not listed joins.
- **Test:** a differential test sets `white-list` on both servers.

### WL-DEV-003: `whitelist on` and `whitelist off` after a restart

- **Rule:** WL-CFG-002.
- **Reference:** the commands write `white-list` to `server.properties`, and the value stays after
  a restart.
- **This server:** the settings come from the environment, and this server has no
  `server.properties`. The commands change the running server only. After a restart, the setting
  applies again.
- **Visible to the client:** yes, after a restart that follows one of the commands.
- **Test:** a differential test does not restart the server after these commands.

### WL-DEV-004: A player that joined before and is not online

- **Rule:** WL-CMD-004.
- **Reference:** the vanilla server also matches the name of a player that joined before, with any
  case. After `WlUp` joins, `whitelist remove wlup` removes the entry of `WlUp`. `bans.md`
  shows the same match for `ban` after the player leaves and after a restart.
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
  `whitelist add wlkeyb`, the list shows `wlkeyb, wlkeya`.
- **U9**: the content of the file after a command writes it, when the file did not exist at the last
  reload. The whitelist still holds the old entries (WL-FILE-005). The experiments did not test this.
