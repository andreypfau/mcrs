# Dimension change: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Packets | PLAY, clientbound: `respawn` 84, `change_difficulty` 10, `entity_event` 34, `commands` 16, `player_position` 73, `set_chunk_cache_center` 96, `player_abilities` 65, `initialize_border` 44, `set_time` 115, `set_default_spawn_position` 99, `game_event` 39, `ticking_state` 130, `ticking_step` 131, `container_set_content` 18, `set_held_slot` 107, `update_mob_effect` 135, `post_effects` 83, `set_health` 106, `set_experience` 105, `remove_entities` 78 |

Source of the version values: `assets/minecraft/version.json`. Source of the packet ids:
`assets/mcrs/reports/packets.json`.

This document states what occurs when a teleport moves an entity from one dimension to another.
It states the packets that a player receives, in order, what other players observe, and what
the server changes. The `teleport` command uses these rules (`command-teleport.md`,
CMD-TP-005). It does not state portals, the respawn after death or the end credits. It does not
prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Teleport**: a move of an entity to a destination: a dimension, a position, a yaw, a pitch
  and a relative set.
- **Relative set**: as `command-teleport.md`, CMD-TP-006, defines it.
- **Old dimension**: the dimension of the entity before the teleport.
- **New dimension**: the destination dimension of the teleport.
- **Dimension change**: a teleport whose new dimension is not the old dimension.
- **Mover**: the entity that a dimension change moves.
- **Observer**: a player other than the mover.
- **Camera**: the entity through which a player sees the world. A spectator can take another
  entity as its camera.
- **Network id**: the integer id of an entity on the wire.
- Provenance labels: `registry.md`, section 0.3. ID prefix: `DIM-CHG`. None of the experiments
  has been run.

---

## 1. A player that changes dimension

### DIM-CHG-001: The player stays the same entity

- **Inputs:** a player mover, a destination.
- **Behavior:** the mover keeps its network id, its UUID, its inventory, its game mode, its
  attributes, its active effects, its experience, its health and its food. The server does not
  make a new player. The player list of the server does not change, and its order does not
  change.
- **Postconditions:**
  - The mover is in the player list of the new dimension. It is at the end of that list
    (`argument-entity.md`, ARG-ENT-DEV-007). It is no longer in the list of the old dimension.
  - A command that holds the mover as its source entity still holds it after the change.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as an operator
in the Overworld, run `execute in minecraft:the_end run tp @s 0 100 0`. Then run
`execute in minecraft:the_end if entity @a[distance=..5]`. The reply is
`commands.execute.conditional.pass`. Run `list`: the order of the names does not change.

### DIM-CHG-002: Changes before the packets

- **Behavior:** before the server sends the packets of DIM-CHG-003:
  1. A player that sleeps wakes up.
  2. The camera of the player becomes the player.
  3. The player leaves its vehicle.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: put a player in
a boat. Run `execute in minecraft:the_nether run tp <player> 0 64 0`. The boat stays in the
Overworld, and the player is in the Nether without a vehicle.

### DIM-CHG-003: Packets to the player, in order

- **Behavior** (kind: wire): the mover receives these packets in this order, in one tick:

| Step | Packet | Content |
|------|--------|---------|
| 1 | `respawn` | DIM-CHG-004 |
| 2 | `change_difficulty` | the difficulty of the server and its lock flag |
| 3 | `entity_event` | the network id of the mover, and the event `24 + p`. `p` is the permission level of the player, from 0 to 4 |
| 4 | `commands` | the command tree for the player (`commands-packet.md`) |
| 5 | `player_position` | a new teleport id, and the position, velocity, rotation and relative set of the teleport |
| 6 | `set_chunk_cache_center` | the chunk that contains the new position |
| 7 | `player_abilities` | the abilities of the player (`command-gamemode.md`, CMD-GM-008) |
| 8 | `initialize_border` | the world border of the new dimension |
| 9 | `set_time` | the game time and the state of every clock (`command-time.md`) |
| 10 | `set_default_spawn_position` | the respawn data of the new dimension |
| 11 | `game_event` × 3 | only when it rains in the new dimension: event 1 (start rain) with 0; event 7 (rain level) with the rain level; event 8 (thunder level) with the thunder level |
| 12 | `game_event` | event 13 (level chunks load start) with 0 |
| 13 | `ticking_state` | the tick rate and the frozen flag of the server |
| 14 | `ticking_step` | the frozen ticks that remain |
| 15 | `container_set_content` | the inventory menu of the player, with the carried item |
| 16 | `set_held_slot` | the selected hotbar slot |
| 17 | `update_mob_effect` × n | one packet for each active effect of the player, without the blend flag |
| 18 | `post_effects` | the list of post effects of the player |

