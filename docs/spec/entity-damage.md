# Entity damage: health, damage and death

Version: see `assets/minecraft/version.json`.

This document states the health of a living entity, the damage that it takes, and its death. It
states one procedure for each hit, the reductions of the damage, the cooldown after a hit, and the
death state. It states three sources of damage completely: a fall, a step on a magma block and a
crowd. Other sources use the same procedure with their own damage type. The procedure is the same
for a player and for a mob. Section 10 states the rules that apply only to a player.

The rules come from the reference implementation. Experiments on a vanilla server confirm the
numeric rules and the packets (section 15).

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **entity**, **living entity**, **server-simulated entity**,
  **fall distance**, **effect position**, **step action**, **fall action**, **crowd damage**:
  `entity-physics.md`, section 0.1, defines these terms.
- **Base entity tick**, **block effects step**, **shift key state**, **f32**, **f64**, **widen**,
  **corpus**, **data pack**: the same section defines these terms.
- **Order key**: the UUID of an entity, as PHY-CMP-013 defines it.
- **Writer key**: as PHY-CMP-017 defines it.
- **Pass**: the order-independent tick of the entities of `entity-physics.md`, section 11.4. Each
  entity reads the other entities at their state from the start of the tick.
- **Sorted queue**, **apply step**: `entity-physics.md`, section 0.1, defines these terms. In this
  document, the entries of the sorted queue are hits.
- **Health**: an f32 value of a living entity, from 0 to the max health.
- **Max health**: the value of the attribute `max_health` of the entity, converted to f32.
- **Damage type**: an entry of the data-pack registry `minecraft:damage_type`.
- **Damage tag**: a tag of the registry `minecraft:damage_type`, for example
  `#minecraft:bypasses_armor`. A damage type "has" a tag when the tag contains the type.
- **Damage source**: the cause of one hit. Section 2 states its parts.
- **Causing entity**, **direct entity**, **source position**: the three optional parts of a damage
  source (DMG-SRC-001).
- **Hit**: one call of the hit procedure of section 3, with a damage source and an amount.
- **Amount**: the f32 damage that a hit asks for, before the cooldown and the reductions.
- **Damage cooldown**: an integer counter of a living entity (DMG-HIT-003).
- **Last amount**: the f32 amount of the last hit that the cooldown accepted.
- **Full hit**: a hit that the cooldown accepts with the whole amount (DMG-HIT-003).
- **Partial hit**: a hit inside the cooldown that applies only the part of the amount above the
  last amount (DMG-HIT-003).
- **Rejected hit**: a hit that changes no health, for example a hit on an immune entity.
- **Applied damage**: the damage after the reductions of section 4, before absorption.
- **Hurt time**: an integer counter of a living entity (DMG-HIT-003). The data name is `HurtTime`.
- **Absorption**: an f32 value of a living entity (DMG-RED-004). The data name is
  `AbsorptionAmount`.
- **Armor value**: the value of the attribute `armor`, rounded down to an integer.
- **Armor toughness**: the value of the attribute `armor_toughness`, converted to f32.
- **Fire-immune**: a property of an entity type.
- **Dying state**: the state of a living entity whose health is 0 or that died (section 8).
- **Death time**: an integer counter of an entity in the dying state. The data name is
  `DeathTime`.
- **Alive**: a living entity is alive when it is not removed and its health is more than 0.
- **Idle time**: as `entity-ai-timing.md`, section 0.1, defines it.
- **Crowd**: a set of living entities that the count of PHY-PSH-007 joins. Each entity of the set
  is a candidate of another entity of the set.
- **Fall multiplier**: the f32 factor that the fall action of a block gives to the fall damage
  (DMG-FALL-004).
- **Tracking players**: the players to which the server sends the updates of an entity, as the
  replication of the entity states them.

### 0.2 Provenance

The labels are those of `entity-physics.md`, section 0.2.
Each rule states its kind and its confidence, for example
`(black_box_confirmed, semantic, confidence: high)`. The kinds are those of `docs/spec/README.md`.
A new or revised rule states them on its provenance line, as `docs/spec/README.md` requires.

### 0.3 ID prefix

Each rule has an ID with the prefix `DMG`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `DMG-ATT` | health and the attributes of damage |
| `DMG-SRC` | the damage source |
| `DMG-HIT` | the hit procedure and the cooldown |
| `DMG-RED` | the reductions of the damage |
| `DMG-FALL` | fall damage |
| `DMG-HOT` | the damage of a magma block |
| `DMG-CRW` | crowd damage |
| `DMG-DTH` | death |
| `DMG-NET` | the packets that a client sees |
| `DMG-PLR` | rules for a player only |
| `DMG-ORD` | the place of damage in a tick |
| `DMG-DEV` | intentional deviations |

### 0.4 Scope

This document states:

- the health, the max health and the attributes that damage reads;
- the damage source, and the procedure of one hit with its cooldown;
- the reductions by armor, by the effect `resistance` and by absorption;
- the damage types `fall`, `hot_floor` and `cramming`, with their sources;
- the death state, the death time, the removal and the death event;
- the packets of damage, health, death and removal;
- the place of each hit in the tick, in vanilla and in the order-independent pass.

Section 12 lists the subjects that are out of scope, with the reason for each.

### 0.5 Artefacts cited

- The corpus: the damage types `assets/minecraft/damage_type/<type>.json`, the damage tags
  `assets/minecraft/tags/damage_type/<tag>.json` and the entity type tag
  `assets/minecraft/tags/entity_type/fall_damage_immune.json`. Section 0.6 lists the members that
  this document uses.
- `assets/mcrs/reports/packets.json`: the packet ids of section 9.
- The game rules `max_entity_cramming` (default 24), `fall_damage` and `fire_damage`.

### 0.6 Damage tags of the three types

The corpus gives these values. The list holds only the tags that a rule of this document reads.

| Damage type | Tags |
|-------------|------|
| `fall` | `bypasses_armor`, `is_fall`, `no_knockback` |
| `hot_floor` | `is_fire`, `no_knockback` |
| `cramming` | `bypasses_armor`, `no_knockback` |
| `player_attack` | `is_player_attack` |

The tag `bypasses_cooldown` is empty in the corpus. The tag `no_impact` contains only `drown`.

## 1. Health and attributes

**DMG-ATT-001** (`black_box_confirmed`, semantic, confidence: high). A living entity has a health.
A new entity starts with its max health. A saved entity keeps its health in `Health`. Each change
of the health clamps the new value to the range from 0 to the max health. When the max health
becomes less than the health, the health changes to the max health.

**DMG-ATT-002** (`black_box_confirmed`, semantic, confidence: high). Damage reads these attributes:

| Attribute | Use |
|-----------|-----|
| `max_health` | the upper limit of the health |
| `armor` | the armor reduction (DMG-RED-001) |
| `armor_toughness` | the armor reduction (DMG-RED-001) |
| `max_absorption` | the upper limit of the absorption (DMG-RED-004) |
| `safe_fall_distance` | the fall damage (DMG-FALL-002) |
| `fall_damage_multiplier` | the fall damage (DMG-FALL-002) |

