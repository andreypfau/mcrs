# Environment damage: fire, lava, air, freezing, contact, void and border

| Field | Value |
| --- | --- |
| Game version | `26.4-snapshot-7` |
| World version (`DataVersion`) | `5127` |
| Protocol version | `1073742169` |
| Data pack format | `126.0` |

This document states the damage that an entity takes from its surroundings. It states fire, lava,
drowning and the air supply, freezing in powder snow, contact with blocks, suffocation, the void
and the world border. Each source uses the hit procedure of `entity-damage.md` with its own damage
type. Fall damage and the damage of a magma block are in `entity-damage.md` (DMG-FALL-001 to
DMG-FALL-005, DMG-HOT-001). This document does not repeat them.

The rules come from the reference implementation. No experiment has run yet. Section 13 lists the
experiments that decide the open points.

## 0. Conventions

### 0.1 Terms

- **Drowning**, **freezing**, **suffocation**: the names of the damage sources of sections 3, 4 and 6.
- **Server**, **client**, **tick**, **entity**, **living entity**, **box**, **position**:
  `entity-damage.md` and `entity-physics.md` define these terms in section 0.1.
- **Base entity tick**, **block effects step**, **f32**, **f64**, **data pack**, **corpus**: the
  same sections define these terms.
- **Hit**, **full hit**, **partial hit**, **rejected hit**, **damage cooldown**, **fire-immune**,
  **alive**, **dying state**: as `entity-damage.md`, section 0.1, defines them.
- **Damage tag**: as `entity-damage.md`, section 0.1, defines it.
- **Contact test**: the test of PHY-GRV-014. The box of the entity touches the cell of a block on the path of the entity in this tick. The
  test shrinks the box by `1.0E-5F` on each side.
- **Fluid contact**: the box of the entity touches a fluid. The fluid model of the physics decides
  the test. Section 12 records it as U1.
- **Eye in water**: the eye position of the entity is inside water. The fluid model decides the
  test (U1).
- **Fire ticks**: an integer of an entity. A value more than 0 means that the entity burns. A
  value of 0 or less is the grace value of ENV-FIR-005.
- **Burning**: the state of an entity whose fire ticks are more than 0 and that is not fire-immune.
- **Ignite for `n` ticks**: the operation of ENV-FIR-003.
- **Air supply**: an integer of an entity. Section 3 states it.
- **Frozen ticks**: an integer of an entity from 0 to 140 or more. Section 4 states it.
- **Age**: the number of ticks since the entity was created or loaded. A load sets it to 0.
- **Rain at a block**: ENV-FIR-007 step 2 defines it.
- **Min height**: the value `min_y` of the dimension type of the dimension of the entity.
- **Border**: the world border of the dimension of the entity. The values of the border are the
  west, east, north and south limits, the buffer and the damage per block.
- **Buffer**: the distance in blocks that the border allows outside its limits before it damages
  a player. The command name is `worldborder damage buffer`. The save name is `safe_zone`.
- **Damage per block**: the f64 value that the command `worldborder damage amount` sets. The
  default is 0.2.

### 0.2 Provenance

The labels are those of `entity-physics.md`, section 0.2. Each rule states its kind and its
confidence, for example `(source_suggested, semantic, confidence: medium)`. The kinds are those of
`docs/spec/README.md`. No rule has a confirmed label yet.

### 0.3 ID prefix

| Prefix | Subject |
|--------|---------|
| `ENV-SRC` | the damage sources of this document and their data |
| `ENV-FIR` | fire, lava and burning |
| `ENV-AIR` | the air supply and drowning |
| `ENV-FRZ` | freezing |
| `ENV-CTC` | contact damage of blocks |
| `ENV-WAL` | suffocation inside a block |
| `ENV-VOID` | the void |
| `ENV-BRD` | the world border |
| `ENV-ORD` | the place of each damage in the tick |
| `ENV-NET` | what a client sees |

### 0.4 Artefacts cited

- The corpus: the damage types of the registry `minecraft:damage_type`, the damage tags, the
  entity type tags and the item tag `freeze_immune_wearables`. Section 1 lists the members that
  this document uses.
- The game rules `fire_damage`, `drowning_damage` and `freeze_damage` of
  `assets/mcrs/registry_definition/game_rule.json`. DMG-PLR-001 states what they do.
- `entity-damage.md`, Appendix A: the fire-immune types.
- `entity-physics.md`: PHY-GRV-014 (the contact test), PHY-RCH-007 (the blocks with an inside
  action), PHY-INP-021 (the blocks that cause suffocation) and PHY-ORD-001, PHY-ORD-002 (the tick).
- `command-weather.md`: the rain level.

## 1. The sources and their data

