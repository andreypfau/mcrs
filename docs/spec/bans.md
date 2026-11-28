# Player bans and address bans: behavioural specification

| Field | Value |
|-------|-------|
| Version | see `assets/minecraft/version.json` |

This document states how a server refuses a banned player or a banned address. It states what a
client, the server console and the server files show. Every
rule comes from a vanilla server in offline mode. Section 7 gives the experiments.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **offline mode**: as `login.md`, section 0.1, defines
  them.
- **Player ban**: an entry in the file `banned-players.json`.
- **Address ban**: an entry in the file `banned-ips.json`.
- **Ban list**: the bans that the server holds in memory.
- **Expired ban**: a ban with an `expires` time in the past.

### 0.2 Provenance

- `black_box_confirmed`: shown by an experiment of section 7.
- `unresolved`: not decided. Section 6 collects these items.

All the results come from one vanilla server on one machine. A rule that rests on one observation
states this.

### 0.3 ID prefix

Each rule has an ID with the prefix `BAN`.

## 1. Files

**BAN-FILE-001** (`black_box_confirmed`, wire). Each file holds a JSON array.

A player ban has these fields:

```json
{
  "uuid": "abe2353f-4d01-3b7f-9d0e-c7a56b0fbf26",
  "name": "blreason",
  "created": "2026-11-02 02:03:43 +0500",
  "source": "Server",
  "expires": "forever",
  "reason": "griefing the spawn"
}
```

An address ban has the field `ip` in place of `uuid` and `name`.

**BAN-FILE-002** (`black_box_confirmed`, semantic). A command that adds a ban without a reason does
not write the field `reason`.

**BAN-FILE-003** (`black_box_confirmed`, semantic). The server reads the files when it starts. It
does not see a change to a file at another time.

**BAN-FILE-004** (`black_box_confirmed`, semantic). When the server reads a player ban at its start,
these defaults apply, and it writes the result to the file.

| Field | Condition | Result |
|-------|-----------|--------|
| `created` | missing or not valid | the time of the read |
| `source` | missing | `(Unknown)` |
| `expires` | missing or not valid | `forever` |
| `reason` | missing | the field stays missing |

**BAN-FILE-005** (`black_box_confirmed`, semantic). The server drops a player ban that does not have
a `uuid`. The ban does not apply, and the write at the start removes it.

**BAN-FILE-006** (`black_box_confirmed`, semantic). A time has the form
`yyyy-MM-dd HH:mm:ss Z`. The server writes the time in its local zone.

**BAN-FILE-007** (`unresolved`). The behaviour of a server when a ban file is not valid JSON is
unresolved (U6). A vanilla server stops during its start, and the file does not change.

## 2. Joining

**BAN-JOIN-001** (`black_box_confirmed`, semantic). The server compares a player ban by the profile id
only. It does not compare the name.

**BAN-JOIN-002** (`black_box_confirmed`, semantic). A banned player gets `login_disconnect` with the
translation key `multiplayer.disconnect.banned.reason` and one argument.

| The ban | The argument |
|---------|--------------|
| has a reason | the text of the reason |
| has no reason | the component with the key `multiplayer.disconnect.banned.reason.default` |

**BAN-JOIN-003** (`black_box_confirmed`, semantic). When a player ban has an `expires` time in the
future, the reason has an `extra` component. Its translation key is
`multiplayer.disconnect.banned.expiration`, and its argument is the time. This rule
rests on one observation. The form of the time text is unresolved (U2).

**BAN-JOIN-004** (`black_box_confirmed`, semantic). An operator has no exception.

**BAN-JOIN-005** (`black_box_confirmed`, semantic). A connection from a banned address gets
`login_disconnect` with the translation key `multiplayer.disconnect.banned_ip.reason` and the reason
as its argument. This rule rests on one observation. An address ban without a reason
and with an `expires` time are unresolved (U3).

**BAN-JOIN-006** (`black_box_confirmed`, semantic). The server checks a player ban before an address
ban. This rule rests on one observation.

**BAN-JOIN-007** (`black_box_confirmed`, semantic). An address ban does not stop the STATUS protocol.

## 3. Expiry

**BAN-EXP-001** (`black_box_confirmed`, semantic). An expired ban does not refuse a player. This
includes the first join after the ban expires. See BAN-DEV-001.

**BAN-EXP-002** (`black_box_confirmed`, unspecified). The time at which a server removes an expired
ban from the ban list and from the file is unspecified.