The base value of each attribute is data of the entity type. ATR-BASE-003 of `attributes.md` and
own that table. ATR-DEF-001 of that document gives the default when a type has no
own value. Modifiers of items and effects change the value, as ATR-VAL-001 of `attributes.md`
states. A mob with no modifier has the base value.

**DMG-ATT-003** (`black_box_confirmed`, semantic, confidence: high). Two properties of the entity
type make it immune to some damage types (DMG-HIT-001). They are the entity type tag
`fall_damage_immune` and the fire-immune property.

The zombified piglin is fire-immune and is not in `fall_damage_immune`. Thus magma does not hurt
it, and a fall does.

## 2. The damage source

**DMG-SRC-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). A damage
source has four parts:

1. the damage type;
2. the causing entity, or none: the entity that is responsible, for example the player that
   attacks;
3. the direct entity, or none: the entity that touches the target, for example an arrow;
4. the source position, or none: a position that the hit comes from, when no entity gives it.

A test of a damage tag on a source is the test of that tag on the damage type of the source.

**DMG-SRC-002** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The sources
of this document have no causing entity, no direct entity and no source position:

| Cause | Damage type | Rule |
|-------|-------------|------|
| a fall action | `fall`, or `stalagmite` on a pointed tip | DMG-FALL-001 |
| the step action of `magma_block` | `hot_floor` | DMG-HOT-001 |
| a crowd | `cramming` | DMG-CRW-001 |

A player attack has the type `player_attack`, with the player as the causing entity and as the
direct entity. `player-attack.md` states that source. It uses the procedure of section 3 with no
change. Section 3 marks the steps that read the causing entity.

## 3. The hit procedure

**DMG-HIT-001** A hit on a living entity first does these tests, in this order. When a test is
true, the hit is rejected and the procedure stops.

1. The entity is removed.
2. All these conditions are true:
   - the entity is invulnerable;
   - the damage type does not have the tag `bypasses_invulnerability`;
   - the causing entity is not a player in creative mode.

   Hook: the saved flag `Invulnerable` and the saved counter `invulnerable_time` make an entity
   invulnerable. Thus a player in creative mode can hit an entity with the flag `Invulnerable`.
3. The damage type has the tag `is_fire` and the entity type is fire-immune.
4. The damage type has the tag `is_fall` and the entity type is in `fall_damage_immune`.
5. For a player, a test of DMG-PLR-001 is true. Two of these tests read the causing entity: the
   game rule `pvp` and the friendly fire of a team (`player-attack.md`, ATK-TGT-004).
6. The entity is in the dying state (section 8).
7. The damage type has the tag `is_fire` and the entity has the effect `fire_resistance`.

Provenance: `source_suggested_black_box_confirmed`; step 5 as DMG-PLR-001. Kind: semantic.
Confidence: high; step 5 as DMG-PLR-001. Experiment for step 2: ATK-E4 of `player-attack.md`;
`tools/mcrs_minecraft_probe/traces/player_attack.json`.

**DMG-HIT-002** (`source_suggested_black_box_confirmed`, semantic, confidence: high). After the
tests of DMG-HIT-001, the hit does these steps before the cooldown:

1. Hook: set the idle time of the entity to 0 (AIT-RFX-006). This step occurs before the
   cooldown, so also a hit that the cooldown rejects sets it.
2. If the amount is less than 0, set it to 0.
3. If the amount is NaN or infinite, set it to the largest finite f32 value.

The reference also changes the amount here for items and for freeze damage. Section 12 lists these
steps.

**DMG-HIT-003** (`black_box_confirmed`, semantic, confidence: high). The cooldown selects one of
three outcomes. Let `a` be the amount, `L` the last amount and `c` the damage cooldown.

1. If `c > 10` and the damage type does not have the tag `bypasses_cooldown`:
   - if `a ≤ L`, the hit is rejected;
   - else the hit is a partial hit. Apply `a - L` (DMG-HIT-004), then set `L = a`.
2. Else the hit is a full hit. Set `L = a`, set `c = 20`, apply `a` (DMG-HIT-004), and set the hurt
   time to 10.

The reductions of section 4 apply to `a - L` in a partial hit, not to `a`. A partial hit does not
change the cooldown and does not change the hurt time.

**DMG-HIT-004** (`black_box_confirmed`, semantic, confidence: high). To apply a damage `x`:

1. Compute the applied damage `y` from `x` with the reductions of section 4.
2. Let `h = max(y - A, 0)`, with `A` the absorption. Set the absorption to `A - (y - h)`.
3. If `h ≠ 0`, set the health to `health - h`, with the clamp of DMG-ATT-001.

All values are f32. The health of the entity data is the result of these f32 operations, so a
client sees the same bits as the server (DMG-NET-002).

**DMG-HIT-005** (`black_box_confirmed`, semantic, confidence: high). After the cooldown, a hit that
is not rejected does these steps:

1. Hook: record the causing entity for the kill credit and for the anger of mobs. A source with
   no causing entity records nothing. `player-attack.md` and `mob-goals.md`
   state these records. The damage record that the panic reads is GOAL-PNC-001.
2. For a full hit:
   - send the damage event (DMG-NET-001);
   - if the damage type does not have the tag `no_impact`, mark the velocity of the entity for
     the next update (DMG-NET-004);
   - if the damage type does not have the tag `no_knockback`, apply the knockback of DMG-KB-001.
     The three types of this document have `no_knockback`.
3. If the health is 0, start the death (DMG-DTH-001). For a full hit, play the death sound
   (DMG-NET-007).
4. Else, for a full hit, play the hurt sound (DMG-NET-007), unless the damage type has the tag
   `no_hurt_sound`.
5. Hook: record the source as the last damage source of the entity.

A partial hit sends no damage event and plays no sound. Its health change reaches the client only
through the entity data.

**DMG-HIT-006** (`black_box_confirmed`, semantic, confidence: high). In each tick, the base entity
tick of a living entity decreases the hurt time by 1 when it is more than 0. Then it decreases the
damage cooldown by 1 when it is more than 0. A player does the second decrease in its server
part instead (DMG-PLR-002). Thus a full hit in tick `T` lets the next full hit occur in tick
`T + 10` at the earliest. The save keeps the hurt time. It does not keep the damage cooldown or the
last amount, so a loaded entity has a damage cooldown of 0.

**DMG-KB-001** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). The default
knockback applies in DMG-HIT-005 step 2. Let `x` and `z` be the horizontal components of the
direction, and let `power` be `0.4F`, widened to f64. This value is `0.4000000059604645`.

1. If the direct entity is a projectile, the direction is the horizontal velocity of the projectile
   with both components negated. The entity moves along the velocity of the projectile.
2. Else, if the damage source has a source position, the direction is the horizontal vector from
   the entity to the source position. The entity moves away from the source position. The source position of a
   damage source with no stored position is the position of its direct entity.
