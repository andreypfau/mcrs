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
- **Type data**: the values that an entity type gives to each entity of the type (section 1).
- **Network id**: the id of an entity on the wire. A player keeps the id of its session. Another
  entity gets an even id (`replication.md`, section 7.2).
- **Tracker**: the state that the server keeps for one subject to send it to viewers. A subject
  has one tracker. All viewers of the subject share it.
- **Tracker tick**: one run of the tracker of a subject. The tracker counts its ticks from 0. The
  first tracker tick is the tick in which the server starts to track the subject. A tracker ticks
  in each tick in which the row of the subject ticks in the entity pass.
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
  (section 3.1).
- **Hook**: a rule whose input no system of G1 produces. G1 does not implement a hook; the system
  that produces the input implements it.
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
- the add of a mob by the `summon` command, and the add and the removal of a mob by another system;
- the `summon` command with the forms that section 3 lists;
- the replication of each entity that is not a player: tracking range, spawn bundle, entity data
  of the base entries, of the zombified piglin and of the four farm animals of `mob-animals.md`,
  updates, head rotation and removal;
- the type data, the entity data and the `summon` keys of the pig, the cow, the sheep and the
  chicken. `mob-animals.md` (ANIM) states their behaviour.

These subjects are out of scope:

| Subject | Reason |
|---------|--------|
| The first spawn setup of a type (equipment, baby, left hand, random attribute bonuses) | `natural-spawning.md` states it for natural spawns and for `summon`. TRK-DEV-001 records the gap until then. `mob-animals.md`, ANIM-SPAWN-001, states it for the four farm animals. |
| Keys of the `nbt` argument of `summon` | No step of `gold-farm.md` needs them. TRK-DEV-002 records the rejection. |
| Health changes, damage, death and the death animation | `entity-damage.md`. TRK-LIFE-004 states only the removal. |
| AI, goals, look control and head turns of a mob | `mob-goals.md`. This document sends the rotations that the row holds. |
| Save and load of mobs, and the removal of a mob when its column unloads | No step of `gold-farm.md` needs them. A row in a section that is not loaded does not tick. |
| The entity data of an item entity | PHY-CMP-026. Section 6 keeps the model general. |
| Passengers, leashes, minecarts on rails, projectiles, item frames | No mob of the farm uses them. Their packets have hooks in TRK-SPN-001 and TRK-UPD-002. |
| Teams and the team colour of a name | No team exists in the server. |
| The kill of a mob by a command, and mobs as targets of a selector | `command-kill.md` states `kill`. A kill of a mob is a hit and a death (`entity-damage.md`), not a removal request (TRK-LIFE-004). `argument-entity.md`, ARG-ENT-013, states how a selector finds mobs. In G1 an entity selector (`@e`, `@n`) finds only players. |
| The move of a mob by `tp` | `command-teleport.md` states it. No step of `gold-farm.md` needs it. |
| The difficulty of a world | The server has no difficulty state. TRK-DEV-003 records the peaceful check of `summon`. |
| The on-ground flag of the moves of a player subject | `replication.md` owns the player codec. Section 12, U4, records the gap. |

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
| summonable | false for a type that `summon` must not make |
| default attributes | the base value of each attribute that an entity of the type has |
| entity data layout | the entries of section 6, by the chain of the type |

The default values are: client tracking range 5, update interval 3, summonable true. The type data
of the reference has more fields: "tracks velocity" (true for each mob, false for the player),
"allowed in peaceful" and "fire immune". G1 reads none of them. G2 adds the fields fire-immune,
the base `armor` and the hit kind (Appendix A of `entity-damage.md` and of `attributes.md`); the
zombified piglin is fire-immune, with the armor 2 and the normal hit kind.

**TRK-TYPE-002** (`source_suggested_black_box_confirmed`, wire, confidence: high). The types of
the farm have these values. TRK-E1 and TRK-E3 confirm the wire id, the range and the interval of
the zombified piglin.

| Type | Wire id | Category | Width × height | Eye height | Range | Interval |
|------|---------|----------|----------------|------------|-------|----------|
| `minecraft:zombified_piglin` | 160 | walker | `0.6F × 1.95F`, scalable | `1.79F` | 8 | 3 |
| `minecraft:player` | (registry) | player | `0.6F × 1.8F` | `1.62F` | 32 | 2 |
| `minecraft:pig` | 103 | walker | `0.9F × 0.9F`, scalable | `0.765F` | 10 | 3 |
| `minecraft:cow` | 30 | walker | `0.9F × 1.4F`, scalable | `1.3F` | 10 | 3 |
| `minecraft:sheep` | 116 | walker | `0.9F × 1.3F`, scalable | `1.235F` | 10 | 3 |
| `minecraft:chicken` | 26 | walker | `0.4F × 0.7F`, scalable | `0.644F` | 10 | 3 |

The player is not summonable. The motion row of the zombified piglin is
`MotionRow::living(Category::Walker, ..)` with these dimensions; its step height is the default
0.6. A baby zombified piglin has the dimensions `0.49F × 0.98F` and the
eye height `0.78F`. G1 makes no baby (TRK-DEV-001).

The four farm animals are summonable and are not fire immune. Their step height is the default
0.6, and their base `armor` is 0. A baby animal has the dimensions of `mob-animals.md`,
ANIM-TYPE-001, not the adult dimensions times an age scale. The type artefact
(`crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json`) confirms the width, the
height, the eye height, the range and the interval of the four types. The wire ids come from
`assets/mcrs/reports/registries.json`.

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
| `spawn_reinforcements` | 0 |

The attributes of a living entity are the common set of ATR-BASE-002 of `attributes.md`, with
the attribute defaults of ATR-DEF-001. The client has the same defaults for each type. Thus the client agrees on
each attribute that the server does not send. The walker tick reads the attributes of
`MotionAttributes`; of the fields of `MotionAttributes`, only `movement_speed` of a zombified
piglin differs from the attribute
default (0.7). In this document, "the base of the type" is the value of this table, and "the
attribute default" is the default of the attribute itself.

