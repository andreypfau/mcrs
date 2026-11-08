# The basic farm animals: pig, cow, sheep and chicken

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Entity types | `minecraft:pig`, `minecraft:cow`, `minecraft:sheep`, `minecraft:chicken` |
| Data | `assets/mcrs/reports/registries.json`, the data pack of the server jar |

This document states the server behaviour of the four basic farm animals, except their goal AI.
It covers these subjects:

- type data, variants, age, love and the result of a breed;
- the interactions of a player and the work of each tick that is not a goal;
- the drops, the save and the parts of the spawn rules that belong to the type.

The document states semantic rules. It does not state an algorithm of the reference. Each rule has
a fidelity class (section 0.3). The fidelity class tells the design what it must keep and what it
can change. Section 13 states the state that each group of rules reads and writes, so that a design
can tick many animals in parallel.

`mob-goals.md` (prefix `GOAL`) states the goals: float, panic, breed, tempt, follow parent, stroll,
look at a player, look around and eat grass. A goal that needs a value of an animal cites the rule
of this document that owns the value. Section 15 lists these rules.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **entity**, **living entity**, **position**, **velocity**,
  **box**, **dimensions**, **on-ground flag**, **AI step**, **travel step**: as `entity-physics.md`,
  section 0.1, defines them.
- **Mob**, **entity type**, **type data**, **entity data**, **default value**, **dirty entry**,
  **spawn request**, **first spawn setup**, **hook**: as `entity-tracking.md`, section 0.1, defines
  them.
- **Birth key**, **merge point**, **pair action**, **decision**, **reflex**, **goal**: as
  `entity-ai-timing.md`, section 0.1, defines them.
- **Animal**: an entity of one of the four types `pig`, `cow`, `sheep` and `chicken`. In this
  document, "animal" never means another type.
- **Age**: a signed integer of each animal, in ticks. A negative age is the time that remains until
  the animal becomes an adult. A positive age is the time that remains until the adult can breed
  again. The age 0 is an adult that can breed.
- **Baby**: an animal with a negative age.
- **Adult**: an animal with an age of 0 or more.
- **Age lock**: a boolean state of an animal. A baby with the age lock does not grow
  (ANIM-AGE-006).
- **Fed growth**: an integer of each animal, in ticks. It is the sum of the age that feeding added to
  the animal while it was a baby (ANIM-AGE-005). The save key is `ForcedAge`.
- **Love time**: an integer of each animal, in ticks. An animal is in love when its love time is more
  than 0.
- **Love cause**: the player that put the animal in love, or no player. The animal keeps the UUID of
  the player.
- **Breeding food**: the item set that puts an adult in love and makes a baby grow (ANIM-FOOD-001).
- **Tempt items**: the item set that makes the tempt goal follow a player (ANIM-FOOD-002).
- **Variant**: the entry of the variant registry of the type that selects the texture and the model
  on the client. The registries are `minecraft:pig_variant`, `minecraft:cow_variant` and
  `minecraft:chicken_variant`. A sheep has no variant. It has a wool colour.
- **Sound variant**: the entry of the sound variant registry of the type that selects the sounds.
  The registries are `minecraft:pig_sound_variant`, `minecraft:cow_sound_variant` and
  `minecraft:chicken_sound_variant`. A sheep has no sound variant.
- **Climate class**: one of `warm`, `cold` and `temperate`, from the biome at a position
  (ANIM-VAR-002).
- **Wool colour**: one of the 16 dye colours. Section 6 of `entity-tracking.md`, TRK-DATA-006, gives
  the colour ids.
- **Sheared**: a boolean state of a sheep. A sheared sheep has no wool.
- **Egg timer**: an integer of each chicken, in ticks, until it lays the next egg.
- **Boost**: a speed increase of a pig that a player rides and steers (ANIM-RIDE-003).
- **Boost length**: the total number of ticks of one boost.
- **Controller**: the passenger that steers a pig (ANIM-RIDE-001).
- **Ambient counter**: an integer of each animal that times the ambient sound (ANIM-TICK-004).
- **Initiator**: of two animals that breed, the animal whose breed goal completes the breed. The
  other animal is the **partner**.
- **Keyed random value**: a random value from a stream (AIT-LAG-008). The stream is a function of
  the world seed, the birth key of the entity, the tick and the purpose of the value. The value does
  not depend on the order of the entities or on the thread count.
- **Creative player**: a player in the game mode `creative`.

### 0.2 Provenance and confidence

The labels of `entity-physics.md`, section 0.2, apply: `source_suggested`, `black_box_confirmed`,
`source_suggested_black_box_confirmed`, `design`, `documentation` and `experiment`. A confirmed
label refers to the experiments of section 12. Each rule states its kind as `docs/spec/README.md`
defines the kinds, and a confidence: high, medium or low.

### 0.3 Fidelity

Each rule has one fidelity class. `mob-goals.md` uses the same three classes.

| Class | Meaning | What a test compares |
|-------|---------|----------------------|
| `exact` | The observable value or timing must equal the reference: a duration, a range, an item set, a drop count, a value of the network data, a sound that a client hears. | each value |
| `distribution` | The rule makes a random choice. The rule states the support, the probabilities and the independence. The generator is free. This server takes a keyed random value (AIT-DEV-002). | the distribution over many runs |
| `envelope` | The rule makes an internal choice. The result must stay inside the stated envelope. | that each result is inside the envelope |

A rule line has the form
`**ANIM-X-001** (provenance, kind, fidelity: class, confidence: level).` When the parts of one rule
have different classes, the rule states the class of each part.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `ANIM-TYPE` | type data that this document owns |
| `ANIM-FOOD` | item sets and goal parameters that goals read |
| `ANIM-VAR` | variants, sound variants and wool colours |
| `ANIM-SPAWN` | the first spawn setup and the spawn rules of the type |
| `ANIM-AGE` | age, growth and age lock |
| `ANIM-LOVE` | love state |
| `ANIM-BREED` | the result of a breed |
| `ANIM-INT` | the interactions of a player |
| `ANIM-RIDE` | the ride and the boost of a pig |
| `ANIM-TICK` | the work of each tick that is not a goal |
| `ANIM-DROP` | drops and experience |
| `ANIM-SAVE` | persistence and the save |
| `ANIM-DEV` | intentional deviations |

### 0.5 Scope

In scope: the server state and the server behaviour of the four animals, except the goals. The
table gives the subjects that other documents own.

| Subject | Where |
|---------|-------|
| Goals, the move, look and jump controls | `mob-goals.md` |
| The wire id, the adult dimensions, the range, the interval and the base attributes of the type | `entity-tracking.md`, TRK-TYPE-002 and TRK-TYPE-004 |
| The entity data entries of each type and their defaults | `entity-tracking.md`, TRK-DATA-006 |
| The keys of `summon` for the animals | `entity-tracking.md`, TRK-SUM-005 |
| Health, damage, the death and the hurt and death sounds as packets | `entity-damage.md` |
| The walker motion and the box | `entity-physics.md` |
| The moves of a vehicle that a client controls | `movement-checks.md`, section 16 |
| The spawn cycle, the mob cap and the biome spawn entries | `natural-spawning.md` (planned, step G3) |
| The loot table system, item entities and experience orbs | the loot specification (planned), PHY-ORD-005, `experience-orbs.md` (planned) |
| What the client draws | `mob-pig.md`, `client-entities.md` |
| The mooshroom, the cow variant model of a mooshroom, the strider and other ageable types | out of scope |
| Leads, name tags and spawn eggs | hooks only (ANIM-INT-009 to ANIM-INT-011) |