3. Else the direction is zero.

The procedure continues in this way:

1. Multiply `power` by `1 - k`, where `k` is the value of the attribute `knockback_resistance`.
   If `power` is 0 or less, stop.
2. While `x * x + z * z < 1.0E-5`, set `x` and `z` to `(u1 - u2) * 0.01`, with new uniform values
   `u1` and `u2` in [0, 1) for each of them.
3. Let `n` be the vector `(x, z)` with length 1, and `v` the velocity of the entity. Set the new
   velocity to `(v.x / 2 - n.x * power, vy, v.z / 2 - n.z * power)`. The new `vy` is
   `min(0.4, v.y / 2 + power)` when the on-ground flag of the entity is true. Else it is `v.y`,
   with no change. The limit 0.4 is the f64 value 0.4, not `0.4F`.
4. Mark the velocity of the entity for the next update (DMG-NET-004).

Step 3 reads the on-ground flag that the entity has at the hit. `entity-physics.md`, PHY-ORD-017,
states its value after a summon. These examples have `power = 0.4000000059604645` and the
direction `(0, -1)`:

| Velocity before the hit | On-ground flag | Velocity after the hit |
|-------------------------|----------------|------------------------|
| `(0, 0, 0)` | true | `(0, 0.4, 0.4000000059604645)` |
| `(0, -0.0784000015258789, 0)` | true | `(0, 0.36080000519752503, 0.4000000059604645)` |
| `(0, 0, 0)` | false | `(0, 0, 0.4000000059604645)` |

The extra knockback of a melee attack and of enchantments has its own rules. The knockback of an
explosion is in `explosions.md`, EXP-KNB-001. The damage types of explosions have the tag
`no_knockback`. The experiments ATK-E1, ATK-E3 and ATK-E9 of `player-attack.md` confirm step 3 for
both values of the on-ground flag. No experiment confirms the direction of a projectile or the
random direction of step 2.

## 4. Reductions

**DMG-RED-001** (`black_box_confirmed`, semantic, confidence: high). If the damage type does not
have the tag `bypasses_armor`, armor reduces the damage `x`. Let `v` be the armor value as f32 and
`t` the armor toughness. All operations are f32:

1. `k = 2 + t / 4`.
2. `r = clamp(v - x / k, v × 0.2, 20)`.
3. `x = x × (1 - r / 25)`.

The types `fall` and `cramming` have `bypasses_armor`. The type `hot_floor` does not. Thus a
zombie with armor 2 loses `0.94` health on magma: `r = clamp(2 - 0.5, 0.4, 20) = 1.5`.

**DMG-RED-002** (`black_box_confirmed`, semantic, confidence: high). The effect `resistance` with
the amplifier `p` reduces the damage `x` when all these conditions are true:

- the entity has the effect;
- the damage type does not have the tag `bypasses_effects`;
- the damage type does not have the tag `bypasses_resistance`.

The reduction is:

1. `n = 25 - (p + 1) × 5`, an integer.
2. `x = max((x × f32(n)) / 25, 0)`, in f32, in this order. `x × (n / 25)` gives other bits.

Thus the amplifier 4 and more gives 0.

**DMG-RED-003** (`source_suggested`, semantic, confidence: medium). The order of the reductions is
armor (DMG-RED-001), then `resistance` (DMG-RED-002), then the protection of enchantments. The
enchantments are items, so section 12 lists them. An entity without items has no protection.

**DMG-RED-004** (`black_box_confirmed`, semantic, confidence: high). Absorption applies after the
reductions (DMG-HIT-004 step 2). It applies to mobs and to players. The value stays in the range
from 0 to the value of `max_absorption`. The effect `absorption` raises `max_absorption` and sets
the absorption. `status-effects.md` states the effect (EFX-ACT-005, EFX-ACT-006).

## 5. Fall damage

**DMG-FALL-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The fall
action of PHY-FALL-001 step 4 gives the fall distance `d` (f64) to the block at the effect
position. The block gives a fall distance `d'`, a fall multiplier `m` and a damage type, from the
table of DMG-FALL-004. Then the entity takes the fall damage of DMG-FALL-002 with these values.

**DMG-FALL-002** (`black_box_confirmed`, semantic, confidence: high). The fall damage of a living
entity, for `d'`, `m` and a damage type:

1. If the entity type is in `fall_damage_immune`, stop. Thus an immune type also gets no fall
   sound.
2. `n = floor((((d' + 1.0E-6) - s) × m) × f)`, in f64, in this order. Here `s` is the value of
   `safe_fall_distance`, `f` is the value of `fall_damage_multiplier`, and `m` is widened. `n` is a
   32-bit signed integer: a value above its range becomes the largest value, and NaN becomes 0.
3. If `n > 0`, play the fall sounds of DMG-FALL-005, then do a hit with the amount `n` as f32.

The game rule `fall_damage` applies only to a player (DMG-PLR-001). A mob takes fall damage when
the game rule is false (experiment D2).

D1 gives the values for a zombified piglin. A fall of 78 blocks onto
`honey_block` gives 15 damage, so the piglin keeps 5 health.

**DMG-FALL-003** Hook. The fall action applies also to each passenger (PHY-RCH-002), with the
same `d'`, `m` and damage type. That hit changes another entity. Thus it is a write of PHY-CMP-015,
and the vehicle is the writer. It is not an own hit of DMG-ORD-002.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

**DMG-FALL-004** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The fall
action of each block. D1 and D12 confirm each row. The farm needs only the default row and
`honey_block`.

| Block | `d'` | `m` | Damage type | Other effect |
|-------|------|-----|-------------|--------------|
| each block not in this table | `d` | `1.0F` | `fall` | none |
| `honey_block` | `d` | `0.2F` | `fall` | entity event 54 and the sound `block.honey_block.slide` at each fall action, also with no damage (DMG-NET-003). When the damage is more than 0, also the fall sound of the block |
| `hay_block` | `d` | `0.2F` | `fall` | none |
| `slime_block` | `d` | `0.0F` | `fall` | none. The block gives no fall damage, also with the shift key state on |
| the block tag `beds`, `shelf_mushroom` | `d × (1 - 0.5F)` | `1.0F` | `fall` | none |
| `powder_snow` | — | — | — | no damage. A fall sound when `d ≥ 4` |
| `pointed_dripstone`, `icicle`, with the properties `vertical_direction=up` and `thickness=tip` | `d + 2.5` | `2.0F` | `stalagmite` | none |
| `pointed_dripstone`, `icicle`, other states | `d` | `1.0F` | `fall` | none |
| `farmland` | `d` | `1.0F` | `fall` | the block domain can trample the farmland |
| `turtle_egg` | `d` | `1.0F` | `fall` | the block domain can break an egg |

The bounce action of a bed and of a slime block is part of `entity-physics.md` (PHY-RST-001). The
block effects of farmland and of the egg are writes of the block domain (DMG-ORD-002). The block
flag `STEP_ACTION` does not tell `magma_block` from the other step blocks.