**ENV-SRC-001** (`source_suggested`, semantic, confidence: high). Each damage of this document is
a hit (DMG-HIT-001 to DMG-HIT-005). The source has no causing entity, no direct entity and no
source position. Thus the damage type decides the tags, and the cooldown (DMG-HIT-003) applies to
each hit. Two hits of different types in one tick share one cooldown.

**ENV-SRC-002** (`source_suggested`, semantic, confidence: high). The sources have these damage
types and amounts. The amount is the f32 value before the reductions of `entity-damage.md`,
section 4.

| Source | Damage type | Amount | Rule |
|--------|-------------|--------|------|
| burning | `on_fire` | 1.0 | ENV-FIR-002 |
| `fire` | `in_fire` | 1.0 | ENV-FIR-004 |
| `soul_fire` | `in_fire` | 2.0 | ENV-FIR-004 |
| lava and `lava_cauldron` | `lava` | 4.0 | ENV-FIR-006 |
| a lit `campfire` | `campfire` | 1.0 | ENV-CTC-002 |
| a lit `soul_campfire` | `campfire` | 2.0 | ENV-CTC-002 |
| underwater, no air | `drown` | 2.0 | ENV-AIR-003 |
| a fish, a squid or a tadpole out of water, no air | `drown` | 2.0 | ENV-AIR-006 |
| an axolotl or a nautilus out of its fluid, no air | `dry_out` | 2.0 | ENV-AIR-006 |
| a full freeze | `freeze` | 1.0, 1.5 or 5.0 | ENV-FRZ-003 |
| `cactus` | `cactus` | 1.0 | ENV-CTC-001 |
| `sweet_berry_bush` | `sweet_berry_bush` | 1.0 | ENV-CTC-003 |
| the eyes inside a block | `in_wall` | 1.0 | ENV-WAL-001 |
| below the min height | `out_of_world` | 4.0 | ENV-VOID-001 |
| outside the border buffer | `outside_border` | 1 or more | ENV-BRD-001 |

**ENV-SRC-003** (`source_suggested`, semantic, confidence: high). The corpus gives these damage
tags to the types. Only the tags that a rule of this document or of `entity-damage.md` reads are
in the table.

| Damage type | Tags |
|-------------|------|
| `in_fire` | `is_fire`, `no_knockback`, `bypasses_shield` |
| `campfire` | `is_fire`, `no_knockback`, `burn_from_stepping`, `bypasses_shield` |
| `lava` | `is_fire`, `no_knockback`, `bypasses_shield` |
| `on_fire` | `is_fire`, `bypasses_armor`, `no_knockback` |
| `drown` | `is_drowning`, `bypasses_armor`, `no_impact`, `no_knockback` |
| `dry_out` | `no_knockback`, `bypasses_shield` |
| `freeze` | `is_freezing`, `bypasses_armor`, `no_knockback` |
| `cactus` | `no_knockback`, `bypasses_shield` |
| `sweet_berry_bush` | `no_knockback`, `bypasses_shield` |
| `in_wall` | `bypasses_armor`, `no_knockback` |
| `out_of_world` | `bypasses_armor`, `bypasses_invulnerability`, `bypasses_resistance`, `no_knockback` |
| `outside_border` | `bypasses_armor`, `no_knockback` |

The tag `panic_environmental_causes` contains `cactus`, `freeze`, `hot_floor`, `sulfur_cube_hot`,
`in_fire`, `lava`, `lightning_bolt` and `on_fire`. The mob documents read it.
The armor reduction (DMG-RED-001) applies to `in_fire`, `lava`, `campfire`, `dry_out`, `cactus` and
`sweet_berry_bush`. It does not apply to the other types of the table.

**ENV-SRC-004** (`source_suggested`, semantic, confidence: high). The corpus has these entity type
tags that this document reads. A type outside the tag has no immunity that the tag gives.

| Tag | Members of this version | Rule |
|-----|-------------------------|------|
| `immune_to_freezing` | `stray`, `polar_bear`, `snow_golem`, `wither`, `frostbite`, `zombie`, `drowned`, `husk` | ENV-FRZ-004 |
| `freeze_hurts_extra_types` | `strider`, `blaze`, `magma_cube` | ENV-FRZ-003 |
| `can_breathe_under_water` | the tag `undead`, `axolotl`, `frog`, `guardian`, `elder_guardian`, `turtle`, `glow_squid`, `cod`, `pufferfish`, `salmon`, `squid`, `tropical_fish`, `tadpole`, `armor_stand`, `copper_golem`, `nautilus` | ENV-AIR-005 |
| `dismounts_underwater` | `camel`, `chicken`, `donkey`, `happy_ghast`, `horse`, `llama`, `mule`, `pig`, `ravager`, `spider`, `strider`, `trader_llama`, `zombie_horse` | ENV-AIR-007 |
| `powder_snow_walkable_mobs` | `rabbit`, `endermite`, `silverfish`, `fox`, `frostbite` | section 12 |