On a vanilla server, `banlist` shows an expired ban until the join of a player with a ban. That join
removes all the expired bans from the ban list. The file keeps them until the next write.

**BAN-EXP-003** (`black_box_confirmed`, semantic). After a join of a player with a ban, `ban` for a
player whose ban has expired works as for a new ban. The new ban replaces the expired ban in the
file. A server can allow this earlier, as BAN-EXP-002 does not fix the time of the removal. The rule rests on one observation.

On a vanilla server, before that join, `ban` for that player replies
`Nothing changed. The player is already banned` and does not change the file. A differential test
ignores `ban` for a player whose expired ban is still in the ban list.

**BAN-EXP-004** (`black_box_confirmed`, semantic). `pardon` for an expired ban that the ban list still
holds removes it, with the usual reply. The rule rests on one observation.

## 4. Commands

### 4.1 Replies

**BAN-CMD-001** (`black_box_confirmed`, semantic). The console shows these replies in English. The
translation keys are unresolved (U1).

| Command | State | Reply |
|---------|-------|-------|
| `ban <target>` | new, no reason | `Banned <name>: Banned by an operator.` |
| `ban <target> <reason>` | new | `Banned <name>: <reason>` |
| `ban <target>` | banned | `Nothing changed. The player is already banned` |
| `pardon <target>` | banned | `Unbanned <name>` |
| `pardon <target>` | not banned | `Nothing changed. The player isn't banned` |
| `ban-ip <address> <reason>` | new | `Banned IP <address>: <reason>` |
| `ban-ip <address>` | banned | `Nothing changed. That IP is already banned` |
| `ban-ip <text>` | neither an address nor the name of a player that is online | `Invalid IP address or unknown player` |
| `pardon-ip <address>` | banned | `Unbanned IP <address>` |
| `pardon-ip <address>` | not banned | `Nothing changed. That IP isn't banned` |
| `pardon-ip <text>` | not an address | `Invalid IP address` |
| `banlist` | no bans | `There are no bans` |
| `banlist` | bans | `There are <n> ban(s):`, then one line for each ban |

A ban line is `<target> was banned by <source>: <reason>`. A ban without a reason shows
`Banned by an operator.`. The order of the lines is unresolved (U4). The levels of the commands are
in `command-permissions.md`.

**BAN-CMD-002** (`black_box_confirmed`, semantic). `banlist players` shows only player bans, and
`banlist ips` shows only address bans. A list with both kinds in `banlist` is
unresolved (U5).

### 4.2 Effects

**BAN-CMD-003** (`black_box_confirmed`, semantic). A command writes the file at once. `source` is
`Server` when the console runs the command.

**BAN-CMD-004** (`black_box_confirmed`, semantic). `ban` disconnects the player when the player is
online, with the translation key `multiplayer.disconnect.banned`. This key has no reason.

**BAN-CMD-005** (`black_box_confirmed`, semantic). `ban` accepts a selector and bans each player that
it selects.

**BAN-CMD-006** (`black_box_confirmed`, semantic). In offline mode, a name of a player that is online
matches with any case. The ban has the id and the name of that player. This part
rests on one observation.

For a name of a player that is not online and that no account holds, `ban` stores the name with the
same case as in the command. The id is the version 3 UUID of `OfflinePlayer:<name>`. Thus
`ban BlCaseOne` refuses the player `BlCaseOne` and does not refuse the player `blcaseone`. See U7.
The rule corrects the reference for some names, and BAN-DEV-002 records the reference behaviour.

**BAN-CMD-007** (`black_box_confirmed`, semantic). `ban-ip` accepts the name of a player that is
online, and it bans the address of that player. It disconnects each online player with that address,
with the translation key `multiplayer.disconnect.ip_banned`. The console adds the line
`This ban affects <n> player(s): <names>`. When no player that is online has that address, the
console does not show this line.

## 5. Intentional deviations

### BAN-DEV-001: The first join after a ban expires

- **Rule:** BAN-EXP-001.
- **Reference:** the first join of a player whose own ban has expired fails, when no earlier join
  has removed that ban. The connection closes before `login_finished`, and the client gets no
  `login_disconnect`. That join removes all the expired bans, and the next join of that player
  works.
- **Visible to the client:** yes. The first join fails with no reason.
- **Test:** a differential test ignores the first join of a player whose ban has expired.

### BAN-DEV-002: The case of a name that is not online