- **Postconditions:**
  - The packets of DIM-CHG-007 (chunks and entities) can come between step 6 and step 18 and
    after step 18. Their position against steps 7 to 18 is free.
  - On its next tick, the mover receives `set_health` and `set_experience`, also when the
    values did not change. The server sends them because a dimension change marks the sent
    values as unknown.
  - The server does not send `forget_level_chunk` for the chunks of the old dimension. The
    client drops them when it reads `respawn`.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: as an operator in
the Overworld with the effect `speed`, capture the packets of
`execute in minecraft:the_nether run tp @s 0 64 0`. Compare the order with the table. Confirm
that one `update_mob_effect` arrives and that no `forget_level_chunk` arrives.

### DIM-CHG-004: The `respawn` packet

- **Behavior** (kind: wire): the fields are in this order:

| Field | Type | Value |
|-------|------|-------|
| dimension type | VarInt | the network id of the dimension type of the new dimension, in the synchronized registry `minecraft:dimension_type` |
| dimension | Identifier | the identifier of the new dimension |
| game mode | VarInt | the id of the game mode of the player |
| previous game mode | VarInt | 0 when the player has no previous game mode; otherwise the id of that mode plus 1 |
| debug | Boolean | true when the new dimension is a debug world |
| flat | Boolean | true when the world is a flat world |
| last death location | optional | a Boolean; when true, then the Identifier of the dimension and the block position of the last death |
| portal cooldown | VarInt | the portal cooldown of the player, in ticks |
| sea level | VarInt | the sea level of the new dimension |
| data to keep | Byte | `3` |

- The game mode ids are `survival` 0, `creative` 1, `adventure` 2 and `spectator` 3.
- The data to keep has two bits. `0x01` tells the client to keep the attribute modifiers.
  `0x02` tells the client to keep the entity data of the player. A dimension change sets both.
- **Postconditions** (kind: client agreement): the client drops its old level. It makes a new
  level for the dimension and dimension type of the packet. It keeps its local player, with the
  attribute modifiers and the entity data. It then waits for chunks
  (DIM-CHG-007).
- **Edge cases:** the fields before the data to keep are the same fields, in the same order, as
  the spawn information of the `login` packet of the PLAY state.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: capture the
`respawn` packet of the experiment of DIM-CHG-003. Decode it with the table. The dimension is
`minecraft:the_nether`, and the data to keep is 3.

### DIM-CHG-005: State that the dimension change sets

- **Behavior:**
  - The player stops the use of its item.
  - When the old dimension is `minecraft:overworld` and the new dimension is
    `minecraft:the_nether`, the server notes the old position as the Nether entry position.
  - The criterion trigger `minecraft:changed_dimension` fires with the old and the new
    dimension.
  - When the old dimension is `minecraft:the_nether`, the new dimension is
    `minecraft:overworld`, and a Nether entry position exists, the trigger
    `minecraft:nether_travel` fires with that position.
  - When the new dimension is not `minecraft:the_nether`, the Nether entry position is cleared.
  - The head yaw of the player becomes its final yaw, as in the same dimension
    (`command-teleport.md`, CMD-TP-005).
- **Postconditions:** a teleport does not start a new wait for the client to load. The server
  does not wait for the `player_loaded` packet after a dimension change by teleport.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in a new world,