The tag `undead` contains the tags `skeletons` and `zombies`, `wither` and `phantom`. The item tag
`freeze_immune_wearables` contains `leather_boots`, `leather_leggings`, `leather_chestplate`,
`leather_helmet` and `leather_horse_armor`. The fire-immune property is not a tag. It is data of
the entity type. `entity-damage.md`, Appendix A lists it, and U2 of that document records the
gap.

## 2. Fire and lava

### 2.1 The state of fire

**ENV-FIR-001** (`source_suggested`, semantic, confidence: high). A living entity has fire ticks.
A new entity has 0. The save keeps them in the short `Fire`, and a load reads that value with no
change. A negative value is valid. The player keeps the value `-20` in a save of a player that is
not burning. The entity data entry 0 (serializer 0, byte) carries the flag `0x01`, the burning
flag. The flag follows the fire ticks in the base entity tick, as ENV-FIR-002 states. A
fire-immune entity never has the flag set and never has fire ticks more than 0 after its next base
entity tick.

**ENV-FIR-002** (`source_suggested`, semantic, confidence: high). In each base entity tick of an
entity, in this order:

1. If the fire ticks are more than 0 and the entity is fire-immune, set them to 0.
2. Else, if the fire ticks are more than 0:
   - when the fire ticks are a multiple of 20 and the entity is not in lava, do a hit. The hit has
     the type `on_fire` and the amount 1.0;
   - then decrease the fire ticks by 1.
3. Set the burning flag to true when the fire ticks are more than 0. Else set it to false.

The hit of step 2 uses the fire ticks before the decrease. Thus an entity can ignite for 160
ticks in the block effects step of tick `T`. It takes a hit in the ticks `T + 1`, `T + 21` and so on.
It takes eight hits in all. It burns out after 160 base entity ticks. A burning entity in lava
takes no `on_fire` hit, but it takes the hit of ENV-FIR-006. The tick of step 3 sees the fire ticks
that step 2 left. Thus a client sees the flag one tick after an ignition.

**ENV-FIR-003** (`source_suggested`, semantic, confidence: high). To ignite an entity for `n`
ticks:

1. If the fire ticks are less than `n`, set them to `n`.
2. Set the frozen ticks to 0 (ENV-FRZ-006).

Step 2 occurs also when step 1 changes nothing. A fire-immune entity is not ignited by ENV-FIR-004
and ENV-FIR-006. Ignite for `s` seconds means ignite for `floor(s × 20)` ticks.

**ENV-FIR-005** (`source_suggested`, semantic, confidence: medium). The block effects step ends
with this test. If the entity is not burning, and no ignition raised the fire ticks in this step,
the step sets the fire ticks to `-g`. Here `g` is the grace of the entity type: 20 for a player and
0 for each other type. The test does not run for an entity that blocks cannot affect. This is a removed entity or an entity with no physics, for example a spectator.

The result is visible for a player only. The fire block of ENV-FIR-004 gives a player in its
cell one fire tick for each tick. A player ignites when the fire ticks reach 0, that is, in the
20th tick of an unbroken stay in a fire cell. A short walk through fire damages the player (the
hit of ENV-FIR-004) and does not ignite it. Each other entity ignites in the first tick of
contact.

### 2.2 Fire blocks, lava and the contact order

**ENV-FIR-004** (`source_suggested`, semantic, confidence: high). The inside action of `fire` and
`soul_fire` acts on each entity that passes the contact test. It does these effects, in this
order:

1. Set the frozen ticks to 0.
2. Ignite the entity, unless it is fire-immune:
   - if the fire ticks are less than 0, increase them by 1;
   - else, if the entity is a player, increase them by a random integer from 1 to 2;
   - then, if the fire ticks are now 0 or more, ignite the entity for 8 seconds.
3. Do a hit of the type `in_fire` with the amount 1.0 for `fire` and 2.0 for `soul_fire`. A
   fire-immune entity does this step too, and the hit is rejected (DMG-HIT-001 step 3).

The ignition of step 2 sets the fire ticks to at least 160. Thus the random increase is visible
only when the fire ticks were 160 or more before.

**ENV-FIR-006** (`source_suggested`, semantic, confidence: high). The inside action of lava and of
`lava_cauldron` acts on each entity that has fluid contact with lava. It also acts on each entity that passes the
contact test for `lava_cauldron`. It does these effects, in this order:

1. Set the frozen ticks to 0.
2. Ignite the entity for 15 seconds, unless it is fire-immune.
3. Unless the entity is fire-immune, do a hit of the type `lava` with the amount 4.0.

A hit that the entity takes (not a rejected hit) plays the sound of ENV-NET-002. A lava contact
ignites the entity at once for each entity type, with no grace.