## 1. Type data

The adult values of each type are in TRK-TYPE-002 and TRK-TYPE-004. This section states the values
that depend on the age and the values that only the animals have.

**ANIM-TYPE-001** (`source_suggested`, semantic, fidelity: exact, confidence: high). A baby has the
dimensions of this table, not the adult dimensions times an age scale. The `scale` attribute
multiplies the dimensions of the table, as it multiplies the adult dimensions.

| Type | Adult width × height | Adult eye height | Baby width × height | Baby eye height |
|------|----------------------|------------------|---------------------|-----------------|
| `pig` | `0.9F × 0.9F` | `0.765F` | `0.45F × 0.45F` | `0.40625F` |
| `cow` | `0.9F × 1.4F` | `1.3F` | `0.45F × 0.7F` | `0.69F` |
| `sheep` | `0.9F × 1.3F` | `1.235F` | `0.45F × 0.65F` | `0.65625F` |
| `chicken` | `0.4F × 0.7F` | `0.644F` | `0.3F × 0.4F` | `0.28125F` |

The adult eye height of the pig is not given as data. It is `0.85F` times the height. The box
changes when the baby state changes (ANIM-AGE-002). Each dimension is scalable.

**ANIM-TYPE-002** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The
passenger attachment point is the point where a rider sits, relative to the position of the
animal:

| Type | Adult | Baby |
|------|-------|------|
| `pig` | `(0, 0.86875F, 0)` | `(0, 0.5F, 0)` |
| `cow` | `(0, 1.36875F, 0)` | `(0, 0.75F, 0)` |
| `sheep` | `(0, 1.2375F, 0)` | `(0, 0.5625F, 0)` |
| `chicken` | `(0, 0.7, −0.1)` | `(0, 0.375F, 0)` |

A rider of a chicken turns its body to the body yaw of the chicken in each tick.

**ANIM-TYPE-003** (`source_suggested`, semantic, fidelity: exact, confidence: high). Each animal
has these properties of the mob category `creature`:

- It is allowed in the difficulty `peaceful`.
- It is not fire immune.
- It does not despawn when it is far from each player (ANIM-SAVE-001).

**ANIM-TYPE-004** (`source_suggested`, semantic, fidelity: exact, confidence: high). The sounds of
an animal come from its sound variant or its type:

| Type | Ambient, hurt, death, step | Eat | Volume | Step volume |
|------|----------------------------|-----|--------|-------------|
| `pig` | the adult set or the baby set of the sound variant | the `eat_sound` of the set | 1.0 | 0.15 |
| `cow` | the one set of the sound variant, for an adult and a baby | none | 0.4 | 0.15 |
| `sheep` | `entity.sheep.ambient`, `entity.sheep.hurt`, `entity.sheep.death`, `entity.sheep.step` | none | 1.0 | 0.15 |
| `chicken` | the adult set or the baby set of the sound variant | none | 1.0 | 0.15 |

The pitch of an ambient, hurt, death or eat sound is `1.0 + 0.2 × (u1 − u2)` for an adult and
`1.5 + 0.2 × (u1 − u2)` for a baby. `u1` and `u2` are two independent uniform values in `[0, 1)`.
The pitch has the fidelity class `distribution`. The pitch of a step sound is 1.0. The sound source
is `neutral`. A silent animal makes no sound. The trigger of a step sound is the walk distance of
the movement rules, a hook of this document.

**ANIM-TYPE-005** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The path
costs of an animal for `pathfinding.md`:

- A node next to fire has the extra cost 16. A node in fire is blocked.
- For a chicken, a node in water has the cost 0. The other three types keep the default cost.
- The stroll target value of a block position is 10 when the block below is `grass_block`. Else it
  is the value from the light level, as `pathfinding.md` states it for the default case.

## 2. Item sets and goal parameters

**ANIM-FOOD-001** (`source_suggested`, semantic, fidelity: exact, confidence: high). The breeding
food of each type is the item tag of this table. The data pack can change a tag.

| Type | Tag | Items in the vanilla data pack |
|------|-----|--------------------------------|
| `pig` | `#minecraft:pig_food` | `carrot`, `potato`, `beetroot` |
| `cow` | `#minecraft:cow_food` | `wheat` |
| `sheep` | `#minecraft:sheep_food` | `wheat` |
| `chicken` | `#minecraft:chicken_food` | `wheat_seeds`, `melon_seeds`, `pumpkin_seeds`, `beetroot_seeds`, `torchflower_seeds`, `pitcher_pod` |

**ANIM-FOOD-002** (`source_suggested`, semantic, fidelity: exact, confidence: high). The tempt
items of a type are its breeding food. A pig also follows a player who holds `carrot_on_a_stick`.
The two tempt sets of the pig belong to two separate tempt goals with the same priority. The tempt
goal does not stop a player who uses the item.

**ANIM-FOOD-003** (`source_suggested`, semantic, fidelity: exact, confidence: high). The goal
parameters of each type. A speed is a multiplier of `movement_speed`. A priority is the priority of
the goal in the goal selection; a smaller number wins.

| Goal | `pig` | `cow` | `sheep` | `chicken` |
|------|-------|-------|---------|-----------|
| float | 0 | 0 | 0 | 0 |
| panic: priority, speed | 1, 1.25 | 1, 2.0 | 1, 1.25 | 1, 1.4 |
| breed: priority, speed | 3, 1.0 | 2, 1.0 | 2, 1.0 | 2, 1.0 |
| tempt: priority, speed | 4, 1.2 (two goals) | 3, 1.25 | 3, 1.1 | 3, 1.0 |
| follow parent: priority, speed | 5, 1.1 | 4, 1.25 | 4, 1.1 | 4, 1.1 |
| eat grass: priority | none | none | 5 | none |
| stroll, avoids water: priority, speed | 6, 1.0 | 5, 1.0 | 6, 1.0 | 5, 1.0 |
| look at a player: priority, range | 7, 6.0 | 6, 6.0 | 7, 6.0 | 6, 6.0 |
| look around: priority | 8 | 7 | 8 | 7 |

The attribute `tempt_range` is 10 for each type (TRK-TYPE-004). No type has an attribute modifier
for a baby. A baby moves with the same `movement_speed` as an adult. A goal reads the baby state
from ANIM-AGE-001.

**ANIM-FOOD-004** (`source_suggested`, semantic, fidelity: exact, confidence: high). Two animals
can mate when all these conditions are true:

- they are two different entities of the same type;
- both are in love (ANIM-LOVE-001).

The breed goal of `mob-goals.md` finds the partner and checks the distance. It then applies
ANIM-BREED-001.

## 3. Variants, sound variants and wool colours

