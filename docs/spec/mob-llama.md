# The llama: server behaviour, goals, caravans and spit

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Entity types | `minecraft:llama`; `minecraft:llama_spit` (the projectile) |
| Data | the data pack of the server jar; `crates/mcrs_minecraft_model/tests/fixtures/vanilla/entity_types.json` |

This document states the server behaviour of the llama. It covers these subjects:

- the type data, the attributes, the variants and the strength;
- the changes to age, love and breed against the farm animals of `mob-animals.md`;
- the work of each tick that comes from the horse family;
- the goal list and the target list, the caravan, the spit and the defence against wolves;
- the drops, the save, and the cost of each world query for a parallel tick.

The rules state results, not algorithms. Each rule has a fidelity class (section 0.3). Section 14
states the world queries of the reference, their cadence, and the shortcuts that keep the result.
The aim is a parallel tick of 50,000 mobs with a small cost for each mob.

`mob-animals.md` (ANIM) states the farm animals. `mob-goals.md` (GOAL) states the goal framework and
the goals of passive animals. `mob-combat-goals.md` (CGOAL) states the target goals and the attack
goals. This document cites those rules, and states only the changes for the llama.

## 0. Conventions

### 0.1 Terms

- **Server**, **tick**, **entity**, **living entity**, **position**, **velocity**, **box**, **f32**,
  **f64**: as `entity-physics.md`, section 0.1, defines them.
- **Mob**, **goal**, **control flag**, **full evaluation**, **evaluation unit**, **reflex**,
  **decision**, **issue tick**, **snapshot**, **proposal**, **grant**, **pair action**, **order
  key**, **idle time**, **follow range**: as `entity-ai-timing.md`, section 0.1, defines them.
- **Goal list**, **target list**, **priority**, **examined**, **start condition**, **continue
  condition**, **start action**, **stop action**, **goal tick**, **per-tick goal**, **evaluation
  goal**, **feet distance**, **eye point**, **order key**, **qualified player**, **gate**,
  **eligible**, **wait**, **busy**, **wake event**, **keyed random value**: as `mob-goals.md`,
  section 0.1, defines them.
- **Age**, **baby**, **adult**, **love time**, **in love**, **love cause**, **age lock**: as
  `mob-animals.md`, section 0.1, defines them.
- **Llama**: an entity of the type `minecraft:llama`. In this document, "llama" never means another
  type.
- **Trader llama**: an entity of the type `minecraft:trader_llama`. It is out of scope (section 13).
  Some queries of this document also accept it.
- **Llama kind**: the set of the two types `minecraft:llama` and `minecraft:trader_llama`.
- **Spit**: an entity of the type `minecraft:llama_spit`.
- **Variant**: one of the four coat colours `creamy`, `white`, `brown` and `gray`, with the ids 0 to
  3.
- **Strength**: an integer from 1 to 5. It gives the number of inventory columns of a chest. It also
  sets the strength of a child (LLAMA-BREED-001).
- **Tamed**: a boolean state of a llama. Only a rider can tame a llama (LLAMA-HOOK-003).
- **Temper**: an integer from 0 to 30. Food and a rider increase it (LLAMA-FEED-001).
- **Bred flag**: a boolean state of a llama. Only the save sets it (LLAMA-TICK-003).
- **Eating flag**: a boolean state of a llama. Only the save sets it (LLAMA-TICK-002).
- **Leashed**: an entity is leashed when it has a leash holder of any kind: a player, a fence knot
  or another entity.
- **Head link**: the llama-kind entity that a llama follows in a caravan, or none.
- **Tail link**: the llama-kind entity that follows a llama in a caravan, or none.
- **In a caravan**: an entity is in a caravan when its head link is not none.
- **Has a tail**: an entity has a tail when its tail link is not none.
- **Anchor**: a llama-kind entity that a llama can select to join. LLAMA-CAR-002 defines the two
  kinds of anchor.
- **Chain above `m` within `n` links**: let `h1` be the head link of `m`, `h2` the head link of
  `h1`, and so on. The chain reaches a leashed entity within `n` links when two conditions are true.
  Some `hi` with `i ≤ n` is leashed. Each of `m`, `h1`, ..., `h(i−1)` is in a caravan.
- **Caravan speed**: the speed modifier that the caravan goal of one llama uses now.
- **Far counter**: an integer of the caravan goal of one llama, in evaluation units.
- **Spit flag**: a boolean state of a llama. A spit sets it (LLAMA-TGT-002).
- **Attack target**: the living entity that the target list of a mob selects (CGOAL-NEAR,
  CGOAL-HURT).
- **Attacker record**: the living entity that last hit the mob, and the game time of that hit
  (DMG-HIT-005, step 1). CGOAL-HURT reads it.
- **Untamed wolf**: an entity of the type `minecraft:wolf` without an owner.

### 0.2 Provenance and confidence

The labels of `mob-goals.md`, section 0.2, apply: `source_suggested`, `black_box_confirmed`,
`source_suggested_black_box_confirmed`, `documentation`, `experiment`, `design` and `unresolved`. A
`black_box_confirmed` label of this document refers to the type artefact. No experiment of section
16 ran. Thus the confidence of a `source_suggested` rule is at most medium, except for constants of
the data pack and of the type. Each rule states its kind as `docs/spec/README.md` defines the kinds.

### 0.3 Fidelity

The three classes of `mob-animals.md`, section 0.3, apply: `exact`, `distribution` and `envelope`. A
rule line has the form `**LLAMA-X-001** (provenance, kind, fidelity: class, confidence: level).`
When the parts of one rule have different classes, the rule states the class of each part. Section
14 gives a class to each shortcut. A shortcut is **exact** when it gives the same state in each tick
as the reference rule, with the same keyed random values.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `LLAMA-TYPE` | type data and attributes |
| `LLAMA-DATA` | entity data on the wire |
| `LLAMA-VAR` | variant and strength |
| `LLAMA-SPAWN` | the first spawn setup and the spawn rules |
| `LLAMA-FEED` | food, temper, age and love changes against ANIM |
| `LLAMA-BREED` | the result of a breed |
| `LLAMA-TICK` | the work of each tick that is not a goal |
| `LLAMA-GOAL` | the goal list, the target list and the parameter changes |
| `LLAMA-CAR` | the caravan |
| `LLAMA-TGT` | the target goals: revenge and wolves |
| `LLAMA-SPIT` | the ranged attack and the spit projectile |
| `LLAMA-DROP` | drops and experience |
| `LLAMA-SAVE` | the save |
| `LLAMA-HOOK` | subjects out of scope, with their data hooks |
| `LLAMA-COST` | the cost of the world queries, and the parallel tick |
| `LLAMA-DEV` | intentional deviations |

### 0.5 Scope

In scope: the server state and the server behaviour of a llama that no player rides, and of its
spit. The table gives the subjects that other documents own.

| Subject | Where |
|---------|-------|
| The goal framework, the controls, the queries of passive goals | `mob-goals.md` |
| The target goals and the attack goals in general | `mob-combat-goals.md` |
| Age, love and the breed steps that are common to animals | `mob-animals.md` |
| Health, the hit, the cooldown, the fall damage | `entity-damage.md` |
| The path search, the stored target, the recompute | `pathfinding.md` |
| The walker motion, the step up, the push | `entity-physics.md` |
| The spawn bundle, the tracker, the entity data codec | `entity-tracking.md` |
| The client model and textures of a llama | a later client specification |
| The chest, the inventory, the decor carpet, a rider, the taming, the trader llama | hooks only (section 13) |
| The leash, the leash holder, the home of a leash | the leash specification (planned); `mob-goals.md`, section 6.7 |
| The wolf, and its avoidance of llamas | the wolf specification (planned); CGOAL-AVOID |

## 1. Type data

**LLAMA-TYPE-001** (`black_box_confirmed`, semantic, fidelity: exact, confidence: high). The
dimensions of a llama:

| State | Width × height | Eye height | Passenger attachment point |
|-------|----------------|------------|----------------------------|
| adult | `0.9F × 1.87F` | `1.7765F` | `(0, 1.37, −0.3)` |
| baby | `0.45F × 0.935F` | `0.88825F` | `(0, 0.81F, −0.15F)` |

The baby values are the adult values times `0.5F`. The baby attachment point is the point `(0,
1.62F, −0.3F)` times `0.5F`. The `scale` attribute multiplies each value, as ANIM-TYPE-001 states.
The update interval is 3 ticks. The tracking range is 10 chunks. A llama is not fire immune. The
rider point does not depend on a stand pose, because a llama cannot stand (LLAMA-TICK-002).

Provenance of the adult values, the interval and the range: the type artefact. The baby values and
the attachment points are `source_suggested`.

**LLAMA-TYPE-002** (`source_suggested`, semantic, fidelity: exact, confidence: high). The base
attributes of a llama. The values in the column "from" come from the horse family; the llama changes
two of them.

| Attribute | Base | From |
|-----------|------|------|
| `max_health` | 53.0 before the first spawn setup; LLAMA-SPAWN-001 changes it | horse family |
| `movement_speed` | `0.175F` | changed for the llama and the donkey family |
| `jump_strength` | 0.5 | changed for the llama and the donkey family |
| `step_height` | 1.0 | horse family |
| `safe_fall_distance` | 6.0 | horse family |
| `fall_damage_multiplier` | 0.5 | horse family |
| `follow_range` | 16.0 | mob default |
| `tempt_range` | 10.0 | animal default |
| each other attribute | the default of the attribute | living entity |

Thus a llama walks over a full block with no jump (`entity-physics.md`). Its follow range is 16, as
for a pig. The value 16 sets the range of the wolf target (LLAMA-TGT-003) and the range of the
revenge target.

**LLAMA-TYPE-003** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The path
data of a llama for `pathfinding.md`:

- The length `L` of each request is `max(follow range, 40)`. With the base follow range, `L` is
  40. A pig has `L = 16`.
- Thus the search region of PATH-REQ-003 has `r = 48`, and the search can visit up to `floor(L × 16)
  = 640` nodes.
- The malus rows and the walk value are those of the animals (ANIM-TYPE-005, GOAL-ANI-005,
  GOAL-POS-004).
- The head turn speed is 10, the pitch limit is 40 and the head yaw limit is 75 (GOAL-ANI-006).

**LLAMA-TYPE-004** (`source_suggested`, semantic, fidelity: exact, confidence: high). The mob
category is `creature`. A llama does not despawn because of the distance to a player or the idle
time (ANIM-SAVE-001). The maximal group of one spawn event in a chunk is 6, not the default 4.

**LLAMA-TYPE-005** (`source_suggested`, semantic, fidelity: exact for the sounds, distribution for
the pitch, confidence: medium). The sounds of a llama:

| Event | Sound | Volume |
|-------|-------|--------|
| ambient | `entity.llama.ambient` | 0.8 |
| hurt | `entity.llama.hurt` | 0.8 |
| death | `entity.llama.death` | 0.8 |
| step | `entity.llama.step` | 0.15, pitch 1.0 |
| eat (LLAMA-FEED-001) | `entity.llama.eat` | 1.0 |
| angry (LLAMA-FEED-002) | `entity.llama.angry` | 0.8 |
| spit (LLAMA-SPIT-002) | `entity.llama.spit` | 1.0 |
| chest (LLAMA-HOOK-001) | `entity.llama.chest` | 1.0 |

The pitch of each sound except the step is `1.0 + 0.2 × (u1 − u2)`, and `1.5 + 0.2 × (u1 − u2)` for
an ambient, hurt, death or angry sound of a baby. The source is `neutral`. A silent llama makes no
sound. The ambient sound follows ANIM-TICK-004 with the reset value −400, not −120. Thus two ambient
sounds are at least 401 ticks apart.

**LLAMA-TYPE-006** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The leash
point of a llama is `(0, 0.75 × e, w / 2)` from its position, before the rotation by the yaw. `e` is
the eye height and `w` the width of the current dimensions. A llama has one leash point. The leash
specification uses this point.

## 2. Entity data

**LLAMA-DATA-001** (`black_box_confirmed`, wire, fidelity: exact, confidence: high). The chain of a
llama is: entity, living entity, mob, ageable mob, horse family, chest family, llama. Indices 0 to
17 are those of TRK-DATA-006 for each animal. The other entries:

| Index | Serializer | Meaning | Default |
|-------|------------|---------|---------|
| 18 | 0 (byte) | horse flags: bit `0x02` tamed, `0x08` bred, `0x10` eating, `0x20` standing, `0x40` mouth open | 0 |
| 19 | 8 (boolean) | the llama has a chest | false |
| 20 | 1 (variable-length integer) | strength | 0 |
| 21 | 1 (variable-length integer) | variant id | 0 (`creamy`) |

The type artefact confirms each index and each serializer. A llama never sets the bits `0x20` and
`0x40` (LLAMA-TICK-002). The entry 20 keeps its default 0 only for a llama that has no first spawn
setup, no load and no breed. Each of these three sets a value from 1 to 5.

## 3. Variant and strength

**LLAMA-VAR-001** (`source_suggested`, semantic, fidelity: exact, confidence: high). The variant is
one of four values: 0 `creamy`, 1 `white`, 2 `brown`, 3 `gray`. The default is `creamy`. An id
outside 0 to 3 changes to the nearest end of the range. The variant is not a registry entry. The
item component `minecraft:llama/variant` of a spawn egg sets it.

**LLAMA-VAR-002** (`source_suggested`, semantic, fidelity: exact, confidence: high). A set of the
strength clamps the value to the range 1 to 5. A load with no strength key thus gives the strength
1.

## 4. The first spawn setup and the spawn rules

**LLAMA-SPAWN-001** (`source_suggested`, semantic, fidelity: distribution, confidence: medium). The
first spawn setup of a llama sets these values. ANIM-SPAWN-001 states the ways of birth that apply
it.

| Value | Rule |
|-------|------|
| strength | with the probability 0.04, a uniform integer from 1 to 5; else a uniform integer from 1 to 3 |
| variant | LLAMA-SPAWN-002 |
| baby | LLAMA-SPAWN-002 |
| `max_health` base | `15 + a + b`, with `a` uniform from 0 to 7 and `b` uniform from 0 to 8 |
| follow range bonus, left handed | as ANIM-SPAWN-001 |

Thus the strength is 1, 2 or 3 each with the probability 0.328, and 4 or 5 each with the probability
0.008. The maximal health is from 15 to 30. A llama without the setup keeps the maximal health 53.
The health becomes equal to the new maximal health at the first tick of the llama.

**LLAMA-SPAWN-002** (`source_suggested`, semantic, fidelity: distribution, confidence: medium). The
first llama of a spawn group (ANIM-SPAWN-002) takes a variant uniform from the four values. Each
later llama of the group takes the same variant. Each later llama is a baby with the probability
0.05, with the age −24000. The first llama is an adult. `summon` makes a group of one llama, so it
never makes a baby with the setup.

**LLAMA-SPAWN-003** (`documentation`, semantic, fidelity: exact, confidence: medium). The spawn
rules of the type are those of the animals (ANIM-SPAWN-003). They state the placement on the ground,
the heightmap `MOTION_BLOCKING_NO_LEAVES`, the block below and the light. The vanilla biomes give
`llama` to the windswept hills biomes with the weight 5 and the group size 4 to 6. They give it to
`savanna_plateau` with the weight 8 and the group size 4. The biome data is the authority.

## 5. Food, temper, age and love

The rules of ANIM-AGE-001, ANIM-AGE-002, ANIM-AGE-006, ANIM-AGE-007, ANIM-LOVE-002 and ANIM-LOVE-003
apply to a llama with no change. This section states the differences.