`attributes.md` owns these values. ATR-BASE-002 states the list of a living entity, and
ATR-BASE-003 with its Appendix A states the base of each type. This rule and TRK-TYPE-004 repeat
the rows that this document needs.

**TRK-TYPE-004** (`source_suggested_black_box_confirmed`, client agreement, confidence: high). The
default attributes of the four farm animals are the attributes of a living entity with their
attribute defaults, and these values:

| Attribute | `pig` | `cow` | `sheep` | `chicken` |
|-----------|-------|-------|---------|-----------|
| `max_health` | 10 | 10 | 8 | 4 |
| `movement_speed` | 0.25 | `0.2F`, widened: `0.20000000298023224` | `0.23F`, widened: `0.23000000417232513` | 0.25 |
| `follow_range` | 16 | 16 | 16 | 16 |
| `tempt_range` | 10 | 10 | 10 | 10 |

`tempt_range` is not syncable. ANIM-E2 of `mob-animals.md` shows `movement_speed` 0.25 and
`follow_range` 16 in the save of a pig.

## 2. The life cycle of a mob

**TRK-LIFE-001** (`source_suggested`, semantic, confidence: high). A mob is a row of the entity
pass of its dimension world. The row has:

- the motion row of the type: the category, the dimensions and the step height of TRK-TYPE-001;
- the living state with the default attributes of TRK-TYPE-003;
- the UUID of PHY-CMP-013, which is also its order key;
- a network id from the ids of the cell;
- the yaw of `Living`, as raw f32 degrees.

The cold state of the mob holds only its entity type. In G1 each entity data value of a mob is a
function of its type (TRK-DATA-003), so G1 stores no entity data value. From G2, the row is the
authority for the health and the dying state (DMG-DTH-005): the entries 6 (pose) and 9 (health)
come from the row, never from the cold state. In G1 the pitch and the
head yaw of a mob are 0 and have no storage; `mob-goals.md` adds them with the look control
(GOAL-CTL-001, GOAL-CTL-004, GOAL-CTL-009). The
walker tick of PHY-ORD-002 moves the mob in each tick. Until `mob-goals.md` gives the mob an AI, the mob has no
steering: its input vector is zero and it does not jump.

**TRK-LIFE-002** (`source_suggested`, semantic, confidence: medium). A spawn request has the
type, the position, the rotation, the velocity, the UUID and the entity data
values that differ from the defaults. These systems make spawn requests:

- the `summon` command (section 3), with the birth source of the kind `command` and the command
  sequence of PHY-CMP-013. The source key is the block of the spawn point, then the block of the
  command source. The sequence numbers the summons of that pair in the tick, from 0. A summon
  without a `UUID` key computes a new UUID from the world seed, the dimension key, the birth clock,
  this source key and this sequence when it runs (PHY-CMP-013). Thus
  the success message of TRK-SUM-004 has the UUID, and the spawn request carries it;
- natural spawning (`natural-spawning.md`), with the birth source of the kind `spawn`;
- any later system that adds a mob, with the birth source that PHY-CMP-013 gives it.

The life cycle of `DomainInput` applies the spawn requests of the tick in the order of their
UUIDs (`physics-architecture.md`, section 3.9). The command effects apply in `DomainInput` before
the life cycle, so the new row ticks first in the `Simulate` of the same tick.

**TRK-LIFE-003** (`black_box_confirmed`, semantic, confidence: high). A new mob of a spawn request
without a rotation has the yaw 0, the pitch 0 and the head yaw 0. Its velocity is zero. TRK-E1
shows these values in `add_entity` for a summoned zombified piglin, with and without the first
spawn setup.

**TRK-LIFE-004** (`source_suggested`, semantic, confidence: medium). Hook. A removal request
names a row and the reason "discarded" (a despawn of `natural-spawning.md`). The life cycle of
`DomainInput` removes the row from its tile, the grid and the cold state. The tracker of the
subject then ends each pairing (TRK-REM-001). A killed mob makes no request: the pass removes its
row in the tick in which its death time reaches 20, on the expiry path of an item, and the tracker
sends `remove_entities` in the same tick (DMG-DTH-003). No system of G1 makes a removal request.

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
SNBT-ARG-002 (`minecraft:nbt_compound_tag`, parser id 21, no properties). `VanillaArgument` has
no variant for `nbt_compound_tag` yet; G1 adds one, with the read of SNBT-ARG-002. The permission
is PERM-LEVEL-001, level `gamemasters`. The client makes the suggestions of
`minecraft:summonable_entities` itself (CMDP-ARG-002), so the server answers no suggestion request
for `<entity>`.

**TRK-SUM-002** (`source_suggested`, semantic, confidence: high). The behaviour:

1. Let `type` be the entity type of `<entity>`. If `type` is not summonable, fail with
   `entity.not_summonable`, with the identifier of the type.
2. Let `pos` be the value of `<pos>`, or the source position when the node is absent. If the block
   position that contains `pos` is outside the spawnable bounds, fail with
   `commands.summon.invalidPosition`.
3. If the difficulty of the source dimension is `peaceful` and `type` is not allowed in peaceful,
   fail with `commands.summon.failed.peaceful`. G1 skips this step (TRK-DEV-003).
4. Let `setup` be true when the `<nbt>` node is absent, and false when it is present. Check the
   compound with TRK-SUM-003.
5. Compute the UUID (TRK-LIFE-002). Make a spawn request of `type` at `pos` in
   the source dimension, with no rotation. When `setup` is true, the mob also gets the first spawn
   setup of its type.
6. If an entity with the same UUID exists in the dimension, fail with
   `commands.summon.failed.uuid`. A new UUID of PHY-CMP-013 cannot collide in practice, so G1 does not
   implement this step.