**ENV-FIR-010** (`source_suggested`, semantic, confidence: medium). In the block effects step,
the effects of the blocks run in this fixed order of types. The hit of a type runs right after its
effect, and a removed or dead entity stops the sequence.

1. The effect "freeze" of powder snow (ENV-FRZ-002).
2. The effect "clear the frozen ticks" of fire, lava and `lava_cauldron`.
3. The ignition of `fire` and `soul_fire`, then its `in_fire` hit.
4. The ignition of lava, then its `lava` hit.
5. The extinguish of ENV-FIR-007 step 1.

The order is visible. An entity that stands in fire and in water in one tick takes the `in_fire`
hit and is extinguished in that tick. Contact blocks of other kinds (section 5) hit at the point
where their block comes in the walk of the box. Section 12 records the walk as U2.

### 2.3 Extinguishing

**ENV-FIR-007** (`source_suggested`, semantic, confidence: medium). The fire ticks change to the
smaller of 0 and the fire ticks (the "clear") in these cases:

1. The entity has fluid contact with water, or it passes the contact test for `water_cauldron`,
   `powder_snow_cauldron` or `powder_snow`. The clear runs in step 5 of ENV-FIR-010.
2. After the block effects of this tick, the entity is in rain. Rain is at the block of its
   position. Or rain is at the block that has the x and z of that block and the y of the top of
   its box.

Rain at a block means all these conditions:

- the dimension has weather, and its rain level is more than 0.2 (`command-weather.md`);
- the block can see the sky;
- no block with the property "blocks motion" is above the block;
- the biome gives rain, and not snow, at that position.

A negative value stays unchanged, so the clear does not remove the grace of ENV-FIR-005. When the
clear of step 1 or 2 changes a burning entity to not burning, the server plays the sound of
ENV-NET-002. A `water_cauldron` or a `powder_snow_cauldron` loses one level when a burning entity passes
the contact test. At level 0 it becomes `cauldron`. A `powder_snow_cauldron` becomes a
`water_cauldron` with one level less.

**ENV-FIR-009** (`source_suggested`, semantic, confidence: medium). A burning entity that passes
the contact test for `powder_snow` destroys that block, with no drops, before the clear of
ENV-FIR-007. This happens when the game rule `mob_griefing` is true or the entity is a player,
and the entity can change that position. U8 records the rule of spawn protection for a player.

### 2.4 Fire resistance and immunities

**ENV-FIR-008** (`source_suggested`, semantic, confidence: high). Fire resistance and fire
immunity act at the hit. They do not act at the ignition.

- A fire-immune type is never burning (ENV-FIR-001, ENV-FIR-002). Its box can touch fire and lava
  with no ignition. A hit with the tag `is_fire` is rejected (DMG-HIT-001 step 3).
- An entity with the effect `fire_resistance` is not fire-immune. It ignites, burns, shows the
  burning flag and can be extinguished. Each hit with the tag `is_fire` is rejected
  (DMG-HIT-001 step 7). Thus it takes no `on_fire`, `in_fire`, `lava` or `campfire` damage.
- A player takes no hit with the tag `is_fire` when the game rule `fire_damage` is false
  (DMG-PLR-001). The player still burns.

## 3. The air supply and drowning

**ENV-AIR-001** (`source_suggested`, wire, confidence: high). A living entity has an air supply
with a maximum. The air supply is the entity data entry 1 (serializer 1, VarInt). The maximum is
300 for each type except these:

| Type | Maximum |
|------|---------|
| `dolphin` | 4800 |
| `axolotl` | 6000 |

A new entity has the maximum. A save keeps the value in `Air`, and a load without `Air` gives the
maximum. The server sends the changed value in `set_entity_data` (DMG-NET-002 states the
mechanism). A change of the value in a tick is therefore visible to the client in that tick. The
value that a client sees is the value after the base entity tick. A value below 0 is valid. It is
in the range from -19 to 0 while an entity drowns.

**ENV-AIR-002** (`source_suggested`, semantic, confidence: high). ENV-AIR-005 states whether a living entity that is alive drowns. The decision applies in the base
entity tick. It applies when the eye is in water and the block at the eye position is not
`bubble_column`.

- When it can drown: the air supply decreases by 1 (ENV-AIR-003 gives the damage).
- When it cannot drown, the air supply is less than the maximum and effects allow the refill:
  the air supply increases by 4 (ENV-AIR-004).

In each other case, the air supply increases by 4 up to the maximum when it is less than the
maximum.

**ENV-AIR-003** (`source_suggested`, semantic, confidence: high). After the decrease of
ENV-AIR-002, when the air supply is -20 or less, the server does these steps:

1. Set the air supply to 0.
2. Send the entity event 67 to the tracking players, and to the entity when it is a player
   (ENV-NET-001).
