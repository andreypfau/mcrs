# Entity damage: health, damage and death

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

This document states the health of a living entity, the damage that it takes, and its death. It
states one procedure for each hit, the reductions of the damage, the cooldown after a hit, and the
death state. It states three sources of damage completely: a fall, a step on a magma block and a
crowd. Other sources use the same procedure with their own damage type. The procedure is the same
for a player and for a mob. Section 10 states the rules that apply only to a player.

The rules come from the reference implementation. Experiments on a vanilla server confirm the
numeric rules and the packets (sections 15 and 16).

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **entity**, **living entity**, **server-simulated entity**,
  **fall distance**, **effect position**, **step action**, **fall action**, **crowd damage**,
  **base entity tick**, **block effects step**, **shift key state**, **f32**, **f64**,
  **widen**, **corpus**, **data pack**: as `entity-physics.md`, section 0.1, defines them.
- **Order key**: as PHY-CMP-013 defines it.
- **Merge point**, **pass**: as `docs/physics-architecture.md`, sections 3.4 and 3.7, defines them.
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
- **Fire-immune**: a property of an entity type. Appendix A lists the living types that have it.
- **Dying state**: the state of a living entity whose health is 0 or that died (section 8).
- **Death time**: an integer counter of an entity in the dying state. The data name is
  `DeathTime`.
- **Alive**: a living entity is alive when it is not removed and its health is more than 0.
- **Idle time**: as `entity-ai-timing.md`, section 0.1, defines it.
- **Crowd**: a set of living entities that the count of PHY-PSH-007 joins: each entity of the set
  is a candidate of another entity of the set.
- **Fall multiplier**: the f32 factor that the fall action of a block gives to the fall damage
  (DMG-FALL-004).
- **Tracking players**: the players to which the server sends the updates of an entity, as the
  replication of the entity states them.

### 0.2 Provenance

The labels are those of `entity-physics.md`, section 0.2. Section 16 holds the captured results.
Each rule states its kind and its confidence, for example
`(black_box_confirmed, semantic, confidence: high)`. The kinds are those of `docs/spec/README.md`.

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
- The game rule table of `crates/mcrs_minecraft_game_rule`: `max_entity_cramming` (default 24),
  `fall_damage`, `fire_damage`.
- Appendix A: the base values of the attributes and the immunities of each living entity type. A
  vanilla server gave these values (experiment D8).

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

The base value of each attribute is data of the entity type (Appendix A). The attribute
definitions in `crates/mcrs_minecraft_entity` give the default when a type has no own value.
Modifiers of items and effects change the value. This document does not state the modifier
rules. A mob with no items and no effects has the base value.

**DMG-ATT-003** (`black_box_confirmed`, semantic, confidence: high). Two properties of the entity
type make it immune to some damage types (DMG-HIT-001): the entity type tag `fall_damage_immune`,
and the fire-immune property of Appendix A.

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

**DMG-HIT-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). A hit on a
living entity first does these tests, in this order. When a test is true, the hit is rejected and
the procedure stops.

1. The entity is removed.
2. The entity is invulnerable and the damage type does not have the tag
   `bypasses_invulnerability`. Hook: the saved flag `Invulnerable` and the saved counter
   `invulnerable_time` make an entity invulnerable. In G2 no system sets them, so no mob is
   invulnerable and the row needs no field for them.
3. The damage type has the tag `is_fire` and the entity type is fire-immune.
4. The damage type has the tag `is_fall` and the entity type is in `fall_damage_immune`.
5. For a player, a test of DMG-PLR-001 is true.
6. The entity is in the dying state (section 8).
7. The damage type has the tag `is_fire` and the entity has the effect `fire_resistance`.

**DMG-HIT-002** (`source_suggested_black_box_confirmed`, semantic, confidence: high). After the
tests of DMG-HIT-001, the hit does these steps before the cooldown:

1. Hook: set the idle time of the entity to 0 (AIT-RFX-006). This step occurs before the
   cooldown, so also a hit that the cooldown rejects sets it. G2 has no idle time; `mob-goals.md`
   and `natural-spawning.md` add it.
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
   no causing entity records nothing. `player-attack.md` and `mob-goals.md` state these records.
2. For a full hit:
   - send the damage event (DMG-NET-001);
   - if the damage type does not have the tag `no_impact`, mark the velocity of the entity for
     the next update (DMG-NET-004);
   - if the damage type does not have the tag `no_knockback`, apply the knockback. The three types
     of this document have `no_knockback`. `player-attack.md` states the knockback.
3. If the health is 0, start the death (DMG-DTH-001). For a full hit, play the death sound
   (DMG-NET-007).
4. Else, for a full hit, play the hurt sound (DMG-NET-007), unless the damage type has the tag
   `no_hurt_sound`.
5. Hook: record the source as the last damage source of the entity. No system of G2 reads it.

A partial hit sends no damage event and plays no sound. Its health change reaches the client only
through the entity data.

