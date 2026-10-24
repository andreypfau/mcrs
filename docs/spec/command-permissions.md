# Command permissions: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states the permission model of commands: the permission checks on nodes, the
permission level of a player, how the client learns the level, and the level that each vanilla
command requires. It does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Node**, **requirement**, **source**, **player tree**, **restricted**, **no-permission
  source**: as `command-parser.md` and `commands-packet.md` define them.
- **Permission**: a fact that a source can hold. A permission is a command level or an atom.
- **Command level**: one of five ordered levels (PERM-LEVEL-001).
- **Atom**: a permission named by an identifier.
- **Permission check**: data that a source passes or fails. It is the requirement of a node.
- **Permission set**: the permissions that one source holds.
- **Operator**: a player in the operator list of the server.
- **Permission level of a player**: the command level of the permission set of that player.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply.

### 0.3 Artefacts cited

- `assets/mcrs/reports/registries.json`: the static registries `minecraft:permission_type` and
  `minecraft:permission_check_type`.
- `commands.json`: the command report of the game, as `commands-packet.md`, section 0.4,
  states. Each node of the report has a `permissions` field, except where the check is
  `always_pass`.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `PERM-CHECK` | permission checks and permissions |
| `PERM-SET` | the permission set of a source |
| `PERM-LEVEL` | command levels and the level of a player |
| `PERM-CMD` | the permissions of the vanilla commands |

---

## 1. Checks and permissions

### PERM-CHECK-001: Permission checks

- **Behavior:** a permission check is one of:
  - `minecraft:always_pass`: every source passes, the no-permission source included;
  - `minecraft:require` with a permission: a source passes when its permission set holds the
    permission.
- **JSON form:**

```json
{ "type": "minecraft:always_pass" }
{ "type": "minecraft:require", "permission": { "type": "minecraft:command_level", "level": "gamemasters" } }
```

- **Invariants:** the two types are the entries of the static registry
  `minecraft:permission_check_type`.

Provenance: `black_box_confirmed` (`assets/mcrs/reports/registries.json`, `commands.json`) for
the types and the form of `require`. `source_suggested` for the form of `always_pass`.
Confidence: high.

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
`source_suggested` for the forms. Confidence: medium. Experiment: none in the game reads a
permission from a data pack today. The form matters only when a data pack starts to carry one.

### PERM-CHECK-003: Atoms of the game

| Atom | Meaning |
|------|---------|
| `minecraft:commands/entity_selectors` | the source can use entity selectors that start with `@` (`argument-entity.md`, ARG-ENT-002) |
| `minecraft:chat/send_messages` | the client can send chat messages |
| `minecraft:chat/send_commands` | the client can send commands |
| `minecraft:chat/receive_player_messages` | the client shows chat messages of players |
| `minecraft:chat/receive_system_messages` | the client shows system messages |
| `minecraft:client/commands/restricted` | the client source passes a restricted node (`commands-packet.md`, CMDP-TREE-004) |

The four `chat/` atoms are permissions of the client. The client takes them from its own
chat restrictions. A server never tests them.

Provenance: `source_suggested`. Confidence: medium.

---

## 2. Permission sets

### PERM-SET-001: The permission set of a player

- **Inputs:** the permission level `L` of the player (PERM-LEVEL-002).
- **Behavior:** the player holds:
  - `command_level` of each level up to and including `L`;
  - the atom `minecraft:commands/entity_selectors` when `L` is `gamemasters` or higher;
  - no other atom.
- **Edge cases:** a player at level `all` holds `command_level` of `all`, so it passes
  `require` of `command_level` `all`.

Provenance: `source_suggested`. Confidence: medium. Experiment: deop a player, and send
`tell @a hi` as that player. The reply has the key `argument.entity.selector.not_allowed`.

### PERM-SET-002: The no-permission source

The no-permission source holds nothing, not even `command_level` of `all`. A node with
`require` of any permission is restricted, `command_level` `all` included. Only a node with
`always_pass` is not restricted.

Provenance: `source_suggested`. Confidence: medium.

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

A level `a` is equal to or higher than a level `b` when the id of `a` is greater than or equal
to the id of `b`. An integer outside 0 to 4 becomes the nearest end: a negative integer becomes
`all`, and an integer above 4 becomes `owners`.

Provenance: `black_box_confirmed` (`commands.json`) for the names `gamemasters`, `admins` and
`owners`. `source_suggested` for the ids, the other names and the clamp. Confidence: medium.

### PERM-LEVEL-002: The level of a player on a dedicated server

- **Behavior:**
  - A player who is not an operator has the level `all`.
  - An operator has the level of its entry in the operator list.
  - An operator entry with no level has the level `all`.
  - The `op` command gives a new entry the default operator level. The default operator level
    is the server property `op-permission-level`, and is `owners` when the property is absent.
- **Postconditions:** the level is an input of the server. This document does not state where
  the server stores it.

Provenance: `source_suggested`. Confidence: medium. Experiment: write an operator entry with no
`level` into `ops.json` of a vanilla server, and join. The player receives the entity event 24.

### PERM-LEVEL-003: The client learns the level

- **Behavior:** the server sends an `entity_event` packet for the entity of the player, with
  the event id 24 plus the id of the level: 24 for `all` to 28 for `owners`. The server then
  sends the player tree (`commands-packet.md`, CMDP-TREE-002).
- **Postconditions:** the client uses the level for features that are not commands. An example
  is the game mode switcher, which needs `gamemasters`.

Provenance: `source_suggested`. Confidence: medium. Experiment: run `op` on a player, and
capture an `entity_event` with the id 28, then `commands`.

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
`unpublish`. `jfr` exists only when the runtime supports the flight recorder.

Provenance: `black_box_confirmed` (`commands.json`). Confidence: high.

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

Provenance: `black_box_confirmed` (`commands.json`). Confidence: high.

### PERM-CMD-003: A node that the player cannot use

- **Behavior:** a node whose check fails for the player:
  - is not in the player tree (`commands-packet.md`, CMDP-TREE-001);
  - is not a candidate in the parse on the server (`command-parser.md`, CMD-PARSE-002). A
    command line that needs it fails with `dispatcher_unknown_command` or
    `dispatcher_unknown_argument`.
- **Edge cases:** when a hidden literal is the token at a position, the parse on the server
  does not try its argument siblings. The client does not know the literal, so it tries the
  argument siblings. The two parses then differ. No vanilla node has a hidden literal with an
  argument sibling that the player can use.

Provenance: `source_suggested`. Confidence: medium. Experiment: deop a player, and send
`gamemode creative`. The reply has the key `command.unknown.command` at position 0.