**ANIM-VAR-001** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact, confidence:
high). Each entry of a variant registry has a list of spawn selectors. A selector has a priority
and an optional condition. The vanilla data pack has these entries for each of the three registries
`pig_variant`, `cow_variant` and `chicken_variant`:

| Entry | Selector | Model |
|-------|----------|-------|
| `minecraft:temperate` | priority 0, no condition | `normal` |
| `minecraft:warm` | priority 1, the biome is in `#minecraft:spawns_warm_variant_farm_animals` | `normal`; for a cow, `warm` |
| `minecraft:cold` | priority 1, the biome is in `#minecraft:spawns_cold_variant_farm_animals` | `cold` |

The model and the textures are client data (`mob-pig.md`, MPIG-VAR-001). The default variant of
each registry is `minecraft:temperate`. When the registry has no such entry, the default is one
entry of the registry. Which entry is unspecified.

**ANIM-VAR-002** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact, confidence:
high). The climate class of a biome:

1. `warm`, when the biome is in `#minecraft:spawns_warm_variant_farm_animals`;
2. else `cold`, when the biome is in `#minecraft:spawns_cold_variant_farm_animals`;
3. else `temperate`.

The vanilla data pack puts `desert`, `warm_ocean`, `lukewarm_ocean`, `deep_lukewarm_ocean`,
`mangrove_swamp` and the tags `is_jungle`, `is_savanna`, `is_nether` and `is_badlands` in the warm
tag. The cold tag holds the snowy, frozen, taiga, windswept, peak, grove and cold ocean biomes,
`deep_dark`, `dappled_forest`, `ice_caves` and the tag `is_end`. The tags are data. A server reads
them from the data pack.

**ANIM-VAR-003** (`source_suggested_black_box_confirmed`, semantic, fidelity: distribution,
confidence: high). The variant of a first spawn setup for a pig, a cow or a chicken at the block
position `p`:

1. Read the biome at `p`.
2. Collect each pair of an entry and one of its selectors whose condition is true for the biome. A
   selector with no condition is true.
3. Keep only the pairs with the highest priority of the collected pairs.
4. Select one kept pair. Each kept pair has the same probability. The variant is the entry of that
   pair.
5. When no pair is kept, the variant does not change.

With the vanilla data, the variant is thus `warm`, `cold` or `temperate` from the climate class of
ANIM-VAR-002, with no random part. A data pack can add entries with equal priorities. Then the
choice is random.

**ANIM-VAR-004** (`source_suggested_black_box_confirmed`, semantic, fidelity: distribution,
confidence: high). The sound variant of a first spawn setup is one entry of the sound variant
registry of the type. Each entry has the same probability. The vanilla entries are:

| Type | Entries | Default |
|------|---------|---------|
| `pig` | `classic`, `big`, `mini` | `classic` |
| `cow` | `classic`, `moody` | `classic` |
| `chicken` | `classic`, `picky` | `classic` |

An entry of a pig or a chicken has an adult set and a baby set of sounds. The baby set of each
vanilla entry of one type is the same set. An entry of a cow has one set. The default applies when
the animal has no first spawn setup, and to the child of a breed (ANIM-BREED-002).

**ANIM-VAR-005** (`source_suggested_black_box_confirmed`, semantic, fidelity: distribution,
confidence: high). The wool colour of a first spawn setup of a sheep comes from the climate class
at its block position. Each row gives a weight out of 100:

| Climate class | Main colour, weight 82 | Weight 5 | Weight 5 | Weight 5 | Weight 3 |
|---------------|------------------------|----------|----------|----------|----------|
| `temperate` | `white` | `black` | `gray` | `light_gray` | `brown` |
| `warm` | `brown` | `gray` | `light_gray` | `white` | `black` |
| `cold` | `black` | `light_gray` | `gray` | `white` | `brown` |

When the main colour wins, the sheep is `pink` with the probability 1/500. Else it has the main
colour. Thus a temperate sheep is `pink` with the probability 0.00164 and `white` with the
probability 0.81836. The default wool colour, without a first spawn setup, is `white`. A new sheep
is not sheared.

## 4. The first spawn setup and the spawn rules

**ANIM-SPAWN-001** (`source_suggested_black_box_confirmed`, semantic, fidelity: distribution,
confidence: high). The first spawn setup of an animal sets these values. Each value is independent
of the others.

| Value | Rule | Types |
|-------|------|-------|
| variant | ANIM-VAR-003 | `pig`, `cow`, `chicken` |
| sound variant | ANIM-VAR-004 | `pig`, `cow`, `chicken` |
| wool colour | ANIM-VAR-005 | `sheep` |
| baby | ANIM-SPAWN-002 | each |
| follow range bonus | a permanent modifier `minecraft:random_spawn_bonus` on `follow_range`, operation `add_multiplied_base`. The amount is `0.11485 × (u1 − u2)`, with two independent uniform values in `[0, 1)`. | each |
| left handed | true with the probability 0.05 | each |

These ways of birth apply the setup:

- `summon` without the `nbt` argument;
- natural spawning, and the spawn of a chunk at its generation;
- a spawner, and a spawn egg that a player uses on a block.

A
breed, a spawn egg that a player uses on an animal of its type, a load and `summon` with `nbt` do
not apply it. Without the setup, each value keeps its default: the default variant and sound variant,
`white` wool, adult, no modifier, right handed.

The chicken also takes its first egg timer when the server creates it (ANIM-TICK-001). This occurs
for each way of birth, with or without the setup. A load then replaces the timer with the saved
value.

**ANIM-SPAWN-002** (`source_suggested_black_box_confirmed`, semantic, fidelity: distribution,
confidence: high). A spawn group is the set of animals that one spawn event makes together, for
example one pack of natural spawning. In a group, the first animal is an adult. Each later animal is
a baby with the probability 0.05, and an adult otherwise. A baby of the setup has the age −24000. A
`summon` makes a group of one animal. Thus `summon` never makes a baby with the setup.

**ANIM-SPAWN-003** (`source_suggested`, semantic, fidelity: exact, confidence: high). The spawn
rules of the type. `natural-spawning.md` applies them.

- The placement is on the ground. The spawn position comes from the heightmap
  `MOTION_BLOCKING_NO_LEAVES`.
- The block below the position must be in `#minecraft:animals_spawnable_on`. The vanilla tag holds
  only `grass_block`.
- The larger of the sky light and the block light at the position must be more than 8. A spawn
  with the reason "trial spawner" does not check the light.

**ANIM-SPAWN-004** (`documentation`, semantic, fidelity: exact, confidence: medium). The biome
data gives the spawn weight and the group size. The vanilla biomes give this entry to most
overworld biomes with grass: `sheep` weight 12, `pig` weight 10, `chicken` weight 10 and `cow`
weight 8, each with the group size 4. `cherry_grove` has `pig` weight 1, group 1 to 2, and `sheep`
weight 2, group 2 to 4. `meadow` has `sheep` weight 2, group 2 to 4. The biome data is the
authority. This document does not repeat it.

## 5. Age

**ANIM-AGE-001** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact, confidence:
high). An animal is a baby when its age is less than 0. The entity data entry 16 of TRK-DATA-006 is
true for a baby. A new baby of a breed or a setup has the age −24000. `summon` can set any age
(TRK-SUM-005).