7. Send the success message of TRK-SUM-004. The result is 1.

When `type` is not a mob, step 5 makes the entity of the category of the type. G1 makes only the
types of TRK-TYPE-002 that are summonable; another type fails with `commands.summon.failed`
(TRK-DEV-005). CMD-EX-022 uses steps 1
to 6 with `pos` from the source and `setup` true.

**TRK-SUM-003** (`source_suggested`, semantic, confidence: medium). The compound of `<nbt>` can
hold no key, except the keys of TRK-SUM-005 for the types that it names. A compound with another
key fails with `commands.summon.failed`. TRK-DEV-002 records this. The empty compound `{}` makes the
mob without the first spawn setup. A compound with a key also makes the mob without the first spawn
setup.

**TRK-SUM-005** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). For the
types `pig`, `cow`, `sheep` and `chicken`, the compound of `<nbt>` can hold these keys. The values
have the meaning of `mob-animals.md`, ANIM-SAVE-002.

| Key | Tag | Types | Effect |
|-----|-----|-------|--------|
| `NoAI` | byte | each | the mob flag "no AI" (index 15, bit 0) |
| `Rotation` | list of two f32: yaw, pitch | each | the yaw, the body yaw and the head yaw take the first value; the pitch takes the second |
| `PersistenceRequired` | byte | each | ANIM-SAVE-001 |
| `Age` | int | each | the age; a negative value makes a baby |
| `AgeLocked` | byte | each | the age lock |
| `InLove` | int | each | the love time |
| `variant` | string, an identifier | `pig`, `cow`, `chicken` | the variant |
| `sound_variant` | string, an identifier | `pig`, `cow`, `chicken` | the sound variant |
| `Color` | byte, from 0 to 15 | `sheep` | the wool colour |
| `Sheared` | byte | `sheep` | the sheared state |
| `EggLayTime` | int | `chicken` | the egg timer |
| `equipment` | compound with only the key `saddle`, whose value is `{id:"minecraft:saddle"}` | `pig` | the saddle, with the default drop chance |

A value with the wrong tag, or an identifier that is not in its registry, does not change the
default, as in the reference. A spawn request then carries the entity data values that differ from
the defaults (TRK-LIFE-002). ANIM-E1 to ANIM-E8 of `mob-animals.md` use these keys on a vanilla
server.

**TRK-SUM-007** (`source_suggested_black_box_confirmed`, semantic, confidence: high). A mob of
`summon` has the idle time 0 (`mob-goals.md`, GOAL-STR-004). No key of TRK-SUM-005 sets the idle
time, and no key stops its increase, except `NoAI:1b`, which stops the AI. These are the effects
of the key `PersistenceRequired:1b` on a farm animal:

- The save holds the key with the value true.
- The mob skips the removal steps of the despawn check (AIT-VAN-020). An animal is not removed by
  these steps without the key either (`mob-animals.md`, ANIM-SAVE-001).
- The goals, the idle time, the entity data, the spawn bundle and the tracking do not change. The
  AI work in each tick does not change.

Thus a pig with `PersistenceRequired:1b` stops its strolls after AI step 99 when no non-spectator
player is near, as a pig without the key does. A benchmark summons each pig with the compound `{}`.
The compound skips the first spawn setup, so the pig is an adult of the default variant.
`mob-goals.md`, GOAL-WAKE-019 and GOAL-WAKE-020, state the motion and the reset of the idle time.

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

**TRK-SUM-006** (`source_suggested`, semantic, confidence: medium). These facts apply to a
sequence of many summons, for example 50,000 runs of `summon pig x y z` from the server console:

- One run makes exactly one mob. The command has no count argument. The server has no limit on the
  number of mobs in a dimension.
- `<pos>` is a `vec3`. An integer x or z gets the centre correction, so `summon pig 10 -60 10`
  puts the pig at `10.5 -60 10.5` (ARG-COORD-003). The y does not change.
- The new mob has the yaw 0, the pitch 0, the head yaw 0 and the body yaw 0 (TRK-LIFE-003). The
  first spawn setup does not change the rotation. Only the key `Rotation` of `<nbt>` sets it
  (TRK-SUM-005).
- Two mobs at the same position overlap. The push of `entity-physics.md` separates them from
  their first tick. The game rule `max_entity_cramming` controls the crowd damage
  (`entity-damage.md`, DMG-CRW-001).
- A summon into a column that does not tick entities still succeeds. The mob does not tick
  (TRK-LIFE-005). In a column that is not loaded, the server also does not track the mob, and a
  selector does not find it. A benchmark must load the area, for example with `forceload`, before
  the summons.
- Each run sends its own success message (TRK-SUM-004) and has the result 1. From the console,
  the message goes to the server log and to the operators (`commands-packet.md`, CMDP-EXEC-004).
- The runs of one tick apply in the order of their command sequence (TRK-LIFE-002).

Experiment: on a superflat world, run `summon pig 10 -60 10` from the console. Run
`data get entity @n[type=pig] Pos` and `data get entity @n[type=pig] Rotation`. The values are
`[10.5d, -60.0d, 10.5d]` and `[0.0f, 0.0f]`.

### 3.1 First spawn setup

The first spawn setup of a zombified piglin gives a `golden_sword` in the main hand, or a
`golden_spear` with the chance 1/20 (TRK-E1 shows the sword in `set_equipment`). The setup has
more random parts: baby, left hand, attribute bonuses. `natural-spawning.md` states them.

The first spawn setup of a farm animal is `mob-animals.md`, ANIM-SPAWN-001. `summon` without
`<nbt>` applies it, so TRK-DEV-001 does not apply to these four types.

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
client draws such a subject as any other entity. G1 does not implement this rule; it uses the
square of TRK-DEV-004.

