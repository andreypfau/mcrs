# Mob entities and entity tracking

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Wire ids | `assets/mcrs/reports/packets.json`, `assets/mcrs/reports/registries.json` |

This document states how a mob exists in a dimension world, how the `summon` command makes one,
and how the server sends the entities that are not players to the clients. It states the
selection of the clients that see an entity, the packets that make an entity appear, the entity
data, the position and rotation updates, and the removal.

The motion of a mob is the walker tick of `entity-physics.md` (PHY-ORD-002). The architecture of
the muxer is `replication.md` (REPL). This document states only behaviour and the choices that the
architecture can make. It does not prescribe the code.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **clientbound**, **serverbound**: as `login.md`,
  section 0.1, defines them.
- **Tick**, **entity**, **living entity**, **position**, **velocity**, **box**, **dimensions**,
  **on-ground flag**, **server-simulated entity**: as `entity-physics.md`, section 0.1, defines
  them.
- **Viewer**, **subject**, **tile**, **tier**, **delta base**, **spawn bundle**: as `replication.md`,
  section 1, defines them.
- **Mob**: a server-simulated living entity that is not a player and not an armor stand, for
  example a zombified piglin.
- **Entity type**: an entry of the registry `minecraft:entity_type`.
- **Type data**: the values that an entity type gives to each entity of the type (section 2).
- **Network id**: the id of an entity on the wire. A player keeps the id of its session. Another
  entity gets an even id (`replication.md`, section 7.2).
- **Tracker**: the state that the server keeps for one subject to send it to viewers. A subject
  has one tracker. All viewers of the subject share it.
- **Tracker tick**: one run of the tracker of a subject. The tracker counts its ticks from 0. The
  first tracker tick is the tick in which the server starts to track the subject.
- **Update**: a tracker tick in which the tracker sends the state of the subject (TRK-UPD-001).
- **Update interval**: the number of tracker ticks between two updates, as type data gives it.
- **Tracking range**: the largest horizontal distance at which a viewer sees a subject
  (TRK-RNG-001).
- **Pairing**: the state of one viewer that sees one subject. A pairing starts with the spawn
  bundle and ends with the removal.
- **Position base**: the position that the viewers of a subject last received. A relative move
  starts from it.
- **Packed angle**: an angle in one byte, 256 units for a full turn (TRK-WIRE-004).
- **Entity data**: the list of values of an entity that the client shows, for example the health
  and the pose. Each value has an index and a serializer.
- **Serializer**: the codec of one entity data value. Its id is on the wire.
- **Default value**: the value that an entity data entry has when the entity is made.
- **Dirty entry**: an entity data entry that changed after the tracker last sent it.
- **Syncable attribute**: an attribute that the server sends to the client.
- **Spawn request**: a request of a system to add a mob to a world (TRK-LIFE-002).
- **First spawn setup**: the random setup of a new mob of the type, for example its equipment
  (section 4.4).
- **Display name**: as `command-execute.md`, section 0, defines it.
- **Spawnable bounds**: as `command-teleport.md`, section 0, defines them.

### 0.2 Provenance

The labels of `entity-physics.md`, section 0.2, apply. `black_box_confirmed` and
`source_suggested_black_box_confirmed` refer to the captured results of section 13. Each rule
states its kind as `docs/spec/README.md` defines the kinds.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `TRK-TYPE` | type data |
| `TRK-LIFE` | the life cycle of a mob |
| `TRK-SUM` | the `summon` command |
| `TRK-RNG` | the selection of the viewers |
| `TRK-SPN` | the spawn bundle |
| `TRK-DATA` | entity data and attributes |
| `TRK-UPD` | updates |
| `TRK-WIRE` | the encodings of the update packets |
| `TRK-REM` | the removal |
| `TRK-CLI` | the client behaviour that the rules use |
| `TRK-DEV` | intentional deviations |

### 0.4 Scope

This document states:

- a mob in the rows of the entity pass, its type data and its attributes;
- the add and the removal of a mob, by a command and by another system;
- the `summon` command with the forms that section 4 lists;
- the replication of each entity that is not a player: tracking range, spawn bundle, entity data
  of the base entries and of the zombified piglin, updates, head rotation and removal.

These subjects are out of scope:

| Subject | Reason |
|---------|--------|
| The first spawn setup of a type (equipment, baby, left hand, random attribute bonuses) | `natural-spawning.md` states it for natural spawns and for `summon`. TRK-DEV-001 records the gap until then. |
| Keys of the `nbt` argument of `summon` | No step of `gold-farm.md` needs them. TRK-DEV-002 records the rejection. |
| Health changes, damage, death and the death animation | `entity-damage.md`. TRK-LIFE-004 states only the removal. |
| AI, goals, look control and head turns of a mob | `mob-goals.md`. This document sends the rotations that the row holds. |
| Save and load of mobs, and the removal of a mob when its column unloads | No step of `gold-farm.md` needs them. A row in a section that is not loaded does not tick. |
| The entity data of an item entity | PHY-CMP-026. Section 6 keeps the model general. |
| Passengers, leashes, minecarts on rails, projectiles, item frames | No mob of the farm uses them. Their packets have hooks in TRK-SPN-001 and TRK-UPD-002. |
| Teams and the team colour of a name | No team exists in the server. |

## 1. Type data

**TRK-TYPE-001** (`source_suggested`, semantic, confidence: high). Each entity type has this type
data:

| Field | Meaning |
|-------|---------|
| wire id | the protocol id of the type in `minecraft:entity_type` |
| category of motion | the category of PHY-ORD-005 |
| dimensions | width, height and eye height of the `standing` pose; fixed or scalable |
| client tracking range | in columns; 0 means that the server never tracks the type |
| update interval | in tracker ticks; "none" means no periodic update |
| tracks velocity | true when the tracker sends the velocity (TRK-UPD-007) |
| summonable | false for a type that `summon` must not make |
| allowed in peaceful | false for a type that `summon` must not make in the difficulty `peaceful` |
| fire immune | true for a type that fire and lava do not damage |
| default attributes | the base value of each attribute that an entity of the type has |
| entity data layout | the entries of section 6, by the chain of the type |