**LLAMA-FEED-001** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The breeding
food of a llama is `#minecraft:llama_food`: `wheat` and `hay_block` in the vanilla data pack. A
player uses such an item on a llama that is not a vehicle. The llama then does these steps, in this
order:

| Item | Heal | Age increase of a baby | Temper increase | Love |
|------|------|------------------------|-----------------|------|
| `wheat` | 2 | 200 ticks | 3 | never |
| `hay_block` | 10 | 1800 ticks | 6 | when tamed, the age is 0 and the love time is 0 or less |

1. Love: for `hay_block`, when the condition of the table is true, the llama enters love as
   ANIM-LOVE-001, steps 2 to 4, states. The item is used.
2. Heal: when the health is more than 0 and less than the maximal health, add the heal amount. The
   health does not go above the maximum. The item is used.
3. Age: when the llama is a baby without the age lock, add the age increase. The age does not go
   above 0. The fed growth of ANIM-AGE-005 does not change. The item is used.
4. Temper: when the temper is less than 30, and the item is used or the llama is not tamed, add the
   temper increase. The temper does not go above 30. The item is used.
5. When the item is used, consume one item (ANIM-INT-001). Play the eat sound of LLAMA-TYPE-005,
   unless the llama is silent.

When no step uses the item, the interaction does nothing, and no later check runs. Thus an untamed
llama with a temper less than 30 eats each `wheat`. A full adult with the temper 30 does not.

The age increase is a fixed amount, not the 10 % of ANIM-AGE-003. A feed sends no game event.

**LLAMA-FEED-002** (`source_suggested`, semantic, fidelity: exact, confidence: medium). Hook. The
checks of a player interaction with a llama, after the checks of every entity (ANIM-INT-001):

1. A llama that is a vehicle, a tamed adult when the player sneaks, and a baby when the player holds
   `golden_dandelion` skip to step 4.
2. A non-empty item of the breeding food: LLAMA-FEED-001.
3. Any other non-empty item on an untamed llama: the llama plays the angry sound. The interaction
   ends.
4. The checks of the horse family: the age lock (ANIM-AGE-006), the chest and the decor, and the
   mount (section 13).

**LLAMA-FEED-003** (`source_suggested`, semantic, fidelity: exact, confidence: medium). Two entities
can mate (ANIM-FOOD-004 for the llama) when all these conditions are true:

- they are two different entities, and the other entity is of the llama kind;
- each of the two is in love, tamed and an adult;
- each of the two has no passenger and is not a passenger;
- the health of each of the two is equal to its maximal health.

Thus a llama can mate with a trader llama. An untamed llama never enters love through food
(LLAMA-FEED-001), so a wild llama never breeds.

## 6. The result of a breed

**LLAMA-BREED-001** (`source_suggested`, semantic, fidelity: distribution, confidence: medium).
ANIM-BREED-001 applies to the llama. The child has the type of the initiator: a llama when the
initiator is a llama. The initiator is `a` and the partner is `b`. The child takes these values:

1. Strength: a uniform integer from 1 to `max(strength of a, strength of b)`. Then add 1 with the
   probability 0.03. The result clamps to 5 (LLAMA-VAR-002).
2. Variant: the variant of `a` or of `b`, each with the probability 1/2.
3. For each of the three attributes of the table, the base value of LLAMA-BREED-002.

| Attribute | Lower bound `lo` | Upper bound `hi` |
|-----------|------------------|------------------|
| `max_health` | 15.0 | 30.0 |
| `jump_strength` | `0.4000000059604645` | 1.0 |
| `movement_speed` | `0.11249999701976776` | `0.3375000059604645` |

The child has no first spawn setup. It does not take the tamed state, the temper or the chest.

**LLAMA-BREED-002** (`source_suggested`, semantic, fidelity: distribution, confidence: medium). The
base value of a child attribute, from the base values `x` of `a` and `y` of `b`, in f64:

1. Clamp `x` and `y` to `[lo, hi]`.
2. Let `g = 0.15 × (hi − lo)` and `s = |x − y| + 2 × g`.
3. Let `q = (u1 + u2 + u3) / 3 − 0.5`, with three independent uniform values in `[0, 1)`.
4. Let `v = (x + y) / 2 + s × q`.
5. When `v > hi`, the result is `hi − (v − hi)`. When `v < lo`, the result is `lo + (lo − v)`. Else
   the result is `v`.

Thus two parents with the base speed `0.175` give a child speed in `[0.14125, 0.20875]`, with a peak
at `0.175`. The health of the child becomes equal to its maximal health at its first tick.

## 7. The work of each tick that is not a goal

The rules of ANIM-AGE-002, ANIM-LOVE-002 and ANIM-TICK-004 apply. The horse family adds the work of
this section. It runs in each tick while the llama is alive. It runs also when the llama has
`NoAI:1b`, because it is not a part of the AI step.

**LLAMA-TICK-001** (`source_suggested`, semantic, fidelity: distribution, confidence: medium). The
step runs in each tick, after the travel step and the push of the AI step (PHY-ORD-002). A llama
that is alive and not in the dying state heals itself with the probability 1/900. The heal adds 1 to
the health when the health is less than the maximal health. The health does not go above the
maximum.

**LLAMA-TICK-002** (`source_suggested`, semantic, fidelity: exact, confidence: medium). These
counters of the horse family have no observable result for a llama:

- The tail counter starts with the probability 1/200 in each tick and ends after 8 ticks. The server
  sends no value of it. A server does not need to compute it.
- The mouth counter starts only from an eat of another horse type. A llama eats with its own rule
  (LLAMA-FEED-001), so the counter never starts. The bit `0x40` of LLAMA-DATA-001 stays clear.
- The stand counter starts only for a type that can stand. A llama cannot stand. The bit `0x20`
  stays clear. A hit does not make a llama stand.
- A llama does not eat grass. The grass eat of the horse family never starts, so the eating flag
  never becomes true by itself.

The eating flag comes only from the save key `EatingHaystack`. While it is true, the llama is
immobile: its sideways input, its forward input and its jump input are zero (PHY-ORD-002). Only the
pull of a leash (leash specification) and a mount by a player clear it. Thus a llama of `summon`
with `EatingHaystack:1b` does not move by itself until one of these events.

**LLAMA-TICK-003** (`source_suggested`, semantic, fidelity: exact for the condition, envelope for
the search, confidence: low). In each tick after LLAMA-TICK-001, a llama does the follow-mother
check. The check acts only when all these conditions are true:

- the llama is a baby, with the bred flag, and the eating flag is false;
- the llama is not in a caravan.

When it acts, the llama finds the nearest entity of the horse family with the bred flag. The entity
must be in the box of the llama grown by 16, at a feet distance of at most 16. The query does not
check the line of sight. When the squared feet distance to that entity is more than 4, the llama
runs a path search to it with the reach range 0.

The search does not change the navigation of the llama (PATH-REQ-001, step 4). In the reference,
the search stores its target, and a later recompute goes to that target; `pathfinding.md`,
PATH-DEV-006, records it. No vanilla rule sets the bred flag except the save key `Bred`. Thus the
check acts only for a baby of `summon` or of a load with `Bred:1b`.

**LLAMA-TICK-004** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The fall
damage of a llama follows DMG-FALL-002 with the attributes of LLAMA-TYPE-002. A llama takes the
damage only when the fall distance is 6 or more. The fall applies also to its passengers
(DMG-FALL-003). A llama plays no horse land sound.

## 8. Goals

**LLAMA-GOAL-001** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The goal
list of a llama, in this order:

| Priority | Goal | Flags | Kind | Parameters |
|----------|------|-------|------|------------|
| 0 | float (GOAL-FLT-001) | `jump` | per tick | |
| 1 | buck a rider (LLAMA-HOOK-003) | `move` | evaluation | `m = 1.2` |
| 2 | follow a caravan (LLAMA-CAR-003) | `move` | evaluation | initial `m = f32(2.1)` |
| 3 | ranged attack (CGOAL-RANGED, LLAMA-SPIT-001) | `move`, `look` | per tick | `m = 1.25`, interval 40, radius 20 |
| 3 | panic (GOAL-PNC-002) | `move` | evaluation | `m = 1.2` |
| 4 | breed (GOAL-BRD-001) | `move`, `look` | evaluation | `m = 1.0`; LLAMA-GOAL-004 |
| 5 | tempt (GOAL-TMP-001) | `move`, `look` | evaluation | item test: the item is in `#minecraft:llama_tempt_items`; `m = 1.25` |
| 6 | follow an adult (GOAL-FOL-001) | none | evaluation | `m = 1.0`; LLAMA-GOAL-004 |
| 7 | stroll (GOAL-STR-001) | `move` | evaluation | `m = 0.7` |
| 8 | look at a player (GOAL-LKP-001) | `look` | evaluation | range 6 |
| 9 | look around (GOAL-LKR-001) | `move`, `look` | per tick | |

In the vanilla data pack, `#minecraft:llama_tempt_items` holds only `hay_block`. Thus `wheat` feeds
a llama but does not tempt it.

**LLAMA-GOAL-002** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The target
list of a llama, in this order:

| Priority | Goal | Rule |
|----------|------|------|
| 1 | revenge (CGOAL-HURT), with no alert to other mobs | LLAMA-TGT-001 |
| 2 | nearest untamed wolf (CGOAL-NEAR) | LLAMA-TGT-003 |

The target selection runs before the goal selection in each AI step (GOAL-FRM-001). Thus a target
that a target goal selects in a full evaluation can start the ranged attack in the same full
evaluation.

**LLAMA-GOAL-003** (`source_suggested`, semantic, fidelity: exact, confidence: medium). These
relations follow from the tables and GOAL-FRM-002. A player can see them:

- The ranged attack and the panic have the same priority. The ranged attack is first in the list.
  When a full evaluation can start both, the ranged attack wins, and the panic does not start. When
  the panic runs, the ranged attack does not start until the panic stops.
- A llama that a survival player hits thus spits at the player and does not panic (LLAMA-SPIT-007).
  A hit with no living attacker, for example a cactus, gives a panic.
- The caravan goal holds `move` with the priority 2. While it runs, the ranged attack, the panic,
  the breed, the tempt and the stroll do not start. Thus a llama that follows a caravan does not
  spit and does not panic.
- The ranged attack interrupts the breed, the tempt, the stroll and the look around.
- The stroll speed is `0.7 × 0.175F`. On grass the steady speed is about 0.033 blocks in each tick
  (GOAL-CTL-002). A pig strolls at about 0.138.

**LLAMA-GOAL-004** (`source_suggested`, semantic, fidelity: exact, confidence: medium). Two queries
of `mob-goals.md` accept the llama kind, not only the type of the mob:

- `Q-PARTNER` (GOAL-BRD-002): the set is the entities of the llama kind. The filter "can mate" is
  LLAMA-FEED-003.
- `Q-ADULT` (GOAL-FOL-002): the set is the entities of the llama kind. Thus a baby llama follows an
  adult trader llama.

Each other part of the two goals is as `mob-goals.md` states.

## 9. The caravan

### 9.1 State

**LLAMA-CAR-001** (`source_suggested`, semantic, fidelity: exact, confidence: medium). Each llama
keeps a head link and a tail link. A new llama, a loaded llama and a child have neither link. The
save does not keep the links. The client sees no link; it sees only the motion. The caravan goal of
the follower writes both links of a pair:

- A join sets the head link of the follower to the anchor, and the tail link of the anchor to the
  follower.
- A stop of the follower clears its head link. It also clears the tail link of its former head.

No other event changes a link. In particular, the death or the removal of a follower does not clear
the tail link of its head (LLAMA-CAR-008).

### 9.2 The join

**LLAMA-CAR-002** (`source_suggested`, semantic, fidelity: exact, confidence: medium). `Q-CARAVAN`:

- Set: the entities of the llama kind, except the mob, whose boxes intersect the box of the mob
  grown by 9 on the `x` and `z` axes and by 4 on the `y` axis. Snapshot of the issue tick.
- Filter: two kinds of anchor:
  - A: the entity is in a caravan and has no tail;
  - B: the entity is leashed, is not in a caravan, and has no tail.
- Select: the anchor A with the smallest feet distance. When no anchor A exists, the anchor B with
  the smallest feet distance.
- Tie: GOAL-QRY-004. LLAMA-DEV-002 records the vanilla order.
- Cadence: in each full evaluation in which the caravan goal is examined, while the mob is not
  leashed and not in a caravan.

The query does not test that an anchor is alive. A llama in the dying state stays an anchor until
its removal.

**LLAMA-CAR-003** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The caravan
goal holds the flag `move`. It is an evaluation goal. Its start condition is true when all these
conditions are true:

1. The mob is not leashed and is not in a caravan.
2. `Q-CARAVAN` has a result `c`.
3. The squared feet distance from the mob to `c` is 4 or more.
4. `c` is leashed, or the chain above `c` reaches a leashed entity within 8 links.

When the condition is true, the mob joins `c` (LLAMA-CAR-001). The join occurs in the start
condition, so it occurs before the start action. The start action does nothing else.

The selection of step 2 comes before the tests of steps 3 and 4. Thus a near anchor that fails step
3 or step 4 blocks the join, also when another anchor would pass. An anchor A always wins over an
anchor B, also when the anchor B is nearer.

**LLAMA-CAR-004** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The chain
limit. A follower stays in a caravan only while the chain above it reaches a leashed entity within 9
links (LLAMA-CAR-005). The join of step 4 of LLAMA-CAR-003 checks 8 links above the anchor, which is
the same limit. Thus one leashed llama leads at most 9 followers when no follower is leashed. A
follower that a player leashes resets the count for the followers behind it.

### 9.3 Follow and stop

**LLAMA-CAR-005** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The continue
condition of the caravan goal. Let `d²` be the squared feet distance to the head link. The goal
continues when all these conditions are true, and the far rule does not stop it:

1. The mob is in a caravan.
2. The head link is alive: not removed, and with a health more than 0.
3. The chain above the mob reaches a leashed entity within 9 links.

The leash of the mob itself does not count in step 3. The far rule then runs:

1. When `d² > 676` and the caravan speed is 3.0 or less: multiply the caravan speed by 1.2, set the
   far counter to 20, and continue.
2. Else, when `d² > 676` and the far counter is 0: stop.
3. Else, when the far counter is more than 0, decrease it by 1. Continue.

The caravan speed is `2.0999999046325684` at the first start of the goal of a llama. The far rule
gives `2.519999885559082` and then `3.023999862670898`, and then no more increase. Thus a follower
that stays more than 26 blocks from its head stops in the 21st full evaluation after its second
increase. The caravan speed does not decrease while the goal runs.

**LLAMA-CAR-006** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The goal tick
of the caravan goal, in each full evaluation while the goal runs:

1. When the mob is not in a caravan, or its leash holder is a fence knot, do nothing.
2. Let `d` be the feet distance to the head link, in f64. Let `u` be the unit vector from the feet
   point of the mob to the feet point of the head link. When the length of the offset is less than
   `1.0E-5F`, `u` is zero.
3. Request a path to the point `p + u × max(d − 2, 0)`, where `p` is the feet point of the mob, with
   the caravan speed (GOAL-NAV-001).

Thus the follower walks to a point 2 blocks before its head, on the line between them. GOAL-NAV-004
applies: a request to the block of the current path target, while that path is not done, gives the
current path.

**LLAMA-CAR-007** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The stop
action of the caravan goal clears the links (LLAMA-CAR-001). It sets the caravan speed to 2.1 in
f64. LLAMA-DEV-005 records the difference from the first start. The stop does not clear the path.

When a member stops, its own follower fails step 3 of LLAMA-CAR-005 at its next full evaluation.
Thus a caravan breaks from the break point to the end, one link in each full evaluation of the next
follower. When the leashed llama loses its leash, the whole caravan behind it breaks in this way.