**ANIM-AGE-002** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact, confidence:
high). In each tick, after the travel step and the push of its AI step (PHY-ORD-002), an animal
that is alive changes its age:

- a baby without the age lock adds 1 to its age;
- an adult with a positive age subtracts 1 from its age;
- else the age does not change.

The change occurs also when the animal has `NoAI` (experiment ANIM-E5). When a change of the age
crosses 0, the entry 16 becomes dirty, and the dimensions change (ANIM-TYPE-001). A baby that
becomes an adult while it rides a boat without space for the adult leaves the boat.

**ANIM-AGE-003** (`source_suggested`, semantic, fidelity: exact, confidence: high). A player feeds
a baby without the age lock with its breeding food. The age increases by `20 × s` ticks, and the
age does not go above 0. Let `r` be the negative of the age before the feed. Then
`s = floor(f32(floor(r / 20)) × 0.1F)`, a number of seconds. Thus a feed takes about 10 % of the
time that remains. A baby with the age −24000 grows to −21600.

**ANIM-AGE-004** (`source_suggested`, semantic, fidelity: exact, confidence: high). A sheep that
eats grass (ANIM-TICK-003) and that is a baby without the age lock adds 1200 ticks to its age. The
age does not go above 0. This increase does not add to the fed growth.

**ANIM-AGE-005** (`source_suggested`, semantic, fidelity: exact, confidence: medium). The fed growth
of an animal starts at 0. A feed of ANIM-AGE-003 adds the increase of the age to the fed growth.
When an increase of ANIM-AGE-003 or ANIM-AGE-004 makes the age equal to 0, the age changes to the
fed growth. Thus a baby that a feed makes adult must wait the fed growth before it can breed. A baby
that grows to 0 in ANIM-AGE-002 keeps the age 0. The fed growth never decreases, and the save keeps
it.

**ANIM-AGE-006** (`source_suggested`, semantic, fidelity: exact, confidence: medium). A player who
uses `golden_dandelion` on a baby toggles its age lock, when the age lock cooldown of the baby is 0.
The steps:

1. Toggle the age lock. The entity data entry 17 becomes dirty.
2. Set the age to −24000.
3. Set the age lock cooldown to 40 ticks.
4. Consume one `golden_dandelion` (ANIM-INT-001).
5. When the age lock is now on, make the animal persistent (ANIM-SAVE-001).
6. Play `item.golden_dandelion.use` when the lock is now on, else `item.golden_dandelion.unuse`, at
   the block position of the animal, source `players`, volume 1.0, pitch 1.0.

An adult does not take this interaction. An age-locked baby does not take the feed of
ANIM-AGE-003. The item then has no effect and the player keeps it.

**ANIM-AGE-007** (`source_suggested`, semantic, fidelity: envelope, confidence: medium). The server
sends these particles to the players near the animal:

- After a feed of ANIM-AGE-003, when the fed growth timer was 0, it starts at 40. In each tick in
  which the timer is a multiple of 4, one `happy_villager` particle appears. Then the timer decreases
  by 1.
- While the age lock cooldown is more than 0, one particle appears in each tick in which the
  cooldown is even: `pause_mob_growth` when the lock is on, else `reset_mob_growth`. Then the
  cooldown decreases by 1.

The particle positions are random. The envelope: on X and on Z, each particle is at most one box
width from the position of the animal. On Y, it is between the position and `1.2 × height + 0.5`
above it. The times and the counts are `exact`.

## 6. Love

**ANIM-LOVE-001** (`source_suggested`, semantic, fidelity: exact, confidence: high). A player feeds
an animal with its breeding food. The animal enters love when all these conditions are true:

- the age is 0;
- the love time is 0 or less.

Then these steps occur:

1. Consume one item of the food (ANIM-INT-001).
2. Set the love time to 600.
3. Set the love cause to the player.
4. Send the entity event 18 to the players that track the animal. The client shows hearts.
5. A pig plays the eat sound of its sound set. The other types play no eat sound.

**ANIM-LOVE-002** (`source_suggested`, semantic, fidelity: exact, confidence: high). In each tick,
an animal changes its love time in this way:

1. When the age is not 0, the love time changes to 0. This check occurs before the goals of the
   tick, and again after the travel step.
2. Else, when the love time is more than 0, it decreases by 1.

When the animal takes a hit that applies damage (DMG-HIT-004), its love time changes to 0.

**ANIM-LOVE-003** (`source_suggested`, semantic, fidelity: exact, confidence: high). The love time
and the love cause are server state. No entity data entry carries them. A client only sees the
entity event 18.

## 7. The result of a breed

**ANIM-BREED-001** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact for the state,
distribution for the random values, confidence: high). The breed goal of the initiator applies the
breed with its partner. The steps:

1. Make the child of ANIM-BREED-002. Its age is −24000. Its position is the position of the
   initiator. Its yaw and its pitch are 0. Its birth key has the source kind `entity` and the birth
   key of the initiator (PHY-CMP-013).
2. Find the love cause of the initiator. If it has none, use the love cause of the partner. If that
   player is online, add 1 to its statistic `minecraft:custom` / `minecraft:animals_bred`. Then run
   the advancement trigger `minecraft:bred_animals` for the player, with both parents and the child.
3. Set the age of both parents to 6000.
4. Set the love time of both parents to 0.
5. Send the entity event 18 for the initiator.
6. When the game rule `mob_drops` is true, add an experience orb at the position of the initiator.
   Its value is uniform from 1 to 7.
7. Add the child to the world.

The breed is a pair action (AIT-CPL-004). ANIM-DEV-002 states the order of the breeds of one tick.

**ANIM-BREED-002** (`source_suggested_black_box_confirmed`, semantic, fidelity: distribution,
confidence: high). The child is a new animal of the type of the parents, without the first spawn
setup. These values differ from the defaults:

| Type | Value | Rule |
|------|-------|------|
| `pig`, `cow`, `chicken` | variant | the variant of the initiator or of the partner, each with the probability 1/2 |
| `sheep` | wool colour | ANIM-BREED-003 |
| `chicken` | egg timer | a new timer of ANIM-TICK-001 |

The child takes the default sound variant `classic`. It does not take the sound variant of a parent
(experiment ANIM-E6). It has no follow range bonus and is right handed.

**ANIM-BREED-003** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact for the mix,
distribution for the fallback, confidence: high). The wool colour of a lamb from the colours `a` of
the initiator and `b` of the partner:

1. Find a shapeless crafting recipe of the data pack with exactly two ingredients: the dye of `a` and
   the dye of `b`. If one exists and its result is a dye, the colour is the colour of that dye.
2. Else the colour is `a` or `b`, each with the probability 1/2.

With the vanilla data pack, step 1 gives this table. The order of `a` and `b` does not matter.

| `a` + `b` | Child |
|-----------|-------|
| `blue` + `green` | `cyan` |
| `black` + `white` | `gray` |
| `blue` + `white` | `light_blue` |
| `gray` + `white` | `light_gray` |
| `green` + `white` | `lime` |
| `purple` + `pink` | `magenta` |
| `red` + `yellow` | `orange` |
| `red` + `white` | `pink` |
| `blue` + `red` | `purple` |