**TRK-RNG-004** (`source_suggested_black_box_confirmed`, client agreement, confidence: medium).
The spawn bundle of a subject goes to a viewer only after the column of the subject reached that
viewer. G1 does not implement this rule (TRK-DEV-004).

TRK-E5 shows the reference: a piglin summoned in a column that waits to be sent gets no spawn
bundle when the column arrives. The pairing starts only at the next check of TRK-RNG-003, when the
viewer or the subject moves to another section; in TRK-E5 that was 25 seconds after the column
arrived. This delay is an accident of the check times (section 11, "Known vanilla quirks"). A
server can pair at the arrival of the column.

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

In this server the tracker of a mob starts in `apply` of the codec, with the position, the
velocity and the angles of the `Spawn` event. `Muxer::publish` runs `apply`, then `hold`, then
`finish_tick` in one tick. Thus a spawn bundle of the first tick uses that start state, and
`finish_tick` then runs the tracker tick `c = 0`. The update of `c = 0` also goes to each viewer
that `hold` added in the same tick.

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
makes with `{}` has the health 20.0, its `max_health`, and the default value in each other entry.
Its spawn bundle thus carries only the entry `(9, 3, 20.0)`. In G1 no entry changes after the spawn,
so the codec derives this entry from the type.

**TRK-DATA-004** (`source_suggested_black_box_confirmed`, client agreement, confidence: medium).
The `update_attributes` of the spawn bundle must hold each syncable attribute whose base value
differs from the base of the type (TRK-TYPE-003), or that has a modifier. It can hold other
syncable attributes. The column "Syncable" of ATR-DEF-001 of `attributes.md` states which
attributes are syncable. The reference holds the attributes that the server read or changed, for
example `movement_speed` of a zombified piglin with its base `0.23F` and no modifier (TRK-E1). G1
sends no entry for an instance with the base of the type and no modifier (ATR-NET-003 of
`attributes.md`). Thus the bundle of a summoned zombified piglin has no `update_attributes`
(TRK-SPN-001). Before it compares a bundle with TRK-E1, a differential test drops each such entry
of the reference, and then each `update_attributes` with no entry. Hook: when an attribute changes
later, the next update sends `update_attributes` with the changed attributes, after
`set_entity_data` (ATR-NET-002 and ATR-NET-003 of `attributes.md`). TRK-E2 shows the modifier
`minecraft:baby`, amount 0.5, operation 1, on `movement_speed` of a baby.

**TRK-DATA-005** (`source_suggested`, wire, confidence: high). Hook. A dirty entry makes the tracker
tick an update (TRK-UPD-001). The update sends `set_entity_data` with each dirty entry in the
order of the indices, after the move packet. The entry is then clean. A player subject also
receives its own `set_entity_data` (MVC-PKT-031).

**TRK-DATA-006** (`source_suggested_black_box_confirmed`, wire, confidence: high). The chain of a
farm animal is: entity, living entity, mob, ageable mob, then the part of the type. The parts
"pathfinder mob" and "animal" add no entry. Indices 0 to 15 are those of TRK-DATA-002. The other
entries:

| Index | Type | Meaning | Serializer | Default |
|-------|------|---------|------------|---------|
| 16 | each | baby | 8 | false |
| 17 | each | age locked | 8 | false |
| 18 | `pig` | boost length, in ticks | 1 | 0 |
| 19 | `pig` | pig variant | 28 | the id of `minecraft:temperate` |
| 20 | `pig` | pig sound variant | 29 | the id of `minecraft:classic` |
| 18 | `cow` | cow variant | 23 | the id of `minecraft:temperate` |
| 19 | `cow` | cow sound variant | 24 | the id of `minecraft:classic` |
| 18 | `sheep` | wool: bits 0 to 3 the colour id, bit 4 sheared | 0 | 0 |
| 18 | `chicken` | chicken variant | 30 | the id of `minecraft:temperate` |
| 19 | `chicken` | chicken sound variant | 31 | the id of `minecraft:classic` |

A variant id is the position of the entry in the registry data that the server sent, from 0
(`mob-pig.md`, MPIG-DATA-001). When the synced registry has no default entry, the default is one
entry of the registry. The colour ids are: 0 `white`, 1 `orange`, 2 `magenta`, 3 `light_blue`,
4 `yellow`, 5 `lime`, 6 `pink`, 7 `gray`, 8 `light_gray`, 9 `cyan`, 10 `purple`, 11 `blue`,
12 `brown`, 13 `green`, 14 `red`, 15 `black`.

The type artefact confirms each index and each serializer. The values of these entries are state of
the animal (`mob-animals.md`), not a function of the type. Thus TRK-DATA-003 does not apply to them.
The spawn bundle of a summoned pig with `{}` carries only the entry `(9, 3, 10.0)`. A pig with the
variant `cold` also carries the entry 19. Experiment: ANIM-E9 of `mob-animals.md`.

## 7. Updates

**TRK-UPD-001** (`source_suggested_black_box_confirmed`, wire, confidence: high). Let `c` be the
tracker tick counter and `n` the update interval. A tracker tick is an update when one of these
conditions is true:

- `c mod n = 0`;
- an entity data entry is dirty;
- the subject got an impulse or jumped in this tick (the "needs sync" mark).

The counter starts at 0, so the first tracker tick is an update. The counter counts only the
tracker ticks, not the server ticks. In the reference, a tracker ticks only when the subject is in
the entity ticking range, has the "needs sync" mark, or moved to another section. In this server,
the entity ticking range is the set of rows that tick in the pass (TRK-LIFE-005). While a row ticks
in each tick, `c` is the tick minus the tick of the first tracker tick. A type with no update
interval updates only for a dirty entry or the mark. TRK-E2 shows updates each 3 ticks of a falling
zombified piglin.

In G1 only the first condition occurs. The dirty entry (TRK-DATA-005) and the "needs sync" mark are
hooks: no system of G1 changes an entry, gives an impulse to a mob or makes it jump.