**DMG-FALL-005** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). When the
fall damage `n` is more than 0, the server plays two sounds at the entity before the hit:

1. the fall sound of the entity: the big sound when `n > 4`, else the small sound. A monster uses
   `entity.hostile.big_fall` and `entity.hostile.small_fall`. Other types use
   `entity.generic.big_fall` and `entity.generic.small_fall`, unless the type has its own sounds;
2. the fall sound of the block under the entity, at `y - 0.2`. The volume is the volume of the
   sound group of that block × 0.5. The pitch is the pitch of the group × 0.75.

Let the fall power `p` be `max(0, floor((d + 1.0E-6) - s))`, in f64 with `d` before the block
changes it. When `p > 0`, the server sends block particles of the block at the effect position
(DMG-NET-007). Air gives no particles.

## 6. The damage of a magma block

**DMG-HOT-001** (`black_box_confirmed`, semantic, confidence: high). The step action of
`magma_block` (PHY-RCH-002) does a hit of the type `hot_floor` with the amount `1.0` on a living
entity. It does nothing when the shift key state of the entity is on. A mob never has it.

The step action occurs in each tick in which the entity is on the ground on the block (PHY-ORD-001
step 14). Thus the cooldown sets the rate. A pig loses 1 health each 10 ticks, and dies in its
tenth hit. A zombie loses `0.94` each 10 ticks because of its armor (DMG-RED-001). A zombified
piglin, a magma cube and a strider take no damage, because they are fire-immune.

Other blocks with a step action do not do damage. Each other damage of a block is an inside
action (PHY-RCH-007) and is out of scope.

## 7. Crowd damage

**DMG-CRW-001** (`black_box_confirmed`, semantic, confidence: high). The crowd damage of PHY-PSH-007
is a hit with the source of DMG-SRC-002. The value `n` of PHY-PSH-007 comes from a resource of the
dimension world, as the game rule `random_tick_speed` does. The host holds the value of
`max_entity_cramming` (default 24). Each dimension world copies it. A read
from the save and a `gamerule` command are hooks.

**DMG-CRW-002** A candidate of PHY-PSH-002 must be alive (PHY-ORD-012). A death thus removes the
entity from the count of each later entity of its crowd in the same tick. This rule states that
effect without the order of the reference:

1. In one tick, the entities of one crowd do their crowd step in the order of their order keys
   (PHY-CMP-013). The lowest key is first.
2. The count of PHY-PSH-007 steps 2 and 4 leaves out each candidate with a lower order key that
   died in this tick. The death can come from each hit of the tick: its fall action, its step
   action, its crowd hit or a hit from another entity.
3. A candidate with a higher order key stays in the count, also when it dies later in the tick.

Only the count leaves out these deaths. The pushes use the values at the start of the tick
(PHY-CMP-012). Thus a death reaches the flag `pushable` of the push in the next tick.

The pass applies the rule through the sorted queue (DMG-ORD-002):

1. The tick of each entity keeps the counts of PHY-PSH-007 steps 2 and 4, its neighbour list and
   its draw of step 3. It does not apply a crowd hit.
2. When the step 2 count is more than `n - 1` and the draw is 0, the entity adds a crowd hit to
   the sorted queue. The crowd hit holds both counts and the neighbour list.
3. The apply step takes the crowd hit at its place in the sorted queue. It finds each entity of
   the neighbour list that died in this tick and has a lower order key. An entity counts only when
   it was a candidate: `pushes(own, dead)`, and the boxes intersect. The apply step subtracts each
   such entity from the step 2 count. It subtracts it from the step 4 count only when that count
   contains it, thus only when the entity was not a passenger.
   The apply step does no second query of the entities.
4. When the step 4 count is still more than `n - 1`, the apply step applies the crowd hit. A death
   from that hit joins the deaths that step 3 subtracts for the later entities.

A tick in which no entity draws 0 in a crowd adds no crowd hit. In a layout of many cells, a death
in the halo of another cell counts one tick late (`docs/physics-architecture.md`, C6).

Thus a crowd of `n + 1` loses at most one entity to crowd damage in one tick. When a death leaves
`n` entities, the crowd damage stops for the others. The experiments D6 and D7 show this:

| Crowd in a cell of 1 × 1 blocks | Health of each | Crowd deaths | Tick of the deaths |
|---------------------------------|----------------|--------------|--------------------|
| 25 | 20 | exactly 1 in each of 16 trials | tick 31 to 35 |
| 30 | 20 | exactly 6 in each of 8 trials | tick 32 to 38; up to 4 in one tick |
| 25 | 5 | exactly 1 in each of 10 trials | tick 1 |
| 30 | 5 | exactly 6 in each of 6 trials | tick 1 |

Without this rule, each of the 25 entities with 5 health dies with the chance 1 in 4 in the first
tick. The crowd then loses about 6 entities, not 1. Section 13 records the difference in the order
as DMG-DEV-001.

Provenance: `black_box_confirmed`. Kind: deviation (DMG-DEV-001). Confidence: high.

**DMG-CRW-003** (`black_box_confirmed`, semantic, confidence: high). The rate of crowd hits follows
from PHY-PSH-007 step 3 and from the cooldown. While the count stays more than `n - 1`:

- the first hit comes after `G + 1` ticks;
- each later hit comes `10 + G` ticks after the previous hit;
- `G` is a geometric variable with the chance `1/4`: `P(G = k) = (3/4)^k × 1/4`.

The mean gap between two hits is 13 ticks. An entity with the health `h` dies at its hit number
`ceil(h / 6)`. A zombified piglin with 20 health dies at its fourth hit, at about tick 43 on
average. The first death of a crowd of 25 comes earlier, because it is the first of 25 entities.

Each crowd hit, also a rejected one, sets the idle time to 0 (DMG-HIT-002 step 1). Thus an entity that
draws crowd damage often does not reach the idle time of the despawn rule (AIT-VAN-020).

## 8. Death

**DMG-DTH-001** (`black_box_confirmed`, semantic, confidence: high). When a hit leaves the health
at 0, the entity dies. The death occurs one time for each entity. It does these steps:

1. Give the kill credit to the causing entity of the last records (DMG-HIT-005 step 1), if there is
   one. `player-attack.md` states the credit.
2. Change to the dying state. From now on, the entity is not alive.
3. Send the death event (DMG-DTH-002).
4. Drop the loot and the experience. Loot is out of scope. `experience.md` states the
   experience (XP-DRP-001). The drop uses the source of the death and the time since the last hit by a player.
5. Send the entity event 3 to the tracking players (DMG-NET-003).
6. Set the pose to `dying` (DMG-NET-002).

**DMG-DTH-002** (`source_suggested`, semantic, confidence: medium). The death event is a fact that
other systems can read. It has the entity, its type, its position, the damage source of the death
and the last player hit (`player-attack.md`). The game event `entity_die` occurs at the position of
the entity. Game events are out of scope.