The default values are: client tracking range 5, update interval 3, tracks velocity true,
summonable true, allowed in peaceful true.

**TRK-TYPE-002** (`source_suggested_black_box_confirmed`, wire, confidence: high). The types of
the farm have these values. TRK-E1 and TRK-E3 confirm the wire id, the range and the interval of
the zombified piglin.

| Type | Wire id | Category | Width × height | Eye height | Range | Interval | Tracks velocity | Peaceful | Fire immune |
|------|---------|----------|----------------|------------|-------|----------|-----------------|----------|-------------|
| `minecraft:zombified_piglin` | 160 | walker | `0.6F × 1.95F`, scalable | `1.79F` | 8 | 3 | yes | no | yes |
| `minecraft:zombie` | (registry) | walker | `0.6F × 1.95F`, scalable | `1.74F` | 8 | 3 | yes | no | no |
| `minecraft:player` | (registry) | player | `0.6F × 1.8F` | `1.62F` | 32 | 2 | no | yes | no |

The player is not summonable. A baby zombified piglin has the dimensions `0.49F × 0.98F` and the
eye height `0.78F`. G1 makes no baby (TRK-DEV-001).

**TRK-TYPE-003** (`source_suggested_black_box_confirmed`, client agreement, confidence: high).
The default attributes of a zombified piglin are the attributes of a living entity with their
attribute defaults, and these values:

| Attribute | Base value |
|-----------|-----------|
| `max_health` | 20 |
| `movement_speed` | `0.23F`, widened: `0.23000000417232513` |
| `follow_range` | 35 |
| `attack_damage` | 5 |
| `armor` | 2 |
| `spawn_reinforcements_chance` | 0 |

The attributes of a living entity are: `max_health`, `knockback_resistance`, `movement_speed`,
`armor`, `armor_toughness`, `max_absorption`, `step_height`, `scale`, `gravity`,
`safe_fall_distance`, `fall_damage_multiplier`, `jump_strength`, `entity_interaction_range`,
`oxygen_bonus`, `burning_time`, `explosion_knockback_resistance`, `water_movement_efficiency`,
`movement_efficiency`, `attack_knockback`, `camera_distance`, `waypoint_transmit_range`,
`bounciness`, `air_drag_modifier`, `friction_modifier`, `name_tag_distance` and
`below_name_distance`. The client has the same defaults for each type. Thus the client agrees on
each attribute that the server does not send. The walker tick reads the attributes of
`MotionAttributes`; for a zombified piglin only `movement_speed` differs from the default.

## 2. The life cycle of a mob

**TRK-LIFE-001** (`source_suggested`, semantic, confidence: high). A mob is a row of the entity
pass of its dimension world. The row has:

- the motion row of the type: the category, the dimensions and the step height of TRK-TYPE-001;
- the living state with the default attributes of TRK-TYPE-003;
- the birth key of PHY-CMP-013 and the UUID of PHY-CMP-024;
- a network id from the ids of the cell;
- the yaw, the pitch and the head yaw, as raw f32 degrees.

The cold state of the mob holds its type and its entity data values (section 6). The walker tick
of PHY-ORD-002 moves the mob in each tick. Until `mob-goals.md` gives the mob an AI, the mob has no
steering: its input vector is zero and it does not jump.

**TRK-LIFE-002** (`source_suggested`, semantic, confidence: medium). A spawn request has the
type, the position, the rotation, the velocity, the source of the birth key and the entity data
values that differ from the defaults. These systems make spawn requests:

- the `summon` command (section 3), with the birth source of the kind `command` and the command
  sequence of PHY-CMP-013;
- natural spawning (`natural-spawning.md`), with the birth source of the kind `spawn`;
- any later system that adds a mob, with the birth source that PHY-CMP-013 gives it.

The life cycle of `DomainInput` applies the spawn requests of the tick in the order of their birth
keys (`physics-architecture.md`, section 3.9). The new row ticks first in the next `Simulate`. At
the spawn, the health entry is the `max_health` value of the mob (TRK-DATA-003).

**TRK-LIFE-003** (`black_box_confirmed`, semantic, confidence: high). A new mob of a spawn request
without a rotation has the yaw 0, the pitch 0 and the head yaw 0. Its velocity is zero. TRK-E1
shows these values in `add_entity` for a summoned zombified piglin, with and without the first
spawn setup.

**TRK-LIFE-004** (`source_suggested`, semantic, confidence: medium). A removal request names a
row and a reason. The reasons are: discarded (a command, a despawn, a merge) and killed. The life
cycle of `DomainInput` removes the row from its tile, the grid and the cold state. The tracker of
the subject then ends each pairing (TRK-REM-001). A killed mob first plays its death
(`entity-damage.md`), and the removal follows after the death animation of 20 ticks.

**TRK-LIFE-005** (`source_suggested`, semantic, confidence: high). A mob row in a section that is
not loaded does not tick, as PHY-ORD-002 and the pass state now. The row stays until a removal
request names it.

## 3. The `summon` command

**TRK-SUM-001** (`source_suggested`, wire, confidence: high). The tree of the command:

```
summon                                       require command_level gamemasters
└── <entity: minecraft:resource>             executable; registry minecraft:entity_type; suggestion type minecraft:summonable_entities
    └── <pos: minecraft:vec3>                executable
        └── <nbt: minecraft:nbt_compound_tag>    executable
```

The argument types are CMD-TIME-004 (`minecraft:resource`), ARG-COORD-003 (`minecraft:vec3`) and
SNBT-ARG-002 (`minecraft:nbt_compound_tag`). The permission is PERM-LEVEL-001, level
`gamemasters`.

**TRK-SUM-002** (`source_suggested`, semantic, confidence: high). The behaviour:

1. Let `type` be the entity type of `<entity>`. If `type` is not summonable, fail with
   `entity.not_summonable`, with the identifier of the type.
2. Let `pos` be the value of `<pos>`, or the source position when the node is absent. If the block
   position that contains `pos` is outside the spawnable bounds, fail with
   `commands.summon.invalidPosition`.
3. If the difficulty of the source dimension is `peaceful` and `type` is not allowed in peaceful,
   fail with `commands.summon.failed.peaceful`.
4. Let `setup` be true when the `<nbt>` node is absent, and false when it is present. Check the
   compound with TRK-SUM-003.
5. Make a spawn request of `type` at `pos` in the source dimension (TRK-LIFE-002), with no
   rotation. When `setup` is true, the mob also gets the first spawn setup of its type.
6. If an entity with the same UUID exists in the dimension, fail with
   `commands.summon.failed.uuid`. A UUID of PHY-CMP-024 cannot collide in practice.
7. Send the success message of TRK-SUM-004. The result is 1.

When `type` is not a mob, step 5 makes the entity of the category of the type. G1 makes only mobs
of the walker category; another type fails with `commands.summon.failed`. CMD-EX-022 uses steps 1
to 6 with `pos` from the source and `setup` true.

**TRK-SUM-003** (`source_suggested`, semantic, confidence: medium). The compound of `<nbt>` can
hold no key. A compound with a key fails with `commands.summon.failed`. TRK-DEV-002 records this.
The empty compound `{}` makes the mob without the first spawn setup.

**TRK-SUM-004** (`source_suggested`, semantic, confidence: high). The success message is
`commands.summon.success`, with the display name of the new entity, as a success message with the
broadcast flag set. The display name of a mob without a custom name is:

- the translatable text `entity.<namespace>.<path>` of its type, for example
  `entity.minecraft.zombified_piglin`;
- with the hover event `show_entity`: the type, the UUID and the same translatable text as the
  name;
- with the insertion: the UUID in its text form with hyphens.

Experiment: as an operator, run `summon minecraft:zombified_piglin ~ ~ ~ {}` and capture the
`system_chat` packet. Compare its text with the list above.

### 3.1 First spawn setup

The first spawn setup of a zombified piglin gives a `golden_sword` in the main hand, or a
`golden_spear` with the chance 1/20 (TRK-E1 shows the sword in `set_equipment`). The setup has
more random parts: baby, left hand, attribute bonuses. `natural-spawning.md` states them.

## 4. The viewers of a subject

**TRK-RNG-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). Let `r` be
the client tracking range of the type, in columns. Let `p` be the server property
`entity-broadcast-range-percentage`, 100 by default. The effective range in blocks is
`floor(p × 16 × r' / 100)`, where `r'` is the largest of `r` and the ranges of the passengers of
the subject, each in columns. Let `d` be the view distance of the viewer: its requested view
distance, clamped to `[2, server view distance]`. The tracking range is
`R = min(effective range, 16 × d)`.

TRK-E3 confirms `R`: with `d = 10`, a zombified piglin at a horizontal distance of 127 and 128
blocks pairs, and at 129 and 140 blocks it does not. With `d = 4`, it pairs at 60 blocks and does
not pair at 70 blocks.

**TRK-RNG-002** (`source_suggested`, semantic, confidence: high). A viewer and a subject pair
when each of these conditions is true:

- the subject is not the own player of the viewer;
- `Δx² + Δz² ≤ R²`, where `Δ` is the position of the viewer minus the position of the subject;
- the client of the viewer has the column of the subject, and the column is not waiting to be
  sent.

The vertical distance does not count. A pairing ends when one condition becomes false.

**TRK-RNG-003** (`source_suggested`, semantic, confidence: medium). The reference checks the
conditions of TRK-RNG-002 again when the subject moves to another section, and for all subjects
when a player moves to another section or joins. A server can check the conditions at another
time. These are valid choices:

- The server pairs at the granularity of tiles (REPL §7.2). A subject whose tile has a nearest
  point at `R` or less from the viewer can pair. The server must pair each subject inside `R`,
  except when the cap or the budget of REPL §7.4 stops it.
- The server must end a pairing when the nearest point of the tile of the subject is more than
  `R + 8` blocks from the viewer. The 8 blocks are the hysteresis of REPL §7.4.

Thus a client can see a subject up to one tile diagonal and 8 blocks farther than `R`. A vanilla
client draws such a subject as any other entity.

**TRK-RNG-004** (`source_suggested`, client agreement, confidence: medium). The spawn bundle of a
subject goes to a viewer only after the column of the subject reached that viewer. Experiment:
TRK-E5.

## 5. The spawn bundle

**TRK-SPN-001** (`source_suggested_black_box_confirmed`, wire, confidence: high). When a pairing
starts, the viewer receives one bundle: `bundle_delimiter`, then the packets of this list in this
order, then `bundle_delimiter`. A packet with no content is absent.

1. `add_entity` (TRK-SPN-002).
2. `set_entity_data` with each entry whose value is not the default value, in the order of the
   indices (TRK-DATA-002).
3. `update_attributes` with the syncable attributes of TRK-DATA-004, when the list is not empty.
4. `set_equipment` with each slot that is not empty, in the order of the slots.
5. Hooks out of scope: `set_passengers` for a vehicle and for a passenger, and
   `set_entity_link` for a leashed mob.

TRK-E1 shows the bundle of a summoned zombified piglin: `add_entity`, then `set_entity_data`
with only index 9, then `update_attributes` with `movement_speed`. With the first spawn setup,
`set_equipment` follows.

**TRK-SPN-002** (`source_suggested_black_box_confirmed`, wire, confidence: high). The fields of
`add_entity`:

| Field | Value |
|-------|-------|
| entity id | the network id, a VarInt |
| UUID | the UUID of the entity |
| type | the wire id of the type, a VarInt |
| x, y, z | the position base of the tracker, three f64 values |
| velocity | the last sent velocity of the tracker, in the form of TRK-WIRE-005 |
| pitch, yaw, head yaw | the last sent packed angles of the tracker, one byte each |
| data | 0 for a mob |