Two sheep of one colour are not in the table. Their lamb has that colour. A server can compute the
table one time from the data pack when the recipes load. ANIM-DEV-003 states the case of two
recipes for one pair.

## 8. Interactions of a player

**ANIM-INT-001** (`source_suggested`, semantic, fidelity: exact, confidence: high). A player uses
an item on an animal (the packet `interact` with the action "interact"). These general rules apply:

- A dead or dying animal takes no interaction.
- The checks run in this order: name tag, spawn egg, the lead and shear checks of every entity, and
  then the checks of the animal in the order of ANIM-INT-002. The first check that acts ends the
  interaction.
- To consume one item means: the count of the stack in the hand decreases by 1. For a creative
  player the count does not change.
- An interaction that acts sends the game event `entity_interact` at the animal.

**ANIM-INT-002** (`source_suggested`, semantic, fidelity: exact, confidence: high). The checks of
each type, in order. The first check whose condition is true acts.

| Type | Order of the checks |
|------|---------------------|
| `pig` | mount (ANIM-INT-008); love or feed (ANIM-LOVE-001, ANIM-AGE-003); age lock (ANIM-AGE-006); saddle (ANIM-INT-007) |
| `cow` | milk (ANIM-INT-003); love or feed; age lock |
| `sheep` | shear (ANIM-INT-004); dye (ANIM-INT-006); love or feed; age lock |
| `chicken` | love or feed; age lock |

The love check acts first. When the animal cannot enter love and is a baby without the age lock,
the feed check acts. When neither acts, the item stays in the hand, and the next check runs.

**ANIM-INT-003** (`source_suggested`, semantic, fidelity: exact, confidence: high). A player who
uses `bucket` on an adult cow milks it:

1. The sound `entity.cow.milk` plays at the player, volume 1.0, pitch 1.0.
2. One `bucket` changes to one `milk_bucket`. When the stack had one bucket, the milk bucket
   replaces it in the hand. Else the stack decreases by 1, and the milk bucket goes into the
   inventory or drops at the player. A creative player keeps the bucket and gets a milk bucket only
   when the inventory has none.

A cow can give milk any number of times. A baby gives no milk.

**ANIM-INT-004** (`source_suggested`, semantic, fidelity: exact for the state, distribution for the
count and the motion, confidence: high). A player who uses `shears` on a sheep that is an adult and
is not sheared shears it:

1. The sound `entity.sheep.shear` plays at the sheep, source `players`, volume 1.0, pitch 1.0.
2. The drops come from the loot table `minecraft:shearing/sheep`. With the vanilla data, the drop is
   1 to 3 wool items of the wool colour, each count with the probability 1/3.
3. Each wool item drops as one item entity of count 1 at the sheep, 1 block above its position. Its
   velocity gets the extra value `(0.1 × (u1 − u2), 0.05 × u3, 0.1 × (u4 − u5))` with independent
   uniform values in `[0, 1)`.
4. The sheep becomes sheared. The entity data entry 18 becomes dirty.
5. The game event `shear` occurs at the sheep.
6. The shears take 1 point of damage, with the rules of the item for unbreaking.

Shears on a baby or on a sheared sheep do nothing, and no later check runs.

**ANIM-INT-005** (`source_suggested`, semantic, fidelity: exact, confidence: medium). A player who
uses `shears` on a saddled pig that has no passenger, and who does not sneak, removes the saddle:

1. The saddle slot becomes empty.
2. The saddle drops as an item entity at the passenger attachment point of ANIM-TYPE-002.
3. The sound `item.saddle.unequip` plays at the pig.
4. The shears take 1 point of damage.
5. The game event `shear` occurs, and the advancement trigger `minecraft:player_sheared_equipment`
   runs.

This check is one of the checks of every entity (ANIM-INT-001). It comes before the mount check.

**ANIM-INT-006** (`source_suggested`, semantic, fidelity: exact, confidence: high). A player uses
a dye of `#minecraft:sheep_wool_dyes` on a sheep that is alive and not sheared. When the colour of
the dye differs from the wool colour, the player dyes the sheep:

1. The sound `item.dye.use` plays at the sheep, source `players`, volume 1.0, pitch 1.0.
2. The wool colour changes to the colour of the dye. The entity data entry 18 becomes dirty.
3. The dye stack decreases by 1.

A dye of the same colour does nothing, and no later check runs. A dye on a sheared sheep goes to
the next check.

**ANIM-INT-007** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact, confidence:
high). A player who uses `saddle` on a pig puts it on, when all these conditions are true:

- the pig is alive and is an adult;
- the saddle slot of the pig is empty;
- the type of the pig is in `#minecraft:can_equip_saddle`.

The steps:

1. One saddle goes into the saddle slot. A creative player keeps the stack.
2. The drop chance of the saddle slot changes to "guaranteed" (ANIM-DROP-004).
3. The sound `entity.pig.saddle` plays at the pig.
4. The tracker sends `set_equipment` with the slot `saddle` (`client-entities.md`, CENTS-EQUIP-001).

A dispenser can also put a saddle on a pig. That is a hook of the dispenser specification.

**ANIM-INT-008** (`source_suggested`, semantic, fidelity: exact, confidence: high). A player
mounts a pig when all these conditions are true:

- the item in the hand is not breeding food of the pig;
- the pig is saddled;
- the pig has no passenger;
- the player does not sneak.

The player becomes the passenger of the pig. The item in the hand can be any item, also an empty
hand or `carrot_on_a_stick`. When the player dismounts, the dismount position comes from the riding
specification (hook).

**ANIM-INT-009** (`source_suggested`, semantic, fidelity: exact, confidence: medium). Hook. A
player who uses a spawn egg of the type of an animal on that animal makes a child. The child is a
baby of ANIM-BREED-002, with the animal as both parents, at the position of the animal. The egg
applies its own components to the child. One egg is consumed.

**ANIM-INT-010** (`source_suggested`, semantic, fidelity: exact, confidence: medium). Hook. Each
animal can take a lead. A baby cannot take the lead from a player who sneaks. The leash
specification states the rest.

**ANIM-INT-011** (`source_suggested`, semantic, fidelity: exact, confidence: medium). Hook. A name
tag on an animal sets its custom name and makes it persistent.

## 9. Riding a pig

**ANIM-RIDE-001** (`source_suggested`, semantic, fidelity: exact, confidence: high). The controller
of a pig is its first passenger, when all these conditions are true:

- the pig is saddled;
- the first passenger is a player;
- the player holds `carrot_on_a_stick` in the main hand or in the off hand.

Else the pig has no controller. A pig without a controller runs its own AI, also with a rider.

**ANIM-RIDE-002** (`source_suggested`, client agreement, fidelity: exact, confidence: medium). A
pig with a controller is a vehicle that the client of the controller moves. `movement-checks.md`,
section 16, checks the moves. On the client, in each tick of the vehicle:

1. The yaw of the pig changes to the yaw of the controller. The pitch changes to half the pitch of
   the controller. The body yaw and the head yaw change to the new yaw.
2. The boost advances (ANIM-RIDE-003).
3. The input is `(0, 0, 1)`: forward, with no strafe.
4. The speed is `f32(movement_speed × 0.225 × b)`, with `b` the boost factor.
5. The travel step of PHY-ORD-004 runs with this input and speed.