**TRK-UPD-002** (`source_suggested_black_box_confirmed`, wire, confidence: high). An update of a
subject that is not a passenger sends, in this order:

1. `set_entity_motion`, when TRK-UPD-007 sends the velocity;
2. the move packet of TRK-UPD-003, when there is one;
3. `set_entity_data` of TRK-DATA-005, and `update_attributes` of TRK-DATA-004;
4. `rotate_head`, when TRK-UPD-008 sends the head yaw.

The signals of the subject in the tick (`damage_event`, `entity_event`) come before the update, and
the velocity of a hurt subject after it (`entity-damage.md`, DMG-NET-003, DMG-NET-004).

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
| the subject needs a precise position (hook), or more than 400 updates came after the last `entity_position_sync`, or the subject stopped a ride, or the on-ground flag differs from the last sent flag | `entity_position_sync` (TRK-WIRE-003); the end of a ride is a hook |
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
packet and `entity_position_sync` of a subject that is not a player carry the on-ground flag of the
row at the end of the tick. A codec that writes a constant flag for such a subject does not
conform. The flag of a player subject is out of scope (section 12, U4).

**TRK-UPD-005** (`source_suggested_black_box_confirmed`, wire, confidence: high). The path of an
update of a living entity is a list of steps. Each step is a position and the number of tracker
ticks since the step before. The tracker makes the list in this way:

1. `k` is the tick count of the open step. It is 0 at the first tracker tick.
2. Hook: in a tracker tick in which the subject bounced on a block, add a step at the position of
   that tick with `k` ticks, when `k > 0`, and set `k` to 0.
3. At an update, add a step at `P` with `k` ticks, when `k > 0`. This is the path of the update.
   It is a stepped path, or the linear path `P` when the list is empty.
4. At the end of each update, clear the list and set `k` to 0. This also occurs at an update that
   sends no move packet or only a rotation.
5. At the end of each tracker tick, after the update, add 1 to `k`.

The "start now" flag of a stepped path is true when the subject has the "needs sync" mark of
TRK-UPD-001; in G1 it is false. An entity that is not living sends the linear path `P`. TRK-E2 shows one step of 3
ticks with the flag false in each update of a falling zombified piglin, and the linear zero move
of the first update.

**TRK-UPD-006** (`source_suggested`, semantic, confidence: medium). Hook: a move that the client
also simulates, for example a piston push, adds its displacement to each open step when its
squared length is more than `1.0E-5F`. No system of G1 makes such a move.

**TRK-UPD-007** (`source_suggested_black_box_confirmed`, wire, confidence: high). When the type
tracks velocity (each mob type), or the subject has the "needs sync" mark, an update compares the velocity `v`
with the last sent velocity `v0`. Let `q = |v − v0|²` in f64. The update sends
`set_entity_motion` with `v`, and sets `v0 = v`, when `q > 1.0E-7`, or when `q > 0` and
`|v|² = 0`. Hook: a hurt subject also sends `set_entity_motion` at the end of its tracker tick, to the
viewers and to itself (`entity-damage.md`). TRK-E2 shows `set_entity_motion` before each move of a
falling zombified piglin.

**TRK-UPD-008** (`source_suggested_black_box_confirmed`, wire, confidence: high). In each update,
the tracker compares the packed head yaw with the last sent packed head yaw. When they differ, it
sends `rotate_head` and keeps the new value. The head yaw goes only in an update. TRK-E2 shows
`rotate_head` only with the updates each 3 ticks.

**TRK-UPD-009** (`source_suggested`, semantic, confidence: medium). A living subject that is not a
player has one tracker, which all viewers share at every tier. The updates of section 7 go to each
viewer that holds the subject, in the tick of the update. The tiers of `replication.md` §7.4 and
its keyframes do not apply to such a subject, and §7.11 applies only to the categories that the
client simulates. The reason is TRK-CLI-001: the client plays all steps of a path in the update
interval, so a slower rate makes the mob move in jumps. `physics-architecture.md` §15, question 58,
records this decision.

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
axis with a delta of 0 as the coordinate of its base, without rounding. Test vector: `B = 0` and
`P = −0.5 / 4096` (exact in f64) give `e(P) = 0` and the delta 0; `f64::round` gives −1.

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
pitch, in this order. TRK-E4 captured the raw frames `39 23 00 40 15` and `39 23 00 8E E0`
after `rotate` of a zombie with no AI to `90 30` and to `200.7 -45`: packet id 57, entity id 35,
on-ground flag 0, then the yaw (64, 142) and the pitch (21, −32). The order of earlier versions
(yaw, pitch, flag) reads the second frame as yaw 0 and pitch 142, which is not a pitch. The
`move_entity_pos_rot` at 4099 ms of TRK-E2 (body yaw −32, head −15) does not contradict this: the
head of a walking mob turns apart from its body. `ClientboundMoveEntityRot` of
`clientbound.rs` writes the flag after the angles, which is the order of earlier versions; section
15, step 1, fixes it. The client rejects a step count that is more than the remaining bytes divided by 7.

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

**TRK-WIRE-004** (`source_suggested_black_box_confirmed`, wire, confidence: high). The packed angle of an f32 angle
`a` in degrees is the low 8 bits of `floor(f32(f32(a × 256.0F) / 360.0F))`, where the floor
gives a 32-bit integer. The client unpacks a byte `b` as the signed value of `b` times `360 / 256`.
The cast of the f32 floor to a 32-bit integer saturates, and NaN gives 0. TRK-E4 confirms these
bytes of the yaw in `add_entity`: 0.7 → 0, 1.0 → 0, 1.4 → 0, 1.40625 → 1, 2.109375 → 1, −0.5 →
255, −1.0 → 255, 359.9 → 255, and the pitch 30 → 21. The value 2.109375 is 1.5 units exactly:
the floor gives 1, a rounding gives 2. `ByteAngle::from_degrees` of `byte_angle.rs`
rounds to the nearest unit. It gives a different byte for each angle with a fraction of a unit of
0.5 or more, for example 1.0 degree (vanilla 0, `from_degrees` 1). `ByteAngle::to_degrees` unpacks
the byte as unsigned; only `mcrs_minecraft_client` reads it, and an unsigned angle is the same turn.
TRK-E4 in section 13 gives the results.