**DMG-DTH-003** In each tick in the dying state, the base entity tick increases the death time
by 1, after the decreases of DMG-HIT-006.
When the death time reaches 20, the server sends the entity event 60 to the tracking players and
removes the entity. Thus the entity stays in the world for 20 ticks after the tick of its death.
It is not in the world in the 20th tick after its death.

The removal occurs in the same tick, on the same path as the removal of an item that reached its
lifetime. The removal is not a request of the life cycle and not a claim. The tracker sends
`remove_entities` in the same tick (TRK-LIFE-004).

An entity whose hurt time or damage cooldown is more than 0, or that is in the dying state, does
not rest. Thus these counters run in each tick (PHY-SKIP-004). A death changes no body. The pushes
of the next tick read the flag `pushable` of the dead entity as false (PHY-CMP-012).

Provenance: `black_box_confirmed`. Kind: semantic. Confidence: high.

**DMG-DTH-004** An entity in the dying state:

- rejects each hit (DMG-HIT-001 step 6);
- is not pushable (PHY-ORD-012), so it is not a candidate of a push or of a crowd count;
- is not selected by an entity selector of a command, for example `@e`;
- has an AI step with no input: no input vector, no jump input and no AI decision;
- still does its travel step and its block effects step, so it can fall and move;
- still pushes the other entities in its push step (PHY-PSH-001);
- keeps its box, because the pose `dying` has the dimensions of the pose `standing` for a mob;
- stays in the world and counts in each count that does not test the alive state, for example the
  mob cap of `natural-spawning.md`;
- keeps the death time in the save.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: medium.

**DMG-DTH-005** (`design`, semantic, confidence: medium). The state that the physics step reads
is the only authority for the health and the dying state. This state holds the health, the
absorption, the last amount, the damage cooldown, the hurt time and the death time. No second copy
of the health or of the pose exists. The tracker reads the entity data entries 6 (pose) and 9
(health) from this state. The pose `dying` follows from the dying state. The grid tests the alive
state for the flag `pushable`.

## 9. Packets that a client sees

**DMG-NET-001** (`black_box_confirmed`, wire, confidence: high). A full hit sends the packet
`damage_event` (play clientbound 25) to the tracking players, and to the entity when it is a player.
The fields are:

| Field | Type | Value |
|-------|------|-------|
| entity | VarInt | the network id of the entity |
| damage type | VarInt | the network id of the damage type (REG-NET-006) |
| causing entity | VarInt | the network id of the causing entity plus 1, or 0 for none |
| direct entity | VarInt | the network id of the direct entity plus 1, or 0 for none |
| source position | Boolean, then 3 × f64 when true | the source position, when the source has one |

With the vanilla data pack, the network ids are 2 for `cactus`, 4 for `cramming`, 10 for `fall`
and 20 for `hot_floor`. A partial hit and a rejected hit send no `damage_event`. The client uses
the packet to play the hurt animation and the hurt sound of the type.

**DMG-NET-002** (`black_box_confirmed`, wire, confidence: high). The health is the entity data
value with the index 9 and the serializer 3 (f32). The pose is the value with the index 6 and the
serializer 20; the pose `dying` is 7. When the health or the pose changes, the next update of the
entity sends the changed values in `set_entity_data` (play clientbound 101). The death sends the
pose and the health in one packet, for example `06 14 07 09 03 00 00 00 00 ff` after the entity id.

**DMG-NET-003** (`black_box_confirmed`, wire, confidence: high). The packet `entity_event` (play
clientbound 34) has the entity id as a 32-bit integer and the event as a byte. Damage uses these
events:

| Event | When | Client effect |
|-------|------|---------------|
| 3 | the death (DMG-DTH-001 step 5) | the death sound; a mob starts the death animation |
| 60 | the removal after 20 ticks (DMG-DTH-003) | the smoke particles of the death |
| 54 | each fall action on `honey_block` (DMG-FALL-004) | the particles of the honey |

The server sends `remove_entities` (play clientbound 78) in the update of the tick of the
removal, after the entity event 60.

The events `damage_event` and `entity_event` go to the tracker as a subject event of the muxer,
a signal of the entity event, with the body of the packet. For one subject in one tick, each viewer gets
the packets in this order:

1. the signals and the sounds, in the order of their causes in the tick. For a hit that kills:
   `damage_event`, the death sound, the spawn of the drops, then the entity event 3. For a fall:
   the fall sounds, `damage_event`, then the hurt sound. A fall onto honey adds the slide sound and
   the entity event 54 first, and the block fall sound of its row last (D13);
2. the update packets of TRK-UPD-002;
3. the velocity of DMG-NET-004;
4. at the removal, `remove_entities` (TRK-REM-001).

D9, D13 and TRK-E1 show this order. The sounds do not go through the tracker; they go to the
players near the position (DMG-NET-007). Thus only their place among the packets of one tick is
stated, not a channel.

**DMG-NET-004** (`black_box_confirmed`, semantic, confidence: high). A full hit of a damage type
without the tag `no_impact` marks the velocity. At the end of the next tracker tick, after the
update packets, the tracker sends `set_entity_motion` (play clientbound 103). The viewers and the
entity itself get it. Then the tracker clears the mark (TRK-UPD-007). The mark does not make the tracker tick an
update. D9 shows the packet after `set_entity_data`.

**DMG-NET-005** (`black_box_confirmed`, wire, confidence: high). The server part of a player sends
`set_health` (play clientbound 106) to that player when one of these values changed since the last
`set_health`:

- the health;
- the food level;
- the state "the saturation is 0".

The fields are the health (f32), the food level (VarInt) and the saturation (f32). The server also
sends the health of the player in its entity data (DMG-NET-002).

**DMG-NET-006** Let the target be a player. A full hit of a damage type without the tag
`no_knockback` sends the packet `hurt_animation` (play clientbound 43) to that player only
(DMG-HIT-005 step 2). The packet also goes when the attribute `knockback_resistance` stops the
knockback (DMG-KB-001 step 1). A hit of a type with the tag `no_knockback` sends no
`hurt_animation`. `player-attack.md`, ATK-NET-004, states the fields.

Provenance: `source_suggested_black_box_confirmed`. Kind: wire. Confidence: high.

**DMG-NET-007** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). The server
sends these sounds as `sound` packets (play clientbound 119) at the position of the entity:

- the hurt sound, at each full hit that does not kill (DMG-HIT-005 step 4);
- the death sound, at a death by a full hit (DMG-HIT-005 step 3);
- the fall sounds of DMG-FALL-005;
- the sound `block.honey_block.slide` of DMG-FALL-004.

A partial hit and a rejected hit send no sound. D9 recorded one `sound` packet after each
`damage_event` of a hit of magma. The vanilla client also plays a hurt sound from `damage_event`;
that is a separate sound of the client. A `sound` packet goes to each player within 16 blocks,
or 16 × the volume when the volume is more than 1. The sound event of a hurt and of a death is data
of the entity type: for a zombified piglin, `entity.zombified_piglin.hurt` and
`entity.zombified_piglin.death`. The default is `entity.generic.hurt` and `entity.generic.death`.
The pitch and the seed are random, so a test compares only the sound event, the source and the
position. D13 decoded the packets. The source is 5 (`hostile`) for a zombified piglin and a
zombie. The volume is 1.0 for a hurt, a death and a fall sound of the entity. The fall sound of
a block has the volume and the pitch of DMG-FALL-005.