**LLAMA-CAR-008** (`source_suggested`, semantic, fidelity: exact, confidence: low). Stale tail
links. A follower that dies or is removed while it is in a caravan does not run its stop action. Its
head keeps the tail link to it. The head thus keeps "has a tail" until the head itself is removed or
unloaded. Thus no new llama can join behind that head. Also, a follower can join an anchor in the
dying state. It stops at its next full evaluation, because step 2 of LLAMA-CAR-005 fails.

**LLAMA-CAR-009** (`source_suggested`, semantic, fidelity: exact, confidence: low). A cycle of links
is possible in rare cases. Example: a leashed llama `a` leads `x`, a player leashes `x`, `y` follows
`x`, and then `a` loses its leash. In the full evaluation before `x` stops, `a` can join `y`. Then
each walk of the chain finds `x` leashed, so the cycle stays. Each chain walk ends after at most 9
links. Thus a cycle never makes a walk endless.

**LLAMA-CAR-010** (`design`, semantic, fidelity: envelope, confidence: medium). The caravan goal is
a decision. The join is a pair action (LLAMA-COST-007). The continue condition reads the links, the
leash states and the health of other llamas from the snapshot. The goal tick reads the position of
the head link from the snapshot. The envelope is the allowed delay of AIT-DEC-003 for the goal
selection.

## 10. Targets and the spit

### 10.1 Revenge

**LLAMA-TGT-001** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The revenge
goal of a llama is CGOAL-HURT with these values:

- No alert to other mobs. No type is ignored.
- The follow distance of the keep check is the follow range, 16 with the base value.
- The keep check needs a line of sight, with the limit of CGOAL-HURT for a target that the mob does
  not see.
- The continue condition has one more step, before the steps of CGOAL-HURT: when the spit flag is
  true, set it to false and stop the goal.

The stop clears the attack target (CGOAL-HURT). The goal starts again only after a new hit, because
the start condition of CGOAL-HURT needs a new time in the attacker record.

**LLAMA-TGT-002** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The spit
flag. A llama keeps a boolean spit flag. It is false for a new or loaded llama. The save does not
keep it.

- Each spit sets it to true (LLAMA-SPIT-002), also a spit at a wolf.
- Only the extra step of LLAMA-TGT-001 sets it to false. That step runs only while the revenge goal
  runs, in a full evaluation.

Thus a spit at an attacker ends the revenge target at the next full evaluation. The flag of a spit
at a wolf stays true. A later revenge start then stops at the next full evaluation, before any spit.
This quirk is vanilla behaviour (section 15).

### 10.2 Wolves

**LLAMA-TGT-003** (`source_suggested`, semantic, fidelity: exact for the query, distribution for the
gate, confidence: medium). The wolf goal of a llama is CGOAL-NEAR with these values:

- Target type: `minecraft:wolf`. Filter: the wolf has no owner.
- Random interval 16 ticks: the gate passes with the chance 1/8 in each full evaluation in which the
  goal is examined (AIT-VAN-011).
- The follow distance `F` is the follow range times 0.25. With the base follow range, `F = 4`. The
  spawn bonus of ANIM-SPAWN-001 changes it by up to about 11 %.
- The start query does not need a reach test. A reach flag of the goal has no effect on the start.
- The keep check does not need a line of sight.

`Q-WOLF`, the start query:

- Set: the wolves whose boxes intersect the box of the mob grown by `F` on each axis. Snapshot of
  the issue tick.
- Filter: no owner; CGOAL-NEAR for an attack target: alive, not a spectator, not invulnerable, not
  on the team of the mob; a feet distance of at most `max(F × v, 2)`, with `v` the visibility factor
  of the wolf (GOAL-QRY-002); a line of sight from the mob (GOAL-QRY-003).
- Select: the smallest distance from the point `(x, e, z)` to the feet point of the wolf. `x` and
  `z` are those of the feet point of the mob, and `e` is the `y` of its eye point.
- Tie: GOAL-QRY-004.
- Cadence: in each full evaluation in which the goal is examined and the gate passes.

Both goals hold the flag `target`, and the revenge goal has the smaller number. Thus the wolf goal
is examined only while the revenge goal does not run.

**LLAMA-TGT-004** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The keep
check of the wolf goal (CGOAL-NEAR) uses the same distance `F`. The goal stops when the squared feet
distance to the wolf is more than `F²`. Thus a llama keeps a wolf target only while the wolf is
within about 4 blocks. The first spit comes 41 ticks after the start of the ranged attack
(LLAMA-SPIT-001). An untamed wolf avoids a llama (CGOAL-AVOID, `mob-wolf.md`). The chance of
that flight reads the strength of the llama. Thus a llama seldom spits at a wild wolf.

### 10.3 The ranged attack

**LLAMA-SPIT-001** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The ranged
attack goal of a llama is CGOAL-RANGED with the speed modifier 1.25 and the radius 20. The minimal
and the maximal interval are both 40 ticks. With equal intervals, the timing is:

- The goal starts with the attack timer −1.
- In the first goal tick, the timer becomes 40.
- In each later goal tick, the timer decreases by 1. When it becomes 0 and the mob sees the target,
  the mob spits (LLAMA-SPIT-002). Then the timer becomes 40.
- When it becomes 0 and the mob does not see the target, no spit occurs. In the next goal tick the
  timer becomes −1, and then 40 again.

Thus the first spit occurs in the 41st goal tick after the start, about 2 seconds. Each later spit
occurs 40 ticks after the one before, while the mob sees the target. The power value of CGOAL-RANGED
does not change a spit.

The goal continues while the attack target is alive, or while the target of the goal is alive and
the path is not done (CGOAL-RANGED). Thus after the revenge goal stops, the ranged attack continues
on the same entity while the mob walks to it.

**LLAMA-SPIT-002** (`source_suggested`, semantic, fidelity: exact for the state, distribution for
the direction and the pitch, confidence: medium). A spit at the target `t` does these steps:

1. Make a spit (LLAMA-SPIT-003) with the llama as its owner. Its position is `(x − k × sin(b), e −
   0.1F, z + k × cos(b))`. Here `(x, z)` is the position of the llama, and `e` is the `y` of its eye
   point. `b` is its body yaw in radians. `k = (w + 1) × 0.5`, with `w` the width of the llama. The
   sine and the cosine are those of the sine table (NUM-TRIG-001).
2. Let `dx` and `dz` be the `x` and `z` of `t` minus those of the llama. Let `dy` be `y(t) + h(t) /
   3` minus the `y` of the spit, with `h(t)` the height of `t`. Let `a` be `sqrt(dx² + dz²) × 0.2F`.
3. The direction is the unit vector of `(dx, dy + a, dz)`. Add three independent values `0.172275 ×
   (u − v)` to its three parts, with `u` and `v` uniform in `[0, 1)`. Multiply the sum by 1.5. The
   result is the velocity of the spit. Its yaw and pitch follow the velocity.
4. Add the spit to the world (a spawn request with a new UUID of the kind `entity`).
5. Play the spit sound of LLAMA-TYPE-005 at the llama, unless the llama is silent.
6. Set the spit flag to true (LLAMA-TGT-002).

The term `a` aims the spit higher for a far target. For an adult, `k = 0.95`.

### 10.4 The spit projectile

**LLAMA-SPIT-003** (`black_box_confirmed`, semantic, fidelity: exact, confidence: high). The type
`minecraft:llama_spit` has the size `0.25F × 0.25F`, the update interval 10 ticks and the tracking
range 4 chunks. It has no loot table. Its entity data is the indices 0 to 7 of TRK-DATA-002, with no
more entries. The add entity packet carries the network id of the owner. The client shows 7 spit
particles from that packet; the server sends no particle.

**LLAMA-SPIT-004** (`source_suggested`, semantic, fidelity: exact, confidence: medium). In each
tick, a spit does these steps:

1. In its first tick, send the game event `projectile_shoot` with the owner.
2. Do the owner check of the projectile specification. Grow the box of the spit along its velocity
   and by 1. The spit leaves its owner when this box touches no box of the owner or of its riders.
3. Do the base entity tick.
4. Do the hit test along the velocity (LLAMA-SPIT-005).
5. Let `q` be the position plus the velocity. Turn the yaw and the pitch toward the velocity.
6. If a block that is not air intersects the box, remove the spit. Else if the spit is in water,
   remove it.
7. Else multiply the velocity by `0.99F`, subtract `0.06` from its `y` part, and set the position to
   `q`.

The spit does not use the collision of the walker. Step 6 tests the box at the position of the start
of the tick.

**LLAMA-SPIT-005** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The hit test
follows the projectile specification. Before the spit leaves its owner, it cannot hit the owner or
an entity in the vehicle chain of the owner. A hit on a block runs the reaction of the block, then
removes the spit. A hit on an entity does these steps:

1. When the owner is a living entity, hit the entity with 1.0 point of the damage type
   `minecraft:spit`. The direct entity is the spit, and the causing entity is the owner. The damage
   scales with the difficulty when the owner is not a player (DMG-SRC-001).
2. When the hit was not rejected, apply the post-attack effects of enchantments.
3. The spit stays. It is not removed.

Thus a spit can pass through an entity and hit a second entity in a later tick. The damage cooldown
(DMG-HIT-003) rejects a second hit on the same entity within 10 ticks. The damage type `spit` is not
in `#minecraft:panic_causes`. Thus a spit does not make an animal panic. A hit by a spit fills the
attacker record with the llama, so a llama that a spit hits spits back (LLAMA-TGT-001).

This rule is a known vanilla quirk (LLAMA-DEV-005). The public documentation does not describe it,
and no public bug report names it. Other single-hit projectiles remove themselves on an entity hit.
Thus the quirk is probably an omission, not a design. This server keeps it for compatibility.

**LLAMA-SPIT-006** (`design`, semantic, fidelity: exact, confidence: medium). The goal tick of the
ranged attack is a reflex in each tick (AIT-RFX-003 for an attack). The spit of LLAMA-SPIT-002 is a
due event of that reflex. The spit is a spawn request with the parent key of the llama. The revenge
goal and the wolf goal are decisions. The spit flag is own state of the llama: the reflex writes it,
and the next decision of the revenge goal reads it.

**LLAMA-SPIT-007** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The order of
a revenge, for a llama that a survival player hits, with no caravan:

1. The hit sets the attacker record and the damage record. The record of GOAL-PNC-001 is fresh.
2. In the next full evaluation, the revenge goal starts with the player as the attack target. The
   ranged attack starts in the same full evaluation. The panic goal is not examined, because the
   ranged attack holds `move` with the same priority.
3. The llama walks to the player and looks at the player. When the player is within 20 blocks and
   the llama saw the player in 5 goal ticks in a row, the llama stops its path.
4. The llama spits in the 41st goal tick. The damage record is no longer fresh, so no panic follows.
5. In the next full evaluation, the revenge goal stops (LLAMA-TGT-001). The ranged attack stops when
   its path is done.

The panic replaces this order only when no revenge target is possible. Examples are a player in
creative mode, a player on the difficulty `peaceful`, and a hit with no living attacker.

## 11. Drops and experience

**LLAMA-DROP-001** (`documentation`, semantic, fidelity: distribution, confidence: high). When an
adult llama dies and the game rule `mob_drops` is true, it drops the loot table
`minecraft:entities/llama`: `leather`, uniform from 0 to 2. The enchantment `looting` at the level
`L` adds `round(L × u)`, as ANIM-DROP-003 states. A baby drops no loot (ANIM-DROP-001).

**LLAMA-DROP-002** (`source_suggested`, semantic, fidelity: distribution, confidence: medium). The
experience of a death follows ANIM-DROP-005: a uniform integer from 1 to 3. The breed orb follows
ANIM-BREED-001, step 6.

**LLAMA-DROP-003** (`source_suggested`, semantic, fidelity: exact, confidence: medium). Hook. At the
death, a llama also drops its equipment, for example the decor carpet. A llama with a chest also
drops one `chest` and each item of its inventory. The equipment specification and section 13 state
these.

## 12. The save

**LLAMA-SAVE-001** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The save of
a llama holds the fields of an animal (ANIM-SAVE-002) and the fields of this table. The variant and
the sound variant of the farm animals do not apply.

| Field | Key | Type | Default when absent |
|-------|-----|------|---------------------|
| variant | `Variant` | int, the id of LLAMA-VAR-001 | 0, `creamy` |
| strength | `Strength` | int | 0, which the load clamps to 1 |
| tamed | `Tame` | boolean | false |
| temper | `Temper` | int | 0 |
| bred flag | `Bred` | boolean | false |
| eating flag | `EatingHaystack` | boolean | false |
| owner | `Owner` | UUID | none |
| has a chest | `ChestedHorse` | boolean | false |
| inventory | `Items` | list of slots, only when it has a chest | empty |

The save does not keep the links of a caravan, the caravan speed, the far counter, the spit flag and
the counters of LLAMA-TICK-002.

## 13. Hooks: subjects out of scope

**LLAMA-HOOK-001** (`source_suggested`, semantic, fidelity: exact, confidence: low). Chest. A player
uses `chest` on a tamed adult with no chest. The llama takes the chest, plays the chest sound and
consumes the item. The inventory then has `3 × strength` slots. Entity data entry 19 carries the
chest. The inventory and its menu are out of scope.

**LLAMA-HOOK-002** (`source_suggested`, semantic, fidelity: exact, confidence: low). Decor. A carpet
in the body equipment slot is the decor of the llama. The client draws it. The equipment
specification states the slot, its drop and its sync.

**LLAMA-HOOK-003** (`source_suggested`, semantic, fidelity: exact for the gates, confidence: low). A
rider and the tamed state. A player mounts an untamed adult llama that has no passenger. The buck
goal (priority 1) then starts when the llama is not tamed, has a passenger, and the default search
`D(5, 4)` (GOAL-POS-002) gives a destination. In each full evaluation of the goal, with the chance
1/25, the player either tames the llama or falls off. The temper and the maximal temper 30 set the
chance. The rest of the taming and each rule of a ridden llama are out of scope. A llama has no
controller: a player can ride a tamed llama, but cannot steer it.

**LLAMA-HOOK-004** (`source_suggested`, semantic, fidelity: exact, confidence: low). Leash. A
leashed llama walks to its holder with the speed modifier 2.0, not 1.0. A llama has one leash point
(LLAMA-TYPE-006). The leash specification states the rest. The caravan reads only the state
"leashed" and the kind of the holder (LLAMA-CAR-006, step 1).

**LLAMA-HOOK-005** (`source_suggested`, semantic, fidelity: exact, confidence: low). Trader llama.
The type `minecraft:trader_llama` has the data, the goals and the caravan of a llama. It adds one
panic goal, two target goals, a despawn timer and the defence of its trader. Its child has its own
type. Its specification is out of scope. This document states only where a query of a llama accepts
it: `Q-CARAVAN`, `Q-PARTNER`, `Q-ADULT` and the "can mate" test.

## 14. Cost and parallelism

This section states the world queries of each part of the llama in the reference, and their cadence.
It then states a shortcut that keeps the result, with its fidelity class. "Own" means the row and
the goal state of the llama itself.

### 14.1 The queries of the reference

**LLAMA-COST-001** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The world
queries of a llama in the reference:

| Part | Query of the reference | Cadence in the reference | Reads of other entities |
|------|------------------------|--------------------------|-------------------------|
| caravan start | `Q-CARAVAN`: an entity box query of the box grown by `(9, 4, 9)`, about `18.9 × 9.9 × 18.9` blocks, then two passes over the result | each full evaluation of each llama that is not leashed and not in a caravan, while the buck goal does not run | head link, tail link, leash state, position, box of each llama-kind entity in the box; up to 8 more head links and leash states for the chain walk |
| caravan continue | a chain walk of up to 9 links | each full evaluation while the goal runs | head links, leash states, health, removal of the members above |
| caravan tick | one path request to a point | each full evaluation while the goal runs | position of the head link |
| wolf start | gate 1/8, then `Q-WOLF`: an entity box query of the box grown by `F ≈ 4`, about `8.9 × 9.9 × 8.9` blocks, with one line of sight ray for each wolf that passes the box and distance tests | each full evaluation in which no target goal runs; the query only when the gate passes | tame state, position, box, alive, spectator, invulnerable, team and visibility inputs of each wolf; blocks along each ray |
| wolf and revenge keep | one distance; one line of sight ray for the revenge goal | each full evaluation while the goal runs | position of the target; blocks along the ray |
| revenge start | none: the attacker record is own | each full evaluation in which the goal is examined | the attacker: alive, spectator, invulnerable, team, the difficulty for a player |
| ranged attack | one line of sight ray and one path request to an entity | each tick while the goal runs | position of the target; blocks |
| spit | the hit test of LLAMA-SPIT-005 | each tick of each spit | boxes of the entities near the spit; blocks |
| panic, breed, tempt, follow an adult, stroll, look goals | as `mob-goals.md`, section 7 | as `mob-goals.md` | as `mob-goals.md` |
| follow-mother | an entity box query of the box grown by 16, and a path search | each tick of a baby with the bred flag | horse-family entities: bred flag, position |
| heal, tail | none | each tick | none |

The caravan start has no gate. Thus in the reference, each idle llama does one entity box query in
each full evaluation, also when no llama of the dimension is leashed. A caravan can start only from
a leashed llama, so most of these queries find nothing.

### 14.2 Shortcuts

**LLAMA-COST-002** (`design`, semantic, fidelity: exact, confidence: medium). The anchor index. The
server keeps the set of the anchors of each dimension: each llama-kind entity that is an anchor A or
an anchor B of LLAMA-CAR-002. The set keeps an anchor in the dying state until its removal. Only
these events change the set: a join, a stop of the caravan goal, a leash, an unleash, a removal and
an add.

Grow the box of a llama by `(9, 4, 9)`. When no anchor of the set has a box that intersects it, the
caravan start condition of the llama is false. In that case the server skips `Q-CARAVAN`. The start
condition draws no random value. Thus the skip gives the same state in each tick. When the set is
empty, the caravan start costs nothing for each llama of the dimension.

**LLAMA-COST-003** (`design`, semantic, fidelity: exact, confidence: medium). The caravan goal under
sleep and wake (GOAL-WAKE-001 to GOAL-WAKE-008):

- Start: no gate. A llama is busy while it is not leashed, not in a caravan, its caravan goal is
  examined, and an anchor is in reach (LLAMA-COST-002). The cost of this test depends on the number
  of anchors, not on the number of llamas. For each anchor, find the llamas whose grown boxes
  intersect it, as GOAL-WAKE-015 does for a tempt.
- Wake events: an anchor that enters the reach of a llama; a stop of a member of a caravan; an
  unleash of a llama. A join and a stop occur in full evaluations that run.
- Continue: while the goal runs, the llama is busy. Each full evaluation walks the chain, applies
  the far rule and requests a path.

**LLAMA-COST-004** (`design`, semantic, fidelity: distribution, confidence: medium). The wolf gate
after a presence test. The server tests first that an untamed wolf has a box that intersects the box
of the llama grown by `F`. Only then does it draw the gate of 1/8, and only after a pass does it run
`Q-WOLF`. The result of `Q-WOLF` is empty without such a wolf, and the gate is independent of the
query. Thus the chance of a start in each full evaluation is that of LLAMA-TGT-003. Under sleep and
wake, the presence test is a part before the gate (GOAL-WAKE-003). The server keeps the set of the
untamed wolves of each dimension. A wolf that enters the reach of a llama, or a wolf that loses its
owner, is a wake event of that llama. When the set is empty, the wolf goal costs nothing.

The class is `distribution`, not `exact`, because the reference draws the gate in each examined full
evaluation. With keyed random values (LLAMA-DEV-001), the draws differ from those of a server with
no presence test. The law of the start time is the same.

**LLAMA-COST-005** (`design`, semantic, fidelity: exact, confidence: medium). Other parts:

- Revenge start: own state only. A hit with a living attacker is wake event 1 of GOAL-WAKE-005. The
  revenge goal, the wolf goal and the ranged attack make a llama busy while they run.
- Heal: when the health is equal to the maximal health, the heal of LLAMA-TICK-001 does nothing. A
  server can skip its draw then. When the health is less, the server can draw the tick of the next
  heal from the geometric law of the chance 1/900.
- Tail counter: no result (LLAMA-TICK-002). A server skips it.
- Follow-mother: the server keeps the set of the babies with the bred flag. A llama outside the set
  does no follow-mother work. The set is empty unless a save or `summon` sets `Bred:1b`.
- Spit: a spit is a short projectile. It lives until it touches a block or water. The cost is the
  hit test of each tick.

### 14.3 Passive mode against a pig

**LLAMA-COST-006** (`design`, semantic, fidelity: exact, confidence: medium). Take an adult llama of
`summon` with `{}` on a dry superflat world. It has no leash and no rider. The dimension has no
wolf, and its only player is a spectator. The queries of one llama for each full evaluation, every 2
ticks:

| Query | Pig, reference | Llama, reference | Llama, with LLAMA-COST-002 and LLAMA-COST-004 |
|-------|----------------|------------------|-----------------------------------------------|
| entity box query, caravan | none | 1 | 0 |
| entity box query, wolves | none | 1/8 on average | 0 |
| player table, tempt (`Q-TEMPT`) | 1 | 1 | 1, or 0 under GOAL-WAKE-015 |
| player table, look (`Q-LOOK`) | after the gate 0.02 | after the gate 0.02 | the same |
| random position search | after the gate 1/60, idle time less than 100 | the same | the same |
| own-state tests | float, panic, breed, follow, look around | the same, and the buck, the revenge, the ranged attack | the same |

Each tick also has the per-tick work: the ambient counter for both types, and for the llama the heal
draw and the tail draw. LLAMA-COST-005 removes the two draws for a llama with full health.

Thus the reference does about 11.25 entity box queries in each second for each idle llama, and none
for a pig. The caravan box is about 4.5 times the volume of the wolf box. With the two indexes, an
idle llama does the same queries as an idle pig. A llama is busy only near an anchor or an untamed
wolf.

The motion cost differs. A stroll of a llama is at about 0.033 blocks in each tick, about one
quarter of the speed of a pig (LLAMA-GOAL-003). A stroll of the same length thus takes about four
times more ticks of the walker and the push. The step height 1.0 adds step tests at a wall. A path
search of a llama can visit 640 nodes, against 256 for a pig (LLAMA-TYPE-003). The benchmark hit of
GOAL-WAKE-020 has the same effect on a llama as on a pig. The type `drown` has no causing entity,
and it is not in `panic_causes`. Thus no revenge and no panic starts.

### 14.4 The parallel tick of a caravan

**LLAMA-COST-007** (`design`, semantic, fidelity: envelope, confidence: medium). A caravan is a
dependency between entities. These rules make the result independent of the thread count:

1. A join is a pair action (AIT-CPL-001). The proposer is the follower. The proposal names the
   anchor `c` of LLAMA-CAR-003.
2. The grant checks these conditions on the current state at the merge point: the anchor is not
   removed and has no tail; the follower is not leashed and not in a caravan; the follower is not
   the anchor.
3. The proposals of one merge point are granted in the order of AIT-LAG-007. The first grant for an
   anchor wins. Each later proposal for that anchor is stale (AIT-CPL-002, step 2). Its goal does
   not start, and the follower tries again in its next full evaluation.
4. A grant writes both links in one step. A new follower is an anchor A for the snapshots of the
   next ticks, not for the proposals of the same merge point.