The server also runs steps 1 and 2. It does not move the pig itself. Its velocity is zero.

**ANIM-RIDE-003** (`source_suggested`, semantic, fidelity: exact for the timing, distribution for
the length, confidence: medium). The boost of a pig has the state "boosting", a counter `t` and the
boost length `T`. `T` is the value of the entity data entry 18 (TRK-DATA-006).

- A boost starts when the controller uses `carrot_on_a_stick` and the pig is not boosting. `T`
  becomes a uniform integer from 140 to 980. The entry 18 becomes dirty. `t` becomes 0.
- When the client receives a value of the entry 18, it starts the boost with `t = 0`.
- In each tick of the pig with a controller, a boosting pig compares `t` with `T` and then adds 1 to
  `t`. When `t` was more than `T`, the pig stops boosting. Thus a boost lasts `T + 2` ticks with a
  controller.
- A tick without a controller does not advance the boost.
- The boost factor is `1 + 1.15F × sin(t / T × π)` in f32 while the pig boosts, else 1.

**ANIM-RIDE-004** (`source_suggested`, semantic, fidelity: exact, confidence: high). The item
`carrot_on_a_stick` has the durability 25. A use that starts a boost takes 7 points of damage, with
the rules of the item for unbreaking. When the item breaks, `fishing_rod` replaces it in the hand. A
use that does not start a boost takes no damage and adds 1 to the statistic
`minecraft:used` / `minecraft:carrot_on_a_stick`.

## 10. The work of each tick that is not a goal

**ANIM-TICK-001** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact for the item
and the conditions, distribution for the timer, confidence: high). A chicken lays an egg in this
way:

- When the server creates a chicken, its egg timer is a uniform integer from 6000 to 11999.
- In each tick in which the chicken is alive, is an adult and is not a chicken jockey, the timer
  decreases by 1. A baby keeps its timer. `NoAI` does not stop the timer.
- When the timer becomes 0 or less, the chicken drops the loot table `minecraft:gameplay/chicken_lay`
  at its position. Then the timer takes a new uniform integer from 6000 to 11999.
- With the vanilla data, the drop is one `egg` for the variant `temperate`, one `brown_egg` for
  `warm` and one `blue_egg` for `cold`.
- When the drop gave an item, the sound `entity.chicken.egg` plays at the chicken, volume 1.0,
  pitch `1.0 + 0.2 × (u1 − u2)`, and the game event `entity_place` occurs.

**ANIM-TICK-002** (`source_suggested`, client agreement, fidelity: exact, confidence: high). The
step occurs at the end of the AI step of a chicken, after the push of PHY-ORD-002 step 9. When the
on-ground flag is false and `v.y < 0`, `v.y` changes to `v.y × 0.6`. The step runs for each chicken, also a baby, a
chicken with `NoAI` and a dying chicken. The client of a server-simulated chicken computes the same
step. The chicken takes no fall damage reduction from this rule. The smaller fall speed gives a
smaller fall distance.

**ANIM-TICK-003** (`source_suggested`, semantic, fidelity: exact, confidence: high). When the eat
grass goal of a sheep (`mob-goals.md`) eats a block, the sheep takes the "ate" event:

1. The sheep stops being sheared. The entity data entry 18 becomes dirty.
2. A baby without the age lock grows by ANIM-AGE-004.

The goal states the block change and the entity event 10.

**ANIM-TICK-004** (`source_suggested`, semantic, fidelity: distribution, confidence: medium). The
ambient sound of an animal. The ambient counter of a new animal is 0. The save does not keep it. In
each tick, in the base entity tick (PHY-ORD-002 step 2):

1. When the animal is alive, take a uniform integer `n` from 0 to 999.
2. If `n` is less than the counter, set the counter to −120 and play the ambient sound
   (ANIM-TYPE-004).
3. Add 1 to the counter.

The hurt sound also sets the counter to −120. Thus two ambient sounds are at least 121 ticks
apart. In a tick with the counter `c` from 1 to 999, the probability of a sound is `c / 1000`.

**ANIM-TICK-005** (`source_suggested`, semantic, fidelity: exact, confidence: medium). Hook. A
lightning bolt that hits a pig, when the difficulty is not `peaceful`, converts the pig into a
`zombified_piglin`:

- The new mob takes these values of the pig: the position, the velocity, the body yaw, the
  on-ground flag and the fall distance.
- It also takes the baby state, the age values, the effects, the absorption and the custom name.
- It also takes `NoAI`, `Silent`, `NoGravity`, the left hand, the fire and the leash.
- A rider of the pig rides the new mob.
- The new mob takes the first equipment of its type (TRK, section 3.1) and becomes persistent.
- The saddle is lost. It does not drop.
- The pig is removed.

On the difficulty `peaceful`, the lightning only damages the pig and sets it on fire, as for each
entity. The conversion waits for the lightning specification.

**ANIM-TICK-006** (`source_suggested`, semantic, fidelity: envelope, confidence: high). The flap
state of a chicken is a value of the client animation. The server sends no value of it, and no
sound comes from it. A server does not need to compute it.

## 11. Drops, experience and persistence

**ANIM-DROP-001** (`source_suggested_black_box_confirmed`, semantic, fidelity: distribution,
confidence: high). When an adult animal dies and the game rule `mob_drops` is true, it drops the
loot table `minecraft:entities/<type>`. A baby drops no loot. The drop occurs also when no player
killed the animal. With the vanilla data:

| Type | Drops | Cooked form |
|------|-------|-------------|
| `pig` | `porkchop`, uniform 1 to 3 | `cooked_porkchop` |
| `cow` | `leather`, uniform 0 to 2; `beef`, uniform 1 to 3 | `cooked_beef` for the beef |
| `sheep` | `mutton`, uniform 1 to 2; one wool of the wool colour when it is not sheared | `cooked_mutton` for the mutton |
| `chicken` | `feather`, uniform 0 to 2; one `chicken` | `cooked_chicken` for the meat |

Each count is a uniform integer, both ends included. The two pools of a type are independent.

**ANIM-DROP-002** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact, confidence:
high). The meat drops in the cooked form when one of these conditions is true:

- the animal is on fire when it dies;
- the direct attacker holds in the main hand an item with an enchantment of
  `#minecraft:smelts_loot`.

**ANIM-DROP-003** (`source_suggested`, semantic, fidelity: distribution, confidence: medium). The
enchantment `looting` of the attacker, at the level `L`, adds `round(L × u)` to the count of each
item of ANIM-DROP-001 except the wool. `u` is a uniform value in `[0, 1)`, independent for each
item.

**ANIM-DROP-004** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact for a
guaranteed slot, distribution otherwise, confidence: high). The saddle of a pig drops at the death
in these cases:

- The slot is guaranteed (ANIM-INT-007, or the save value of the drop chance is more than 1). The
  saddle always drops, with its damage and its components, also with no player kill.
- Else, the drop chance of the slot is the default 0.085. The saddle drops only when a player killed
  the pig, with that probability.

The rules for each equipment slot of a mob are in the equipment specification. A saddle from
`summon` with an `equipment` key has the default chance (experiment ANIM-E7).