**DMG-HIT-006** (`black_box_confirmed`, semantic, confidence: high). In each tick, the base entity
tick of a living entity decreases the hurt time by 1 when it is more than 0. Then it decreases the
damage cooldown by 1 when it is more than 0. A player does the second decrease in its server
part instead (DMG-PLR-002). Thus a full hit in tick `T` lets the next full hit occur in tick
`T + 10` at the earliest. The save keeps the hurt time. It does not keep the damage cooldown or the
last amount, so a loaded entity has a damage cooldown of 0.

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
the absorption. `effects.md` will state the effect.

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

D1 (section 16) gives the values for a zombified piglin. A fall of 78 blocks onto
`honey_block` gives 15 damage, so the piglin keeps 5 health.

**DMG-FALL-003** (`source_suggested`, semantic, confidence: medium). Hook. The fall action applies
also to each passenger (PHY-RCH-002), with the same `d'`, `m` and damage type. That hit writes
another row, so it is a write of PHY-CMP-015 at the merge point, not a hit of DMG-ORD-002. G2 has
no passengers, so G2 does not implement this rule.

**DMG-FALL-004** (`source_suggested_black_box_confirmed`, semantic, confidence: high). The fall
action of each block. D1 confirms the default row, `honey_block` and `hay_block`. The farm needs
only the default row and `honey_block`; G2 implements these two rows. The other rows wait for D12.

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
flag `STEP_ACTION` does not tell `magma_block` from the other step blocks. The named block data
of `mcrs_minecraft_collision_table` gets a hot floor flag and the fall multiplier; this document
allows that change of the kernel crate.

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
dimension world, as `RandomTickSpeed` does: the host holds the value of `max_entity_cramming`
(default 24), and each dimension world copies it. The pass reads it into `PushRules`. A read
from the save and a `gamerule` command are hooks.

**DMG-CRW-002** (`black_box_confirmed`, semantic, confidence: high). A candidate of PHY-PSH-002 must
be alive (PHY-ORD-012). A death thus removes the entity from the count of each later entity of its
crowd in the same tick. This rule states that effect without the order of the reference:

1. In one tick, the entities of one crowd do their crowd step in the order of their order keys
   (PHY-CMP-013). The lowest key is first.
2. The count of PHY-PSH-007 steps 2 and 4 leaves out each candidate with a lower order key that
   died in this tick. The death can come from its fall action, its step action or its crowd hit.
3. A candidate with a higher order key stays in the count, also when it dies later in the tick.

Only the count leaves out these deaths. The pushes use the values at the start of the tick
(PHY-CMP-012), so a death reaches the flag `pushable` of the push in the next tick.

The pass applies the rule in a crowd fix-up. The fix-up runs in `Simulate`, in the system
`entity_tick`, after the main pass and before `Project`:

1. The main pass keeps, for each entity, the counts of PHY-PSH-007 steps 2 and 4, its neighbour
   list and its draw of step 3. It does not apply a crowd hit.
2. The fix-up takes each entity whose step 2 count is more than `n - 1` and whose draw is 0, in
   the order of the order keys.
3. From both counts it subtracts each entity of its neighbour list that died in this tick and has
   a lower order key. An entity counts only when it was a candidate: `pushes(own, dead)`, and the
   boxes intersect. The fix-up does no second query of the grid.
4. When the step 4 count is still more than `n - 1`, it applies the crowd hit. A death from that
   hit joins the deaths that step 3 subtracts for the later entities.

The fix-up does no work in a tick in which no entity draws 0 in a crowd. In a layout of many cells,
a death in the halo of another cell counts one tick late (`docs/physics-architecture.md`, C6).

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
4. Drop the loot and the experience. Loot is out of scope. `experience-orbs.md` states the
   experience. The drop uses the source of the death and the time since the last hit by a player.
5. Send the entity event 3 to the tracking players (DMG-NET-003).
6. Set the pose to `dying` (DMG-NET-002).

**DMG-DTH-002** (`source_suggested`, semantic, confidence: medium). The death event is a fact that
other systems can read. It has the entity, its type, its position, the damage source of the death
and the last player hit (`player-attack.md`). The game event `entity_die` occurs at the position of
the entity. Game events are out of scope.

**DMG-DTH-003** (`black_box_confirmed`, semantic, confidence: high). In each tick in the dying
state, the base entity tick increases the death time by 1, after the decreases of DMG-HIT-006.
When the death time reaches 20, the server sends the entity event 60 to the tracking players and
removes the entity. Thus the entity stays in the world for 20 ticks after the tick of its death.
It is not in the world in the 20th tick after its death.

The pass removes the row in the same tick, on the path of an item that reached its lifetime: the
row goes to the expired list of its tile and leaves the tile, and `entity_tick` removes its cold
state. The removal is not a request of the life cycle and not a claim. The tracker sends
`remove_entities` in the same tick (TRK-LIFE-004).

A row whose hurt time or damage cooldown is more than 0, or that is in the dying state, does not
rest, so these counters run in each tick (PHY-SKIP-004). A death changes no body,
so it marks its tile dirty. The back buffer of the grid then holds the flag `pushable` false at
the next freeze.