3. Do a hit of the type `drown` with the amount 2.0.

Thus an entity with the maximum 300 that goes under water takes its first hit in the 320th tick
under water. It takes one hit in each 20 ticks after that, while it stays. The cooldown does not
change this rate. The air supply that a client sees counts down from 300 to -20 in that time
and then shows 0.

**ENV-AIR-004** (`source_suggested`, semantic, confidence: high). The increase is the smaller of
the air supply plus 4 and the maximum. Thus an entity that surfaces from 0 refills in 75 ticks
for the maximum 300. The type `dolphin` is an exception: it refills to its maximum in one tick.

**ENV-AIR-005** (`source_suggested`, semantic, confidence: high). A living entity cannot drown in
water in these cases:

- its type is in the tag `can_breathe_under_water`;
- it has one of the effects `water_breathing`, `conduit_power` or `breath_of_the_nautilus`;
- it is a player with the invulnerable ability (creative mode and spectator mode).

The refill of ENV-AIR-002 does not run for an entity with the effect `breath_of_the_nautilus` and
with neither `water_breathing` nor `conduit_power`. The type `iron_golem` never decreases its air
supply, so it never drowns. The attribute `oxygen_bonus` (default 0) with a value `b` more than 0
makes the decrease of ENV-AIR-002 occur with the probability `1 / (b + 1)` in each tick.

The game rule `drowning_damage` set to false makes DMG-PLR-001 reject the hit of a player. The
air supply of the player still changes, and the client still sees it.

**ENV-AIR-006** (`source_suggested`, semantic, confidence: medium). Some types have an own air
rule. It applies in the same tick as the base entity tick, after the rules above. It replaces
the value of the air supply in that tick.

| Types | In their fluid | Not in their fluid |
|-------|----------------|--------------------|
| `cod`, `salmon`, `pufferfish`, `tropical_fish`, `tadpole`, `squid`, `glow_squid` | the air supply is 300 | the air supply decreases by 1 from its value before the tick |
| `nautilus` | the air supply is 300 | the same decrease |
| `axolotl` | the air supply is the maximum | the same decrease |

"In their fluid" is "in water" for the fish, the squid, the tadpole and the nautilus. For the
axolotl it is "in water or in rain". When the air supply reaches -20 or less after the decrease,
the server sets it to 0 and does a hit with the amount 2.0. The hit has the type `drown` for the
fish, the squid, the tadpole, and the damage type `dry_out` for the nautilus and the axolotl. The
rule does not apply to a dead entity. It does not apply to an axolotl or a nautilus with no AI.
The hit is the same, so a dry fish dies in 320 ticks and takes one hit in each 20 ticks after
that. The axolotl takes its first hit in the 6020th tick out of water. The dolphin uses ENV-AIR-002 and
ENV-AIR-003 with its maximum of 4800, and its refill is ENV-AIR-004.

**ENV-AIR-007** (`source_suggested`, semantic, confidence: medium). In the same tick as
ENV-AIR-002, an entity dismounts when two conditions are true. Its eye is in water and the block
at the eye is not `bubble_column`. It rides a vehicle with a type in the tag
`dismounts_underwater`. This happens
whether the entity drowns or not.

## 4. Freezing in powder snow

**ENV-FRZ-001** (`source_suggested`, wire, confidence: high). An entity has frozen ticks. The
entity data entry 7 (serializer 1, VarInt) carries them, and the save keeps them in the int
`TicksFrozen` when they are more than 0. The value that makes the entity fully frozen is 140.
The client shows the freeze overlay in proportion to `min(frozen, 140) / 140`.

**ENV-FRZ-002** (`source_suggested`, semantic, confidence: high). The entity "can freeze" when
all these are true:

- its type is not in the tag `immune_to_freezing`;
- it is not burning;
- it is not a spectator;
- no armor slot holds an item of the item tag `freeze_immune_wearables`.

In the block effects step, the inside action of `powder_snow` marks the entity "in powder snow"
for this tick. When the entity can freeze, the action also adds 1 to its frozen ticks, up to 140.
The mark is false at the start of each base entity tick. Then the freezing step runs after the
block effects of a living entity on the server. In this step the frozen ticks decrease by 2, to a
minimum of 0, when the entity cannot freeze. They decrease in the same way when the entity is not
marked and has no effect `freezing`. Thus an entity that stays in
powder snow needs 140 ticks to freeze fully and 70 ticks to thaw fully.

**ENV-FRZ-003** (`source_suggested`, semantic, confidence: medium). The server does a hit of the type `freeze`
in the freezing step. It does the hit when all these conditions are true:

- the age of the entity is a multiple of 40;
- the frozen ticks are 140 or more;
- the entity can freeze. The amount is 1.0, or 1.5 when the entity has
the effect `freezing`. The amount changes before the cooldown: it is multiplied by 5 when the
type of the entity is in the tag `freeze_hurts_extra_types`. The phase of the 40 ticks starts at
the age 0 of each load, so it is not saved.

**ENV-FRZ-004** (`source_suggested`, semantic, confidence: high). A player takes no `freeze` hit
when the game rule `freeze_damage` is false (DMG-PLR-001). The frozen ticks still change.

**ENV-FRZ-005** (`source_suggested`, client agreement, confidence: medium). A living entity with
the attribute `movement_speed` has a transient modifier of the id `minecraft:powder_snow` while
its frozen ticks are more than 0. The operation is `add_value`. The amount is
`-0.05F × (f32(min(frozen, 140)) / 140F)`. Each operation is an f32 operation: the quotient
first, then the product. The amount is that f32, widened to f64. For example, 139 frozen ticks
give the bits `bfa96ac9e0000000`. The server removes the modifier when the frozen ticks are 0.
The server sends the change in the packet that carries the attributes. The client uses it to
predict the movement of the player.

**ENV-FRZ-006** (`source_suggested`, semantic, confidence: high). These events set the frozen
ticks to 0: ignition (ENV-FIR-003), the contact of `fire`, `soul_fire`, lava and `lava_cauldron`
(ENV-FIR-010 step 2).

## 5. Contact damage of blocks

**ENV-CTC-001** (`source_suggested`, semantic, confidence: high). `cactus` acts on each entity
that passes the contact test for its cell. The cell is the whole cell, not the smaller shape of
the cactus. An entity that stands next to a cactus side touches the cell, so it takes the hit.
The hit has the type `cactus` and the amount 1.0. The action does nothing else.

**ENV-CTC-002** (`source_suggested`, semantic, confidence: high). `campfire` and `soul_campfire`
act on a living entity that passes the contact test for their cell, when the block state has
`lit` true. The hit has the type `campfire` and the amount 1.0 for `campfire` and 2.0 for
`soul_campfire`. The campfire does not ignite the entity.

**ENV-CTC-003** (`source_suggested`, semantic, confidence: high). `sweet_berry_bush` acts on a
living entity that passes the contact test, and that is not a `fox` or a `bee`. The block sets the
stuck multiplier of PHY-GRV-014. Then it does a hit of the type `sweet_berry_bush` with the amount
1.0, when both conditions are true:

- the property `age` of the block is not 0;
- in the last tick the entity moved by 0.003 or more along the x axis or the z axis.

An entity that does not move in a bush takes no hit. A bush with `age` 0 never hits.

**ENV-CTC-004** (`source_suggested`, semantic, confidence: high). These contact damages exist in
other documents or do not apply:

- `magma_block`: DMG-HOT-001;
- a fall onto a pointed tip: DMG-FALL-001 with the type `stalagmite`;
- `fire`, `soul_fire`, lava and `lava_cauldron`: ENV-FIR-004 and ENV-FIR-006;
- `wither_rose` gives an effect and no damage, and section 12 lists it.

## 6. Suffocation inside a block

**ENV-WAL-001** (`source_suggested`, semantic, confidence: medium). In the base entity tick of a
living entity that is alive, the entity is "inside a wall" when all these are true:

- it does not sleep and it has physics (a spectator has none);
- let `w = 0.8 × width` as f32, where `width` is the width of its box. Let `E` be the box of the
  width `w` and `w`, of the height 1.0E-6, at the eye position. A block cell that `E` touches
  has a block state that causes suffocation (PHY-INP-021);
- the collision shape of that state, moved to the cell, overlaps `E` with a volume more than 0.

The result is one hit of the type `in_wall` with the amount 1.0 in each such tick. When the
hit is accepted, the cooldown makes one full hit in each 10 ticks. A player that is inside a wall
takes no border damage (ENV-BRD-001) in that tick.

## 7. The void

**ENV-VOID-001** (`source_suggested`, semantic, confidence: high). One of these happens in the base entity tick of an
entity. It happens when the y of its position is less than the min height minus 64:

- A living entity does a hit of the type `out_of_world` with the amount 4.0. The type has the
  tags `bypasses_invulnerability` and `bypasses_resistance`. Thus a player in creative mode takes
  the hit, and the effect `resistance` does not reduce it. The cooldown allows one full hit in
  each 10 ticks, so the entity loses 4 health in each 10 ticks.
- Each other entity is removed. This includes an item and an arrow.

The min height of the overworld is -64, so the void begins at y below -128. The nether and the end
have `min_y` 0, so the void begins at y below -64. The test uses the position of the entity, not
its box.

## 8. The world border

