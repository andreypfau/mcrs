# Command permissions: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states the permission model of commands. The subjects are the permission checks
on nodes, the permission level of a player and how the client learns the level. The last
subject is the level that each vanilla command requires. The document does not prescribe an
architecture.

## 0. Conventions

### 0.1 Terms

- **Node**, **requirement**, **source**, **player tree**: as `command-parser.md` and
  `commands-packet.md` define them.
- **Permission**: a fact that a source can hold. A permission is a command level or an atom.
- **Command level**: one of five ordered levels (PERM-LEVEL-001).
- **Atom**: a permission named by an identifier.
- **Permission check**: data that a source passes or fails. It is the requirement of a node.
- **Permission set**: the permissions that one source holds.
- **Restricted**: a node whose permission check is `require` (PERM-SET-002).
- **Operator**: a player in the operator list of the server.
- **Permission level of a player**: the command level of the permission set of that player.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of
`README.md`, section "Semantics, not accidents".

### 0.3 Artefacts cited

- `assets/mcrs/reports/registries.json`: the static registries `minecraft:permission_type` and
  `minecraft:permission_check_type`.
- `commands.json`: the command report of the game, as `commands-packet.md`, section 0.4,
  states. The report writes a `permissions` field only for some nodes (PERM-DEV-001).

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `PERM-CHECK` | permission checks and permissions |
| `PERM-SET` | the permission set of a source |
| `PERM-LEVEL` | command levels and the level of a player |
| `PERM-CMD` | the permissions of the vanilla commands |
| `PERM-DEV` | intentional deviations from the reference |

---

## 1. Checks and permissions

### PERM-CHECK-001: Permission checks

- **Behavior:** a permission check is one of:
  - `minecraft:always_pass`: every source passes;
  - `minecraft:require` with a permission: a source passes when its permission set holds the
    permission.
- **JSON form:**

```json
{ "type": "minecraft:always_pass" }
{ "type": "minecraft:require", "permission": { "type": "minecraft:command_level", "level": "gamemasters" } }
```

- **Invariants:** the two types are the entries of the static registry
  `minecraft:permission_check_type`. Every node has a check. A node with no stated check has
  the check `always_pass`.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`) for the types.
`source_suggested` for the forms of `require` and `always_pass`. Kind: wire. Confidence:
medium. Experiment: run `dumpReports`, and read the `permissions` field of the node `gamemode`
in `commands.json`. It has the form of `require` above.

### PERM-CHECK-002: Permissions

- **Behavior:** a permission is one of:
  - `minecraft:command_level` with a `level`: the name of a command level (PERM-LEVEL-001);
  - `minecraft:atom` with an `id`: an identifier.
- **JSON form:** an atom can also be written as a bare identifier string. The two forms below
  are equal:

```json
{ "type": "minecraft:atom", "id": "minecraft:commands/entity_selectors" }
"minecraft:commands/entity_selectors"
```

- **Invariants:** the two types are the entries of the static registry
  `minecraft:permission_type`.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`) for the types.
`source_suggested` for the forms. Kind: wire. Confidence: medium. Experiment: none in the game
reads a permission from a data pack today. The form matters only when a data pack starts to
carry one.

### PERM-CHECK-003: Atoms of the game

The server tests one atom:

| Atom | Meaning |
|------|---------|
| `minecraft:commands/entity_selectors` | the source can use entity selectors that start with `@` (`argument-entity.md`, ARG-ENT-002) |

The client also has these atoms. They are internal to the client. They never reach the server
and are not on the wire.

| Atom | Meaning |
|------|---------|
| `minecraft:chat/send_messages` | the client can send chat messages |
| `minecraft:chat/send_commands` | the client can send commands |
| `minecraft:chat/receive_player_messages` | the client shows chat messages of players |
| `minecraft:chat/receive_system_messages` | the client shows system messages |
| `minecraft:client/commands/restricted` | the client source passes a restricted node (`commands-packet.md`, CMDP-TREE-004) |

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as in
PERM-SET-001.

---

## 2. Permission sets

### PERM-SET-001: The permission set of a player

- **Inputs:** the permission level `L` of the player (PERM-LEVEL-002).
- **Behavior:** the player holds:
  - `command_level` of `L` and of each level below `L`;
  - the atom `minecraft:commands/entity_selectors` when `L` is `gamemasters` or higher;
  - no other atom.
- **Edge cases:** a player at level `all` holds `command_level` of `all`, so it passes
  `require` of `command_level` `all`.
- **Postconditions:** the client accepts entity selectors that start with `@` only when the
  last level that it received is `gamemasters` or higher (PERM-LEVEL-003).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.
Experiment: deop a player, and send `tell @a hi` as that player. The reply has the key
`argument.entity.selector.not_allowed`.

### PERM-SET-002: Restricted nodes

A node is restricted when its check is `require`, with any permission, `command_level` `all`
included. A node with the check `always_pass` is not restricted.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: capture `commands`
for an operator at level `owners`. The node `gamemode` has the flag `0x20`, and the node `help`
does not have it.

---

## 3. Command levels

### PERM-LEVEL-001: The five levels

| Id | Name |
|----|------|
| 0 | `all` |
| 1 | `moderators` |
| 2 | `gamemasters` |
| 3 | `admins` |
| 4 | `owners` |

Compare two levels by their ids. A level is equal to or higher than another level when its id
is greater than or equal to the id of the other level.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: run `dumpReports`,
and read the names `gamemasters`, `admins` and `owners` in `commands.json`.