**DMG-DTH-004** (`source_suggested_black_box_confirmed`, semantic, confidence: medium). An entity in
the dying state:

- rejects each hit (DMG-HIT-001 step 6);
- is not pushable (PHY-ORD-012), so it is not a candidate of a push or of a crowd count;
- is not selected by an entity selector of a command, for example `@e`;
- has an AI step with no input: no input vector, no jump input and no AI decision;
- still does its travel step and its block effects step, so it can fall and move;
- still pushes the other entities in its push step (PHY-PSH-001);
- keeps its box, because the pose `dying` has the dimensions of the pose `standing` for a mob;
- keeps its row and counts in each count that does not test the alive state, for example the
  mob cap of `natural-spawning.md`;
- keeps the death time in the save. Hook: G2 saves no mob.

**DMG-DTH-005** (`design`, semantic, confidence: medium). The row of the entity is the authority
for the health and the dying state. The living state of the row (`Living` of
`mcrs_minecraft_physics`) holds the health, the absorption, the last amount, the damage cooldown,
the hurt time and the death time. This document allows these fields in the kernel crate. The
cold state does not keep the entity data entries 6 (pose) and 9 (health): the tracker reads the
health from the row, and the pose `dying` follows from the dying state. The grid row tests the
alive state for the flag `pushable`.

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
`EntityEvent::Signal`, with the body of the packet. For one subject in one tick, each viewer gets
the packets in this order:

1. the signals, in the order of their causes in the tick: for one hit, `damage_event` before the
   entity event 3;
2. the update packets of TRK-UPD-002;
3. the velocity of DMG-NET-004;
4. at the removal, `remove_entities` (TRK-REM-001).

D9 and TRK-E1 show this order.

**DMG-NET-004** (`black_box_confirmed`, semantic, confidence: high). A full hit of a damage type
without the tag `no_impact` marks the velocity. At the end of the next tracker tick, after the
update packets, the tracker sends `set_entity_motion` (play clientbound 103) to the viewers and to
the entity itself, and clears the mark (TRK-UPD-007). The mark does not make the tracker tick an
update. D9 shows the packet after `set_entity_data`.

**DMG-NET-005** (`black_box_confirmed`, wire, confidence: high). The server part of a player sends
`set_health` (play clientbound 106) to that player when one of these values changed since the last
`set_health`:

- the health;
- the food level;
- the state "the saturation is 0".

The fields are the health (f32), the food level (VarInt) and the saturation (f32). The server also
sends the health of the player in its entity data (DMG-NET-002).

**DMG-NET-006** (`black_box_confirmed`, wire, confidence: high). The packet `hurt_animation` (play
clientbound 43) goes only to a player that gets a knockback (DMG-HIT-005 step 2).
`player-attack.md` states the packet.

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
The pitch and the seed are random, so a test compares only the sound event and the position. The
exact bytes of these packets are a pending capture (D13).

The fall particles go in `level_particles` (play clientbound 48) to each player within 32 blocks.
The count is an integer: `c = trunc(150.0 × min(f64(0.2F) + p / 15.0, 2.5))`, in f64, with `p`
the fall power of DMG-FALL-005.

## 10. Rules for a player only

Section 10 is not in the exit of G2: the fall distance of a player comes from the moves of the
client, and no farm mechanic hurts the player. G2 implements none of it.

**DMG-PLR-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). A hit on a
player is rejected in each of these cases:

- the player has the invulnerable ability of creative mode or spectator mode, and the damage type
  does not have the tag `bypasses_invulnerability`;
- the damage type has the tag `is_fall` and the game rule `fall_damage` is false;
- the damage type has the tag `is_fire` and the game rule `fire_damage` is false;
- the damage type has the tag `is_drowning` and the game rule `drowning_damage` is false;
- the damage type has the tag `is_freezing` and the game rule `freeze_damage` is false.

A mob does not read these game rules.

**DMG-PLR-002** (`source_suggested`, semantic, confidence: medium). The damage cooldown of a player
decreases in the server part of the player (PHY-ORD-010 step 6), not in its base entity tick. The
server part runs in each server tick, also when the client sends no move.

## 11. The place of damage in the tick

**DMG-ORD-001** (`source_suggested_black_box_confirmed`, semantic, confidence: high). In the tick of
a living entity that is not a player (PHY-ORD-002), damage occurs at these steps:

1. Step 2, the base entity tick: the decreases of DMG-HIT-006, then the death time of DMG-DTH-003.
2. Step 8, the block effects step, in the order of PHY-ORD-001 step 14: the hit of the step action
   (DMG-HOT-001), then the inside actions, then the hit of the fall action (DMG-FALL-001).
3. Step 9, the push step: the crowd hit, before the pushes (PHY-PSH-007).

A hit from another entity, for example an attack, applies at the merge point at the start of the
next tick (PHY-CMP-015, AIT-RFX-006).