**TRK-WIRE-005** (`source_suggested_black_box_confirmed`, wire, confidence: high). The velocity of
`add_entity` uses the low-precision vector form of `LpVec3` in `mcrs_minecraft_protocol_game`. The
body of `set_entity_motion` is the entity id, a VarInt, then the velocity in the same form. TRK-E2
decodes the captured velocities with that form. The protocol crate has no struct for
`set_entity_motion`; section 15, step 1, adds it.

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
- **This server:** the keys of TRK-SUM-005 load into a farm animal. Each other key fails.
- **Visible to the client:** yes. A command with a key fails with `commands.summon.failed`, and no
  entity appears. The server never gives a different meaning to a key.
- **Test:** a differential test uses only `{}` or no compound.

### TRK-DEV-003: No peaceful check

- **Rule:** TRK-SUM-002, step 3.
- **Reference:** in the difficulty `peaceful`, `summon` of a type that is not allowed in peaceful
  fails with `commands.summon.failed.peaceful`.
- **Visible to the client:** yes, only in the difficulty `peaceful`. The server has no difficulty
  state, so `summon` of a zombified piglin always makes it. The step comes back with the difficulty.
- **Test:** a differential test runs on a difficulty that is not `peaceful`.

### TRK-DEV-004: Pairing by a square of tiles

- **Rule:** TRK-RNG-002 to TRK-RNG-004.
- **Reference:** a viewer sees a subject at a horizontal distance of `R` or less, with `R` of
  TRK-RNG-001, and only after its client has the column of the subject.
- **G1:** `VIEW_RADIUS` is 4 tiles on X and Z (`physics-architecture.md` §15, question 57). A viewer
  holds each subject of each tile at a tile distance of 4 or less from its own tile, subject to the
  cap and the budget of REPL §7.4. The server does not use the view distance of the viewer, the
  range of the type or the column of the subject.
- **Visible to the client:** yes. A tile is 32 blocks, so each subject inside 128 blocks pairs,
  which is `R` of a zombified piglin at a view distance of 8 or more. A subject up to 160 blocks
  away on each axis (226 blocks on the diagonal) can also pair. A client with a view distance of
  less than 8 sees mobs in columns that it does not have (U2).
- **Test:** a differential test compares only the subjects inside `R`, and runs the viewer at the
  view distance 8 or more.

### TRK-DEV-005: Only the types of the type table

- **Rule:** TRK-SUM-002, the paragraph after step 7.
- **Reference:** `summon` makes each summonable type.
- **Visible to the client:** yes. `summon` of a type outside TRK-TYPE-002 fails with
  `commands.summon.failed`, and no entity appears. Each later type table removes the failure for
  its types.
- **Test:** a differential test summons only the types of TRK-TYPE-002.

### Known vanilla quirks

| Rule | Quirk |
|------|-------|
| TRK-SPN-001 | The first update of the reference sends `set_entity_data` and `update_attributes` again, with the values of the spawn bundle. A server does not need to send them again. |
| TRK-RNG-004 | A subject whose column waits to be sent at its spawn stays hidden until the viewer or the subject moves to another section (TRK-E5). |
| TRK-UPD-001 | Under `/tick freeze`, the reference still runs the trackers in each server tick, and a `/tick step` moves the entity only once. An experiment on the update timing must not freeze the ticks. |

## 12. Unresolved

- **U1**: no corpus gives the type data of TRK-TYPE-001. Section 15 adds an entity definition
  corpus to the oracle; this also resolves U2 of `entity-physics.md`.
- **U2**: TRK-RNG-004 has no observation of a vanilla client that receives an entity before its
  column. The vanilla server never sends one (TRK-E5), so only this server can show it, after G1:
  run item 3 of section 15.4 with the client view distance 2 and a mob 40 blocks away.
- **U3**: the exact entry of the effect particle list (serializer 17) and of the custom name
  (serializer 6) have no captured bytes. G1 sends neither.
- **U4**: the player codec writes the on-ground flag `true` in each move of a player subject. The
  accepted state of a player has the flag, but no step of `gold-farm.md` sends it, and no rule
  states yet whether a change of the flag of a player forces `entity_position_sync`.

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
| TRK-E4 | Run `summon zombie 0.5 -60 4.5 {NoAI:1b,Rotation:[1.0f,0f],Tags:["e4"]}`, then the same with the yaws `0.7f`, `1.4f`, `-1.0f`, `359.9f`, `1.40625f`, `2.109375f`, and `[-0.5f,30f]`. Read the angle bytes of `add_entity`. Run `rotate @e[tag=e4,limit=1] 90 30` and `200.7 -45` and log the raw frame of `move_entity_rot`. | TRK-WIRE-004 |
| TRK-E5 | Let the probe answer `chunk_batch_finished` with 0.5 chunks for each tick, so that far columns wait. 4 seconds after the join, run `summon zombified_piglin 120.5 -60 0.5 {NoAI:1b,PersistenceRequired:1b}` and the same at `4.5 -60 4.5`. Log the time of `level_chunk_with_light` of column `(7, 0)` and of each `add_entity`. After 30 seconds, teleport the probe 17 blocks in `x`. | TRK-RNG-004 |
| TRK-E6 | Copy `~/mc-worlds/upgraded/hermitcraft10` to a temporary directory and remove its data packs. Start the vanilla server on it with `difficulty=hard` and `view-distance=10`. Join with the probe and teleport it to `-319.5 175 -609.5` in `minecraft:the_nether`, yaw `-90`. Run `gamerule spawn_monsters false` and `kill @e[type=zombified_piglin]`. Run `execute in minecraft:the_nether run summon minecraft:zombified_piglin -316.5 196 -609.5 {}`. Wait 200 ticks without a freeze. Run `data get entity @e[type=zombified_piglin,limit=1,sort=nearest] Pos` from the console and log the decoded packets of the piglin. Write the end `Pos` and the update list to `crates/mcrs_minecraft_server/tests/fixtures/g1/vanilla_farm_drop.txt`. | section 15.4 |