### PERM-LEVEL-002: The level of a player

- **Behavior:**
  - A player who is not an operator has the level `all`.
  - An operator has the level that the configuration of the server gives it.
  - The `op` command gives a new operator the default operator level. The default operator
    level is `owners`.
- **Postconditions:** the level is an input of the server. The configuration comes through
  the plugin. This document does not state where the server stores the operator list, or the
  format of a stored list.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a vanilla
server with no `op-permission-level` property, run `op` on a player. The player receives the
entity event 28.

### PERM-LEVEL-003: The client learns the level

- **Behavior:** the server sends an `entity_event` packet for the entity of the player, with
  the event id 24 plus the id of the level. The ids are 24 for `all` to 28 for `owners`. The
  server sends the packet at the times of `commands-packet.md`, CMDP-TREE-002.
- **Postconditions:** the client uses the level for entity selectors (PERM-SET-001) and for
  features that are not commands. An example is the game mode switcher, which needs
  `gamemasters`.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.
Experiment: run `op` on a player, and capture an `entity_event` with the id 28.

---

## 4. The vanilla commands

### PERM-CMD-001: Levels of the root literals

The permission check of each root literal of the vanilla tree is `require` of the
`command_level` in the table, or `always_pass`. The table lists every root literal of the
report, which the game builds with the commands of the dedicated server and of the integrated
server.

| Check | Root literals |
|-------|---------------|
| `always_pass` | `help`, `list`, `me`, `msg`, `random`, `teammsg`, `tell`, `tm`, `trigger`, `w` |
| `gamemasters` | `advancement`, `attribute`, `bossbar`, `clear`, `clone`, `compute`, `damage`, `data`, `datapack`, `defaultgamemode`, `dialog`, `difficulty`, `effect`, `enchant`, `execute`, `experience`, `fetchprofile`, `fill`, `fillbiome`, `forceload`, `function`, `gamemode`, `gamerule`, `give`, `item`, `kill`, `locate`, `loot`, `particle`, `place`, `playsound`, `posteffect`, `recipe`, `reload`, `return`, `ride`, `rotate`, `say`, `schedule`, `scoreboard`, `seed`, `setblock`, `setworldspawn`, `spawnpoint`, `spectate`, `spreadplayers`, `stopsound`, `stopwatch`, `summon`, `swing`, `tag`, `team`, `teleport`, `tellraw`, `test`, `time`, `title`, `tp`, `version`, `waypoint`, `weather`, `worldborder`, `xp` |
| `admins` | `ban`, `ban-ip`, `banlist`, `debug`, `deop`, `kick`, `op`, `pardon`, `pardon-ip`, `setidletimeout`, `tick`, `transfer`, `whitelist` |
| `owners` | `jfr`, `perf`, `publish`, `save-all`, `save-off`, `save-on`, `stop`, `unpublish` |

No root literal requires `moderators`. A dedicated server does not register `publish` and
`unpublish`. An integrated server gives `seed` and `version` the check `always_pass`. A
dedicated server and the report give them `gamemasters`. The game registers `jfr` only when its
runtime has a flight recorder. That condition depends on the environment, so a parity test
must not compare the presence of `jfr`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`dumpReports`, and compare the `permissions` field of each root literal in `commands.json` with
the table.

### PERM-CMD-002: Checks below the root

These nodes below a root literal have their own check. Every other node below a root literal
has `always_pass`.

| Path | Check |
|------|-------|
| `datapack create` | `owners` |
| `debug function` | `admins` |
| `random reset` | `gamemasters` |
| `random roll <range> <sequence>` | `gamemasters` |
| `random value <range> <sequence>` | `gamemasters` |

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`dumpReports`, and list every node below a root literal in `commands.json` that has a
`permissions` field. The list equals the table.

### PERM-CMD-003: A node that the player cannot use

- **Behavior:** a node whose check fails for the player:
  - is not in the player tree (`commands-packet.md`, CMDP-TREE-001);
  - is not a candidate in the parse on the server (`command-parser.md`, CMD-PARSE-002). A
    command line that needs it fails with `dispatcher_unknown_command` or
    `dispatcher_unknown_argument`.
- **Edge cases:** a token at a position can be the name of a literal that the player cannot
  use. The parse then tries the argument siblings of that literal. The client does not know
  the literal, so its parse gives the same result (PERM-DEV-002).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: deop a
player, and send `gamemode creative`. The reply has the key `command.unknown.command` at
position 0.

---

## 5. Intentional deviations

### PERM-DEV-001: The report omits the check `always_pass`

- **Rule:** PERM-CHECK-001.
- **Reference:** the reference report writes a `permissions` field only for a node that has an
  explicit permission check. Most nodes with the check `always_pass` have no explicit check,
  so their node in the report has no field. A node with an explicit `always_pass` has the field
  `{ "type": "minecraft:always_pass" }`. Only `seed` and `version` on an integrated server have
  an explicit `always_pass`.
- **Visible to the client:** no. The difference is only in `commands.json`.
- **Test:** a comparison of `commands.json` treats a node with no `permissions` field as a node
  with the check `always_pass`.

### PERM-DEV-002: A hidden literal does not block its argument siblings

`command-parser.md`, CMD-DEV-003, records this deviation for PERM-CMD-003. No vanilla node has
a hidden literal with an argument sibling that the player can use.
