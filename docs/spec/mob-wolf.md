# The wolf: type data, anger, goals, hunting and cost

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |
| Entity type | `minecraft:wolf` |
| Data | `assets/mcrs/reports/registries.json`, the data pack of the server jar, the type artefact |

This document states the server behaviour of the wolf. It covers the type data, the entity data,
the variants, the anger, the tame state, the goal lists, the work of each tick that is not a goal,
the hunt of other mobs, and the drops. It gives the cost of each part, so that a design can tick
many wolves in parallel among sheep and llamas.

The rules state results, not algorithms. Each rule has a fidelity class (`mob-goals.md`, section
0.3). The wolf is an animal in the sense of the age, the love and the breed of `mob-animals.md`
(prefix `ANIM`). This document states only the differences. `mob-goals.md` (prefix `GOAL`) states
the goal framework and the passive goals. `mob-combat-goals.md` (prefix `CGOAL`) states the combat
goals and the anger of a neutral mob.

## 0. Conventions

### 0.1 Terms

- The terms of `mob-goals.md`, section 0.1, of `mob-animals.md`, section 0.1, and of
  `mob-combat-goals.md`, section 0.1, apply. In this document, "animal" has the meaning of
  `mob-animals.md` only in a reference to an ANIM rule.
- **Wolf**: an entity of the type `minecraft:wolf`.
- **Wild wolf**: a wolf that is not tame.
- **Prey**: an entity of the type `sheep`, `rabbit` or `fox`.
- **Skeleton kind**: an entity of the type `skeleton`, `stray`, `wither_skeleton`, `bogged` or
  `parched`.
- **Llama kind**: an entity of the type `llama` or `trader_llama`.
- **Strength**: the strength of a llama kind, an integer from 1 to 5. The llama specification
  states it. A llama from `summon` with no `Strength` key has the strength 1.
- **Wet**, **shaking**, **shake progress**: three server states of a wolf (WOLF-TICK-001).
- **Interested**: the entity data flag of a wolf that begs (WOLF-DATA-001, entry 20).
- **Passive mode**: the state of a wild adult wolf with no attack target, no running target goal,
  no player within 8 blocks, no llama kind within its avoid box, and no prey, skeleton kind or
  turtle in its search box.

### 0.2 Provenance, kind, confidence and fidelity

The labels of `mob-goals.md`, sections 0.2 and 0.3, apply. No rule of this document has a captured
result yet. Section 14 attaches the experiments.

### 0.3 ID prefix

| Prefix | Subject |
|--------|---------|
| `WOLF-TYPE` | type data and attributes |
| `WOLF-DATA` | entity data |
| `WOLF-VAR` | variants and sound variants |
| `WOLF-AGE` | age, love and breed: differences to ANIM |
| `WOLF-ANGER` | the anger of a wolf |
| `WOLF-TAME` | the tame state and its hooks |
| `WOLF-GOAL` | the goal list |
| `WOLF-TGT` | the target list |
| `WOLF-TICK` | the work of each tick that is not a goal |
| `WOLF-HUNT` | the hunt of other mobs, and the flight from llamas |
| `WOLF-DROP` | drops and experience |
| `WOLF-WAKE` | the full evaluations that a wolf can skip |
| `WOLF-COST` | the cost of a wolf, and the comparison to a pig |
| `WOLF-DEV` | intentional deviations |

### 0.4 Scope

In scope: the wild wolf among sheep, llamas and players, with all the state that a client sees.
The tame state is in scope as data, with hooks for the interactions.

Out of scope:

| Subject | Where |
|---------|-------|
| What the client draws: models, textures, the tail, the head roll, the shake | a client specification |
| The armor of a wolf, its damage, its repair and its crack sound | hook (WOLF-TAME-004) |
| The interactions of a player with a tame wolf: feed, dye, armor, sit order | hook (WOLF-TAME-003) |
| The leash of a wolf | hook; a wolf that is angry cannot take a leash |
| Natural spawning and the pack | `natural-spawning.md` (planned); hook WOLF-VAR-003 |
| The llama, its strength, its spit and its attack on wolves | the llama specification |
| Rain | hook. WOLF-TICK-003 states the dry world |

## 1. Type data

**WOLF-TYPE-001** (`source_suggested_black_box_confirmed`, wire, confidence: high, fidelity:
exact). The type data of the wolf (TRK-TYPE-001):

| Field | Value |
|-------|-------|
| Wire id | 154 |
| Category | walker, mob category `creature` |
| Adult width × height | `0.6F × 0.85F`, scalable |
| Adult eye height | `0.68F` |
| Baby width × height, eye height | `0.3F × 0.425F`, `0.34375F` |
| Passenger attachment | adult `(0, 0.81875, −0.0625)`; baby `(0, 0.4375F, 0)` |
| Client tracking range | 10 |
| Update interval | 3 |
| Fire immune | no |

The type artefact confirms the adult size, the eye height, the range and the interval. The wire id
comes from `assets/mcrs/reports/registries.json`. A baby has the baby dimensions, not the adult
dimensions times a scale (as ANIM-TYPE-001).

**WOLF-TYPE-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The default
attributes of the wolf are those of a living entity (TRK-TYPE-003), with these values:

| Attribute | Base value |
|-----------|-----------|
| `max_health` | 8 for a wild wolf; 40 for a tame wolf (WOLF-TAME-002) |
| `movement_speed` | `0.3F`, widened: `0.30000001192092896` |
| `attack_damage` | 4 |
| `follow_range` | 16 |
| `tempt_range` | 10 |

`entity-damage.md`, Appendix A, gives the health, the armor and the hit kind. The step height is the
default 0.6. On grass, a wolf with the speed modifier 1 walks about `0.3² × 2.2026 = 0.198` blocks in
each tick (GOAL-CTL-002).

**WOLF-TYPE-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). The malus table
of the wolf (PATH-MAL-001):

| Path type | Malus |
|-----------|-------|
| `FIRE_IN_NEIGHBOR` | 16 |
| `FIRE` | −1 |
| `POWDER_SNOW` | −1 |
| `ON_TOP_OF_POWDER_SNOW` | −1 |

The follow owner goal changes the malus of `WATER` while it runs (CGOAL-OWNER-004). The walk value of
a candidate is GOAL-POS-004: 10 above `grass_block`, else the light value.

**WOLF-TYPE-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). The head turn
speed is 10, and the head yaw limit is 75. The pitch limit is 20 while the sitting pose is set, else
40. The breeding food is the item tag `wolf_food`. In `26.4-snapshot-7` the tag holds `#meat`, `cod`,
`cooked_cod`, `salmon`, `cooked_salmon`, `tropical_fish`, `pufferfish` and `rabbit_stew`. A wolf has
no tempt goal. The interesting items of the beg goal are `bone` and each item of `wolf_food`.

**WOLF-TYPE-005** (`source_suggested`, semantic, confidence: high, fidelity: distribution). The
sounds of a wolf come from its sound variant (WOLF-VAR-002). The sound set is the baby set of the
variant for a baby, else the adult set. The volume is 0.4, and 0.15 for a step.

- Ambient: the growl sound while the wolf is angry. Else, with a chance of 1 in 3, the pant sound,
  or the whine sound for a tame wolf with less than 20 health. Else the ambient sound.
- Hurt: the hurt sound of the set. A hit that the armor takes plays `entity.wolf.armor_damage`
  (hook).
- Death: the death sound of the set.

The ambient sound follows ANIM-TICK-004, with the interval 120.

## 2. Entity data

**WOLF-DATA-001** (`source_suggested_black_box_confirmed`, wire, confidence: high, fidelity:
exact). The chain of the wolf is: entity, living entity, mob, ageable mob, tame mob, then the wolf.
Indices 0 to 17 are those of TRK-DATA-002 and TRK-DATA-006. The other entries:

| Index | Meaning | Serializer | Default |
|-------|---------|------------|---------|
| 18 | tame flags: bit 0 sitting pose, bit 2 tame | 0 | 0 |
| 19 | owner: an optional UUID of a living entity | 13 | absent |
| 20 | interested | 8 | false |
| 21 | collar colour, a colour id (TRK-DATA-006) | 1 | 14 (`red`) |
| 22 | anger end time, a game time | 2 | −1 |
| 23 | wolf variant | 25 | the id of `minecraft:pale` |
| 24 | wolf sound variant | 26 | the id of `minecraft:classic` |

The type artefact confirms each index and each serializer. A variant id is the position of the
entry in the registry data that the server sent, from 0 (TRK-DATA-006).

**WOLF-DATA-002** (`source_suggested`, wire, confidence: high, fidelity: exact). These events change
an entry. Each change makes the entry dirty (TRK-DATA-005):

| Entry | Writer |
|-------|--------|
| 15, bit 2 (aggressive) | the start and the stop of the melee goal (CGOAL-MELEE-003) |
| 16 | the age crosses 0 (ANIM-AGE-002) |
| 18 | the sit goal; a tame action (WOLF-TAME-001) |
| 20 | the start and the stop of the beg goal |
| 22 | the anger update (WOLF-ANGER-002) |
| 23, 24 | the first spawn setup, the breed, `summon` and a load |

A wild wolf in passive mode changes no entry in most ticks.

## 3. Variants and sound variants

**WOLF-VAR-001** (`documentation`, semantic, confidence: high, fidelity: exact). The registry
`minecraft:wolf_variant` of the data pack has nine entries. Each entry has a spawn selector:

| Entry | Selector |
|-------|----------|
| `minecraft:pale` | priority 0, no condition. It is the default |
| `minecraft:spotted` | priority 1, the biome is in `#minecraft:is_savanna` |
| `minecraft:snowy` | priority 1, the biome is `minecraft:grove` |
| `minecraft:black` | priority 1, the biome is `minecraft:old_growth_pine_taiga` |
| `minecraft:ashen` | priority 1, the biome is `minecraft:snowy_taiga` |
| `minecraft:rusty` | priority 1, the biome is in `#minecraft:is_jungle` |
| `minecraft:woods` | priority 1, the biome is `minecraft:forest` |
| `minecraft:chestnut` | priority 1, the biome is `minecraft:old_growth_spruce_taiga` |
| `minecraft:striped` | priority 1, the biome is in `#minecraft:is_badlands` |

The textures are client data. The server sends only the variant id. The data pack is the authority.
The server selects a variant from the selectors as ANIM-VAR-003 states for the farm animals.