**ENV-BRD-001** (`source_suggested`, semantic, confidence: high). The border damages only a
player. In the base entity tick of a player that is alive, and that is not inside a wall
(ENV-WAL-001):

1. If the box of the player is inside the border, stop. The box is inside when two conditions are true. Its minimum x and z
   are not less than the west and north limits. Its maximum x and z, less `1.0E-5F`, are less
   than the east and south limits.
2. Let `d` be the smallest of the four distances from the x and z of the position to the limits:
   `x - west`, `east - x`, `z - north`, `south - z`. The value `d` is less than 0 outside the
   border. Let `b = d + buffer`, in f64.
3. If `b` is less than 0 and the damage per block is more than 0, do a hit of the type
   `outside_border`. The amount is `max(1, floor(-b × damage per block))`, as an integer, converted
   to f32.

Thus a player 5 blocks outside the limit, with the default buffer 5.0, takes no damage. A player
15 blocks outside takes `floor(10 × 0.2) = 2` health in each hit. The hit is accepted by the
cooldown DMG-HIT-003 in each 10 ticks, or earlier when the amount rises. A mob takes no border
damage.

**ENV-BRD-002** (`source_suggested`, semantic, confidence: high). The buffer and the damage per
block belong to the border of each dimension. Their defaults are the buffer 5.0 and the damage
per block 0.2. The save of the border keeps them as `safe_zone` and `damage_per_block`. The
commands `worldborder damage buffer` and `worldborder damage amount` set them. The amount of the
command is an f32 argument with a minimum of 0.

## 9. Order and what a client sees

**ENV-ORD-001** (`source_suggested`, semantic, confidence: medium). For a living entity that is
not a player, the damage of this document occurs at the steps of DMG-ORD-001:

1. Step 2, the base entity tick, in this order:
   1. the fluid contact update;
   2. the burning hit and the fire ticks (ENV-FIR-002);
   3. the void (ENV-VOID-001);
   4. the burning flag;
   5. the wall hit (ENV-WAL-001), or the border hit for a player (ENV-BRD-001);
   6. the air supply and the drowning hit (section 3);
   7. the decreases of DMG-HIT-006.
2. Step 8, the block effects step (ENV-FIR-010, section 5), then the rain clear of ENV-FIR-007,
   then ENV-FIR-005.
3. After step 8, the freezing step (ENV-FRZ-002, ENV-FRZ-003).

A hit of step 1 occurs before the cooldown decrease of the same tick. A player runs these steps in
its own tick in the same order, and its cooldown decrease is in its server part (DMG-PLR-002).
The order decides the damage type that a client sees when two hits meet in one cooldown
(DMG-ORD-003).

**ENV-NET-001** (`source_suggested`, wire, confidence: medium). The packets of this document are:

| What | Packet | Content |
|------|--------|---------|
| a full hit | `damage_event` (DMG-NET-001) | the network id of the damage type of the source |
| the burning flag, the air supply, the frozen ticks | `set_entity_data` (DMG-NET-002) | entries 0, 1 and 7 |
| drowning, ENV-AIR-003 step 2 | `entity_event` (DMG-NET-003) | the event 67, which starts the bubble particles on the client |
| the speed modifier | the attribute packet | ENV-FRZ-005 |

The damage event goes to the tracking players and to the entity when it is a player. The event 67
goes to the same viewers. A partial hit and a rejected hit send no `damage_event`.

**ENV-NET-002** (`source_suggested`, semantic, confidence: low). The server plays two sounds as
`sound` packets at the position of the entity, with the source of the entity type.

| Sound | When | Volume | Pitch |
|-------|------|--------|-------|
| `entity.generic.burn` | a hit of the type `lava` is accepted, and the entity is not silent | 0.4 | `2.0 + r × 0.4`, `r` random in `[0, 1)` |
| `entity.generic.extinguish_fire` | a burning entity or a freezing entity stops (ENV-FIR-007), and no such sound played in the last 20 ticks | 0.7 | `1.6 + (r1 - r2) × 0.4` |

The pitch is random, so a test compares only the event, the source and the position. A freezing
entity stops when the effects of the block effects step change its frozen ticks from more than 0
to 0.

## 10. Out of scope

| Subject | Reason |
|---------|--------|
| a hit on a non-living entity: an item that burns, a boat in lava | `item-entity.md` and the vehicle documents |
| the sources of ignition from a mob, a projectile, an enchantment, a lightning bolt and the sun | each belongs to its document, and ENV-FIR-003 states the operation |
| the effects as a system: `fire_resistance`, `water_breathing`, `conduit_power`, `breath_of_the_nautilus`, `freezing`, `resistance` | each rule reads only the existence of the effect |
| the attribute modifiers of enchantments: `oxygen_bonus` from respiration, protection against fire | items are out of scope (DMG-RED-003) |
| the effect `wither` of `wither_rose`, the poison, the starvation | not an environmental hit of this document |
| the world border size, its movement, its warning screen and its collision | not damage |
| the death of a player and its message | `entity-damage.md`, section 12 |
| the swim state, the current and the buoyancy of fluids | the fluid model of the physics |
| the collision of `powder_snow` and the types in `powder_snow_walkable_mobs` | physics, not damage |
| the particles of fire, bubbles and snow | no observer in a game rule |