A viewer that pairs later gets the base, not the current position. The next update moves it from
the base. Before the first update, the base is the position of the entity when the tracker starts,
and the last sent velocity and angles are its values at that time.

## 6. Entity data

**TRK-DATA-001** (`source_suggested_black_box_confirmed`, wire, confidence: high). The entity data
of a type is a list of entries. Each entry has an index, a serializer and a default value. The
entries of a type are the entries of each part of its chain, in the order of the chain. The
indices count from 0 without a gap. The chain of a zombified piglin is: entity, living entity,
mob, zombie. The parts monster and zombified piglin add no entry.

The serializer ids are:

| Id | Serializer | Id | Serializer |
|----|------------|----|------------|
| 0 | byte | 11 | optional block position |
| 1 | VarInt | 12 | direction |
| 2 | VarLong | 13 | optional living entity reference |
| 3 | f32 | 14 | block state |
| 4 | string | 15 | optional block state |
| 5 | text component | 16 | particle |
| 6 | optional text component | 17 | list of particles |
| 7 | item stack | 18 | villager data |
| 8 | boolean | 19 | optional unsigned int |
| 9 | rotations | 20 | pose |
| 10 | block position | 21 to 43 | the variants and states of `mcrs_minecraft_protocol_game::entity::MetaDataValue`, in its order |

The order of `MetaDataValue` in `crates/mcrs_minecraft_protocol_game/src/entity.rs` agrees with
the vanilla registration order for ids 0 to 43.

**TRK-DATA-002** (`source_suggested_black_box_confirmed`, wire, confidence: high). The entries
of a zombified piglin:

| Index | Part | Meaning | Serializer | Default |
|-------|------|---------|------------|---------|
| 0 | entity | shared flags: bit 0 on fire, bit 1 shift key, bit 3 sprint, bit 4 swim, bit 5 invisible, bit 6 glowing, bit 7 fall flying | 0 | 0 |
| 1 | entity | air supply | 1 | 300 |
| 2 | entity | custom name | 6 | absent |
| 3 | entity | custom name visible | 8 | false |
| 4 | entity | silent | 8 | false |
| 5 | entity | no gravity | 8 | false |
| 6 | entity | pose | 20 | `standing` (0) |
| 7 | entity | ticks frozen | 1 | 0 |
| 8 | living entity | living flags: bit 0 item in use, bit 1 off hand, bit 2 spin attack | 0 | 0 |
| 9 | living entity | health | 3 | 1.0 |
| 10 | living entity | effect particles | 17 | empty list |
| 11 | living entity | effect ambience | 8 | false |
| 12 | living entity | arrows in the body | 1 | 0 |
| 13 | living entity | stingers in the body | 1 | 0 |
| 14 | living entity | sleeping position | 11 | absent |
| 15 | mob | mob flags: bit 0 no AI, bit 1 left handed, bit 2 aggressive | 0 | 0 |
| 16 | zombie | baby | 8 | false |
| 17 | zombie | special type | 1 | 0 |
| 18 | zombie | drowned conversion | 8 | false |
| 19 | zombie | frostbite conversion | 8 | false |

Indices 0 to 7 are the base entries of every entity type. Indices 8 to 14 are the entries of each
living entity. Index 15 is the entry of each mob. TRK-E1 and TRK-E2 confirm indices 2, 6, 9, 15 and
16 with their serializers.

**TRK-DATA-003** (`black_box_confirmed`, wire, confidence: high). A zombified piglin that `summon`
makes with `{}` has the health 20.0 and the default value in each other entry. Its spawn bundle
thus carries only the entry `(9, 3, 20.0)`.

**TRK-DATA-004** (`source_suggested_black_box_confirmed`, client agreement, confidence: medium).
The `update_attributes` of the spawn bundle must hold each syncable attribute whose base value
differs from the default of TRK-TYPE-003, or that has a modifier. It can hold other syncable
attributes. The reference holds the attributes that the server read or changed, for example
`movement_speed` of a zombified piglin with its default base `0.23F`. When an attribute changes
later, the next update sends `update_attributes` with the changed attributes, after
`set_entity_data`. TRK-E2 shows the modifier `minecraft:baby`, amount 0.5, operation 1, on
`movement_speed` of a baby.

**TRK-DATA-005** (`source_suggested`, wire, confidence: high). A dirty entry makes the tracker
tick an update (TRK-UPD-001). The update sends `set_entity_data` with each dirty entry in the
order of the indices, after the move packet. The entry is then clean. A player subject also
receives its own `set_entity_data` (MVC-PKT-031).

## 7. Updates

**TRK-UPD-001** (`source_suggested_black_box_confirmed`, wire, confidence: high). Let `c` be the
tracker tick counter and `n` the update interval. A tracker tick is an update when one of these
conditions is true:

- `c mod n = 0`;
- an entity data entry is dirty;
- the subject got an impulse or jumped in this tick (the "needs sync" mark).

The counter starts at 0, so the first tracker tick is an update. The counter counts only the
tracker ticks, not the server ticks. A tracker ticks only when the subject is in the entity
ticking range, has the "needs sync" mark, or moved to another section in this tick. A type with no update interval updates only for a dirty entry or
the mark. TRK-E2 shows updates each 3 ticks of a falling zombified piglin.

**TRK-UPD-002** (`source_suggested_black_box_confirmed`, wire, confidence: high). An update of a
subject that is not a passenger sends, in this order:

1. `set_entity_motion`, when TRK-UPD-007 sends the velocity;
2. the move packet of TRK-UPD-003, when there is one;
3. `set_entity_data` of TRK-DATA-005, and `update_attributes` of TRK-DATA-004;
4. `rotate_head`, when TRK-UPD-008 sends the head yaw.