The fall particles go in `level_particles` (play clientbound 48) to each player within 32 blocks.
The count is an integer: `c = trunc(150.0 × min(f64(0.2F) + p / 15.0, 2.5))`, in f64, with `p`
the fall power of DMG-FALL-005.

## 10. Rules for a player only

The fall distance of a player comes from the moves of the client.

**DMG-PLR-001** A hit on a player is rejected in each of these cases:

- the causing entity is a player, and the game rule `pvp` is false;
- the causing entity is a player on the same team as the target, and that team does not allow
  friendly fire. A target with no team does not reject the hit by this test. Teams are a hook;
- the player has the invulnerable ability of creative mode or spectator mode, and the damage type
  does not have the tag `bypasses_invulnerability`;
- the damage type has the tag `is_fall` and the game rule `fall_damage` is false;
- the damage type has the tag `is_fire` and the game rule `fire_damage` is false;
- the damage type has the tag `is_drowning` and the game rule `drowning_damage` is false;
- the damage type has the tag `is_freezing` and the game rule `freeze_damage` is false.

A mob does not read these game rules. The tests of the causing entity apply to each damage type,
not only to an attack. The causing entity can be the target itself, for example the player that
primed a TNT. ATK-TGT-004 of `player-attack.md` applies these tests to an attack.

Provenance: `source_suggested_black_box_confirmed`; `source_suggested` for the test of the team.
Kind: semantic. Confidence: high; medium for the test of the team. Experiment for the test of
`pvp` and the test of creative mode: ATK-E7 of `player-attack.md`;
`tools/mcrs_minecraft_probe/traces/player_attack.json`.

**DMG-PLR-003** (`source_suggested`, semantic, confidence: high). A hit on a player scales with the
difficulty when the damage type has the scaling `always`. The damage type has the scaling `always`
in its data. The scaling `when_caused_by_living_non_player` applies when the causing entity is a
living entity that is not a player. The test of DMG-PLR-001 and the test for the invulnerable
ability come first. Then the amount changes in this way:

- on peaceful, the amount is 0;
- on easy, the amount is `min(amount / 2 + 1, amount)`;
- on normal, the amount does not change;
- on hard, the amount is `amount * 3 / 2`.

A hit with the amount 0 after this step is rejected. The three damage types of this document have
the scaling `when_caused_by_living_non_player`. Their sources have no causing entity, so they do
not scale. The damage types `explosion` and `player_explosion` have the scaling `always`.

**DMG-PLR-002** (`source_suggested`, semantic, confidence: medium). The damage cooldown of a player
decreases in the server part of the player (PHY-ORD-010 step 6), not in its base entity tick. The
server part runs in each server tick, also when the client sends no move.

## 11. The place of damage in the tick

**DMG-ORD-001** In the tick of a living entity that is not a player (PHY-ORD-002), damage occurs at
these steps:

1. Step 2, the base entity tick: the decreases of DMG-HIT-006, then the death time of DMG-DTH-003.
2. Step 8, the block effects step, in the order of PHY-ORD-001 step 14. First comes the hit of the
   step action (DMG-HOT-001), then the inside actions, then the hit of the fall action
   (DMG-FALL-001).
3. Step 9, the push step: the crowd hit, before the pushes (PHY-PSH-007).

In the pass, a hit from another entity, for example an attack, applies in the apply step of the
tick that wrote it (PHY-CMP-015, AIT-RFX-006). Each tick has one apply step. It comes after the
tick of each entity of the pass, and before the server sends the changes of the tick. Thus the
target does its own tick with the health from before the hit. The update of the same tick sends
the new health. A hit from another cell can apply in the apply step of the next tick (PHY-CMP-015).

The order is visible. An entity that lands on magma takes the `hot_floor` hit first, as a full
hit. The fall hit of the same tick is then a partial hit, so the damage event names `hot_floor`.

Provenance: `source_suggested_black_box_confirmed`. Kind: semantic. Confidence: high.

**DMG-ORD-002** In the pass, the own hits of an entity are entries of the sorted queue, as the hits
from other entities are. The own hits are the hits
that the tick of the entity gives to itself in step 8 and later:

- the hit of the step action (DMG-HOT-001);
- the hits of the inside actions (`environment-damage.md`, ENV-FIR-010 and section 5);
- the hit of the fall action (DMG-FALL-001);
- the freezing hit (`environment-damage.md`, ENV-FRZ-003);
- the crowd hit (DMG-CRW-002).

The tick of the entity reads the block at the effect position from the block copy of the pass
(PHY-CMP-009). It adds the own hits to the sorted queue with its own writer key, in the order
of DMG-ORD-001 and ENV-ORD-001. The apply step applies them in that order (PHY-CMP-015). The hits of
step 2, the base entity tick, apply at their step in the tick, in the order of PHY-CMP-015. The
apply step after the pass does not contain them. A death there changes the AI step and the travel
step of the same tick (DMG-DTH-004).

An own hit does not stop the tick of the entity. Each effect of step 8 runs, also when an earlier
own hit of the tick kills the entity in the apply step. Thus, in the pass, an own hit never stops
the sequence of ENV-FIR-010. The tick does not preview its own hits. DMG-DEV-003 records this
difference.

The apply step applies the hits on one target in the order of the writer key (PHY-CMP-015). The
own hits have the writer key of the target. Thus a hit from a writer with a lower writer key comes
before the own hits. A hit from a writer with a higher writer key comes after them. The tick
order of the entities does not change this order. DMG-DEV-004 records the difference.

With the vanilla data pack, the result of the tick is the same as with each own hit at its step,
except for the cases of DMG-DEV-003 and DMG-DEV-004. The damage types of the own hits have the tag
`no_knockback`, so an own hit changes no velocity. A dying entity still does its push step
(DMG-DTH-004). The tick reads the other entities at their state from the start of the tick
(PHY-CMP-012). The reason for the sorted queue: a user of the library can change or cancel each hit
before the apply step. DMG-DEV-002 records the case of a data pack that removes the tag.

The packets of each hit keep the order of DMG-NET-003. The part of an action that writes a block
stays a write of the block domain (PHY-BLK-001). The cooldown makes the order of the hits visible
(DMG-ORD-001).

Provenance: `design`. Kind: deviation (DMG-DEV-002, DMG-DEV-003, DMG-DEV-004). Confidence: medium.