**WOLF-VAR-002** (`documentation`, semantic, confidence: high, fidelity: exact). The registry
`minecraft:wolf_sound_variant` has seven entries: `classic`, `puglin`, `sad`, `angry`, `grumpy`,
`big` and `cute`. Each entry has an adult set and a baby set. Each set names the ambient, death,
growl, hurt, pant, whine and step sounds. The default is `classic`.

**WOLF-VAR-003** (`source_suggested`, semantic, confidence: high, fidelity: distribution). The
first spawn setup of a wolf does these steps:

1. When the setup has the pack data of a wolf, take the variant of the pack. Else select a variant
   for the biome at the block position (WOLF-VAR-001), and start the pack data with it.
2. Select the sound variant uniformly from all entries of the sound variant registry. With the
   vanilla data pack, each entry has the chance 1/7.
3. The pack data never makes a baby. A wolf of the first spawn setup is an adult.

A wolf from `summon minecraft:wolf <pos> {}` skips the setup (TRK-SUM-003). It has the variant
`pale` and the sound variant `classic`. The save keeps the variant as `variant`, the sound variant as
`sound_variant` and the collar colour as `CollarColor`.

## 4. Age, love and breed

**WOLF-AGE-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). These ANIM rules
apply to the wolf with no change: ANIM-AGE-001 to ANIM-AGE-003, ANIM-AGE-005 to ANIM-AGE-007,
ANIM-LOVE-001 to ANIM-LOVE-003. The breeding food is `wolf_food`. A wild wolf can enter love when a
player feeds it an item of `wolf_food`. A player who holds `bone` tries to tame a wild wolf in place
of the feed (WOLF-TAME-001).

**WOLF-AGE-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). Two wolves can
mate when all these conditions are true. This rule replaces ANIM-FOOD-004 for the wolf.

- they are two different wolves, and both are tame;
- the partner does not have the sitting pose;
- both are in love.

Thus a wild wolf in love never finds a partner. Its breed goal never starts, but it is busy
(GOAL-WAKE-004) until its love time ends.

**WOLF-AGE-003** (`source_suggested`, semantic, confidence: high, fidelity: distribution). The child
of a breed differs from ANIM-BREED-002 in these values:

- the variant is the variant of the initiator or of the partner, each with the chance 1/2;
- the sound variant is uniform over the sound variant registry. It is not `classic` by default;
- when the initiator is tame: the child is tame, with the owner of the initiator, and with the
  tame side effects (WOLF-TAME-002). Its collar colour is the mix of the two collar colours by the
  rule of ANIM-BREED-003.

The other steps of ANIM-BREED-001 apply.

## 5. Anger

**WOLF-ANGER-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The wolf is a
neutral mob (CGOAL-RESET-ANGER-001). Its anger end time is the entity data entry 22. The law of the
anger duration is uniform over the integers from 400 to 780 ticks. A client sees an angry wolf: the
anger end time is more than the game time.

**WOLF-ANGER-002** (`source_suggested`, semantic, confidence: high, fidelity: exact for the state,
distribution for the duration). In each tick in which the wolf ticks, after its AI step and its
travel step, the wolf does the anger update of CGOAL-RESET-ANGER-003 with the option "stay angry
while a target is present" true. The update also runs for a wolf with `NoAI:1b`.

These consequences follow:

- Each attack target makes the wolf angry: a sheep of a hunt, a skeleton, a llama, a player.
- While the attack target exists, each tick draws a new duration and writes the end time. The entry
  22 changes in almost each tick. Thus the tracker sends `set_entity_data` with the entry 22 in each
  tick of a hunt (TRK-UPD-001).
- When the target of the anger dies and is a mob, the anger stops in the next update: the end time
  becomes −1, and the attack target and the hurt-by record clear.
- When the attack target becomes none for another reason, the wolf stays angry until the end time.
  Then step 3 of CGOAL-RESET-ANGER-003 stops the anger.

**WOLF-ANGER-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). While a wolf
is angry:

- its ambient sound is the growl (WOLF-TYPE-005);
- a player cannot tame it with a bone (WOLF-TAME-001);
- it cannot take a leash.

## 6. Tame state

**WOLF-TAME-001** (`source_suggested`, semantic, confidence: medium, fidelity: distribution). Hook.
A player uses `bone` on a wild wolf that is not angry. The server consumes one bone (ANIM-INT-001).
With a chance of 1 in 3, the wolf becomes tame:

1. set the tame bit and the owner to the player, with the tame side effects (WOLF-TAME-002);
2. run the advancement trigger `minecraft:tame_animal`;
3. clear the path; set the attack target to none; set "ordered to sit";
4. send `entity_event` 7 for the wolf.

Else the server sends `entity_event` 6. A wolf that is ordered to sit and on the ground starts the
sit goal in its next full evaluation.

**WOLF-TAME-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The tame side
effects. When the wolf becomes tame, the base of `max_health` becomes 40, and the health becomes 40.
When it becomes wild, the base becomes 8. A load of a save with an owner sets the tame bit with no
side effects: the saved attributes stay. A load of a save with no owner sets the wolf wild with the
side effects. The save keeps the owner as `Owner` and "ordered to sit" as `Sitting`. A load sets the
sitting pose from `Sitting`.

**WOLF-TAME-003** (`source_suggested`, semantic, confidence: medium, fidelity: exact). Hook. The
interactions of a player with a tame wolf: feed with `wolf_food` to heal, dye the collar with an
item of `wolf_collar_dyes`, put on or repair the body armor, and toggle "ordered to sit". A toggle by
the owner also clears the jump input, the path and the attack target.