The order is that of each update in TRK-E1 and TRK-E2. A passenger, a minecart on rails and an
arrow have other forms; they are hooks out of scope.

**TRK-UPD-003** (`source_suggested_black_box_confirmed`, wire, confidence: high). Let `P` be the
position at the end of the tick and `B` the position base. Let `yaw'` and `pitch'` be the packed
angles of the row, and `yaw0` and `pitch0` the last sent packed angles.

- The position changed when `|P − B|²` in f64 is at least `7.6293945E-6F` widened.
- The position is due when it changed or when `c mod 60 = 0`.
- The rotation is due when `yaw' ≠ yaw0` or `pitch' ≠ pitch0`.

The tracker selects the packet in this order:

| Condition | Packet |
|-----------|--------|
| the subject needs a precise position, or more than 400 updates came after the last `entity_position_sync`, or the subject stopped a ride, or the on-ground flag differs from the last sent flag | `entity_position_sync` (TRK-WIRE-003) |
| the position is due, and the path of TRK-UPD-005 does not fit TRK-WIRE-001 | `entity_position_sync` |
| the position is due and the rotation is due | `move_entity_pos_rot` |
| the position is due | `move_entity_pos` |
| the rotation is due | `move_entity_rot` |
| none | no move packet |

After a packet with a position, `B = P`. After a packet with a rotation, `yaw0 = yaw'` and
`pitch0 = pitch'`. `entity_position_sync` has both. It also sets the last sent on-ground flag and
starts the count of 400 again. An item has the extra precise case of PHY-CMP-002.

TRK-E2 shows the landing of a zombified piglin: the on-ground flag changes, and the update sends
`entity_position_sync`. It shows a zero step each 60 tracker ticks of a piglin at rest.

**TRK-UPD-004** (`source_suggested_black_box_confirmed`, wire, confidence: high). Each move
packet and `entity_position_sync` carry the on-ground flag of the row at the end of the tick. A
codec that writes a constant flag does not conform.

**TRK-UPD-005** (`source_suggested_black_box_confirmed`, wire, confidence: high). The path of an
update of a living entity is a list of steps. Each step is a position and the number of tracker
ticks since the step before. The tracker makes the list in this way:

1. In each tracker tick, add 1 to the tick count `k` of the open step.
2. In a tracker tick in which the subject bounced on a block, add a step at the position of that
   tick with `k` ticks, when `k > 0`, and set `k` to 0.
3. At an update, add a step at `P` with `k` ticks, when `k > 0`. Send the list as a stepped path.
   If the list is empty, send the linear path `P`. Then clear the list and set `k` to 0.

The "start now" flag of a stepped path is true when the subject has the "needs sync" mark of
TRK-UPD-001. An entity that is not living sends the linear path `P`. TRK-E2 shows one step of 3
ticks with the flag false in each update of a falling zombified piglin, and the linear zero move
of the first update.

**TRK-UPD-006** (`source_suggested`, semantic, confidence: medium). Hook: a move that the client
also simulates, for example a piston push, adds its displacement to each open step when its
squared length is more than `1.0E-5F`. No system of G1 makes such a move.

**TRK-UPD-007** (`source_suggested_black_box_confirmed`, wire, confidence: high). When the type
tracks velocity, or the subject has the "needs sync" mark, an update compares the velocity `v`
with the last sent velocity `v0`. Let `q = |v − v0|²` in f64. The update sends
`set_entity_motion` with `v`, and sets `v0 = v`, when `q > 1.0E-7`, or when `q > 0` and
`|v|² = 0`. A hurt subject also sends `set_entity_motion` at the end of its tracker tick, to the
viewers and to itself (`entity-damage.md`). TRK-E2 shows `set_entity_motion` before each move of a
falling zombified piglin.

**TRK-UPD-008** (`source_suggested_black_box_confirmed`, wire, confidence: high). In each update,
the tracker compares the packed head yaw with the last sent packed head yaw. When they differ, it
sends `rotate_head` and keeps the new value. The head yaw goes only in an update. TRK-E2 shows
`rotate_head` only with the updates each 3 ticks.

**TRK-UPD-009** (`source_suggested`, semantic, confidence: medium). The server sends each update
of a living subject that is not a player at every tier, at the update interval of its type.
`replication.md` tiers 1 to 3 do not lower the rate of such a subject. The reason is TRK-CLI-001:
the client plays all steps of a path in the update interval, so a slower rate makes the mob move
in jumps. `physics-architecture.md` §15, question 58, records this decision.

**TRK-UPD-010** (`unresolved`, unspecified). The tick of the first update, relative to the tick
of the spawn, is a choice of the server. The reference runs the first update in the tick after
`summon`. A parity test must compare updates from the first update on, not by server tick.

## 8. Wire encodings

**TRK-WIRE-001** (`source_suggested_black_box_confirmed`, wire, confidence: high). A relative
position unit is `1/4096` block. For a coordinate `a`, let `e(a) = floor(a × 4096 + 0.5)` on
the f64 product: a half rounds up, also for a negative value. `f64::round` rounds a negative half
away from zero, so `position_delta` of `delta_base.rs` differs for such a value. The delta of an axis from `B` to `P` is
`e(P) − e(B)`. A delta fits when each axis is in `[−32768, 32767]`. A stepped path fits when the
delta of each step from the step before fits; the first step starts at `B`. The client decodes an
axis with a delta of 0 as the coordinate of its base, without rounding.

**TRK-WIRE-002** (`source_suggested_black_box_confirmed`, wire, confidence: high). The body of
`move_entity_pos` is:

1. the entity id, a VarInt;
2. the properties, a VarInt: bit 0 is the on-ground flag, and the bits from 1 up hold the step
   count `s`;
3. when `s = 0`: the delta of the linear path, three signed 16-bit integers `x`, `y`, `z`;
4. when `s > 0`: the "start now" flag, a boolean, then `s` steps. A step is the tick count, a
   VarInt, then the delta, three signed 16-bit integers.

