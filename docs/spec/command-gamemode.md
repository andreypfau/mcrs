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

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: run `dumpReports`
(`tools/vanilla-oracle`), and compare the node `gamemode` in `commands.json` with the tree above.

### CMD-GM-002: Targets

- Without `<target>`, the target is the source. If the source is not a player, the command
  fails with `permissions.requires.player`.
- With `<target>`, the targets are the players that the selector finds
  (`argument-entity.md`, ARG-ENT-014), in that order. If it finds none, the command fails with
  `argument.entity.notfound.player`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: from the server
console, run `gamemode creative`. It fails with `permissions.requires.player`. Then run
`gamemode creative @a[name=nobody]`. It fails with `argument.entity.notfound.player`.

### CMD-GM-003: Change of one target

- **Behavior:** if the target already has the game mode, nothing happens for it. Otherwise:
  - The game mode of the target changes from the old mode to the new mode. The target keeps
    the old mode as its previous game mode. Its abilities take the values of the new mode
    (CMD-GM-007).
  - When the target flies, the new mode is not spectator, and the target is near the ground,
    the flight of the target stops. The target is near the ground in this case: its box
    touches no collider, and the free space below it is less than 1 block.
  - The sleep count of the dimension of the target takes the new mode into account at once.
  - A change to creative ends the fall damage protection that an impulse, for example a wind
    charge, gave the target.
  - When the new mode is spectator:
    - the entities on the shoulders of the target leave it;
    - the target leaves its vehicle;
    - the target stops the use of its item;
    - the location effects of the enchantments of the target stop;
    - the target becomes invisible, and its effect particles disappear.
  - When the new mode is not spectator, the camera of the target becomes the target. When the
    old mode is spectator, the location effects of its enchantments start again.
  - When the new mode is not spectator, the active effects of the target control its
    invisibility and its effect particles again. Only the effect `invisibility` makes it
    invisible.
  - The feedback of CMD-GM-004 follows the change.
- **Packets** (kind: wire):
  - The target receives `game_event` with the event 3 (change game mode) and the id of the new
    mode as its value.
  - After that `game_event`, the target receives `player_abilities` with its new abilities
    (CMD-GM-008). When the client reads `game_event`, it applies the table of CMD-GM-007 to its
    own abilities. Only a later `player_abilities` gives it the values of the server.
  - Every player of the server receives `player_info_update` with the action
    `update_game_mode` for the target. The order of this packet against the two other packets
    is free.
- **Postconditions:** this is the same change as the change that the game mode switcher of the
  client asks for.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
in spectator mode, stand on the ground and run `gamemode creative`. The client does not fly
after the change.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
in survival mode, run `gamemode survival`. No message arrives, and the command result is 0.

### CMD-GM-005: Result

The result of the command is the number of targets that changed.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with two players
online, one of them in creative mode, run `scoreboard objectives add r dummy` and
`execute store result score @s r run gamemode creative @a`. The score is 1.

### CMD-GM-007: Abilities of each mode

- **Behavior:** when a player gets a game mode, its abilities take these values. "Keep" means
  that the ability keeps its value from before the change.

| Mode | invulnerable | flying | may fly | instant build | may build |
|------|--------------|--------|---------|---------------|-----------|
| `survival` | false | false | false | false | true |
| `creative` | true | keep | true | true | true |
| `adventure` | false | false | false | false | false |
| `spectator` | true | true | true | false | false |

- The mode does not change the flying speed and the walking speed. Their defaults are `0.05`
  and `0.1`. A player keeps the two speeds in its saved data. No vanilla command changes them.
- `flying` is the only ability that is not a function of the mode alone:
  - A change to `creative` keeps the old value. Then the ground rule of CMD-GM-003 can stop the
    flight. Only a player that was in `spectator` can fly before the change.
  - A change to any other mode sets the value of the table. A player that flies in `creative`
    and changes to `survival` stops its flight.
  - Serverbound `player_abilities` sets `flying` to its flying flag when `may fly` is true. When
    `may fly` is false, it sets `flying` to false. The server sends no answer.
  - A player that loads from saved data gets its saved `flying`. Then the table of its mode
    applies, so the saved value stays only in `creative`.
- **Postconditions:** `may build` is not on the wire. The client applies the table to its own
  abilities when it reads `game_event` with the event 3.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator in
creative mode, fly at 10 blocks above the ground. Run `gamemode spectator`, then
`gamemode creative`. Capture the last `player_abilities`: its flag byte is `0x0F`. Then run
`gamemode survival`. The flag byte of the last `player_abilities` is `0x00`, and the player
falls.

### CMD-GM-008: The packet `player_abilities`

- **Clientbound** (kind: wire): the fields are in this order.

| Field | Type | Value |
|-------|------|-------|
| flags | byte | the flag bits below |
| flying speed | float | the flying speed of the player |
| walking speed | float | the walking speed of the player |

| Bit | Ability |
|-----|---------|
| `0x01` | invulnerable |
| `0x02` | flying |
| `0x04` | may fly |
| `0x08` | instant build |

- The client sets each of the six values from the packet. It does not change `may build`.
- **Serverbound** (kind: wire): one byte. Bit `0x02` is the flying flag. The server ignores the
  other bits. The client sends this packet when it starts or stops a flight.
- **On join** (kind: wire): the player receives clientbound `player_abilities` after `login`
  and `change_difficulty`, and before `set_held_slot`. A join without this packet is a
  deviation. The player also receives it after each change of dimension.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: join in survival
mode with the default speeds. Capture the first `player_abilities`. Its bytes are
`00 3D 4C CC CD 3D CC CC CD`.

---

## Intentional deviations

### CMD-GM-006: Order of the packets

- **Rule:** CMD-GM-003, packets.
- **Reference:** the target receives `player_abilities`, `player_info_update`, `game_event` and
  `player_abilities`, in that order. The first `player_abilities` has no effect on the client,
  because `game_event` and the second `player_abilities` replace all of its values.
- **Visible to the client:** no.
- **Test:** a differential test does not compare the order of these packets. It does not
  require the first `player_abilities`.