**WOLF-TAME-004** (`source_suggested`, semantic, confidence: medium, fidelity: exact). Hook. A hit
on a wolf clears "ordered to sit", before the tests of DMG-HIT-001 except the invulnerability. The
body armor `wolf_armor` takes a hit whose damage type is not in `bypasses_wolf_armor`.

**WOLF-TAME-005** (`source_suggested`, semantic, confidence: high, fidelity: exact). The "wants to
attack" test of a tame wolf for a candidate `C` and the owner `O` (CGOAL-OWNER-001) is false when one
of these is true:

- `C` is a `creeper`, a `ghast` or an `armor_stand`;
- `C` is a tame wolf with the owner `O`;
- `C` is a player, `O` is a player, and the team rules do not let `O` harm `C` (hook);
- `C` is a tame horse kind, or another tame mob.

Else it is true.

## 7. The goal list

**WOLF-GOAL-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The goal list
of the wolf, in this order:

| Priority | Goal | Flags | Parameters |
|----------|------|-------|------------|
| 1 | float (GOAL-FLT-001) | `jump` | |
| 1 | panic (GOAL-PNC-002), tame form (WOLF-GOAL-002) | `move` | `m = 1.5`; damage tag `panic_environmental_causes` |
| 2 | sit when ordered (CGOAL-OWNER-006) | `jump`, `move` | |
| 3 | avoid entity (CGOAL-AVOID-001), wolf form (WOLF-GOAL-003) | `move` | type set: llama kind; `D = 24`; `m1 = 1.5`; `m2 = 1.5` |
| 4 | leap at target (CGOAL-LEAP-001) | `jump`, `move` | `y0 = 0.4` |
| 5 | melee attack (CGOAL-MELEE-001) | `move`, `look` | `m = 1.0`; follow a target that it does not see; `I = 20` |
| 6 | follow owner (CGOAL-OWNER-004) | `move`, `look` | `m = 1.0`; `a = 10`; `b = 2` |
| 7 | breed (GOAL-BRD-001) | `move`, `look` | `m = 1.0` |
| 8 | stroll (GOAL-STR-001) | `move` | `m = 1.0` |
| 9 | beg (CGOAL-OWNER-007) | `look` | `L = 8` |
| 10 | look at a player (GOAL-LKP-001) | `look` | range 8 |
| 10 | look around (GOAL-LKR-001) | `move`, `look` | |

The look goal of the wolf has the range 8 in place of 6. Each part of GOAL-LKP-001 and
`Q-LOOK` uses 8.

**WOLF-GOAL-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The panic of a
wolf reads the damage tag `panic_environmental_causes`, not `panic_causes`. In `26.4-snapshot-7` the
tag holds `cactus`, `freeze`, `hot_floor`, `sulfur_cube_hot`, `in_fire`, `lava`,
`lightning_bolt` and `on_fire`. Thus a melee hit, a projectile and a player attack do not make a
wolf panic. The tame form adds a teleport to the owner in each goal tick (CGOAL-OWNER-005).

**WOLF-GOAL-003** (`source_suggested`, semantic, confidence: high, fidelity: distribution for the
draw, exact for each other part). The wolf form of the avoid goal:

- Extra start test (CGOAL-AVOID-005): the entity to avoid is a llama kind, the wolf is not tame,
  and the strength `s` of the llama is at least a random integer uniform in `[0, 5)`. The chance is
  `min(s + 1, 5) / 5`: 2/5 for strength 1, 1 for strength 4 or 5.
- Start action: set the attack target to none. Then the start action of CGOAL-AVOID-005.
- Goal tick: set the attack target to none. Then the goal tick of CGOAL-AVOID-005.

The test runs after the base start condition: after `Q-AVOID`, the away search and the path search.

**WOLF-GOAL-004** (`design`, semantic, confidence: high, fidelity: distribution for the draw,
exact for each other part). This server tests "the wolf is not tame" before `Q-AVOID`. It draws the
strength test after step 1 of CGOAL-AVOID-004, before the away search (CGOAL-AVOID-007). A tame
wolf and a failed draw do no away search and no path search. CGOAL-DEV-002 records the change of
the order of the draws.

**WOLF-GOAL-005** (`source_suggested`, semantic, confidence: high, fidelity: exact). These relations
follow from the table and GOAL-FRM-002. A player can see them:

- The panic and the sit interrupt each other goal that holds `move`. The float holds only `jump`,
  so it blocks the leap and the sit.
- The avoid goal interrupts the leap, the melee, the follow owner, the breed, the stroll and the look
  around. While it runs, it clears the attack target in each full evaluation.
- The leap interrupts the melee. The melee then waits for its next start check (CGOAL-MELEE-005).
- The melee holds `look`, so the beg and the look goals do not run during an attack.
- The avoid goal is examined in each full evaluation in which the panic and the sit do not run.

## 8. The target list

**WOLF-TGT-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The target list
of the wolf, in this order:

| Priority | Goal | Parameters |
|----------|------|------------|
| 1 | owner-hurt-by target (CGOAL-OWNER-001) | the test of WOLF-TAME-005 |
| 2 | owner-hurt target (CGOAL-OWNER-002) | the test of WOLF-TAME-005 |
| 3 | hurt-by target (CGOAL-HURT-002) | no ignored type; alert with no excepted type |
| 4 | nearest attackable target (CGOAL-NEAR-004) | the players; `r = 10`; must see; filter: the wolf is angry at the player (CGOAL-RESET-ANGER-004) |
| 5 | nearest attackable target, non-tame variant (CGOAL-NEAR-007) | type set: each animal; `r = 10`; filter: the candidate is a prey |
| 6 | nearest attackable target, non-tame variant | type set: `turtle`; `r = 10`; filter: the turtle is a baby and is not in water |
| 7 | nearest attackable target (CGOAL-NEAR-004) | type set: skeleton kind; `r = 10`; does not need to see |
| 8 | reset universal anger (CGOAL-RESET-ANGER-005) | alert others of the same type |

Each goal of priority 1 to 7 holds `target`. Each goal uses the follow distance 16. "Each animal"
means each entity whose type can breed and age as an animal. It includes each wolf, each llama kind
and each sheep, before the filter.

**WOLF-TGT-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). These relations
follow:

- A wild wolf hunts prey and baby turtles. A tame wolf does not, but a tame wolf attacks each
  skeleton kind that it sees.
- The hurt-by goal interrupts a hunt and a skeleton attack. A wolf that a llama spits at targets the
  llama, and alerts each wild wolf in the alert box with no attack target.
- The hunt of prey interrupts an attack on a skeleton kind (5 before 7). An attack on a skeleton
  kind does not interrupt a hunt.
- A wild wolf attacks a player only after the player hit it (hurt-by) or after the wolf received an
  alert, or while it is angry at the player. The player goal of priority 4 renews the attack on a
  player that the anger names.

## 9. The work of each tick that is not a goal

**WOLF-TICK-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The wet state.
A wolf keeps three server states: wet, shaking and the shake progress `p`, an f32. They start
false, false and 0. The save does not keep them. In each tick in which the wolf is alive, after its
AI step and travel step, the wolf does these steps:

1. When the wolf is in water or in rain: set wet. When it is shaking: send `entity_event` 56 for the
   wolf, clear shaking and set `p` to 0.
2. Else, when it is shaking:
   1. When `p` is 0: play `entity.wolf.shake` with the volume 0.4 and the pitch
      `(u1 - u2) × 0.2 + 1`, with `u1` and `u2` uniform in `[0, 1)`. Emit the game event
      `entity_action`.
   2. Let `p0 = p`. Add `0.05F` to `p` (f32).
   3. When `p0` is 2.0 or more: clear wet and shaking, and set `p` to 0.

The particles of the shake are client work. The server sends none.

**WOLF-TICK-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The start of a
shake. In each tick, after the travel step of its AI step, the wolf starts to shake when all these
conditions are true: it is wet, it is not shaking, it has no path that is not done, and it is on the
ground. The start sets shaking, sets `p` to 0, and sends `entity_event` 8 for the wolf. Step 2 of
WOLF-TICK-001 then runs in the same tick and plays the sound.

The f32 sum of `0.05F` reaches 2.0 or more in `p0` only in the 42nd step. Thus a shake that water
does not stop lasts 42 ticks, and it plays one sound. A death clears the wet state and the shake.

**WOLF-TICK-003** (`design`, semantic, confidence: medium, fidelity: exact under the conditions
below). A server without the fluid model and without rain uses "not in water and not in rain" for
each wolf (GOAL-FLT-003). Then no wolf becomes wet, no shake starts, and the steps of WOLF-TICK-001
and WOLF-TICK-002 do nothing. The rule is exact on a dry world in clear weather, as GOAL-FLT-003
states. Rain at the position of a wolf is a hook.

**WOLF-TICK-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). The head roll
of an interested wolf is client work. The server keeps no observable state for it. Only the entry 20
reaches the client.

**WOLF-TICK-005** (`source_suggested`, semantic, confidence: high, fidelity: exact). The per-tick
work of a wolf, in each tick in which it ticks:

1. the age step and the love step (ANIM-AGE-002, ANIM-LOVE-002);
2. the ambient sound (WOLF-TYPE-005, ANIM-TICK-004);
3. the start of a shake (WOLF-TICK-002), and the anger update (WOLF-ANGER-002), both in the AI step;
4. the wet and shake steps (WOLF-TICK-001).

## 10. Hunting and flight

**WOLF-HUNT-001** (`source_suggested`, semantic, confidence: high, fidelity: distribution for the
waits, exact for each other part). The hunt of a wild wolf. In each full evaluation, the hunt goal
of priority 5 passes its gate with the chance 1/5. Then it selects the nearest prey that it sees
within 16 blocks (`max(16 × v, 2)`). Its start sets the attack target. In the same full evaluation
the melee goal can start, when 20 game ticks passed after its last start check. The melee goal
needs a path, or the prey in reach. Then the wolf runs to the prey with the speed modifier 1.0 and
hits it each 20 ticks while it is in reach and in sight.

The hunt goal stops when the prey is dead, out of sight, or more than `max(16 × v, 2)` blocks away
(CGOAL-NEAR-007). Its stop sets the attack target to none. The melee goal then stops in its next
full evaluation.

**WOLF-HUNT-002** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The result of
a hunt for each prey type, with the attack damage 4 and no armor on the prey:

| Prey | Health | Hits to kill | Reaction to the first hit |
|------|--------|--------------|---------------------------|
| `sheep` | 8 | 2 | panic with `m = 1.25`, about 0.182 blocks in each tick (GOAL-PNC-002) |
| `rabbit` | 3 | 1 | none |
| `fox` | 10 | 3 | the fox specification |

`mob_attack` is in `panic_causes`, so a sheep panics after each hit. A wolf with `m = 1.0` walks
about 0.198 blocks in each tick, so it catches a sheep in panic. The kill of a prey ends the anger
of the wolf (WOLF-ANGER-002). A wild wolf gives no player credit, so the prey drops no experience
(ANIM-DROP-005). Its loot drops (ANIM-DROP-001). A tame wolf gives the credit of its owner: the prey
drops experience as after a hit by the owner.

**WOLF-HUNT-003** (`source_suggested`, semantic, confidence: medium, fidelity: distribution). The
leap during a hunt. While the prey is 2 to 4 blocks away and the wolf is on the ground, the leap goal
starts with the chance 1/3 in each full evaluation. It sets the velocity toward the prey with the
vertical speed 0.4. It stops the melee goal: the aggressive bit clears and the path clears. After
the landing, the melee goal starts again at its next start check, at most 20 ticks after its last
check. Thus a hunt alternates leaps and runs.

**WOLF-HUNT-004** (`source_suggested`, semantic, confidence: high, fidelity: distribution for the
draw, exact for each other part). The flight from a llama kind. In each full evaluation in which the
avoid goal is examined, a wild wolf with a llama kind in sight within 24 blocks (`max(24 × v, 2)`),
inside the avoid box, tries a flight. The flight starts when the away search and the path search
succeed and the strength draw passes (WOLF-GOAL-003). The wolf then runs away with the speed modifier
1.5, about 0.446 blocks in each tick on grass. The flight clears the attack target at its start and
in each full evaluation while it runs.

A flight that interrupts a hunt leaves the hunt goal running: the hunt goal reads its own candidate,
not the attack target (CGOAL-NEAR-007). Thus after the flight the wolf has no attack target, but the
hunt goal holds `target`. No other target goal of a larger priority number can start. The wolf does
not attack again until the hunt goal stops by CGOAL-NEAR-007, or a target goal of priority 1 to 4
starts. WOLF-DEV-002 states that this server keeps this behaviour.

**WOLF-HUNT-005** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The skeleton
kind. Each wolf, wild or tame, can select a skeleton kind that it sees within 16 blocks. The goal
does not need to see the target to continue. It continues while the attack target exists and is
within the follow distance 16 (CGOAL-NEAR-003). The flight from a llama clears the
attack target, so it ends this goal in the next full evaluation.

**WOLF-HUNT-006** (`source_suggested`, semantic, confidence: medium, fidelity: exact). A llama kind
that hits a wolf with a spit sets the hurt-by record of the wolf: the causing entity of the spit is
the llama. The hurt-by goal then starts, and the wolf alerts the wolves of CGOAL-HURT-004. Each alerted
wolf runs to the llama, unless its avoid goal starts first. The llama specification states which
wolves a llama attacks.

## 11. Drops and experience

**WOLF-DROP-001** (`source_suggested`, semantic, confidence: high, fidelity: distribution). The loot
table `minecraft:entities/wolf` is empty. A wolf drops its body armor and the items of its equipment
slots by the general rules (hook). A dead wolf drops experience by the rule of ANIM-DROP-005: an
adult that a player damaged in the last 100 ticks drops a uniform integer from 1 to 3, when
`mob_drops` is true.

**WOLF-DROP-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). A tame wolf
that dies sends its death message to its owner, when the owner is a player online and the game rule
`show_death_messages` is true. A wolf never despawns by the distance rules (ANIM-SAVE-001).

## 12. Sleep, wake and cost

### 12.1 The full evaluations that a wolf can skip

**WOLF-WAKE-001** (`design`, semantic, confidence: medium, fidelity: exact). The rules of
GOAL-WAKE-001 to GOAL-WAKE-008 apply to the wolf, with these additions. A full evaluation of a wolf
is quiet only when the conditions of GOAL-WAKE-002 are true, and the probes of WOLF-WAKE-003 are all
empty.

**WOLF-WAKE-002** (`design`, semantic, confidence: medium, fidelity: distribution for the gates).
The gates of the wolf (GOAL-WAKE-003):

| Goal | `p` | Parts before the gate |
|------|-----|-----------------------|
| stroll | `1/60` | no passenger controls the wolf; the idle time is less than 100 |
| look at a player | `0.02` | none |
| look around | `0.02` | none |
| leap | `1/3` | no controlling passenger; an attack target 2 to 4 blocks away; on the ground |
| target goal, priority 4 | `1/5` | none |
| target goals, priority 5 and 6 | `1/5` | the wolf is not tame |
| target goal, priority 7 | `1/5` | none |

The empty-probe rule of CGOAL-NEAR-011 applies to the four target goals.

**WOLF-WAKE-003** (`design`, semantic, confidence: medium, fidelity: exact). The goals with no
gate need a probe in each full evaluation. A wolf is busy when one probe is not empty:

| Goal | Probe | Empty when |
|------|-------|-----------|
| avoid (priority 3) | the llama kind index, in the avoid box | the wolf is tame, or no llama kind intersects the box |
| beg (priority 9) | the player table | no qualified player within 8 blocks holds `bone` or `wolf_food` |
| target goal, priority 4 | the anger state | the anger target is none or is not a player, and the wolf is not angry at all players (CGOAL-RESET-ANGER-004) |
| target goal, priority 5 | the prey index, in the search box | the wolf is tame, or no prey intersects the box |
| target goal, priority 6 | the turtle index, in the search box | the wolf is tame, or no baby turtle on land intersects the box |
| target goal, priority 7 | the skeleton kind index, in the search box | no skeleton kind intersects the box |

The probe of priority 4 reads only the own state. A probe that is empty gives the same result as
the query of the reference, because the query result is a subset of the probe result.

**WOLF-WAKE-004** (`design`, semantic, confidence: medium, fidelity: exact). A wolf is also busy
while one of these conditions is true: it has an attack target; a target goal of priority 1 to 7
runs; the avoid, the leap, the melee, the follow owner or the beg goal runs; it is tame and its
owner is present; it is in love or is a baby (GOAL-WAKE-004). These are its wake events, in
addition to GOAL-WAKE-005: an alert of CGOAL-HURT-004 that sets its attack target, and a change of
the tame state.

### 12.2 Cost

**WOLF-COST-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The reference
work of a wild adult wolf in passive mode, in each full evaluation (each second tick):

| Part | Query | Cadence |
|------|-------|---------|
| avoid (3) | enumerate each llama kind in the box of the wolf grown by 24 horizontally and 3 vertically: about `48.6 × 6.85 × 48.6` blocks | each evaluation; no gate |
| beg (9) | scan the player table; one ray for each qualified player within 8 blocks | each evaluation; no gate |
| look (10) | scan the player table, with rays | when the gate `0.02` passes |
| target 4 | scan the player table; the filter "angry at" comes before the range and the ray | gate `1/5` |
| target 5 | enumerate each animal in the box grown by 16 on each axis: about `32.6 × 32.85 × 32.6` blocks | tame test, then gate `1/5` |
| target 6 | enumerate each turtle in the same box | tame test, then gate `1/5` |
| target 7 | enumerate each skeleton kind in the same box | gate `1/5` |
| melee start check | none while the attack target is none | at most each 20 ticks |
| other goals | own state and the gates of the stroll and the look around | each evaluation |

Thus the reference does one box enumeration of about 16,000 cubic blocks, and on average 0.6 box
enumerations of about 35,000 cubic blocks, in each full evaluation. The enumeration of target 5
visits each animal of the box: the sheep, the llamas and the other wolves. A dense pen with `n`
animals in each such box costs about `0.2 × n` type tests for target 5 alone, in each full
evaluation.

**WOLF-COST-002** (`design`, semantic, confidence: high, fidelity: exact). This server answers each
query of WOLF-COST-001 with the probes of WOLF-WAKE-003. In passive mode each probe is empty. Then
the work of a wolf in a full evaluation is:

- four index probes: the llama kind index, the prey index, the turtle index and the skeleton kind
  index. Each probe reads the cells of a sparse index that the box intersects;
- one read of the player table for the beg, as for the tempt of a pig. With no non-spectator player
  in the dimension, this read is empty at once;
- the gates of the stroll, the look and the look around, as for a pig, as waits (GOAL-WAKE-003).

With all probes empty and no wait due, the evaluation is quiet (WOLF-WAKE-001). The cost then
equals the cost of a quiet pig, plus four probes. A server can do the probes for many wolves in one
batch, because each probe reads only the index and the box of the wolf.

**WOLF-COST-003** (`design`, semantic, confidence: medium, fidelity: exact). The comparison with an
adult pig in passive mode (GOAL-WAKE-019):

| Work in each full evaluation | Pig | Wolf, reference | Wolf, this server |
|------------------------------|-----|-----------------|-------------------|
| player table reads | 1 (tempt) | 1 (beg) + 0.2 (target 4) + look gate | 1 (beg), the target 4 probe reads only own state |
| box enumerations | 0 | 1 + 0.6 on average | 0 |
| index probes | 0 | 0 | 4 |
| rays | 0 with no player near | 1 for each llama kind in range, each player in 8 blocks, each candidate of a passed gate | 0 in passive mode |
| path searches | at a stroll start | at a stroll start; at each flight try near a llama | at a stroll start; at a flight try that passed the strength draw |
| entity data writes | 0 | 0 | 0 |

Outside passive mode, the main costs of a wolf are:

- a hunt: one ray in each full evaluation for the hunt continue, the melee path requests each 4 to
  10 ticks while the prey moves to another block, and one write of the entry 22 in each tick
  (WOLF-ANGER-002);
- a llama near: in each examined evaluation, the rays to the llamas in range, then the away search
  and one path search before the strength draw in the reference. WOLF-GOAL-004 removes the away
  search and the path search for a failed draw and for a tame wolf.

## 13. Parallelism notes

| Part | Place | Reads | Writes | Effects on other entities |
|------|-------|-------|--------|---------------------------|
| goal list and target list | decisions (CGOAL section 7) | own state; snapshot; indices | own guidance; own attack target | alerts (pair actions); melee hits |
| anger update | reflex, each tick | own anger state; liveness of the stored target | anger state; entry 22; attack target | none |
| wet and shake | reflex, each tick | own fluid and rain flags, on-ground flag, path state | three own states; entity events 8 and 56; one sound | none |
| age, love, ambient sound | reflex, each tick | own counters | own counters; sound packets | none |
| melee hit | reflex | own cooldown; target box; ray | a hit on the target | the hit procedure on the target (DMG-ORD-002) |
| tame action | an event of a packet | the wolf, the item | the wolf; the stack | hook |