`move_entity_pos_rot` has the same body, then the packed yaw and the packed pitch, one byte each.
`move_entity_rot` has the entity id, the on-ground flag as a boolean, the packed yaw and the packed
pitch. The client rejects a step count that is more than the remaining bytes divided by 7.

The codec of `crates/mcrs_minecraft_protocol_game/src/clientbound.rs` (`VecDelta::Stepped`) does
not write the "start now" flag. A vanilla client misreads each stepped move of that codec.
Section 15, step 1, fixes it.

**TRK-WIRE-003** (`source_suggested_black_box_confirmed`, wire, confidence: high). The body of
`entity_position_sync` is:

1. the entity id, a VarInt;
2. the path: the kind, a VarInt, 0 for linear and 1 for stepped;
   - linear: the position, three f64 values;
   - stepped: the step count, a VarInt, then each step as three f64 values and the tick count, a
     VarInt, then the "start now" flag, a boolean;
3. the yaw and the pitch, two f32 values in degrees, not packed;
4. the on-ground flag, a boolean.

The path is the path of TRK-UPD-005 with absolute positions. TRK-E2 shows a stepped path of one
step at the landing. `PositionPath::Stepped` of the same file does not write the trailing flag.

**TRK-WIRE-004** (`source_suggested`, wire, confidence: high). The packed angle of an f32 angle
`a` in degrees is the low 8 bits of `floor(f32(f32(a × 256.0F) / 360.0F))`, where the floor
gives a 32-bit integer. The client unpacks a byte `b` as the signed value of `b` times `360 / 256`.
`ByteAngle::from_degrees` of `byte_angle.rs` rounds to the nearest unit. It gives a different byte
for each angle with a fraction of a unit of 0.5 or more, for example 1.0 degree (vanilla 0,
`from_degrees` 1). Experiment: TRK-E4.

**TRK-WIRE-005** (`source_suggested`, wire, confidence: high). The velocity of `add_entity` and
`set_entity_motion` uses the low-precision vector form of `LpVec3` in
`mcrs_minecraft_protocol_game`. TRK-E1 decodes the captured velocities with that form.

**TRK-WIRE-006** (`source_suggested`, wire, confidence: high). `remove_entities` holds a list of
network ids, each a VarInt. The reference sends one packet with one id for each removal. A server
can put more ids in one packet; the client removes each id of the list.

## 9. Removal

**TRK-REM-001** (`source_suggested_black_box_confirmed`, wire, confidence: high). When a pairing
ends, the viewer receives `remove_entities` with the network id of the subject. A pairing ends
when TRK-RNG-002 or TRK-RNG-003 ends it, when the subject leaves the world (TRK-LIFE-004), or when
the subject leaves the dimension. TRK-E1 shows the removal of a subject that teleports out of the
range, and of all subjects when the viewer teleports 400 blocks away. The removal of a killed
zombified piglin comes 20 ticks after its death, after `entity_event` 60.

## 10. Client behaviour

**TRK-CLI-001** (`source_suggested`, client agreement, confidence: medium). The vanilla client
interpolates a living entity over `m` client ticks. For a player, `m` is 3. For another living
entity, `m` is the update interval of its type. When a move arrives:

- When the "start now" flag is true, the client drops the steps that it did not play.
- It adds the steps of the path. A linear path is one step whose weight is the update interval of
  the type. A step of a stepped path has its tick count as its weight.
- It sets the time that remains to `m`.

In each tick, it plays the share `1 / (time that remains)` of the weight that remains. Thus the
client plays all queued steps within `m` ticks of the last move. A rotation of the move is
spread over the steps in proportion to their tick counts.

**TRK-CLI-002** (`source_suggested`, client agreement, confidence: medium). The client ignores a
move, a data, a motion or a removal packet for a network id that it does not know (REPL C1).

## 11. Intentional deviations

### TRK-DEV-001: No first spawn setup

- **Rule:** TRK-SUM-002, step 5.
- **Reference:** `summon` without `<nbt>` gives the mob the first spawn setup of its type: for a
  zombified piglin a golden sword or a golden spear, the chance of a baby and of the left hand,
  and random attribute bonuses.
- **Visible to the client:** yes. Until `natural-spawning.md` lands, a summoned zombified piglin
  holds nothing, is never a baby and is never left handed.
- **Test:** a differential test summons with `{}`, which skips the setup in both servers.

### TRK-DEV-002: Keys of the `nbt` argument

- **Rule:** TRK-SUM-003.
- **Reference:** the reference loads each key of the compound into the new entity, and ignores an
  unknown key.
- **Visible to the client:** yes. A command with a key fails with `commands.summon.failed`, and no
  entity appears. The server never gives a different meaning to a key.
- **Test:** a differential test uses only `{}` or no compound.

### Known vanilla quirks

| Rule | Quirk |
|------|-------|
| TRK-SPN-001 | The first update of the reference sends `set_entity_data` and `update_attributes` again, with the values of the spawn bundle. A server does not need to send them again. |
| TRK-UPD-001 | Under `/tick freeze`, the reference still runs the trackers in each server tick, and a `/tick step` moves the entity only once. An experiment on the update timing must not freeze the ticks. |

## 12. Unresolved

- **U1**: no corpus gives the type data of TRK-TYPE-001. Section 15 adds an entity definition
  corpus to the oracle; this also resolves U2 of `entity-physics.md`.
- **U2**: TRK-RNG-004 has no observation of a vanilla client that receives an entity before its
  column.
- **U3**: the exact entry of the effect particle list (serializer 17) and of the custom name
  (serializer 6) have no captured bytes. G1 sends neither.

## 13. Experiments and captured results

The probe is a client in offline mode, with the compression off. It logs in, answers the
configuration, `keep_alive`, `chunk_batch_finished` and `player_position`, and decodes the
entity packets. The console of the server runs the commands.