The order is visible. An entity that lands on magma takes the `hot_floor` hit first, as a full
hit. The fall hit of the same tick is then a partial hit, so the damage event names `hot_floor`.

**DMG-ORD-002** (`design`, semantic, confidence: medium). In the order-independent pass, the hits of
the step action and of the fall action apply inside the tick of the own entity, at the step of
DMG-ORD-001. They read the block at the effect position from the block copy of the pass
(PHY-CMP-009). The pass applies the actions `Step` and `Fall` that `tick_entity` gives for the
row, in the order of the kernel, before the push of the row. The crowd hit applies in the crowd
fix-up of DMG-CRW-002, after the other hits of the entity, because it changes no velocity. These
hits write only the own row (PHY-CMP-012), so they need no merge
point. This narrows AIT-RFX-006, which puts the hits of other entities at the merge point. The part
of an action that writes a block stays an effect of `WorldWrite`
(`docs/physics-architecture.md`, section 3.7). The cooldown makes the order of the hits visible
(DMG-ORD-001).

**DMG-ORD-003** (`black_box_confirmed`, semantic, confidence: high). Two hits on one entity in one
tick, with the amounts `a` and `b` and no reduction, change the health by `max(a, b)` in each order.
The first is a full hit. The second is a partial hit with the difference, or a rejected hit. The
order changes only the damage type of the damage event and of the death. Armor and `resistance`
are not linear, so with them the total can differ by a small amount.

## 12. Out of scope

| Subject | Reason | Hook |
|---------|--------|------|
| items: shields, helmets with `damages_helmet` (the amount × 0.75), the wear of armor, the protection of enchantments, the immunity of enchantments, the totem of undying | the farm uses no items (`gold-farm.md`) | DMG-HIT-002, DMG-RED-003, DMG-DTH-001 |
| the attack of a player, the knockback, the kill credit, the sweep | `player-attack.md` (G6) | DMG-SRC-002, DMG-HIT-005 |
| the experience of a death | `experience-orbs.md` (G7) | DMG-DTH-001 step 4 |
| loot and the drop of equipment | item entities are out of scope | DMG-DTH-001 step 4 |
| effects as a system: `resistance`, `absorption`, `fire_resistance`, and the effects that act on a hit | no farm mechanic gives an effect to a mob | DMG-RED-002, DMG-RED-004 |
| freeze damage (× 5 for `freeze_hurts_extra_types`), drowning, suffocation, the void, fire, lava, the world border, starvation | the farm does not use them | DMG-HIT-001 |
| the inside actions that damage: cactus, campfire, sweet berry bush, fire, lava cauldron | the farm does not use them | PHY-RCH-007 |
| entity types with an own hit procedure: `armor_stand`, `ender_dragon`, `squid`, `glow_squid`, and others | their behaviour is type data of a later document | Appendix A |
| the difficulty scale of a hit on a player (the field `scaling`) | the sources of this document have no causing entity, so they never scale | DMG-SRC-002 |
| impulses, for example a wind charge, that cancel the fall damage of the launch | the farm has no impulse | DMG-FALL-001 |
| the death of a player: respawn, the death screen, `player_combat_kill` | the farm player does not die | DMG-DTH-001 |
| the natural regeneration of a player, food and exhaustion | `food.md` does not exist yet | DMG-NET-005 |
| game events (`entity_damage`, `entity_die`) | no farm block hears them | DMG-DTH-002 |
| the scoreboard criteria of damage and death | no farm mechanic uses them | DMG-DTH-001 step 1 |

## 13. Intentional deviations

### DMG-DEV-001: Crowd deaths in the order of the order keys

- **Rule:** DMG-CRW-002.
- **Reference:** the reference ticks the entities in the order of its tick list. A death removes
  the entity from the count of each entity that ticks later in the same tick.
- **Visible to the client:** yes, as the choice of the entity that dies. The number of deaths of
  a crowd in one tick is the same. The tick of a death can differ, as PHY-DEV-004 states.
- **Test:** a differential test compares the number of crowd deaths and the distribution of the
  ticks of the deaths. It does not compare which entity dies.

## 14. Unresolved

- **U1.** No artefact gives the hurt sound, the death sound or the fall sounds of each entity type.
  DMG-NET-007 gives the values of the farm. A later document must give the table.
- **U2.** No artefact gives the fire-immune property. Appendix A gives the values that a vanilla
  server showed (D8). The corpus gives `fall_damage_immune` as a tag.
- **U3.** Some types have an own hit procedure (section 12). D8 shows `armor_stand`, `squid`,
  `glow_squid` and `ender_dragon` as immune to each type, because their procedures differ. Appendix A
  marks them.

## 15. Experiments

Each experiment ran on a vanilla dedicated server of `26.4-snapshot-7`, through its console. The
world was flat, with `difficulty hard`, `spawn_mobs false` and `pause-when-empty-seconds=-1`.
`tick freeze` and `tick step 1` gave single ticks. `data get entity <uuid> <path>` read the
values; a UUID selector also reads an entity in the dying state.