5. A stop is a write of the follower to its own head link, and a clear of the tail link of its
   former head. The tail link of the head names the follower (LLAMA-CAR-001), so only one stop can
   clear it. The clear applies at the merge point as an unconditional write: clear the tail link
   when it names this follower.
6. The chain walks of LLAMA-CAR-003 and LLAMA-CAR-005 read the snapshot. They read at most 9 rows
   each.

The envelope: the shape of each caravan (which llama follows which) can differ from the reference
when two llamas select one anchor in one tick. The reference then gives the anchor to the llama that
ticks first; that order is not a rule. A break of a caravan travels down the chain one link in each
full evaluation, with the extra delay of the snapshot. These invariants stay true. Each llama has at
most one head link and one tail link. When the tail link of `b` names `a`, the head link of `a`
names `b`. Only the stale links of LLAMA-CAR-008 break this rule.

## 15. Intentional deviations and known vanilla quirks

### LLAMA-DEV-001: Keyed random values

- **Rule:** each rule with the fidelity class `distribution`.
- **Reference:** the strength, the variant and the health of the setup take values from the random
  source of the world. The gates, the heal, the spit direction and the breed values take values from
  the random source of the llama.
- **This server:** each value is a keyed random value (AIT-LAG-008). The part names the rule, for
  example "llama strength" or "spit direction".
- **Visible to the client:** no, as a distribution.
- **Test:** a differential test compares distributions over many runs.

### LLAMA-DEV-002: Ties in `Q-CARAVAN` by the order key

- **Rule:** LLAMA-CAR-002.
- **Reference:** of two anchors at the same squared distance, the anchor that comes later in the
  order of the entity storage wins.
- **This server:** the anchor with the smaller order key wins (GOAL-QRY-004).
- **Visible to the client:** rarely. Only two anchors at the same distance give a difference.
- **Test:** a differential test accepts each anchor at the smallest distance.

### LLAMA-DEV-003: The join of one anchor in one tick

- **Rule:** LLAMA-COST-007.
- **Reference:** a join writes the links at once, in the tick order. A llama that ticks later in the
  same tick sees the new links. It can then select another anchor in the same tick, or join the new
  follower.
- **This server:** a stale join retries in the next full evaluation. A new follower is an anchor
  only for the snapshots of later ticks.
- **Visible to the client:** yes, rarely. A caravan of many llamas that form at one time can take a
  few more ticks to form, and its order can differ.
- **Test:** a test checks the invariants of LLAMA-COST-007, the chain limit and the join distance.
  It does not compare the order.

### LLAMA-DEV-004: The wolf gate after the presence test

- **Rule:** LLAMA-COST-004.
- **Reference:** the gate draws in each examined full evaluation, before the query.
- **This server:** the gate draws only when an untamed wolf is in reach.
- **Visible to the client:** no, as a distribution.
- **Test:** a differential test compares the distribution of the time from the arrival of a wolf to
  the start of the target.

### LLAMA-DEV-005: Known vanilla quirks

These quirks are vanilla behaviour. This server keeps them.

| Rule | Quirk |
|------|-------|
| LLAMA-CAR-005, LLAMA-CAR-007 | The first run of the caravan goal of a llama uses the speed `2.0999999046325684`. Each later run uses `2.1`. |
| LLAMA-CAR-003 | A near anchor that fails the distance or chain test blocks a join to a farther anchor. |
| LLAMA-CAR-008 | A dead or removed follower leaves a stale tail link. No llama can join behind its head. |
| LLAMA-CAR-009 | A cycle of links can form and stay. |
| LLAMA-TGT-002 | The spit flag of a spit at a wolf stays true, and stops the next revenge at once. |
| LLAMA-TGT-004 | The wolf target ends beyond about 4 blocks, before the first spit in most cases. |
| LLAMA-SPIT-005 | A spit is not removed when it hits an entity. It can hit a second entity behind the first. Probably an omission; a later option can remove the spit on an entity hit. |
| LLAMA-TICK-002 | A llama with the save flag `EatingHaystack:1b` stays immobile. |
| LLAMA-TICK-003 | The follow-mother search does not move the baby. It only changes the stored target of the navigation. |
| LLAMA-FEED-001 | `wheat` feeds a llama but does not tempt it. |

## 16. Experiments

None of these experiments ran. Use the settings of `mob-goals.md`, section 10.1.

| Experiment | Steps | Expected result | Rules |
|------------|-------|-----------------|-------|
| LLAMA-E1 | Summon 400 llamas without `nbt` in `savanna_plateau`. Read `Strength`, `Variant` and the base of `max_health`. | Strength 1 to 3 at about 0.328 each, 4 and 5 at about 0.008. Health 15 to 30. Variants uniform. | LLAMA-SPAWN-001, LLAMA-SPAWN-002 |
| LLAMA-E2 | Summon a llama with `{}`. Read the save. | `Strength` 1, `Variant` 0, maximal health 53. | LLAMA-VAR-002, LLAMA-SPAWN-001 |
| LLAMA-E3 | Leash one llama to a fence. Summon 12 llamas in a line 3 blocks apart, within 9 blocks of each other. Record the head of each llama with a scripted client or a debug command. | A chain of 9 followers. The other llamas do not join. | LLAMA-CAR-003, LLAMA-CAR-004 |
| LLAMA-E4 | Lead a caravan with a leash. Remove the leash. Record the tick of the stop of each follower. | Each follower stops one full evaluation after the one ahead, or in the same tick. | LLAMA-CAR-005, LLAMA-CAR-007 |
| LLAMA-E5 | Hit a llama as a survival player at 8 blocks. Record the spit, its tick, and the attack target. Hit it again after its spit. | One spit 41 ticks after the start. The revenge ends after the spit. The second hit gives a second spit. | LLAMA-TGT-001, LLAMA-SPIT-001, LLAMA-SPIT-007 |
| LLAMA-E6 | Summon a llama and a wolf with `NoAI:1b` 3 blocks apart. Record the spits. Repeat at 5 blocks. | Spits every 40 ticks at 3 blocks; no target at 5 blocks. | LLAMA-TGT-003, LLAMA-TGT-004 |
| LLAMA-E7 | Spit at a target behind two pigs in a line. | The spit can damage both pigs. | LLAMA-SPIT-005 |
| LLAMA-E8 | Feed `wheat` and `hay_block` to a baby with the age −24000 and to an untamed adult. Read `Age`, `Temper` and `InLove`. | Age −23800 and −22000; temper 3 and 6; no love for the untamed adult. | LLAMA-FEED-001 |

## 17. Unresolved

- **U1**: no experiment of section 16 ran. The caravan, the spit and the strength rest on the
  source.
- **U2**: `entity-tracking.md`, TRK-TYPE-002, TRK-TYPE-004 and TRK-DATA-006, have no row for the
  llama and the spit. LLAMA-TYPE-001, LLAMA-TYPE-002, LLAMA-DATA-001 and LLAMA-SPIT-003 give the
  values. That document must add them.
- **U3**: AIT-VAN-011 states that only a goal that must see the target checks the line of sight. The
  source checks the line of sight in the start query of each nearest-target goal. This is also true
  for the wolf goal of a llama, which need not see its target to keep it. LLAMA-TGT-003 and the
  combat test of `mob-combat-goals.md` follow the source. AIT-VAN-011 needs a correction.
- **U4**: the spawn bundle of a llama with the setup can carry the health 53 before the first tick
  clamps it (LLAMA-SPAWN-001). No capture shows the value that a client receives.
- **U5**: the hit test of a projectile (LLAMA-SPIT-005) and the owner check belong to a projectile
  specification that does not exist yet. The margin of the hit test is unspecified here.
- **U6**: the leash specification does not exist. LLAMA-HOOK-004 and the state "leashed" of the
  caravan depend on it. The home of a leash also limits the revenge target (CGOAL-HURT).
- **U7**: closed. The search of LLAMA-TICK-003 does not change the navigation (PATH-DEV-006).