as an operator, run `execute in minecraft:the_nether run tp @s 0 64 0`. The advancement
`minecraft:story/enter_the_nether` becomes done.

### DIM-CHG-006: What observers see

- **Behavior:**
  - Each observer in the old dimension that saw the mover receives `remove_entities` with the
    network id of the mover.
  - Each observer in the new dimension that is in range receives the spawn packets of the mover,
    as for any entity that enters its range.
  - No observer receives `player_info_update` because of the dimension change. The mover stays
    in the tab list of every player.
- **Postconditions:** the server moves the mover from the entity tracking of the old dimension
  to that of the new dimension in the same tick.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with the players
`A` and `B` near each other in the Overworld, run
`execute in minecraft:the_nether run tp A 0 64 0`. `B` receives `remove_entities` with the id of
`A`. `B` still sees `A` in the tab list.

### DIM-CHG-007: Chunks and entities of the new dimension

- **Behavior:**
  - The view of the mover in the new dimension starts empty. The server then sends the chunks
    around the new position, with the same rules as for a player that joins. The view distance
    does not change.
  - The server loads and generates the chunks of the new dimension as for a player that
    joins. A chunk that is not loaded does not stop the teleport.
  - The mover receives the spawn packets of the entities of the new dimension that are in its
    range.
  - The server stops the tracking of each entity of the old dimension for the mover.
- **Postconditions:**
  - In the reference, the mover also receives one `remove_entities` for each entity of the old
    dimension that it saw. The client has already dropped those entities when it read
    `respawn`. These packets are of the kind unspecified: an implementation can send them or
    not, and a parity test must not compare them.
  - The client shows its load screen after event 13 of `game_event`. It hides the screen when
    it has the chunk at its position.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the
Overworld, run `execute in minecraft:the_end run tp @s 5000 100 5000`, where no chunk exists.
The player arrives at once, the load screen shows, and the chunks arrive in batches. The command
result is 1.

### DIM-CHG-008: Spectators that watch the mover

- **Behavior:** after the mover arrives, each player of the old dimension whose camera is the
  mover gets the same teleport. That player changes dimension with these rules. Then its camera
  becomes itself.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: player `B` in
spectator mode takes `A` as its camera. Run `execute in minecraft:the_nether run tp A 0 64 0`.
`B` also moves to the Nether at `0.5 64 0.5`, and `B` no longer spectates `A`.

---

## 2. An entity that is not a player

### DIM-CHG-009: A new entity replaces the mover

- **Inputs:** a mover that is not a player, a destination.
- **Behavior:**
  1. The mover leaves its vehicle.
  2. Each passenger of the mover dismounts. Each passenger gets its own teleport to the new
     dimension, with the same offset to the mover and the same rotation difference.
  3. The server makes a new entity of the type of the mover in the new dimension. If the type
     cannot exist there, the teleport fails, and the mover stays.
  4. The new entity copies the data of the mover, the UUID included. It gets a new network id.
  5. The server removes the mover from the old dimension. It adds the new entity at the
     destination.
  6. Each passenger that arrived mounts the new entity.
  7. Spectators that watch the mover follow, as DIM-CHG-008 states.
- **Postconditions:**
  - Observers in the old dimension receive `remove_entities` for the old network id. Observers
    in the new dimension in range receive the spawn packets of the new entity.
  - A command that holds the mover as its source entity still holds the removed mover, not the
    new entity.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: summon a pig
with the tag `p` in the Overworld. Run
`execute in minecraft:the_nether run tp @e[tag=p,limit=1] 0 64 0`. A pig with the same UUID is
in the Nether, and its network id differs.

---

## 3. Open questions

- DIM-CHG-003: the position of the spawn packets of entities of the new dimension against steps
  7 to 18 is not measured. Experiment: capture the packets of the experiment of DIM-CHG-003 with
  a pig near the destination. Record where `add_entity` of the pig arrives.
- DIM-CHG-001: the passengers of a player that changes dimension. These rules do not state
  them. Experiment: put an entity on a player with a data pack or a mod, move the player to the
  Nether, and record where the entity is.