**ANIM-DROP-005** (`source_suggested`, semantic, fidelity: distribution, confidence: high). An
animal that dies drops experience when all these conditions are true:

- a player damaged it in the last 100 ticks;
- it is an adult;
- the game rule `mob_drops` is true.

The amount is a uniform integer from 1 to 3. A chicken jockey drops 10. `experience-orbs.md` states
the orbs.

**ANIM-SAVE-001** (`source_suggested`, semantic, fidelity: exact, confidence: high). An animal never
despawns because of the distance to a player or the idle time of AIT-VAN-020. Only a chicken
jockey despawns with these rules. The peaceful rule of AIT-VAN-020 step 1 does not remove an
animal. An animal is persistent when it has `PersistenceRequired:1b`. The age lock (ANIM-AGE-006)
and a name tag set this flag.

**ANIM-SAVE-002** (`source_suggested_black_box_confirmed`, semantic, fidelity: exact, confidence:
high). The save of an animal holds the fields of a mob and these fields. The save format of an
entity is the interface, so the table gives the keys.

| Field | Key | Type | Default when absent | Types |
|-------|-----|------|---------------------|-------|
| age | `Age` | int | 0 | each |
| fed growth | `ForcedAge` | int | 0 | each |
| age lock | `AgeLocked` | boolean | false | each |
| love time | `InLove` | int | 0 | each |
| love cause | `LoveCause` | UUID, int array of 4 | none | each |
| variant | `variant` | identifier | the default variant | `pig`, `cow`, `chicken` |
| sound variant | `sound_variant` | identifier | the default sound variant | `pig`, `cow`, `chicken` |
| wool colour | `Color` | byte, the colour id | 0, `white` | `sheep` |
| sheared | `Sheared` | boolean | false | `sheep` |
| egg timer | `EggLayTime` | int | a new random timer | `chicken` |
| chicken jockey | `IsChickenJockey` | boolean | false | `chicken` |
| saddle | `equipment.saddle` | item stack | empty | `pig` |
| drop chance of the saddle | `drop_chances.saddle` | float | 0.085 | `pig` |

A variant or a sound variant whose identifier is not in its registry keeps the default. The boost
of a pig is not saved. The ambient counter, the fed growth timer and the age lock cooldown are not
saved.

## 12. Experiments

The experiments ran on a vanilla dedicated server `26.4-snapshot-7` on 2026-11-08, with a flat
world, `difficulty=easy`, no player, and commands on the console. `fillbiome` set `snowy_plains` and
`desert` in two areas. `data get entity` and `execute if entity ... nbt=` read the state.

| Experiment | Steps | Result | Rules |
|------------|-------|--------|-------|
| ANIM-E1 | Run `summon pig 0.5 -60 0.5 {}`. Read the entity. | `variant` `temperate`, `sound_variant` `classic`, `Age` 0, `ForcedAge` 0, `AgeLocked` false, `InLove` 0, `Health` 10.0, `attributes` empty. | ANIM-SPAWN-001, ANIM-SAVE-002, TRK-TYPE-004 |
| ANIM-E2 | Summon 300 pigs without `nbt` in `plains`. Count the values. | 300 adults, 300 `temperate`. Sound variants: `classic` 90, `big` 99, `mini` 111. `LeftHanded` 14. `attributes`: `movement_speed` base 0.25, `follow_range` base 16 with `random_spawn_bonus` 0.0170. | ANIM-VAR-003, ANIM-VAR-004, ANIM-SPAWN-001, ANIM-SPAWN-002 |
| ANIM-E3 | Summon pigs, cows and chickens without `nbt` in `snowy_plains`, `desert` and `plains`. Count each variant. | `snowy_plains` gives `cold`, `desert` gives `warm`, `plains` gives `temperate`, for each type. Chickens: 16 of 30 `picky`. | ANIM-VAR-002, ANIM-VAR-003 |
| ANIM-E4 | Summon 600 sheep without `nbt`, 200 in each of the three biomes. Count each `Color`. | Only `white` 182, `brown` 180, `black` 168, `gray` 39, `light_gray` 31. Some sheep died of cramming before the count. | ANIM-VAR-005 |
| ANIM-E5 | Summon chickens with `{EggLayTime:5,NoAI:1b}`, with the variants `warm` and `cold`, and a baby with `Age:-100`. Wait 50 ticks. | One `brown_egg` and one `blue_egg`. The adults have new timers 8505 and 10672. The baby keeps 5, and its age is −50. | ANIM-TICK-001, ANIM-AGE-002 |
| ANIM-E6 | Summon two sheep with `InLove:600` and the colours `red` and `yellow`. Summon two cold pigs with `InLove:600` and `sound_variant` `big`. Wait. | Each pair makes one baby at the position of one parent, age −24000. Parents 6000, and one parent is one tick behind the other. Lamb `orange`. Piglet `cold` with `sound_variant` `classic`. Two orbs, each of value 7. Rotation `[0, 0]`. | ANIM-BREED-001 to ANIM-BREED-003, ANIM-DEV-002 |
| ANIM-E7 | With `NoAI`, kill 20 sheep of `light_blue`, a baby sheep, a sheared sheep, a cow with `Fire:200s`, a pig with a saddle from `equipment`, and a pig with the saddle and `drop_chances:{saddle:2.0f}`. | 32 `mutton` for 21 adults, 20 `light_blue_wool`, nothing from the baby. `cooked_beef` 2 and no leather from the burning cow. No saddle from the first pig. The saddle from the second pig. No experience orb. | ANIM-DROP-001, ANIM-DROP-002, ANIM-DROP-004, ANIM-DROP-005 |
| ANIM-E8 | Summon a cow with `{NoAI:1b,Age:-24000,AgeLocked:1b}`. Wait 100 ticks. | `Age` stays −24000. | ANIM-AGE-002 |

These experiments are planned. They need a client in the world.

| Experiment | Steps | Rules |
|------------|-------|-------|
| ANIM-E9 | Capture the spawn bundle of a pig with the variant `cold` and `sound_variant` `big`, and of a sheep with `Color:14b,Sheared:1b`. | TRK-DATA-006 |
| ANIM-E10 | Feed a baby cow with the age −24000 five times. Read `Age` and `ForcedAge` after each feed. Feed until it is an adult and read `Age`. | ANIM-AGE-003, ANIM-AGE-005 |
| ANIM-E11 | Feed an adult cow, read `InLove`, hit it, read `InLove`. | ANIM-LOVE-001, ANIM-LOVE-002 |
| ANIM-E12 | Shear 100 sheep and count the wool of each shear. | ANIM-INT-004 |
| ANIM-E13 | Ride a saddled pig with a carrot on a stick. Boost. Record the entry 18 and the damage of the item. Boost again at once. | ANIM-RIDE-003, ANIM-RIDE-004 |
| ANIM-E14 | Use `golden_dandelion` on a baby two times within 40 ticks, then after 40 ticks. Read `AgeLocked`, `Age` and `PersistenceRequired`. | ANIM-AGE-006 |
| ANIM-E15 | Drop a chicken from 20 blocks. Record `set_entity_motion` and the fall damage. | ANIM-TICK-002 |
| ANIM-E16 | Record the times of the ambient sounds of one pig with `NoAI` for 20000 ticks. | ANIM-TICK-004 |

