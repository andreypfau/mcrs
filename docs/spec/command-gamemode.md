# The `gamemode` command: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states the tree of the `gamemode` command, what it changes, and the messages
that it sends. It does not prescribe an architecture.

## 0. Conventions

- **Node**, **executable**, **source**, **error**: as `command-parser.md` defines them.
- **Operator**, **system message**: as `commands-packet.md` defines them.
- **Target**: a player whose game mode the command sets.
- **Mode text**: the translation `gameMode.<name>`, where `<name>` is the keyword of the game
  mode (`argument-keywords.md`, ARG-KW-001).
- Provenance labels: `registry.md`, section 0.3. ID prefix: `CMD-GM`. None of the experiments
  has been run.

---

### CMD-GM-001: Tree

```
gamemode                                  require command_level gamemasters
└── <gamemode: minecraft:gamemode>        executable
    └── <target: minecraft:entity>        executable; players, any amount (wire byte 0x02)
```

Provenance: `black_box_confirmed` (`commands.json`). Confidence: high.

### CMD-GM-002: Targets

- Without `<target>`, the target is the source. If the source is not a player, the command
  fails with `permissions.requires.player`.
- With `<target>`, the targets are the players that the selector finds
  (`argument-entity.md`, ARG-ENT-014), in that order. If it finds none, the command fails with
  `argument.entity.notfound.player`.

Provenance: `source_suggested`. Confidence: high.

### CMD-GM-003: Change of one target

- **Behavior:** if the target already has the game mode, nothing happens for it. Otherwise:
  1. The game mode of the target changes. Every player receives `player_info_update` with the
     action `update_game_mode` for the target.
  2. The target receives `game_event` with the event 3 (change game mode) and the id of the
     mode as its value, and then `player_abilities`.
  3. When the new mode is not spectator, and the target flies near the ground, it stops
     flying.
  4. The feedback of CMD-GM-004 follows.
- **Postconditions:** this is the same change as the change that the game mode switcher of the
  client asks for. The game mode specification states the full change; it does not exist yet.

Provenance: `source_suggested`. Confidence: medium. Experiment: run `gamemode creative` and
capture the packets to the player and to a second player.

### CMD-GM-004: Feedback

- **Behavior:** for each target that changed, in order:
  - If the target is the source entity: a success message
    `commands.gamemode.success.self` with the mode text, with the broadcast flag set.
  - Otherwise: when the game rule `send_command_feedback` is true, the target receives the
    system message `gameMode.changed` with the mode text. Then a success message
    `commands.gamemode.success.other` with the display name of the target and the mode text,
    with the broadcast flag set.
- **Postconditions:** success messages and the broadcast follow `commands-packet.md`,
  CMDP-EXEC-004. A target that did not change gets no message.

Provenance: `source_suggested`. Confidence: medium. Experiment: as an operator in survival
mode, run `gamemode survival`. No message arrives, and the command result is 0.

### CMD-GM-005: Result

The result of the command is the number of targets that changed.

Provenance: `source_suggested`. Confidence: high.