- **Rule:** BAN-CMD-006.
- **Reference, a player that joined before:** the name matches with any case, also after a restart.
  The ban has the id and the name of that player. After `BlKnown` joins and leaves, `ban blknown`
  and `ban BlKnown` both refuse `BlKnown` and do not refuse `blknown`.
- **Reference, a name that the server does not know:** the vanilla server stores the name in lower
  case, with the id of the lower-case name. After `ban BlCaseOne`, the player `BlCaseOne` joins and
  the server refuses the player `blcaseone`. `whitelist.md`, WL-DEV-001, records the same
  behaviour for `whitelist add`.
- **This server:** it does not keep the players that joined, for the reason in `whitelist.md`,
  WL-DEV-004. It uses the case in the command for each name that is not online. `ban BlKnown` refuses
  `BlKnown`, as on the vanilla server. `ban blknown` does not refuse `BlKnown` when that player is not
  online.
- **Visible to the client:** yes, in the reply, in the ban file and at the join.
- **Test:** a differential test uses the exact case for a player that joined before and is not
  online. It skips `ban` with an upper-case letter for a name that the server does not know.

## 6. Unresolved

- **U1**: the translation keys of the replies in BAN-CMD-001.
- **U2**: the form of the time in BAN-JOIN-003.
- **U3**: an address ban without a reason, an address ban with an `expires` time, and the defaults of
  BAN-FILE-004 for an address ban. Also a player ban without `name`, and a player ban with a `uuid`
  that is not valid.
- **U4**: the order of the lines of `banlist`.
- **U5**: `banlist` with both kinds of ban.
- **U6**: whether a server must stop or continue when a ban file is not valid JSON (BAN-FILE-007). A
  start with an empty ban list lets a banned player join. `whitelist.md`, WL-FILE-004, states that a
  whitelist file that is not valid JSON gives an empty whitelist, at a reload and at a start. A
  vanilla server thus treats the two kinds of file differently.
- **U7**: `ban` looks for a name at an external service. A name that an account holds can get another
  result. The experiments did not test this.
- **U8**: an IPv6 address, and an address ban with a host name.
- **U9**: whether a server must apply a ban to a player that is online when the ban comes from a file
  edit. The server does not see that edit before the next start.

## 7. Experiments

Use a vanilla server of the game version in the table at the top of this document, with
`online-mode=false`. Connect a scripted client for each join. Reset the lists between groups. Use long
lower-case names that no account holds, except where a step names a player.

| Experiment | Steps |
|------------|-------|
| E1 | Join as `blonline`. Run `ban blonline`. Read the file. Run `ban blonline`, `banlist`, `banlist players` and `banlist ips`. |
| E2 | Run `ban blonline`, then join. Run `ban blreason griefing the spawn`, then join. Run `pardon` two times, then join. |
| E3 | With the server running, write a ban to the file, then join as that player. |
| E4 | Write player bans with missing or invalid fields to the file. Start the server. Run `banlist`, join as each player, and read the file. |
| E5 | Write `not json` to one ban file. Start the server. Do this for each file. |
| E6 | Join as a player. Run `op`, then `ban`, then join again. |
| E7 | Write an expired ban and a ban for another player. Start the server. Run `banlist`, and join as each player. |
| E8 | Write three expired bans. Start the server. Join as each player, and run `banlist` after each join. Run `ban` for a new name, and read the file. |
| E9 | Join as two players. Run `ban @a`. |
| E10 | Run each command of `ban-ip` and `pardon-ip` with `10.0.0.1` and `notanip`. Read the file. |
| E11 | Join as two players from one address. Run `ban-ip` with the name of one player and a reason. Join as a third player. Send a STATUS request. |
| E12 | Run `ban blboth player reason` and `ban-ip 127.0.0.1 address reason`. Join as `blboth`. |
| E13 | Run `ban BlCaseOne`. Join as `BlCaseOne` and as `blcaseone`. |
| E14 | Use a new name with an upper-case letter for each step. Join and leave, run `ban` with the same case, and join in each case. Do this with `ban` in lower case. Do this with a restart before `ban`. Join, and run `ban` in lower case while the player is online. Run `ban` for a name that did not join, and join in each case. Read the file after each `ban`. |
| E15 | Write four expired bans. Start the server. Run `ban` two times for the first name, and read the file. Run `pardon` for the second name, and run `banlist`. Join as the first name, and run `banlist`. Run `ban` for the third name, read the file, and join as that name. Join as the fourth name two times. |