## 13. Parallelism notes

This section helps the design of a tick that runs 50,000 animals in parallel. "Own state" is the
state of one animal. A rule that only reads and writes own state can run for each animal in any
order. A rule that changes another entity or the world needs a merge point.

| Group | Rules | Reads | Writes | Cadence | Effects on other entities |
|-------|-------|-------|--------|---------|---------------------------|
| age step | ANIM-AGE-002 | age, age lock, alive | age; entry 16 and the box at a crossing | each tick | none. A crossing changes the box, so the grid sees it at the next freeze |
| love step | ANIM-LOVE-002 | age, love time | love time | each tick | none |
| growth particles | ANIM-AGE-007 | two timers | two timers; particle packets | each tick while a timer is more than 0 | packets only |
| egg | ANIM-TICK-001 | egg timer, age, alive, jockey flag, variant | egg timer | each tick; an event about one time in 9000 ticks | a new item entity, and a sound. The item is a spawn request with a birth key of the kind `entity` |
| chicken fall | ANIM-TICK-002 | on-ground flag, velocity | velocity | each tick, inside the hot tick after the push | none |
| ambient sound | ANIM-TICK-004 | counter, alive | counter | each tick | a sound packet, about one time in 400 ticks |
| boost | ANIM-RIDE-003 | boost state, controller | boost state | each tick with a controller | none |
| breed | ANIM-BREED-001 | both parents | both parents; a child; an orb; a statistic | an event | a pair action at the merge point (AIT-CPL-004); the child and the orb are spawn requests |
| interactions | section 8 | the animal, the item in the hand | the animal, the stack, the inventory | an event of a packet | the item stack of the player and new item entities. The player domain applies the stack change |
| death drops | section 11 | the animal at its death, the attacker | none | an event | item entities and orbs as spawn requests |
| lightning | ANIM-TICK-005 | the pig | removal and a new mob | an event | a spawn request and a removal request |

These conclusions follow:

- The per-tick work of an animal that is not a goal reads and writes only its own state. It needs
  four small counters (age, love time, ambient counter, egg timer) and two short timers. Each can
  run in the tile task of the animal.
- An idle animal at rest changes no entity data in most ticks. The tracker sends an update only
  for the crossings of the age and the rare events.
- An animal at rest with no event due can skip its work in a tick. The skip must keep the
  counters exact. For example, the egg timer and the age can advance by `n` at a later
  tick, when nothing reads them before. A random event in the skipped ticks must keep its
  distribution. The ambient counter can find the tick of the next sound in one draw, from the
  distribution of ANIM-TICK-004.
- Each random value of these rules is a keyed random value (ANIM-DEV-001). The purpose of a value
  names the rule, for example "egg timer" or "ambient". Thus the result does not depend on the
  order of the tiles.
- Each new entity of these rules is a spawn request with a birth key of the kind `entity` and the
  parent key of the animal. The sequence counts the requests of one animal in one tick. The life
  cycle of `DomainInput` applies them in the order of the birth keys.

## 14. Intentional deviations

### ANIM-DEV-001: Keyed random values

- **Rule:** each rule with the fidelity class `distribution`.
- **Reference:** the egg timer, the sounds and the drops take values from the random source of the
  animal. The first spawn setup, the variant and the wool colour take values from the random source
  of the dimension. Loot takes values from the random sequence of the table. The values thus depend
  on the order of the entities.
- **This server:** each value is a keyed random value. The distribution stays the same.
- **Visible to the client:** no, as a distribution.
- **Test:** a differential test compares distributions over many runs.

### ANIM-DEV-002: The order of the breeds in one tick

- **Rule:** ANIM-BREED-001.
- **Reference:** the breed occurs in the tick of the initiator, in the order of the tick list. The
  partner does its own age step in the same tick when it ticks after the initiator. Thus its age
  is one less than the age of the initiator (experiment ANIM-E6).
- **This server:** the breed is a pair action at the merge point at the start of a tick
  (AIT-CPL-004). When both animals of a pair complete the breed goal in one tick, the animal with
  the smaller birth key is the initiator. Both parents then do the age step in the same tick, so
  their ages are equal.
- **Visible to the client:** no. The age of a parent differs by at most 1 tick.
- **Test:** a test accepts a difference of 1 in the ages of the parents.

### ANIM-DEV-003: Two recipes for one pair of colours

- **Rule:** ANIM-BREED-003.
- **Reference:** when the data pack has two recipes for one pair of dyes, the reference takes the
  first recipe that its recipe search finds. That order depends on the internal tables of the
  recipes.
- **This server:** the recipe with the smallest identifier wins. The order of the two parents does
  not matter.
- **Visible to the client:** only with a data pack that adds such a recipe. The vanilla data pack
  has none.
- **Test:** no test with the vanilla data.

### Known vanilla quirks

These quirks are vanilla behaviour. This server keeps them.

| Rule | Quirk |
|------|-------|
| ANIM-BREED-002 | The child takes the default sound variant, not the sound variant of a parent. |
| ANIM-AGE-005 | A baby that a feed makes adult waits the fed growth before it can breed. |
| ANIM-AGE-006 | The age lock resets the age of the baby to −24000, also when the player removes the lock. |
| ANIM-INT-006 | A creative player keeps the dye only through the general rule of ANIM-INT-001. |
| ANIM-TICK-005 | The saddle of a struck pig is lost. |
| ANIM-DROP-004 | A saddle from `summon` does not drop at a death without a player. |

## 15. Unresolved

- **U1**: ANIM-E9 to ANIM-E16 need a client. The values of ANIM-AGE-003, ANIM-RIDE-003 and
  ANIM-TICK-004 have no capture.
- **U2**: the exact form of the particle positions of ANIM-AGE-007 is an envelope. No test captured
  them.
- **U3**: `natural-spawning.md` must state how one spawn event forms a spawn group for
  ANIM-SPAWN-002. This applies to natural spawning and to the spawn of a chunk at its generation.
- **U4**: the entry `follow_range` is not syncable. This document assumes that the spawn bundle
  holds `movement_speed` with the base of the type (TRK-DATA-004). ANIM-E9 checks it.

## 16. Rules that `mob-goals.md` uses

| Value | Rule |
|-------|------|
| breeding food, per type | ANIM-FOOD-001 |
| tempt items, per type, and the extra tempt goal of the pig | ANIM-FOOD-002 |
| priorities and speed multipliers of each goal, per type | ANIM-FOOD-003 |
| the condition "can mate" | ANIM-FOOD-004 |
| the result of a breed | ANIM-BREED-001 |
| baby state, for follow parent and for the baby speed | ANIM-AGE-001 |
| love state, for the breed goal | ANIM-LOVE-001, ANIM-LOVE-002 |
| the "ate" event of the eat grass goal | ANIM-TICK-003 |
| path costs and the stroll target value | ANIM-TYPE-005 |
| the controller of a pig, which replaces the AI | ANIM-RIDE-001 |
| the tempt range | TRK-TYPE-004 (`tempt_range` 10) |