TRK-E1 to TRK-E6 ran on `26.4-snapshot-7` on 2026-11-08.

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
| 3248 (another mob of the world, id 65) | `move_entity_rot` id 65, ground 1, yaw 91, pitch 0; `rotate_head 65 91` |
| 4099 | the AI starts to walk: `set_entity_motion`, `move_entity_pos_rot` stepped `(3, 673, 0, 673)` with rotation `(-32, 0)`, `rotate_head -15` |

The baby has `set_entity_data [(9, 3, 20.0), (15, 0, 1), (16, 8, true)]` and the attribute
`movement_speed` with the modifier `minecraft:baby`, amount 0.5, operation 1. The named piglin has
the entry `(2, 6, present)` and no health entry in the decoded prefix.

**TRK-E3**: with the view distance 10, the probe received `add_entity` for the piglins at 127 and
128 blocks and nothing for 129 and 140 blocks. With the view distance 4, it received the piglin at
60 blocks and not the piglin at 70 blocks.

**TRK-E4**: the bytes of TRK-WIRE-004, and the frames of `move_entity_rot` of TRK-WIRE-002. The raw capture is outside the repository. A test that needs it adds only the part that it checks, as a fixture of `mcrs_minecraft_fixtures`.

**TRK-E5**: column `(7, 0)` arrived 5 seconds after the summon. The near piglin paired at once. The
piglin at `120.5 -60 0.5`, 126 blocks from the probe, got no `add_entity` until the probe moved to
another section, 25 seconds after the column arrived. A first run with zombies showed nothing for
another reason: a zombie with no AI burns in the sun and dies.

**TRK-E6**: the point `-317.0 197 -609.0` is on a platform, not in a shaft: the block
`(-317, 196, -609)` is solid. A scan of `execute if block` at `y = 196` and `y = 190` shows the
shaft at the block column `x = -317`, `z = -610` to `-608`, so the summon point is
`-316.5 196 -609.5`. The piglin fell to `y = 174.9375` (the top of honey), at `t = 26` ticks, and
then walked 1 block in `z`. Its end position `-316.70 174.9375 -608.50` is inside the kill box of
`gold-farm.md` §8. The updates of the fall are those of TRK-E2. The landing came one tick after the
update before it: the fall damage made the health entry dirty, which made an update
(TRK-UPD-001); the update sent `entity_position_sync` with one step of 1 tick, after
`entity_event 54` of the honey block. The raw capture is outside the repository. A test that needs it adds only the part that it checks, as a fixture of `mcrs_minecraft_fixtures`.

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

1. **Wire fixes** (`mcrs_minecraft_protocol_game`).
   - Add the "start now" flag to the stepped `VecDelta`, before the steps, and to the stepped
     `PositionPath`, after the steps (TRK-WIRE-002, TRK-WIRE-003). The player codec passes `false`.
   - Write `ClientboundMoveEntityRot` as id, on-ground flag, yaw, pitch (TRK-WIRE-002).
   - Add `ClientboundSetEntityMotion`: a VarInt id and an `LpVec3` (TRK-WIRE-005).
   - Make `ByteAngle::from_degrees` follow TRK-WIRE-004.
   - Change the existing tests that encode the old bytes, and name the rules in the commit.
2. **Type data** (`mcrs_minecraft_entity`, or a small crate under the rust-style rules). The rows
   of TRK-TYPE-002: wire id, motion row (`MotionRow::living(Category::Walker, ..)` with the
   dimensions of the table), range, interval, summonable, and the base `movement_speed` `0.23F`.
3. **Mob rows** (`mcrs_minecraft_engine_physics`). Spawn requests in the life cycle of
   `DomainInput` (TRK-LIFE-001 to TRK-LIFE-003). The cold state is a side table from `EntityId` to
   the entity type (`physics-architecture.md` §15, question 56). Add the birth source
   `BirthSource::Command { spawn, source }` to `mcrs_minecraft_entity_key`. Its kind number is 2
   and its source key is the block of the spawn point, then the block of the command source
   (PHY-CMP-013). `BirthSequences` numbers the summons of one pair in one tick, from 0. A kernel change is allowed here, because PHY-CMP-013
   requires the source and the kernel lacks it. The removal requests of TRK-LIFE-004 are a hook: G1 adds no producer.
4. **The `summon` command** (`mcrs_minecraft_command_vanilla`). The argument
   `minecraft:nbt_compound_tag` in `VanillaArgument`. TRK-SUM-001 to TRK-SUM-004: the handler
   computes the UUID, writes the spawn request as a command effect, and sends the
   message.
5. **Tile deltas of mobs** (Output stage of the dimension). The engine pass keeps no mover list.
   In each tick, publish one `Move` event for each mob row that ticked, with the position, the
   velocity, the yaw and the on-ground flag. Publish `Spawn` in the tick of the spawn, and `Leave`
   and `Enter` when `move_rows` moves a row to another tile. The tile delta data is
   `ReplicationData`: `PlayerData` for a player or `MobReport` for a mob, because a muxer link has
   one codec.