These conclusions follow:

- Each per-tick part of a wolf reads and writes only the own wolf, except the melee hit. A tile task
  can run them.
- The snapshot must publish these values of each wolf: the tame state, the owner reference, the
  attack target, the type, and the box. The llama, the prey and the skeleton kinds must publish their
  type and box to the type indices.
- The type indices change only when an entity moves into another cell, is added or is removed. They
  can be built one time in each tick, after the move of each entity.
- Each random value is a keyed random value. The purpose names the rule, for example "wolf anger
  duration" or "wolf avoid strength".

## 14. Intentional deviations, unresolved items and experiments

### WOLF-DEV-001: The order of the avoid work

- **Rule:** WOLF-GOAL-004.
- **Reference:** the reference makes the away search and the path search before the tame test and
  the strength draw.
- **Visible to the client:** no, as a distribution.
- **Test:** E-W4.

### WOLF-DEV-002: Kept vanilla quirks

These quirks are vanilla behaviour. This server keeps them.

| Rule | Quirk |
|------|-------|
| WOLF-HUNT-004 | After a flight, the hunt goal can hold `target` with no attack target. |
| WOLF-HUNT-003 | A leap stops the melee goal, which waits up to 20 ticks to start again. |
| WOLF-ANGER-002 | Any attack target makes the wolf angry, and the entry 22 changes in almost each tick of a hunt. |
| CGOAL-OWNER-008 | A nearer player with no bone blocks the beg toward a player with a bone. |
| CGOAL-OWNER-006 | A tame wolf with no present owner sits, and its sit goal stops and starts in each full evaluation. |
| WOLF-VAR-003 | A wolf of the first spawn setup is never a baby. |

### WOLF-DEV-003: Optional coalesced anger end time

- **Rule:** WOLF-ANGER-002. This is an option. The default is the reference behaviour.
- **Option:** while the attack target exists, send the entry 22 only when its value changes from
  "not angry" to "angry", and at the last tick with the attack target. Draw the duration only at
  that last tick.
- **Reference:** the reference draws a duration and changes the entry in each tick.
- **Visible to the client:** the client sees the same anger state in each tick, because each value
  in between is more than the game time. The count of `set_entity_data` packets is smaller. The save
  of a wolf during a hunt must write a drawn end time.
- **Test:** compare the anger state that a client computes in each tick.

### 14.1 Unresolved

- **U1**: the llama specification must state the strength of a llama and the spit. WOLF-GOAL-003
  reads the strength.
- **U2**: rain at the position of a wolf needs the weather and the precipitation of the biome
  (WOLF-TICK-003).
- **U3**: the knockback of a melee hit is a hook (CGOAL U1).
- **U4**: the fox and the turtle have no specification. The prey index and the turtle index can be
  empty in the benchmark.
- **U5**: the selection of a variant by its spawn selectors refers to ANIM-VAR-003. A general rule
  for each variant registry must replace it.

### 14.2 Experiments with a vanilla server

The setup of `mob-goals.md`, section 10.1, applies.

| Experiment | Steps | Expected result | Rules |
|------------|-------|-----------------|-------|
| E-W1 | Summon a wolf with `{}`. Read the spawn bundle. | The entries 23 and 24 are absent or `pale` and `classic`. The entry 22 is absent. | WOLF-DATA-001, WOLF-VAR-003 |
| E-W2 | Summon 200 wolves with no compound in a `forest`. Count the variants and the sound variants. | Each variant is `woods`. Each sound variant has a chance of about 1/7. No wolf is a baby. | WOLF-VAR-003 |
| E-W3 | Let a wolf hunt a sheep. Record each `set_entity_data` of the wolf. | The entry 22 changes in almost each tick of the hunt. It becomes −1 in the tick after the sheep dies. | WOLF-ANGER-002 |
| E-W4 | Put a wild wolf and a llama with `Strength:1` 12 blocks apart on grass, 400 times. Repeat with `Strength:5` and with a tame wolf. | The flight rate per examined evaluation is 2/5 of the rate with strength 5. A tame wolf never flees. | WOLF-GOAL-003 |
| E-W5 | Put a wolf in water for 20 ticks, then on dry ground. Record the entity events and the sound. | Event 8 when the wolf lands with no path; one shake sound; the shake ends after 42 ticks. | WOLF-TICK-001, WOLF-TICK-002 |
| E-W6 | Hit a wolf with `damage <wolf> 1 minecraft:cactus` and with a player attack. | The wolf panics after the cactus damage only. | WOLF-GOAL-002 |
| E-W7 | Let a wolf hunt a sheep 10 blocks away, and put a llama with `Strength:5` 20 blocks away. Record the attack target. | After the flight the wolf does not attack the sheep while the sheep stays in sight within 16 blocks. | WOLF-HUNT-004 |
| E-W8 | Summon 1,000 wild wolves on a dry superflat world with a spectator only. Profile the target selection. | Each wolf enumerates the llama box in each full evaluation, and about 0.6 other boxes. | WOLF-COST-001 |