Start the vanilla dedicated server of the version of this document with `level-type=minecraft:flat`,
`view-distance=10`, `pause-when-empty-seconds=-1` and `online-mode=false`. Put the probe at
`0.5 -60 0.5`. The ground of the flat world is at `y = -60`.

| Experiment | Steps | Rules |
|------------|-------|-------|
| TRK-E1 | Freeze the ticks. Run `summon zombified_piglin 0.5 -40 4.5 {}`, then `summon zombified_piglin 3.5 -60 4.5`. Step 40 ticks. Kill the first piglin, step 25 ticks. Teleport the second piglin to `200.5 -60 0.5`. Teleport the probe to `400.5 -60 0.5`. | TRK-TYPE-002, TRK-LIFE-003, TRK-SPN-001, TRK-SPN-002, TRK-DATA-003, TRK-REM-001 |
| TRK-E2 | Do not freeze. Run `summon zombified_piglin 0.5 -40 4.5 {}` and log 5 seconds with the time of each packet. Run `summon zombified_piglin 6.5 -60 4.5 {IsBaby:1b,NoAI:1b}` and `summon zombified_piglin 8.5 -60 4.5 {NoAI:1b,CustomName:'x'}`. | TRK-DATA-002, TRK-DATA-004, TRK-UPD-001 to TRK-UPD-005, TRK-UPD-007, TRK-UPD-008 |
| TRK-E3 | With the probe view distance 10, summon a piglin with `{NoAI:1b}` at the horizontal distances 127, 128, 129 and 140. With the view distance 4, summon at 60 and 70. | TRK-RNG-001, TRK-RNG-002 |
| TRK-E4 | Run `summon zombie 0.5 -60 4.5 {NoAI:1b,Rotation:[1.0f,0f]}` and with `0.7f`, `1.4f` and `-1.0f`. Read the yaw byte of `add_entity`. | TRK-WIRE-004 |
| TRK-E5 | With a vanilla client and `view-distance=2`, summon a piglin in a column at the edge of the view and record when the client draws it. | TRK-RNG-004 |
| TRK-E6 | Run the exit test of section 15 on the farm save in the vanilla server, and in this server with a vanilla client. | section 15 |

TRK-E1 to TRK-E3 ran on `26.4-snapshot-7` on 2026-11-08. TRK-E4 to TRK-E6 have not run.

**TRK-E1**, the spawn bundles (network ids of that run):

```text
add_entity 51 minecraft:zombified_piglin (0.5, -40.0, 4.5) velocity (0, 0, 0) pitch 0 yaw 0 head 0 data 0
set_entity_data 51 [(9, serializer 3, 20.0)]
update_attributes 51 [movement_speed base 0.23000000417232513, no modifier]
add_entity 52 minecraft:zombified_piglin (3.5, -60.0, 4.5) velocity (0, 0, 0) pitch 0 yaw 0 head 0 data 0
set_entity_data 52 [(9, serializer 3, 20.0)]
update_attributes 52 [movement_speed base 0.23000000417232513]
set_equipment 52 slot 0 (main hand), count 1, item golden_sword
```

A piglin with `{NoAI:1b}` adds the entry `(15, serializer 0, 1)`. The death of a piglin sends
`entity_event 3`, then `set_entity_data [(6, serializer 20, 7), (9, serializer 3, 0.0)]` (pose
`dying`, health 0), and 20 ticks later `entity_event 60` and `remove_entities`. The teleport out of
the range sends `remove_entities [52]` in the same tick. The teleport of the probe removes each
piglin.

**TRK-E2**, a zombified piglin that falls from `y = -40` to `y = -60`. The times are in
milliseconds after `add_entity`; 50 ms is one tick.

| Time | Packets |
|------|---------|
| 50 | `set_entity_motion (0, -0.078374, 0)`; `move_entity_pos` linear `(0, 0, 0)`, ground 0 |
| 198 | `set_entity_motion (0, -0.30434, 0)`; `move_entity_pos` stepped, start 0, one step `(3 ticks, 0, -1901, 0)`, ground 0 |
| 346 | the same with `-0.516938` and `(3, 0, -4622, 0)` |
| 498 | `-0.717085`, `(3, 0, -7183, 0)` |
| 645 | `-0.905451`, `(3, 0, -9594, 0)` |
| 798 | `-1.082708`, `(3, 0, -11862, 0)` |
| 945 | `-1.249588`, `(3, 0, -13997, 0)` |
| 1099 | `-1.40658`, `(3, 0, -16006, 0)` |
| 1249 | `set_entity_motion (0, -0.078374, 0)`; `entity_position_sync` stepped, one step `(0.5, -60.0, 4.5)` after 3 ticks, start 0, yaw 0, pitch 0, ground 1; `set_entity_data [(9, 3, 3.0)]` (fall damage); `set_entity_motion (0, -0.078374, 0)` (hurt) |
| 3048 | `move_entity_pos` stepped `(3, 0, 0, 0)`, ground 1: the update of `c mod 60 = 0` |
| 4099 | the AI starts to walk: `set_entity_motion`, `move_entity_pos_rot` stepped `(3, 673, 0, 673)` with rotation `(-32, 0)`, `rotate_head -15` |

The baby has `set_entity_data [(9, 3, 20.0), (15, 0, 1), (16, 8, true)]` and the attribute
`movement_speed` with the modifier `minecraft:baby`, amount 0.5, operation 1. The named piglin has
the entry `(2, 6, present)` and no health entry in the decoded prefix.

**TRK-E3**: with the view distance 10, the probe received `add_entity` for the piglins at 127 and
128 blocks and nothing for 129 and 140 blocks. With the view distance 4, it received the piglin at
60 blocks and not the piglin at 70 blocks.

## 14. Relation to other documents

- `entity-physics.md`: PHY-CMP-001 and PHY-CMP-002 state the update form of each category. This
  document refines PHY-CMP-002 for a living entity with TRK-UPD-003 and TRK-UPD-005.