6. **Codec of mobs** (`mcrs_minecraft_replication`). One codec of the world handles players and
   mobs. For each mob subject it keeps the tracker of section 7: the start tick, the position base,
   the last sent packed angles, the last sent velocity and on-ground flag, the update count for
   400, and the open step `k`. The muxer calls `codec.blob` only for due tiles and tiers, so the
   mob updates do not come from `blob`:
   - `apply` records the event of each mob of the tick.
   - `finish_tick` runs the tracker tick of each mob (TRK-UPD-001 to TRK-UPD-008) and owes the
     bytes of each update to each viewer that holds the subject in the hold set of the codec, as for
     `Joined`.
   - `blob` writes nothing for a mob, and `refresh` and the keyframes do nothing for a mob.
   - `hold` writes the spawn bundle of section 5 and the removal of section 9.

   The tracker uses `position_delta::encode` of `mcrs_minecraft_protocol_game`, with the rounding
   of TRK-WIRE-001. It shares the move choice of TRK-UPD-003 (`move_choice.rs`) with
   `DeltaBase::advance` of the players. It does not reuse `DeltaBase::advance` or `Motion`: they
   drop the steps that do not move, write a constant on-ground flag, send a keyframe each 100 ticks
   and couple the head to the yaw.
7. **Range** (`mcrs_minecraft_engine`). Set `VIEW_RADIUS` to 4 tiles on X and Z (TRK-DEV-004).
   `entity_replication.rs` uses the same radius for the players, so a viewer also subscribes to
   81 tiles of players instead of 49. G1 accepts this cost: a player has the range 160 blocks at
   the view distance 10, so the change also brings players nearer to the vanilla range.
   Write TRK-RNG-001 as a pure function with the cases of TRK-E3, for the decision of question 57.

### 15.2 Data tables and their sources

| Table | Source |
|-------|--------|
| Wire ids of entity types and attributes | `assets/mcrs/reports/registries.json`, already in `EntityType` and `Attribute` |
| Packet ids | `assets/mcrs/reports/packets.json`, already in the packet tables |
| Type data of TRK-TYPE-001 | until the corpus exists, the table of TRK-TYPE-002. Later, a corpus `assets/mcrs/entity_definition/<type>.json` from the `dumpDefinitions` task of `tools/vanilla-oracle` (data source set) |
| Entity data layout | TRK-DATA-002 |
| Serializer ids | TRK-DATA-001; the existing `MetaDataValue` |
| Base attributes | TRK-TYPE-003; the existing `MotionAttributes` for the motion part |
| Tags | none |

### 15.3 Tests

- Codec, wire: a stepped `move_entity_pos` and `entity_position_sync` round-trip with the flag;
  the bytes of one stepped move equal TRK-E2 (`(3, 0, -1901, 0)`, start 0, ground 0);
  `move_entity_rot` writes the flag before the angles; `set_entity_motion` round-trips.
- Codec, numbers: the test vector of TRK-WIRE-001 and the bytes of TRK-WIRE-004.
- Tracker, pure: feed the row positions of a falling piglin, one for each tick. The packets equal
  TRK-E2 from the first update to the landing: the linear zero move at `c = 0`, one update each 3
  ticks, the velocity before the move, one step of 3 ticks, `entity_position_sync` at the landing
  with the ground flag 1, and a zero step at `c = 60` while the piglin rests.
- Spawn bundle: the bundle of a summoned piglin equals TRK-E1 without the entries that
  TRK-DATA-004 drops.
- Range: the pure function of TRK-RNG-001 with the cases of TRK-E3.
- `summon`: the failures of TRK-SUM-002 steps 1 and 2 and of TRK-SUM-003, and the message of
  TRK-SUM-004 with the UUID of the row.
- Determinism: `crates/mcrs_minecraft_server/tests/it/tick_determinism.rs` hashes no `EntityRows`
  today. Add the rows (position, velocity, flags, key) of mobs and items to the world hash.

### 15.4 Exit test

The exit of step G1 of `gold-farm.md` §4: a summoned piglin falls into the chamber, and a vanilla
client shows it.

1. Harness, automated (synthetic world). Extend `ClientModel` of
   `crates/mcrs_minecraft_server/tests/it/entity_replication.rs`, which decodes `add_entity`,
   `VecDelta` and `PositionPath`. Add `set_entity_data`, `set_entity_motion`, the "start now"
   flag and the on-ground flag. Build a shaft of air 20 blocks high over a floor, with a viewer 10
   blocks away. Run `summon minecraft:zombified_piglin <top of the shaft> {}` and tick 40 times.
   Check the spawn bundle, an update each 3 ticks, the landing sync with the ground flag 1, and
   that the decoded position equals the row position at the floor. This is "harness only".
2. Raw, the save. The fixture of TRK-E6 holds the vanilla fall. The test of this server is a local test, `#[ignore = "needs the farm save"]`,
   that loads `~/mc-worlds/upgraded/hermitcraft10`, runs the command of TRK-E6 and ticks 200 times.
   The row must end in the kill box: centre `-317 176 -609`, half sizes `1.5, 1.5, 1.6`, as the
   measurement app of `gold-farm.md` §8 states it. The end `y` must also equal the vanilla `y`,
   `174.9375`, within `1.0E-6`. The vanilla piglin has an AI, so `x` and `z` differ.
3. Raw, a vanilla client (automated). Run this server on the save. Start the vanilla client of
   `tools/vanilla-client` with an offline name; it joins with no human. From the console, teleport
   the player to `-319.5 175 -609.5` in the nether and run the command of TRK-E6. The check passes
   when the client stays connected for 60 ticks after the summon: a vanilla client disconnects on
   a move packet that it cannot decode. After the landing, take a screenshot of the client window
   with the macOS tool `screencapture -l <window id>` to `target/g1/vanilla_client_chamber.png`.
   Get the window id from the window list of Core Graphics, for example
   `python3 -c 'import Quartz; print([w["kCGWindowNumber"] for w in Quartz.CGWindowListCopyWindowInfo(0, 0) if w.get("kCGWindowOwnerName") == "java"])'`
   (`pyobjc-framework-Quartz`); the client of `tools/vanilla-client` runs in the `java` process.