| Id | Steps | Rules |
|----|-------|-------|
| D1 | Build a shaft of glass of 1 × 1 blocks on `honey_block`, `stone` and `hay_block`. Summon a zombified piglin `d` blocks above the top of the floor, for `d` = 3, 4, 24, 50, 78 and 78.0625. Wait 8 seconds. Read `Health`. | DMG-FALL-002, DMG-FALL-004 |
| D2 | Set `fall_damage` to false. Drop a zombified piglin 10 blocks onto stone. Read `Health`. | DMG-FALL-002, DMG-PLR-001 |
| D3 | Freeze the tick. Hit a zombified piglin, a zombie and three pigs with `/damage` of `cactus` and `cramming`. Give one pig `absorption` 0 and one pig `resistance` 0 before. Read `Health`, `HurtTime` and `AbsorptionAmount` after each tick. | DMG-HIT-003, DMG-HIT-004, DMG-HIT-006, DMG-RED-001, DMG-RED-002, DMG-RED-004 |
| D4 | Freeze the tick. Kill a zombified piglin with `/damage … 100 minecraft:generic`. Step one tick at a time. Read `DeathTime` and `Health` by UUID, and test `@e`. | DMG-DTH-003, DMG-DTH-004 |
| D5 | Put a pig, a zombie and a zombified piglin in cells of 1 × 1 blocks on `magma_block`. Step 130 ticks. Read `Health` and `HurtTime` after each tick. | DMG-HOT-001, DMG-HIT-006 |
| D6 | Put 25 or 30 zombified piglins in a cell of 1 × 1 blocks on `honey_block`. Step 200 ticks. Read the `Health` of each entity after each tick. Repeat with `max_entity_cramming` 26 and 0, and with `NoAI`. | DMG-CRW-001, DMG-CRW-002, DMG-CRW-003 |
| D7 | Repeat D6 with `Health:5f` for 40 ticks, and with `max_health` 1000 and `Health:1000f` for 400 ticks. | DMG-CRW-002, DMG-CRW-003 |
| D8 | For each entity type, summon it with `NoAI` and `NoGravity`. Read the base value of each attribute of DMG-ATT-002 with `/attribute … base get`, and `Health`. Do a `/damage` of `fall`, `hot_floor`, `cramming` and `player_attack` with the amounts 0.001, 0.002, 0.003 and 0.004, so that the cooldown accepts each one. Record each reply "invulnerable". | DMG-ATT-001, DMG-ATT-003, Appendix A |
| D9 | Join the server with an offline client that answers only the keep-alive, the teleport and the chunk batches. Run D1 (13 blocks onto honey), D5 for a pig, the kills and the hits of D3 near the player, and hits on the player. Record the clientbound packets 1, 25, 34, 43, 48, 69, 78, 101, 103, 106 and 119. | section 9 |

These checks are still open:

| Id | Steps | Rules |
|----|-------|-------|
| D10 | Drop a zombie onto magma from 10 blocks. Record the type of the damage event. | DMG-ORD-001 |
| D11 | Hit a player with a knockback type and a source entity. Record `hurt_animation`. | DMG-NET-006 |
| D12 | Drop a mob onto a bed, a slime block, powder snow and a dripstone tip. Read `Health`. | DMG-FALL-004 |
| D13 | Repeat D9 and decode each `sound` packet: the sound event id, the source and the position. | DMG-NET-007 |
| D14 | With `tools/vanilla-client`, watch a zombified piglin fall onto honey, take crowd hits and die, with the server of this project. The client must show the hurt, the death and the removal, with no disconnect. | section 9 |

The owner has not yet confirmed the acceptance of the EULA for new runs. D10 to D14 wait for it.

## 16. Captured results

**D1.** Health after the fall:

| `d` | `honey_block` | `stone` | `hay_block` |
|-----|---------------|---------|-------------|
| 3 | 20 | 20 | 20 |
| 4 | 20 | 19 | 20 |
| 24 | 16 | dead | 16 |
| 50 | 11 | dead | 11 |
| 78 | 5 | dead | 5 |
| 78.0625 | 5 | dead | 5 |

**D2.** The piglin had 13 health after a fall of 10 blocks with `fall_damage` false.

**D3.** Values after each hit, read in the same frozen tick. The tick count starts at the first hit.

| Tick | Hit | Zombified piglin, armor 2 | Pig |
|------|-----|---------------------------|-----|
| 0 | `cactus` 5 / 2 | 15.08, hurt time 10 | 8.0 |
| 2 | `cactus` 3 / 1 | rejected | rejected |
| 3 | `cactus` 8 / 3 | 12.139999, hurt time 7 | 7.0 |
| 9 | `cactus` 8 / 3 | rejected | rejected |
| 10 | `cactus` 8 / 3 | 4.267999, hurt time 10 | 4.0 |
| 20 | `cramming` 1 / 1 | 3.2679992 | 3.0 |