**DMG-ORD-003** (`black_box_confirmed`, semantic, confidence: high). Two hits on one entity in one
tick, with the amounts `a` and `b`, no reduction and no damage type of the tag `bypasses_cooldown`,
change the health by the same value in each order. Let `c` and `L` be the values of DMG-HIT-003
before the hits. When `c` is 10 or less, the value is `max(a, b)`: the first is a full hit, and the second is a partial hit with the
difference, or a rejected hit. When `c > 10`, the value is `max(a, b, L) - L`. The
order changes only the damage type of the damage event and of the death. Armor and `resistance`
are not linear, so with them the total can differ by a small amount.

A hit with a damage type of the tag `bypasses_cooldown` is always a full hit (DMG-HIT-003), so the
order then changes the health. Example: the health is 20, a hit of 2 has that tag, and a hit of 5
does not. The order "2, then 5" gives 15. The order "5, then 2" gives 13. The tag is empty in the
corpus, so only a data pack makes this case.

## 12. Out of scope

| Subject | Where it is stated, or the reason | Hook |
|---------|-----------------------------------|------|
| items: shields, helmets with `damages_helmet` (the amount × 0.75), the wear of armor, the protection of enchantments, the immunity of enchantments, the totem of undying | Out of scope. | DMG-HIT-002, DMG-RED-003, DMG-DTH-001 |
| the attack of a player, the knockback of an attack, the kill credit, the sweep | `player-attack.md` (ATK-DMG-001 to ATK-DMG-005, ATK-KB-001 to ATK-KB-005, ATK-SWP-001, ATK-SWP-002, ATK-CRD-001 to ATK-CRD-004) | DMG-SRC-002, DMG-HIT-005 |
| the experience of a death | `experience.md` (XP-DRP-001 to XP-DRP-003) | DMG-DTH-001 step 4 |
| loot and the drop of equipment | item entities are out of scope; `player-death.md` states the drop of a player (PLD-DRP-001 to PLD-DRP-006) | DMG-DTH-001 step 4 |
| effects as a system: `resistance`, `absorption`, `fire_resistance`, and the effects that act on a hit | `status-effects.md` (EFX-ACT-001 to EFX-ACT-011) | DMG-RED-002, DMG-RED-004 |
| fire, lava, drowning, freezing, suffocation, the void and the world border | `environment-damage.md` (ENV-FIR-001 to ENV-BRD-002) | DMG-HIT-001, DMG-PLR-001 |
| the inside actions that damage: cactus, campfire, sweet berry bush, fire, lava cauldron | `environment-damage.md` (ENV-CTC-001 to ENV-CTC-004) | PHY-RCH-007 |
| starvation | `hunger.md` (HNG-STV-001, HNG-STV-002) | DMG-HIT-001 |
| explosion damage and explosion knockback | `explosions.md` (EXP-DMG-001 to EXP-DMG-003, EXP-KNB-001) | DMG-SRC-002, DMG-PLR-003 |
| the damage of an arrow and of a snowball | `projectiles.md` (PRJ-ENT-001 to PRJ-ENT-007, PRJ-SNW-001 to PRJ-SNW-003) | DMG-SRC-002 |
| entity types with an own hit procedure: `armor_stand`, `ender_dragon`, `squid`, `glow_squid`, and others | their behaviour is type data of a later document | the entity type data |
| the difficulty scale of a hit on a mob | DMG-PLR-003 states the rule for a player only | DMG-PLR-003 |
| impulses, for example a wind charge, that cancel the fall damage of the launch | the farm has no impulse | DMG-FALL-001 |
| the death of a player: respawn, the death screen, `player_combat_kill`, the drop of the inventory | `player-death.md` (PLD-DTH-001 to PLD-RST-004) | DMG-DTH-001 |
| the natural regeneration of a player, food and exhaustion | `hunger.md` (HNG-REG-001 to HNG-REG-005, HNG-EXH-001 to HNG-EXH-007) | DMG-NET-005 |
| the save of the health and of the dying state of a player | `persistence.md` (SAVE-PLAYER-001); no document saves a mob | DMG-DTH-004 |
| game events (`entity_damage`, `entity_die`) | no farm block hears them | DMG-DTH-002 |
| the scoreboard criteria of damage and death | no farm mechanic uses them | DMG-DTH-001 step 1 |

## 13. Intentional deviations

### DMG-DEV-001: Crowd deaths in the order of the order keys

- **Rule:** DMG-CRW-002.
- **Reference:** the reference ticks the entities in the order of its tick list. A death removes
  the entity from the count of each entity that ticks later in the same tick.
- **Reason:** the apply step applies the hits of the sorted queue in the order of the order key of
  the target (PHY-CMP-015). Thus the deaths of a crowd do not depend on the thread count or on the
  order of the births. In a layout of many cells, a death in the halo of another cell counts one
  tick late (DMG-CRW-002). Then the cell layout can change the crowd hits of that tick.
- **Visible to the client:** yes, as the choice of the entity that dies. When the crowd is in one
  cell and no other hit kills an entity of the crowd in the tick, the number of deaths of the crowd
  in that tick is the same. Across cells, each cell can apply a lethal crowd hit in one tick.
  The tick of a death can differ, as PHY-DEV-004 states.
- **Bug reference:** none.
- **Test:** for a crowd in one cell that no other hit kills, a differential test compares the
  number of crowd deaths and the distribution of the ticks of the deaths. It does not compare
  which entity dies.

### DMG-DEV-002: Own hits after the tick of the entity

- **Rule:** DMG-ORD-002.
- **Reference:** the reference applies each own hit at its step in the tick of the entity. The hits
  of the block effects step apply at step 8, and the freezing hit applies after them. The crowd hit
  applies at step 9, before the pushes.
- **Reason:** each hit goes through the sorted queue, so a user of the library can change or
  cancel it. The velocity is the same while the damage types of these hits have the tag
  `no_knockback`. DMG-DEV-003 records the inside effects after a death. DMG-DEV-004 records the
  order of the own hits and the hits of other entities.
- **Visible to the client:** the velocity, only with a data pack that removes the tag
  `no_knockback` from the damage type of an own hit, for example `fall` or `cramming`. Then the
  knockback of such a hit applies after the push of the tick, not before it. The velocity after
  the tick can differ.
- **Bug reference:** none.
- **Test:** a differential test with such a data pack compares the health. It does not compare the
  velocity, the position or the packets that carry them, for example `set_entity_motion`.

### DMG-DEV-003: Inside effects after a death by an own hit

- **Rule:** DMG-ORD-002.
- **Reference:** the reference applies each own hit at its step. A death in step 8 stops the inside
  effects that follow in that step (ENV-FIR-010). The death can come from the hit of the step
  action or from an inside hit.
- **Reason:** an own hit applies after the tick of the entity, in the apply step (PHY-CMP-015). The
  tick does not know of the death. Thus the inside effects that follow the hit in the same tick
  still run on the entity that then dies. Examples are an ignition, an extinguish and the freeze of
  powder snow. mcrs does not preview the own hits, and it does not stop the tick early. A preview
  cannot know which hit a user of the library cancels before the apply step.