## 11. Intentional deviations

None. Each reference behaviour that this document states is behaviour that a player can meet in
normal play, and the document keeps it.

## 12. Unresolved

- **U1.** The fluid model of the physics does not exist as a specification. The tests "fluid
  contact" and "eye in water" need the height of the fluid in a cell and the box of the entity. A
  later document must state them. Until then an implementation can use the box of the entity for
  the first test and the eye position for the second.
- **U2.** The block effects of one tick can come in several steps along the path of a long move,
  for example a fast fall. ENV-FIR-010 states the order inside one step. The order between two steps,
  and the number of steps in one tick, are not specified. PHY-GRV-014 has the same open point as
  its U7.
- **U3.** PHY-INP-021 defines the blocks that cause suffocation for the client push-out. ENV-WAL-001
  uses the same definition for the server hit. Experiment E7 must confirm that both agree.
- **U4.** The set of the types that the air rule of ENV-AIR-006 covers comes from the reference.
  No artefact lists it. The mob documents can extend it.
- **U5.** The maximum air supply of a type is data of the entity type. No artefact gives it. The
  table of ENV-AIR-001 comes from the reference and covers the types in the corpus.
- **U6.** The fire-immune property has no artefact (U2 of `entity-damage.md`). ENV-FIR-008 uses
  its Appendix A.
- **U7.** The age that sets the phase of ENV-FRZ-003 resets at a load. It is not known whether a
  player, a respawn or a change of dimension also resets it.
- **U8.** The rule of spawn protection can stop a player from clearing `powder_snow` in
  ENV-FIR-009. No specification states that rule.

## 13. Experiments

None of these experiments has run. Each runs on a vanilla dedicated server of `26.4-snapshot-7`
through its console, as the experiments of `entity-damage.md`, section 15. The world is flat, with
`difficulty hard`, `spawn_mobs false` and `tick freeze` with `tick step 1`.

| Id | Steps | Rules |
|----|-------|-------|
| E1 | Put a pig in a cell of `fire`, and another in a cell of `soul_fire`. Step 200 ticks. Read `Health`, `Fire` and `HurtTime` after each tick. | ENV-FIR-002, ENV-FIR-004, ENV-FIR-010 |
| E2 | Join with an offline client. Walk the player through a `fire` cell for 5 ticks, then stay in it for 40 ticks. Record `Fire` and the clientbound entity data. | ENV-FIR-004, ENV-FIR-005 |
| E3 | Put a zombified piglin, a pig with `fire_resistance` and a pig in lava. Read `Fire` and `Health` for 100 ticks. Record the sound packets. | ENV-FIR-006, ENV-FIR-008, ENV-NET-002 |
| E4 | Burn a pig for 160 ticks on land. Read `Fire` in each tick. Repeat with water, with a `water_cauldron`, with rain, and with `powder_snow` with `mob_griefing` true and false. | ENV-FIR-002, ENV-FIR-007, ENV-FIR-009 |
| E5 | Put a pig, a cod, a dolphin, an axolotl and a player in a water column. Read `Air` in each tick for 400 ticks. Repeat out of water. Record entity data and the packets with the event 67. | ENV-AIR-001 to ENV-AIR-006 |
| E6 | Put a pig, a `husk`, a `blaze` and a player with `leather_boots` in powder snow for 300 ticks. Read `TicksFrozen`, `Health` and the attribute `movement_speed` in each tick. | ENV-FRZ-001 to ENV-FRZ-005 |
| E7 | Put a pig in a `stone` block with `/data merge entity`. Read `Health` for 40 ticks. Put a player in the same block and record the packets. | ENV-WAL-001, U3 |
| E8 | Put a pig, an item and a player in creative mode at y -140 in the overworld, and at y -70 in the nether. Read `Health` for 40 ticks. | ENV-VOID-001 |
| E9 | Set the border to 20 blocks. Set `damage buffer` to 0, 5 and 20, and `damage amount` to 0.2 and 2. Put a player and a pig at 3, 10 and 30 blocks outside the limit. Read `Health` for 60 ticks. | ENV-BRD-001, ENV-BRD-002 |
| E10 | Walk a pig, an idle pig and a fox through a `sweet_berry_bush` with `age` 0 and 3, and a pig next to a `cactus` side. Read `Health` in each tick. | ENV-CTC-001, ENV-CTC-003 |