The zombie lost 5.904 to `cactus` 6. The pig with `absorption` 0 (4 absorption) had 9.0 health and
0 absorption after `cactus` 5. The pig with `resistance` 0 had 6.0 health after `cactus` 5.

**D4.** `DeathTime` was `k` after `k` ticks, for `k` from 0 to 19. After 20 ticks the UUID found no
entity. `@e` did not select the entity in any of these ticks.

**D5.** The pig lost 1.0 in ticks 2, 12, 22 and so on, and was gone after its tenth hit. The zombie
lost 0.94 at the same ticks: 19.06, 18.119999, 17.179998. The zombified piglin kept 20.0.

**D6, D7.** The rows of DMG-CRW-002, and:

- every crowd hit was 6.0;
- with `max_entity_cramming` 26 and 0, a crowd of 25 had no hit in 120 ticks;
- with `NoAI`, 4 trials gave one death each, at ticks 31 to 33;
- with 1000 health, 2328 hits gave a mean gap of 12.96 ticks. The gaps 10, 11, 12, 13 and 14 had
  566, 424, 322, 220 and 175 hits, a ratio near 3/4. The mean tick of the first hit was 4.27.

**D8.** Appendix A.

**D9.** Hex of the packet bodies after the packet id. The piglin was entity 2, the pig entity 3.

| Event | Packet | Body |
|-------|--------|------|
| a fall of 13 blocks onto honey | `entity_event` | `00000002 36` |
| | `damage_event` | `02 0a 00 00 00` |
| | `set_entity_data` | `02 09 03 41900000 ff` (18.0) |
| | `set_entity_motion` | `02 …` |
| a hit of magma on the pig | `damage_event` | `03 14 00 00 00`, each 10 ticks |
| a crowd hit | `damage_event` | `02 04 00 00 00` |
| a partial hit of 8 after 6 | `set_entity_data` only | `02 09 03 40800000 ff` (4.0) |
| the death of the pig | `entity_event` | `00000003 03` |
| | `set_entity_data` | `03 06 14 07 09 03 00000000 ff` |
| 20 ticks later | `entity_event`, `remove_entities` | `00000003 3c`, `01 03` |
| a crowd hit of 3 on the player | `damage_event`, `set_health` | `01 04 00 00 00`, `41880000 14 40a00000` |

No `hurt_animation` arrived for `cramming`, `cactus` and `magic`. Each of them has
`no_knockback`.

## 17. Implementation

### 17.1 What the engine does now

- `crates/mcrs_minecraft_engine_physics/src/entity_pass.rs`, `tick_tile`: the pass calls
  `actions.clear()` after `tick_entity`. Thus it drops each `BlockAction::Step` and
  `BlockAction::Fall` that the kernel gives.
- The same function ignores `PushOutcome::crowd_damage` of `push_entity`. It uses the count only
  to wake the entity.
- The pass uses `PushRules::default()`, so `max_entity_cramming` is always 24.
- No row has a health, a damage cooldown, a hurt time or a death time. The grid flag `PUSHABLE`
  does not test the alive state.
- `max_entity_cramming` has no runtime value: `mcrs_minecraft_game_rule` holds only definitions,
  and the save reads only `advance_time`.
- The muxer `EntityEvent` has no event for `damage_event` or `entity_event`.

### 17.2 Order of work

1. The state of DMG-DTH-005 in `Living`. A new row has the max health. No save names: G2 saves no
   mob.
2. The hit procedure of section 3 and the reductions of section 4, as a function of
   `mcrs_minecraft_physics` with no Bevy. Its input is the row state, the type data and the damage
   type with its tags.
3. The fall action and the step action: apply them in the pass (DMG-ORD-002) with the rows of
   DMG-FALL-004 that G2 needs and DMG-HOT-001. The pass still drops the other actions, and the
   block parts of these actions, with `actions.clear()`, until a consumer in `WorldWrite` exists.
4. The crowd hit: the resource of DMG-CRW-001, and the fix-up of DMG-CRW-002.
5. Death: the dying state, the death time, the in-pass removal of DMG-DTH-003, the rest rule and
   the dirty tile, and the alive test of the flag `PUSHABLE`.
6. Packets: `EntityEvent::Signal` with `damage_event` and the entity events 3, 54 and 60, the
   entity data of the health and the pose from the row, and the velocity mark.
7. Sounds and particles (DMG-NET-007), after the rest.

Not in G2: section 10 and `set_health`; DMG-FALL-003; the hooks of DMG-HIT-002 step 1, DMG-HIT-005
steps 1 and 5, and DMG-DTH-002; the alive test of entity selectors (no selector reaches mobs yet).

### 17.3 Exit test

From `gold-farm.md`, section 4, step G2:

1. A zombified piglin that falls 78 blocks onto `honey_block` has 5 health. Use the shaft of D1,
   or the farm save with a piglin summoned at `-317.5 253 -609.5` over the honey at `y` 174.