- **Visible to the client:** yes. In the death tick, the burning flag, the fire ticks and the frozen
  ticks of the dying entity can differ. In the next tick, the dying entity does its block effects
  step again (DMG-DTH-004), and it gets the effects that the reference gives. The fire ticks and the
  frozen ticks carry over from tick to tick (ENV-FIR-002, ENV-FRZ-002). Thus their difference can
  stay after the death tick, until the counters reach the same limit.
- **Bug reference:** none.
- **Test:** a differential test that compares the flags of a dying entity uses this entry. From the
  death tick on, it accepts a difference of the burning flag, the fire ticks and the frozen ticks.

### DMG-DEV-004: Hits of other entities in the order of the writer key

- **Rule:** DMG-ORD-002.
- **Reference:** the reference applies the hits on one target in the order of its tick list. An
  entity that ticks before the target hits it before the own hits of the target. An entity that
  ticks after the target hits it after them. Example: an entity B ticks after the target X, and B
  attacks X in that tick. X lands on magma in the same tick. The `hot_floor` hit of X is the full
  hit. The attack of B is then a partial hit or a rejected hit.
- **Reason:** in mcrs, the apply step orders the hits by the writer key (PHY-CMP-015). Let B have a
  lower UUID than X. Then the attack of B applies first and is the full hit. The `hot_floor` hit and
  the fall hit of X are then partial hits or rejected hits. The tick list of the reference depends
  on the order of the births, the loads and the cell layout. The writer key is total and fixed.
  Thus, for the writes that reach the same apply step, the result does not depend on the thread
  count or on the cell layout. A write from another cell can apply one tick late (PHY-CMP-015).
  In the reference, a hit from an entity that ticks before the target sees the damage cooldown
  before the decrease of DMG-HIT-006. In mcrs, it sees the cooldown after that decrease. Example:
  the cooldown is 11 and the last amount is 10. In the reference, an external hit of 5 is rejected,
  and the own hit of 2 is then full: the health changes by 2. In mcrs, both hits see the cooldown
  10: the health changes by 5.
- **Visible to the client:** yes, when two writers hit one target in one tick. The damage type of
  `damage_event` and of the death can differ. Only a full hit gives knockback, `hurt_animation` and
  a hurt sound (DMG-HIT-005), so these can differ too. Without reductions, without the tag
  `bypasses_cooldown`, and with a cooldown other than 11 at the start of the tick, the health after
  the tick is the same (DMG-ORD-003).
- **Bug reference:** none.
- **Test:** when two writers hit one target in one tick without a reduction of armor or
  `resistance`, no damage type has the tag `bypasses_cooldown`, and the cooldown of the target is
  not 11 at the start of the tick, a differential test compares
  the health after the tick. Else it does not compare the health (DMG-ORD-003). It does not compare the damage type of `damage_event`, the knockback, the
  `hurt_animation` packet or the hurt sound of that tick.

## 14. Unresolved

- **U1.** No artefact gives the hurt sound, the death sound or the fall sounds of each entity type.
  DMG-NET-007 gives the values of the farm. A later document must give the table.
- **U2.** No artefact gives the fire-immune property. The corpus gives `fall_damage_immune` as a tag.
- **U3.** Some types have an own hit procedure (section 12). D8 shows `armor_stand`, `squid`,
  `glow_squid` and `ender_dragon` as immune to each type, because their procedures differ.

## 15. Experiments

The experiments ran on a vanilla dedicated server through its console. The rules cite them as D1 to D13.

| Id | Steps | Rules |
|----|-------|-------|
| D1 | Build a shaft of glass of 1 × 1 blocks on `honey_block`, `stone` and `hay_block`. Summon a zombified piglin `d` blocks above the top of the floor, for `d` = 3, 4, 24, 50, 78 and 78.0625. Wait 8 seconds. Read `Health`. | DMG-FALL-002, DMG-FALL-004 |
| D2 | Set `fall_damage` to false. Drop a zombified piglin 10 blocks onto stone. Read `Health`. | DMG-FALL-002, DMG-PLR-001 |
| D3 | Freeze the tick. Hit a zombified piglin, a zombie and three pigs with `/damage` of `cactus` and `cramming`. Give one pig `absorption` 0 and one pig `resistance` 0 before. Read `Health`, `HurtTime` and `AbsorptionAmount` after each tick. | DMG-HIT-003, DMG-HIT-004, DMG-HIT-006, DMG-RED-001, DMG-RED-002, DMG-RED-004 |
| D4 | Freeze the tick. Kill a zombified piglin with `/damage … 100 minecraft:generic`. Step one tick at a time. Read `DeathTime` and `Health` by UUID, and test `@e`. | DMG-DTH-003, DMG-DTH-004 |
| D5 | Put a pig, a zombie and a zombified piglin in cells of 1 × 1 blocks on `magma_block`. Step 130 ticks. Read `Health` and `HurtTime` after each tick. | DMG-HOT-001, DMG-HIT-006 |
| D6 | Put 25 or 30 zombified piglins in a cell of 1 × 1 blocks on `honey_block`. Step 200 ticks. Read the `Health` of each entity after each tick. Repeat with `max_entity_cramming` 26 and 0, and with `NoAI`. | DMG-CRW-001, DMG-CRW-002, DMG-CRW-003 |
| D7 | Repeat D6 with `Health:5f` for 40 ticks, and with `max_health` 1000 and `Health:1000f` for 400 ticks. | DMG-CRW-002, DMG-CRW-003 |
| D8 | For each entity type, summon it with `NoAI` and `NoGravity`. Read the base value of each attribute of DMG-ATT-002 with `/attribute … base get`, and `Health`. Do a `/damage` of `fall`, `hot_floor`, `cramming` and `player_attack` with the amounts 0.001, 0.002, 0.003 and 0.004, so that the cooldown accepts each one. Record each reply "invulnerable". | DMG-ATT-001, DMG-ATT-003 |
| D9 | Join the server with an offline client that answers only the keep-alive, the teleport and the chunk batches. Run D1 (13 blocks onto honey), D5 for a pig, the kills and the hits of D3 near the player, and hits on the player. Record the clientbound packets 1, 25, 34, 43, 48, 69, 78, 101, 103, 106 and 119. | section 9 |
| D10 | With the client of D9, drop a zombie 10 blocks onto magma. Record the packets and read `Health`. | DMG-ORD-001 |
| D11 | Hit the player of D9 with `/damage Probe 1 minecraft:mob_attack by <piglin>`. Record the packets. | DMG-NET-001, DMG-NET-006 |
| D12 | Drop a zombified piglin 10 and 24 blocks onto a bed, `slime_block`, `powder_snow`, `pointed_dripstone` and `icicle` with the tip up, and `shelf_mushroom`. Read `Health` after 6 seconds. | DMG-FALL-004, DMG-FALL-005 |
| D13 | With the client of D9, hit a zombified piglin with `cramming` 6, then kill it with `cramming` 100. Decode each `sound` packet by the registry `minecraft:sound_event`. | DMG-NET-003, DMG-NET-007 |