- `replication.md`: section 7.11 states the update rule of the categories that the client
  simulates. TRK-UPD-009 states the rule of the living subjects. The claim C5 of section 5 does
  not hold for a living entity that is not a player (TRK-CLI-001); `physics-architecture.md` §15,
  question 58, records it.
- `command-execute.md`: CMD-EX-022 uses TRK-SUM-002.

## 15. Implementation

### 15.1 Order of work

1. **Wire fixes** (`mcrs_minecraft_protocol_game`). Add the "start now" flag to the stepped
   `VecDelta` (before the steps) and to the stepped `PositionPath` (after the steps)
   (TRK-WIRE-002, TRK-WIRE-003). Make `ByteAngle::from_degrees` follow TRK-WIRE-004. Change the
   existing tests that encode the old bytes, and name the rules in the commit. The player codec
   passes `false` for the flag.
2. **Type data** (`mcrs_minecraft_entity` or a new small crate under the rust-style rules). A
   table of TRK-TYPE-001 for the types of TRK-TYPE-002. A motion row of the zombified piglin with
   the eye height `1.79F`, next to `MotionRow::zombie`.
3. **Mob rows** (`mcrs_minecraft_engine_physics`). Spawn and removal requests in the life cycle of
   `DomainInput` (TRK-LIFE-001 to TRK-LIFE-004). A birth source of the kind `command`. The cold
   state in a side table keyed by `EntityId`, as `ItemStacks`; `physics-architecture.md` §15,
   question 56, records this choice.
4. **The `summon` command** (`mcrs_minecraft_command_vanilla`). TRK-SUM-001 to TRK-SUM-004; it
   writes a spawn request.
5. **Tile deltas of mobs** (Output stage). The pass publishes a spawn, a move with the on-ground
   flag, the velocity and the rotations, a data change and a despawn of each mob in its tile
   delta, from the mover lists.
6. **Codec of mobs** (`mcrs_minecraft_replication`). One codec of the world handles players and
   mobs, because a muxer link has one codec. For each mob subject: the tracker of section 7
   (counter, position base, last sent angles, velocity, on-ground flag, open step), the spawn
   bundle of section 5 and the removal of section 9. A mob updates at its type interval at every
   tier (TRK-UPD-009).
7. **Range** (`mcrs_minecraft_engine`, muxer planner). Pair by TRK-RNG-001 to TRK-RNG-004.
   `physics-architecture.md` §15, question 57, records the gap of the fixed view radius of 3
   tiles.

### 15.2 Data tables and their sources

| Table | Source |
|-------|--------|
| Wire ids of entity types and attributes | `assets/mcrs/reports/registries.json`, already in `EntityType` and `Attribute` |
| Packet ids | `assets/mcrs/reports/packets.json`, already in the packet tables |
| Type data of TRK-TYPE-001 | until the corpus exists, the table of TRK-TYPE-002. Then a new corpus `assets/mcrs/entity_definition/<type>.json` from the `dumpDefinitions` task of `tools/vanilla-oracle` (data source set): dimensions, eye height, fixed flag, range, interval, tracks velocity, summonable, allowed in peaceful, fire immune, default attributes |
| Entity data layout | TRK-DATA-002 for the chain of the zombified piglin; the corpus can add the layout of each type later |
| Serializer ids | TRK-DATA-001; the existing `MetaDataValue` |
| Default attributes | TRK-TYPE-003; the existing `MotionAttributes` for the motion part |
| Tags | none |

### 15.3 Tests

- Codec, wire: a stepped `move_entity_pos` and `entity_position_sync` round-trip with the flag;
  the bytes of one stepped move equal the bytes of TRK-E2 (`(3, 0, -1901, 0)`, start 0, ground 0).
- Codec, packed angles: the bytes of TRK-WIRE-004 for 0, 1.0, 1.4, 359.9 and -1.0 degrees.
- Tracker, pure: feed the row positions of a falling piglin, one for each tick. The packets equal
  the sequence of TRK-E2 from the first update to the landing: one update each 3 ticks, the
  velocity before the move, one step of 3 ticks, `entity_position_sync` at the landing with the
  ground flag 1, and a zero step at `c mod 60 = 0`.
- Spawn bundle: the bundle of a summoned piglin equals TRK-E1 without `update_attributes`, or
  with it when the codec sends `movement_speed` (TRK-DATA-004 allows both).
- Range: a pure test of TRK-RNG-001 with the cases of TRK-E3.
- `summon`: each failure of TRK-SUM-002 and TRK-SUM-003, and the success message of TRK-SUM-004.
- Determinism: the world hash of `tick_determinism.rs` covers the mob rows.

### 15.4 Exit test

The exit of step G1 of `gold-farm.md` §4: a summoned piglin falls into the chamber, and a vanilla
client shows it.

1. Harness, automated (synthetic world): in `crates/mcrs_minecraft_server/tests/it`, build a
   shaft of air 20 blocks high over a floor, with a viewer 10 blocks away. Run
   `summon minecraft:zombified_piglin <top of the shaft> {}`. Tick 40 times. Decode the bytes of the
   viewer with `mcrs_minecraft_client_world`. Check the spawn bundle, the update each 3 ticks,
   the landing sync with the ground flag 1, and that the decoded position equals the row position
   at the floor. This is "harness only".
2. Raw, differential (TRK-E6): run the vanilla server on a copy of the farm save, put a player at
   `-319.5 175 -609.5` in the nether, and run
   `execute in minecraft:the_nether run summon minecraft:zombified_piglin -317.0 197 -609.0 {}`.
   The measurement app of `gold-farm.md` §8 counts the piglin in the `kill` area after it lands.
   Run the same on this server. The piglin must end in the chamber box of the app.
3. Raw, client (blocked: human): run this server on the save, join with the vanilla client of
   `tools/vanilla-client`, stand at `-319.5 175 -609.5` and run the command of item 2. The client
   must show the piglin from the summon to the chamber. Save a screenshot of the piglin in the
   chamber at `target/g1/vanilla_client_chamber.png`.