2. Crowds of 25 die at the vanilla rate. `summon` of G1 takes no `Health` and no attribute, so the
   test builds each crowd as rows of the engine harness (`test_scene.rs`) in a cell of 1 × 1 blocks
   on `honey_block`. Each trial starts the birth clock at another value, so the keyed draws differ.
   Tick 1 is the first `Simulate` after the rows exist. Run at least 16 trials of each crowd. The
   test passes when all these conditions are true:
   - a crowd of 25 with 20 health loses exactly one entity in each trial, in tick 31 to 40. The
     vanilla range was 31 to 35; tick 36 or later has a chance of about 1% for one trial, and
     tick 41 or later is near 0;
   - a crowd of 30 with 20 health loses exactly 6 entities;
   - a crowd of 25 with 5 health loses exactly one entity, in tick 1; a crowd of 30 loses 6;
   - over at least 2000 hits with 1000 health, the mean gap is 13 ± 0.3 ticks, and the gap is
     never less than 10.

### 17.4 Data tables

| Table | Source |
|-------|--------|
| damage types and their network ids | the corpus `damage_type/`, the registry (REG-ENTRY-004, REG-NET-006) |
| the damage tags of 0.6 and of DMG-HIT-001 | the corpus `tags/damage_type/` |
| `fall_damage_immune` | the corpus `tags/entity_type/` |
| the base attributes, fire-immune, the own hit procedures | the type table of `entity-tracking.md` (TRK-TYPE-001), with the values of Appendix A. That table gains the fields `armor`, fire-immune and the hit kind (normal or own) |
| the fall action of each block | DMG-FALL-004 of this document |
| the magma step action and the fall multiplier | the named block data of `mcrs_minecraft_collision_table` (DMG-FALL-004) |
| `max_entity_cramming` | the resource of DMG-CRW-001 |
| the hurt and death sounds | DMG-NET-007; U1 for the other types |

### 17.5 Tests

- The hit procedure, as a kernel test with the values of D3: the f32 bits of 15.08, 12.139999,
  4.267999 and 3.2679992, the zombie value 14.096, absorption and `resistance`. These values catch
  a wrong order of the reductions and a cooldown with `≥ 10` instead of `> 10`.
- The fall damage of the table of DMG-FALL-002, with the values of D1.
- The crowd order of DMG-CRW-002 with crowds of 5 health: 25 give one death and 30 give six. A
  crowd hit that ignores the deaths of the tick gives about six deaths for 25.
- A wire test for `damage_event`, the entity events and the entity data of the death against the
  hex of D9. It does not compare `sound` packets.
- The exit test of 17.3 in the engine.

## Appendix A: base attributes and immunities of the living types

The values come from D8. The columns `fall` and `fire` state the immunity to the damage types `fall`
and `hot_floor`: `yes` for immune. Only the `own` types are immune to `cramming`. `own` marks a type whose own hit
procedure rejected each type in D8 (U3). The attribute `armor_toughness` and `max_absorption` are 0
for each type. A type that is not in the table uses the defaults of DMG-ATT-002 and no immunity.

| Type | `max_health` | `armor` | `safe_fall_distance` | `fall_damage_multiplier` | `fall` | `fire` |
|------|-------------|---------|----------------------|--------------------------|--------|--------|
| `allay` | 20 | 0 | 3 | 1 | yes | — |
| `armadillo` | 12 | 0 | 3 | 1 | — | — |
| `armor_stand` | 20 | 0 | 3 | 1 | own | own |
| `axolotl` | 14 | 0 | 3 | 1 | — | — |
| `bat` | 6 | 0 | 3 | 1 | yes | — |
| `bee` | 10 | 0 | 3 | 1 | yes | — |
| `blaze` | 20 | 0 | 3 | 1 | yes | yes |
| `bogged` | 16 | 0 | 3 | 1 | — | — |
| `breeze` | 30 | 0 | 3 | 1 | yes | — |
| `camel` | 32 | 0 | 6 | 0.5 | — | — |
| `camel_husk` | 32 | 0 | 6 | 0.5 | — | — |
| `cat` | 10 | 0 | 3 | 1 | yes | — |
| `cave_spider` | 12 | 0 | 3 | 1 | — | — |
| `chicken` | 4 | 0 | 3 | 1 | yes | — |
| `cod` | 3 | 0 | 3 | 1 | — | — |
| `copper_golem` | 12 | 0 | 3 | 1 | yes | — |
| `cow` | 10 | 0 | 3 | 1 | — | — |
| `creaking` | 1 | 0 | 3 | 1 | — | — |
| `creeper` | 20 | 0 | 3 | 1 | — | — |
| `dolphin` | 10 | 0 | 3 | 1 | — | — |
| `donkey` | 53 | 0 | 6 | 0.5 | — | — |
| `drowned` | 20 | 2 | 3 | 1 | — | — |
| `elder_guardian` | 80 | 0 | 3 | 1 | — | — |
| `enderman` | 40 | 0 | 3 | 1 | — | — |
| `endermite` | 8 | 0 | 3 | 1 | — | — |
| `ender_dragon` | 200 | 0 | 3 | 1 | own | own |
| `evoker` | 24 | 0 | 3 | 1 | — | — |
| `fox` | 10 | 0 | 5 | 1 | — | — |
| `frog` | 10 | 0 | 3 | 1 | — | — |
| `frostbite` | 24 | 2 | 3 | 1 | — | — |
| `ghast` | 10 | 0 | 3 | 1 | yes | yes |
| `giant` | 100 | 0 | 3 | 1 | — | — |
| `glow_squid` | 10 | 0 | 3 | 1 | own | own |
| `goat` | 10 | 0 | 3 | 1 | — | — |
| `guardian` | 30 | 0 | 3 | 1 | — | — |
| `happy_ghast` | 20 | 0 | 3 | 1 | yes | — |
| `hoglin` | 40 | 0 | 3 | 1 | — | — |
| `horse` | 53 | 0 | 6 | 0.5 | — | — |
| `husk` | 20 | 2 | 3 | 1 | — | — |
| `illusioner` | 32 | 0 | 3 | 1 | — | — |
| `iron_golem` | 100 | 0 | 3 | 1 | yes | — |
| `llama` | 53 | 0 | 6 | 0.5 | — | — |
| `magma_cube` | 1 | 3 | 3 | 1 | yes | yes |
| `mannequin` | 20 | 0 | 3 | 1 | — | — |
| `mooshroom` | 10 | 0 | 3 | 1 | — | — |
| `mule` | 53 | 0 | 6 | 0.5 | — | — |
| `nautilus` | 15 | 0 | 3 | 1 | — | — |
| `ocelot` | 10 | 0 | 3 | 1 | yes | — |
| `panda` | 20 | 0 | 3 | 1 | — | — |
| `parched` | 16 | 0 | 3 | 1 | — | — |
| `parrot` | 6 | 0 | 3 | 1 | yes | — |
| `phantom` | 20 | 0 | 3 | 1 | yes | — |
| `pig` | 10 | 0 | 3 | 1 | — | — |
| `piglin` | 16 | 0 | 3 | 1 | — | — |
| `piglin_brute` | 50 | 0 | 3 | 1 | — | — |
| `pillager` | 24 | 0 | 3 | 1 | — | — |
| `polar_bear` | 30 | 0 | 3 | 1 | — | — |
| `pufferfish` | 3 | 0 | 3 | 1 | — | — |
| `rabbit` | 3 | 0 | 3 | 1 | — | — |
| `ravager` | 100 | 0 | 3 | 1 | — | — |
| `salmon` | 3 | 0 | 3 | 1 | — | — |
| `sheep` | 8 | 0 | 3 | 1 | — | — |
| `shulker` | 30 | 0 | 3 | 1 | yes | yes |
| `silverfish` | 8 | 0 | 3 | 1 | — | — |
| `skeleton` | 20 | 0 | 3 | 1 | — | — |
| `skeleton_horse` | 15 | 0 | 6 | 0.5 | — | — |
| `slime` | 1 | 0 | 3 | 1 | — | — |
| `sniffer` | 14 | 0 | 3 | 1 | — | — |
| `snow_golem` | 4 | 0 | 3 | 1 | yes | — |
| `spider` | 16 | 0 | 3 | 1 | — | — |
| `squid` | 10 | 0 | 3 | 1 | own | own |
| `stray` | 20 | 0 | 3 | 1 | — | — |
| `strider` | 20 | 0 | 3 | 1 | — | yes |
| `sulfur_cube` | 4 | 0 | 3 | 1 | — | — |
| `tadpole` | 6 | 0 | 3 | 1 | — | — |
| `trader_llama` | 53 | 0 | 6 | 0.5 | — | — |
| `tropical_fish` | 3 | 0 | 3 | 1 | — | — |
| `turtle` | 30 | 0 | 3 | 1 | — | — |
| `vex` | 14 | 0 | 3 | 1 | — | yes |
| `villager` | 20 | 0 | 3 | 1 | — | — |
| `vindicator` | 24 | 0 | 3 | 1 | — | — |
| `wandering_trader` | 20 | 0 | 3 | 1 | — | — |
| `warden` | 500 | 0 | 3 | 1 | — | yes |
| `witch` | 26 | 0 | 3 | 1 | — | — |
| `wither` | 300 | 4 | 3 | 1 | yes | yes |
| `wither_skeleton` | 20 | 0 | 3 | 1 | — | yes |
| `wolf` | 8 | 0 | 3 | 1 | — | — |
| `zoglin` | 40 | 0 | 3 | 1 | — | yes |
| `zombie` | 20 | 2 | 3 | 1 | — | — |
| `zombie_horse` | 25 | 0 | 6 | 0.5 | — | — |
| `zombie_nautilus` | 15 | 0 | 3 | 1 | — | — |
| `zombie_villager` | 20 | 2 | 3 | 1 | — | — |
| `zombified_piglin` | 20 | 2 | 3 | 1 | — | yes |

The `player` type is not in D8. A player has `max_health` 20, `armor` 0 and the defaults of
DMG-ATT-002.
