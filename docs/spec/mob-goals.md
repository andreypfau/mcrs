# Mob goals: the goal framework, the controls and the goals of passive animals

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

This document states what the goals of a mob do, as behaviour that an observer can see. An
observer is a player, a farm, a vanilla client or a test. The document states the goal framework,
the controls that turn and move a mob, and the goals of the pig, the cow, the sheep and the
chicken. It gives hooks for the goals of hostile mobs.

The rules state results, not algorithms. Each rule states how much freedom an implementation has,
with a fidelity class (section 0.3). A later design step selects the algorithms. The aim of that
step is a parallel implementation that ticks 50,000 animals with a small cost in each tick. The
rules must also permit a library that a Java mod of the vanilla game can call.

`entity-ai-timing.md` (AIT) states when each part of the AI runs, and which parts can be late.
This document does not restate the cadence. It assigns each part to a reflex or to a decision with
the rule AIT-SPL-001. `pathfinding.md` (PATH) states the path search and the path follower. This
document states only the requests that the goals make. `mob-animals.md` (ANIM) states the data of
each animal type, for example its attributes, its food items, its love state and its age.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **entity**, **living entity**, **position**, **box**,
  **velocity**, **input vector**, **jump input**, **travel step**, **f32**, **f64**: as
  `entity-physics.md`, section 0.1, defines them.
- **Mob**, **goal**, **control flag**, **goal selection**, **target selection**, **full
  evaluation**, **evaluation unit**, **path**, **guidance**, **path follower**, **move control**,
  **look control**, **jump control**, **look target**, **line of sight**, **idle time**,
  **follow range**, **reflex**, **decision**, **issue tick**, **snapshot**, **application**,
  **pair action**, **proposal**, **grant**, **player table**: as
  `entity-ai-timing.md`, section 0.1, defines them.
- **Request**, **reach range**, **speed modifier**, **done**, **mob path type**, **malus**,
  **legacy solid flag**: as `pathfinding.md`, section 0.1 and PATH-REQ-001, define them.
- **Order key**: the UUID of an entity, compared as PHY-CMP-013 states.
- **Feet point**: the position of an entity.
- **Eye point**: the point `(x, y + e, z)` of an entity at position `(x, y, z)`, where `e` is the
  eye height of its current dimensions.
- **Feet distance**: the Euclidean distance between the feet points of two entities, in f64.
- **Goal list**: the ordered list of the goals of one mob type, with a priority for each goal. The
  order is the order of the tables of section 5.
- **Priority**: an integer of each goal in a goal list. A smaller number is a higher priority.
- **Target list**: the goal list of the target selection. The goals of a target list hold the flag
  `target`.
- **Active goal**: a goal that started and did not stop.
- **Holder**: the active goal that holds a control flag.
- **Examined**: a goal that does not run is examined in a full evaluation when each of its flags is
  free or held by a goal that it can interrupt (GOAL-FRM-002). Only an examined goal evaluates its
  start condition.
- **Start condition**, **continue condition**: the two conditions of a goal. The goal selection
  evaluates them only in a full evaluation.
- **Start action**, **stop action**, **goal tick**: the work that a goal does when it starts, when
  it stops, and in each tick in which it ticks.
- **Per-tick goal**: a goal that ticks in each tick. Each other goal is an **evaluation goal**. An
  evaluation goal ticks only in full evaluations (AIT-VAN-002, AIT-VAN-005).
- **Disabled flag**: a control flag that the mob does not let any goal hold (GOAL-FRM-008).
- **In panic**: the state of a mob while its panic goal runs.
- **Yaw**: the rotation of an entity about the vertical axis, in degrees. The move control turns it.
- **Pitch**: the rotation of the head of a mob up or down, in degrees. A positive pitch is down.
- **Head yaw**: the yaw of the head of a mob.
- **Body yaw**: the yaw of the body of a mob. The server keeps it. It does not send it.
- **Yaw to a point**: for an offset `(dx, dz)` from the mob to a point, the value
  `f32(atan2(dz, dx) × 180 / π) - 90`, in degrees.
- **Wrap**: the angle `a mod 360`, changed to the range `[-180, 180)`.
- **Turn toward**: to turn an angle `a` toward an angle `b` by at most `s` is to add
  `clamp(wrap(b - a), -s, s)` to `a`.
- **Limit to**: to limit an angle `a` to `w` degrees from an angle `b` is to change `a` to
  `b - clamp(wrap(b - a), -w, w)`. Thus `a` moves by the smallest amount that puts it within `w` of
  `b`.
- **Head turn speed**: the largest change of the head yaw in one tick, toward a look target, in
  degrees. The default is 10.
- **Pitch limit**: the largest pitch, up or down. The default is 40.
- **Head yaw limit**: the largest difference between the head yaw and the body yaw. The default is
  75.
- **Look hold**: the number of ticks that a look target stays after a goal sets it.
- **Damage record**: the damage type and the game time of the last hit on a living entity. Only a
  hit that the cooldown accepts and that a shield does not block fully sets it (GOAL-PNC-001).
- **Fresh**: a damage record is fresh when the game time minus its game time is 40 or less.
- **Visibility factor**: a number of a target entity toward one mob, from 0 to 1 (GOAL-QRY-002).
- **Qualified player**: a player that is not a spectator and is alive.
- **Adult**, **baby**: a mob of age 0 or more, and a mob of age less than 0 (ANIM-AGE-001).
- **In love**: the love state of an animal. It is true while the love time is more than 0
  (ANIM-LOVE-001, ANIM-LOVE-002).
- **Solid render**: a property of a block state. The state can occlude, and its occlusion shape is
  the full block.
- **Stable floor**: a block position whose block below has a solid render state.
- **Random position search**: the search for a random destination of GOAL-POS-002 or GOAL-POS-003.
- **Candidate**: one block position that a random position search tries.
- **Walk value**: a number of a candidate for one mob type. A random position search selects the
  candidate with the largest walk value (GOAL-POS-004).
- **Home**: an optional block position and radius of a mob. The save keeps them as `home_pos` and
  `home_radius`. A leash also sets a home.
- **Water**: a fluid state of the fluid tag `water`. Flowing water and a waterlogged block are
  water.
- **Calm-down**: the time after the stop of a tempt goal in which the goal does not start
  (GOAL-TMP-004).
- **World query**: a read of the world that a goal makes. Section 0.5 states the form.
- **Keyed random value**: a random value from the stream of AIT-LAG-008.
- **Gate**: the random chance in a start condition, for example the chance of `0.02` of the look
  goal (GOAL-FRM-007).
- **Eligible**: a goal with a gate is eligible in a full evaluation when it is examined there, and
  each part of its start condition before the gate is true (GOAL-WAKE-003).
- **Wait**: the number of eligible full evaluations up to the first one in which a gate passes
  (GOAL-WAKE-003).
- **Due**: a wait or a goal timer is due in the full evaluation in which it ends.
- **Asleep**: the state of a mob in a full evaluation that the AI stage skips (GOAL-WAKE-001).
- **Quiet evaluation**: a full evaluation that the AI stage can skip (GOAL-WAKE-002).
- **Busy**: the state of a mob whose full evaluations all run (GOAL-WAKE-004).
- **Wake event**: an event that makes the next full evaluation of a mob run (GOAL-WAKE-005).

### 0.2 Provenance

- `black_box_confirmed`: shown by a captured result in section 11.
- `source_suggested_black_box_confirmed`: taken from the reference implementation, and consistent
  with a captured result in section 11.
- `source_suggested`: taken from the reference implementation, with no captured result. The
  confidence is at most medium. Section 10 attaches an experiment.
- `documentation`: taken from the data pack or from public documentation.
- `experiment`: shown by an experiment that did not use a vanilla server, for example a test of
  this server.
- `design`: a rule of this server's own design. It does not state vanilla behaviour.
- `unresolved`: not decided. Section 9 collects these items.

### 0.3 Kind, confidence and fidelity

Each rule states its provenance, its kind, its confidence and its fidelity, for example
`(source_suggested, semantic, confidence: medium, fidelity: exact)`. The kinds are those of
`docs/spec/README.md`. The fidelity tells an implementation how much freedom it has:

- `exact`: the observable value or timing must match. Examples are a duration, a range, a speed,
  an item set, and a priority relation that a player can see. Where the vanilla server has a
  random jitter, for example the phase of a full evaluation, the rule names it, and AIT gives the
  allowed delay.
- `distribution`: a random choice. The rule states the support, the probabilities and the
  independence. These must match. The generator is free. A keyed random value is the default
  generator (AIT-LAG-008, AIT-DEV-002). A test compares distributions over many runs.
- `envelope`: an internal choice. The result must stay inside a stated envelope, for example "a
  path to the target that the path search of PATH gives", or "no later than `n` ticks after an
  event". The rule states the envelope. The implementation selects the value inside it.

A rule with the fidelity `exact` can contain a random value. Then the random value has the fidelity
`distribution`, and each other part of the rule is exact.

### 0.4 ID prefix

Each rule has an ID with the prefix `GOAL`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `GOAL-FRM` | the goal framework |
| `GOAL-CTL` | the controls: move, look, jump and body rotation |
| `GOAL-QRY` | the common parts of world queries |
| `GOAL-NAV` | the path requests of goals |
| `GOAL-POS` | the random position searches |
| `GOAL-FLT` | float in a fluid |
| `GOAL-PNC` | panic |
| `GOAL-BRD` | breed approach |
| `GOAL-TMP` | tempt |
| `GOAL-FOL` | follow an adult |
| `GOAL-STR` | random stroll that avoids water |
| `GOAL-LKP` | look at a player |
| `GOAL-LKR` | random look around |
| `GOAL-EAT` | eat grass |
| `GOAL-WAKE` | sleep and wake: the full evaluations that the AI stage can skip |
| `GOAL-ANI` | the goal lists and the data of the animals |
| `GOAL-DEV` | intentional deviations |

### 0.5 The form of a world query

Each world query has an ID of the form `Q-<NAME>` and these parts:

- **Set**: the entities or the block positions that the query can return. The set has a shape, a
  range and the time of the snapshot.
- **Filter**: the conditions that each member of the result must pass.
- **Select**: the rule that selects one member, or "all".
- **Tie**: the rule for two members that the select rule cannot separate.
- **Cadence**: when the goal makes the query.

An implementation can answer a query from a spatial index, in a batch for many mobs. The result
must equal the result of the definition. The query reads the snapshot of its issue tick
(AIT-LAG-001), or the live state in the exact-parity mode (AIT-LAG-011).

### 0.6 Scope

This document states:

- the goal framework: priorities, control flags, the start, the continue, the stop and the tick of
  a goal, and the interrupt of an active goal;
- the look control, the body rotation and the rotations that a client sees;
- the requests that goals make to the navigation, and the random position searches;
- the goals of the pig, the cow, the sheep and the chicken;
- the place of each part as a reflex or a decision, and the reads, the writes and the world queries
  of each goal (section 7).
- the full evaluations that the AI stage can skip, and the events that wake a mob (section 4.10).

These subjects are out of scope:

| Subject | Where |
|---------|-------|
| The cadence of the goal selection, the delay of a decision, pair actions in general | `entity-ai-timing.md` (AIT) |
| The path search, the path follower, the move control `move_to` and the jump of the move control | `pathfinding.md` (PATH) |
| The walker tick, the input vector and the jump from the ground | `entity-physics.md` (PHY) |
| The packets of the rotations | `entity-tracking.md` (TRK) |
| The hit, the damage cooldown and the damage types | `entity-damage.md` (DMG) |
| Attributes, dimensions, variants, age and growth, love state and timers, offspring, food items, drops, milk, shear, wool regrowth, eggs, saddles and the rider state, sounds, summon data | `mob-animals.md` (ANIM) |
| The slow fall and the wing flap of a chicken | ANIM. They are not a part of the AI |
| The control of a ridden pig by its rider, and the boost of a carrot on a stick | ANIM, and a later specification of riders. GOAL-ANI-007 states only the AI side |
| The leash, the mob that rides a mob (a chicken jockey), the mooshroom | hooks in section 6 |
| The goals of hostile mobs | hooks in section 6 |
| The AI of a mob with a brain, for example a villager, a goat or an axolotl | a later specification |

### 0.7 Artefacts cited

- The data pack of `26.4-snapshot-7`: the damage type tags `panic_causes` and
  `panic_environmental_causes`; the fluid tags `entity_floatable` and `water`; the block tag
  `edible_for_sheep`; the item tags `pig_food`, `cow_food`, `sheep_food` and `chicken_food`; the
  game rule `mob_griefing`; the attribute `tempt_range`.
- In `26.4-snapshot-7`, `edible_for_sheep` holds `short_grass`, `short_dry_grass`,
  `tall_dry_grass` and `fern`. `entity_floatable` holds `#water`.
- The item component `mob_visibility` (GOAL-QRY-002).
- The packets `entity_event`, `level_event`, `rotate_head`, `move_entity_rot` and
  `move_entity_pos_rot` (`assets/mcrs/reports/packets.json`).

## 1. The goal framework

### 1.1 Goal lists and control flags

**GOAL-FRM-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). Each mob type
with goals has two lists: the goal list and the target list. The goal list holds the goals that
hold the flags `move`, `look` and `jump`, or no flag. The target list holds the goals that hold the
flag `target`. Each list has its own holders. The target selection runs before the goal selection
in each AI step (AIT-VAN-001).

Each goal of a list has a priority. Two goals can have the same priority. The list keeps the order
of section 5, also for two goals with the same priority. Each goal of the animals of section 5
permits an interrupt.

**GOAL-FRM-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). A full
evaluation of one list does these steps (AIT-VAN-002):

1. Stop each active goal that holds a disabled flag, or whose continue condition is false. Do the
   stop action of each such goal.
2. Release the flags of the goals that stopped.
3. Examine each goal that does not run, in the order of the list. A goal is examined when all these
   conditions are true:
   - it holds no disabled flag;
   - for each of its flags, the flag has no holder, or the holder permits an interrupt and has a
     priority number that is larger than the number of this goal.

   For an examined goal, evaluate the start condition. When the condition is true, stop each holder
   of its flags and do its stop action. Then the goal takes its flags, and does its start action.
4. Tick each active goal, in the order of the list.

In a tick that is not a full evaluation, the list ticks only its active per-tick goals, in the
order of the list.

Thus a goal never interrupts a goal of the same priority. Of two goals with the same priority and
the same flags, the first in the list wins when both can start. A goal that starts in step 3 ticks
in step 4 of the same tick. A goal that stops in step 3 does not tick.

A goal that stops in step 3 releases its flags only in step 2 of the next full evaluation. Until
then, each of its flags that the new goal did not take still names it as the holder. A later goal
of step 3 can need such a flag. That goal is examined only when its priority number is smaller than
the number of the stopped goal. For example, the panic goal of a pig takes only `move` from a tempt
goal. The tempt goal still holds `look`, so the look goal (priority 7) is not examined in that
evaluation.

**GOAL-FRM-003** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity:
exact). A goal that holds no flag is examined in each full evaluation while it does not run. It
does not block another goal, and no other goal interrupts it. It runs beside the holders of all flags.

A goal with no flag can write the path of the mob. Such a write replaces the path that a holder of
`move` requested (GOAL-NAV-005). E-G3 shows this: the follow goal of a baby pig replaces the path of
its random stroll every 10 ticks.

**GOAL-FRM-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). The stop action
of a goal applies at once. When a goal stops in step 3 of GOAL-FRM-002, its stop action applies
before the start action of the goal that interrupts it. For example, a random stroll clears the
path when it stops, and then the panic goal requests its own path in the same tick.

**GOAL-FRM-005** (`source_suggested`, semantic, confidence: high, fidelity: exact). In one tick,
the goals write the guidance state in this order:

1. the stop actions and the start actions of step 3, in the order of the list;
2. the goal ticks of step 4, in the order of the list.

When two writes change the same part of the guidance state, for example the path or the look
target, the last write stays.

**GOAL-FRM-006** (`source_suggested`, semantic, confidence: high, fidelity: exact). A timer of an
evaluation goal counts evaluation units. A delay of `d` ticks is `ceil(d / 2)` evaluation units
(AIT-VAN-003). A per-tick goal counts ticks. Each goal of section 4 states its kind and its timers.

**GOAL-FRM-007** (`source_suggested`, semantic, confidence: high, fidelity: distribution). A random
chance in a start condition is drawn only when the goal is examined. For example, a chance of
`0.02` in a start condition is a chance of `0.02` in each full evaluation in which the goal is
examined. It is not drawn while another goal blocks a flag of the goal. The draws of one goal are
independent of the draws of other goals and of other mobs.

### 1.2 When the goals do not run

**GOAL-FRM-008** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The disabled
flags of a mob change only when its tick count is a multiple of 5 (AIT-VAN-026). Then `move` and
`look` are disabled when a mob passenger controls the mob. `jump` is disabled in that case, and
also when the mob rides a boat. Else the three flags are enabled. A player passenger does not
disable a flag.

**GOAL-FRM-009** (`source_suggested`, semantic, confidence: high, fidelity: exact). A mob does no AI
step in a tick when one of these is true:

- it has `NoAI:1b`;
- it is dead, or in the dying state (DMG-DTH-001). It then also has a zero sideways input, a zero
  forward input and no jump input;
- a player controls it. The client of that player moves the mob (GOAL-ANI-007);
- it does not tick (PHY-ORD-011).

In such a tick the goals do not change. A active goal stays a active goal. Its timers do not
count.

**GOAL-FRM-010** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The save does
not keep the state of the goals. After a load or a summon, no goal runs, and the tick count of the
mob starts at 0. The tick count is 1 in the first AI step, so the first two AI steps are full
evaluations (AIT-VAN-002).

**GOAL-FRM-011** (`source_suggested`, semantic, confidence: high, fidelity: exact). The active
state of some goals is a property that other mobs read. A mob is in panic while its panic goal
runs. The breed goal of another animal reads it (GOAL-BRD-001). The snapshot of AIT-LAG-001 must
contain it.

### 1.3 Reflexes and decisions

**GOAL-FRM-012** (`design`, semantic, confidence: medium, fidelity: envelope). Each goal of
section 4 states which of its parts are reflexes and which are decisions, by the rule AIT-SPL-001.
A decision applies 1 to `max_lag` ticks after its issue tick (AIT-LAG-002). The envelope of each
goal is the allowed delay of AIT-DEC-003. A timer that a decision starts counts from the
application (AIT-LAG-010).

A goal start that is a decision takes its flags at the application. Until the application, the
current holders keep the flags. When a reflex took a flag that the decision needs, the decision is
stale (AIT-LAG-005).

## 2. The controls

### 2.1 Rotations

**GOAL-CTL-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). A mob has four
rotations: the yaw, the pitch, the head yaw and the body yaw. Each is an f32 value in degrees. The
head yaw and the body yaw are not wrapped; a turn adds to them. When a mob enters the world, the
head yaw and the body yaw are equal to the yaw. A mob of natural spawning gets a random yaw from 0
to 360. A mob of `summon` gets the yaw 0, with or without the first spawn setup (TRK-LIFE-003,
TRK-SUM-006). ANIM-SPAWN-001 states the spawn of each animal type.

The row of each mob holds the four rotations. The body yaw also needs two values of the body
rotation: the stable head yaw and the stable count (GOAL-CTL-006). Both start at 0.

**GOAL-CTL-002** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity:
exact). The goals of this document do not turn the yaw directly. The move control turns it, by at
most 90 degrees in one tick (PATH-MOV-002). The move control sets the mob speed to `f32(m × v)`, where `m` is the speed
modifier of the request and `v` is the attribute `movement_speed`. It sets the forward input to the
same value (PATH-MOV-001).

The walker tick multiplies the input by the mob speed (PHY-ORD-004). Thus the steady speed on a
flat floor is proportional to `(m × v)²`. On grass, a mob with the speed `s = m × v` walks
`s² × 2.2026` blocks in each tick. E-G2 measures 0.1377 for a pig with `m = 1` and 0.2150 with
`m = 1.25`.

### 2.2 The look control

**GOAL-CTL-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). A goal sets a
look target with three values: a point, a head turn speed and a pitch limit. A goal that names no
speed and no limit uses the head turn speed and the pitch limit of the mob type (GOAL-ANI-006). A
look target at an entity uses the eye point of that entity. Each set starts a look hold of 2 ticks
(AIT-VAN-013).

**GOAL-CTL-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). The look
control runs in each tick after the move control (AIT-VAN-001, step 6). Let `(dx, dy, dz)` be the
look point minus the feet point of the mob, with `dy` from the eye point. Let `h` be
`sqrt(dx² + dz²)`. The look control does these steps:

1. Set the pitch to 0.
2. If the look hold is more than 0:
   1. Decrease the look hold by 1.
   2. If `|dx|` or `|dz|` is more than `1.0E-5`, turn the head yaw toward the yaw to the point, by
      at most the head turn speed.
   3. If `|dy|` or `h` is more than `1.0E-5`, set the pitch to
      `clamp(wrap(-atan2(dy, h) × 180 / π), -l, l)`, where `l` is the pitch limit.
3. Else, turn the head yaw toward the body yaw by at most 10.
4. If the mob has a path that is not done, limit the head yaw to the head yaw limit from the body
   yaw.

Thus the pitch is not a smooth turn. It is the pitch to the point, clamped to the pitch limit, in
each tick of a look hold. When no look hold remains, the pitch is 0 at once, and the head returns to
the body at 10 degrees in each tick.

**GOAL-CTL-005** (`design`, semantic, confidence: medium, fidelity: envelope). A look target that a
decision sets follows AIT-LAG-009. When the evaluation goal that sets it keeps its look, the hold
lasts until the next application of the same goal, and at most `2 + max_lag - 1` ticks after the
application. The envelope: the head must not return toward the body between two full evaluations
of a goal that keeps its look.

### 2.3 The body rotation

**GOAL-CTL-006** (`source_suggested`, semantic, confidence: high, fidelity: exact). The body
rotation runs in each tick of a mob, after the travel step. It runs also for a mob with `NoAI:1b`
and for a mob that a player controls. Let `(dx, dz)` be the
horizontal change of the position in this tick. The body rotation does these steps:

1. If `dx² + dz²` is more than `2.5000003E-7`: set the body yaw to the yaw, and limit the head yaw
   to the head yaw limit from the body yaw. Set the stable head yaw to the head yaw, and the stable
   count to 0. Stop.
2. If the first passenger of the mob is a mob, stop.
3. If the head yaw differs from the stable head yaw by more than 15 degrees: set the stable count to
   0, and the stable head yaw to the head yaw. Limit the body yaw to the head yaw limit from the
   head yaw. Stop.
4. Else add 1 to the stable count. When the stable count is more than 10, let
   `f = min(1, (count - 10) / 10)`. Limit the body yaw to `L × (1 - f)` degrees from the head yaw,
   where `L` is the head yaw limit.

Thus a mob that walks keeps its body on its yaw. A mob that stands turns its body only when the head
goes past the head yaw limit. After the head stays still for 10 ticks, the body follows the head in
the next 10 ticks, and then faces the same way as the head. GOAL-DEV-003 states the comparison of
step 3.

**GOAL-CTL-007** (`design`, semantic, confidence: medium, fidelity: exact). The look control and the
body rotation are a part of the guidance, so they are reflexes (AIT-RFX-001). They read only the
state of their own mob and its guidance state. They write only the pitch, the head yaw, the body
yaw and the two stable values of their own mob.

### 2.4 The jump control and the client view

**GOAL-CTL-008** (`source_suggested`, semantic, confidence: high, fidelity: exact). A goal or the
move control can ask for a jump in a tick. The jump control then sets the jump input for that tick
only (PATH-MOV-003). PHY-INP-004 applies the jump.

**GOAL-CTL-009** (`source_suggested`, wire, confidence: high, fidelity: exact). A client sees three
rotations of a mob: the yaw and the pitch in the rotation fields of `add_entity`,
`move_entity_rot` and `move_entity_pos_rot`, and the head yaw in `rotate_head` (TRK-UPD-008). The
server does not send the body yaw. The client computes its own body yaw. The pitch and the head yaw
are values that this document adds to the row (TRK-LIFE-001).

## 3. Queries, requests and random positions

### 3.1 Common parts of world queries

**GOAL-QRY-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). A distance in a
world query is the feet distance, unless the rule names another point. A query that names a box
uses the box of the mob grown by the stated amounts on each side. An entity is in such a box when
its own box intersects it.

**GOAL-QRY-002** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The
visibility factor of a target entity toward a mob is the product of these factors:

- 0.8, when the target sneaks;
- `0.7 × max(c, 0.1)`, when the target is invisible. `c` is the number of its armor slots for the
  head, the chest, the legs and the feet that hold an item, divided by 4;
- for each equipped item with the component `mob_visibility`, in the slot of its `equippable`
  component, whose entity types contain the type of the mob: the visibility value of the component.

A query with a range `R` and a visibility test accepts a target at a feet distance of at most
`max(R × v, 2)`, where `v` is the visibility factor.

**GOAL-QRY-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). A line of sight
check from a mob to a target is true when all these conditions are true:

- both are in the same dimension;
- the distance from the eye point of the mob to the eye point of the target is at most 128;
- a ray from the eye point of the mob to the eye point of the target hits no collision shape of a
  block. The ray uses the collision shapes for the mob, and does not stop at fluids.

The cache of AIT-VAN-004 can keep the result for one tick.

**GOAL-QRY-004** (`design`, semantic, confidence: high, fidelity: exact). When the select rule of a
query finds two members at the same distance, it selects the member with the smaller order key.
GOAL-DEV-001 records the vanilla order.

**GOAL-QRY-005** (`design`, semantic, confidence: medium, fidelity: exact under the conditions
below). The player table of AIT-CDN-002 holds these values of each player. The tempt query and the
look query read only these values:

- the feet point, and the eye height of the current pose of the player;
- the flags spectator and alive;
- the shift key state (`entity-physics.md`, section 0.1);
- the item in the main hand and the item in the off hand;
- the vehicle chain: the vehicle of the player, the vehicle of that vehicle, and so on.

The visibility factor of GOAL-QRY-002 is 0.8 for a player whose shift key is down, and 1 for each
other player. The rule is exact for a player that is not invisible and wears no item with `mob_visibility` for
the type of the mob. The vanilla items give `mob_visibility` only for
the skeleton, the zombie, the creeper, the piglin and the piglin brute. Thus the rule is exact for
the four animals of section 5 and each vanilla item, while the player is not invisible. Section 9,
U3, records the parts that are deferred.

A benchmark with a watcher player meets the conditions when the player has no effect
`invisibility`. A watcher in the game mode `spectator` is not a qualified player. It does not
tempt an animal, an animal does not look at it, and it does not reset the idle time (AIT-VAN-020).

### 3.2 Path requests

**GOAL-NAV-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). A request to a
point goes to the block position of the point with the reach range 1 (PATH-REQ-001). When the
search gives no path, the mob has no path after the application.

**GOAL-NAV-002** (`source_suggested_black_box_confirmed`, semantic, confidence: medium, fidelity:
exact). A request to an entity goes to the block position of the feet point of that entity, with the
reach range 1. When the search gives no path, the mob keeps its current path (PATH-REQ-004, step
1). E-G3 shows this: a
summoned baby is not on the ground in its first tick, so its first request gives no path
(PATH-REQ-001, step 2).

**GOAL-NAV-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). A goal can clear
the path. Then the mob has no path, and the move control stops the forward input in the next step
`move_to` (PATH-MOV-001).

**GOAL-NAV-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). A request with
the target of the current path, while that path is not done, gives the current path and keeps its
next node (PATH-REQ-001, step 3). It changes the speed modifier to the speed modifier of the
request. Thus a goal that asks for a path to an entity in each full evaluation causes a new search
only when the entity moves into another block position. An implementation can skip such a request
when it knows that the result is the current path.

**GOAL-NAV-005** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity:
exact). A mob has one path. Each goal reads and writes the same path. A continue condition that
reads "the path is not done" reads that path, also when another goal wrote it. E-G3 shows a random
stroll that continues on the path of the follow goal.

**GOAL-NAV-006** (`design`, semantic, confidence: medium, fidelity: envelope). Each request is a
decision (PATH-REQ-005). The envelope is the allowed delay of AIT-DEC-003 for a path search.

### 3.3 Random position searches

**GOAL-POS-001** (`source_suggested`, semantic, confidence: medium, fidelity: exact). A random
position search with a horizontal range `h` is restricted when both of these are true:

- the mob has a home, with the radius `r`;
- the distance from the centre of the home block to the feet point is less than `r + h + 1`.

A candidate passes the home test when the search is not restricted, or when the squared
distance from the candidate to the home block is less than `r²`.

**GOAL-POS-002** (`source_suggested_black_box_confirmed`, semantic, confidence: medium, fidelity:
distribution). The default search `D(h, v)` makes 10 tries. Each try does these steps:

1. Draw `a` and `c`, two integers uniform in `[-h, h]`, and `b`, an integer uniform in `[-v, v]`. The
   three draws are independent.
2. If the mob has a home and `h > 1`, move `a` by `u × h / 2` toward the home. `u` is uniform in
   `[0, 1)`. Subtract when the `x` of the mob is more than the `x` of the home, else add. Do the same
   for `c` on the `z` axis, with a new draw.
3. The candidate is the block position of the point `(x + a, y + b, z + c)`, where `(x, y, z)` is
   the feet point.
4. Reject the candidate when one of these is true:
   - its `y` is outside the build height of the dimension;
   - it fails the home test (GOAL-POS-001);
   - it is not a stable floor;
   - the malus of its mob path type for this mob is not 0 (PATH-TYP-004, PATH-MAL-001).

The result is the candidate with the largest walk value. Of two candidates with the same walk
value, the earlier try wins. The search gives the bottom centre of that block position. When each
try fails, the search gives no result.

E-G2 shows a consequence. On a flat floor with no blocks above it, only `b = 0` gives a stable
floor that is not blocked. Each try succeeds with a chance of 1 in 9, so a search fails with a
chance of `(8/9)^10`, about 0.31. Then the goal does not start in that full evaluation.

**GOAL-POS-003** (`source_suggested`, semantic, confidence: medium, fidelity: distribution). The land
search `L(h, v)` makes 10 tries. Each try does steps 1 to 3 of GOAL-POS-002, and then these steps:

1. Reject the candidate when its `y` is outside the build height, when it fails the home test, or
   when it is not a stable floor.
2. While the block at the candidate has the legacy solid flag, and the `y` is not more than the
   highest `y` of the dimension, move the candidate up by 1.
3. Reject the candidate when its block has water, or when the malus of its mob path type for this
   mob is not 0.

The selection of the result is that of GOAL-POS-002.

**GOAL-POS-004** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The walk value
of a candidate `p` for an animal of section 5 is:

- 10, when the block below `p` is `grass_block`;
- else `lerp(a, c, 1) - 0.5`. Here `a` is the ambient light of the dimension type, and
  `c = b / (4 - 3 × b)`, with `b = l / 15`. `l` is the larger of the block light at `p` and the sky
  light at `p` minus the sky darkness of the dimension. All values are f32.

Thus an animal prefers grass, and else a bright position.

**GOAL-POS-005** (`design`, semantic, confidence: medium, fidelity: envelope). A random position
search is a part of the decision of its goal (AIT-DEC-001). It reads the blocks and the light of the
snapshot of the issue tick. The envelope of its result is the support of GOAL-POS-002 or GOAL-POS-003
for that snapshot. The probabilities must match the stated draws.

**GOAL-POS-006** (`design`, semantic, confidence: medium, fidelity: exact under the conditions
below, else envelope). A server without a light engine on the server side uses this walk value in
place of GOAL-POS-004:

- 10, when the block below the candidate is `grass_block`;
- else 0.5.

The value 0.5 is the value of GOAL-POS-004 for `l = 15` and the ambient light 0. The rule is exact
when all these conditions are true at each candidate that is not above `grass_block`:

- the dimension type has the ambient light 0, for example the overworld;
- the sky light at the candidate is 15;
- the sky darkness is 0, for example in clear weather at noon.

Outside these conditions the fidelity is `envelope`. The selection can then differ only between two
candidates that are both not above `grass_block`. A candidate above `grass_block` wins over them in
both rules.

On a superflat world with a grass surface and no roof, each accepted candidate on the floor of a pen
is above `grass_block`. Only a candidate on top of a fence or a wall can have the
value 0.5. The search also needs the solid render property and the legacy solid flag of each state
of the world (PATH section 12.3). For the benchmark these are `air`, `bedrock`, `dirt`,
`grass_block` and the block of the pen.

## 4. The goals

### 4.1 Float in a fluid

**GOAL-FLT-001** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The float goal
holds the flag `jump`. It is a per-tick goal. A mob type with this goal has the navigation flag
`can_float` true (PATH-NOD-010, PATH-NOD-001).

- Start condition and continue condition: the height of the fluid of the tag `entity_floatable` at
  the mob is more than the jump threshold, or the mob is in lava. The jump threshold is 0.4 for an
  eye height of 0.4 or more, and 0 for a smaller eye height.
- Goal tick: ask for a jump with a chance of `0.8` (fidelity `distribution`, one independent draw in
  each tick).

The goal has no start action and no stop action.

**GOAL-FLT-002** (`design`, semantic, confidence: medium, fidelity: exact). The float goal is a
reflex (AIT-RFX-005). Its start condition, its continue condition and its goal tick run in the hot
tick. They read only the fluid height of the mob and the lava flag of its own row.

**GOAL-FLT-003** (`design`, semantic, confidence: medium, fidelity: exact under the condition
below). A server without the fluid model of `entity-physics.md` uses these values for each mob:

- the fluid height of each fluid tag is 0;
- the lava flag is false;
- the in-water flag of GOAL-STR-002 is false.

Thus the float goal never starts, and the stroll always uses the search for a mob that is not
in water.

The rule is exact when the box of the mob touches no block whose fluid state is not empty. A block
with water, lava or a waterlogged state is such a block. A dry superflat world with a pen of fences
meets the condition. The water tests of `Q-WATER` and GOAL-POS-003, step 3, read the fluid state of
a block, not the fluid height. They do not need the fluid model and stay as they are.

### 4.2 Panic

**GOAL-PNC-001** (`source_suggested_black_box_confirmed`, semantic, confidence: medium, fidelity:
exact). A living entity keeps a damage record. A hit sets the record to its damage type and the
current game time. Only a hit that the cooldown accepts (DMG-HIT-003) and that a shield does not
block fully sets it. A partial hit sets the record too. Only the panic goal of this document reads the
record. This rule fills the hook of DMG-HIT-005, step 1, for the panic.

**GOAL-PNC-002** (`source_suggested_black_box_confirmed`, semantic, confidence: medium, fidelity:
exact). The panic goal holds the flag `move`. It is an evaluation goal. Each animal type gives its
speed modifier `m` (section 5). The start condition is true when the damage record is fresh, its
damage type is in the tag `panic_causes`, and a destination exists. The destination is:

1. the result of `Q-WATER`, when the mob is on fire and the block at its block position has an
   empty collision shape;
2. else, and also when `Q-WATER` has no result, the result of the default search `D(5, 4)`
   (GOAL-POS-002).

The start condition is false when the destination step gives no result.

- Start action: request a path to the destination with the speed modifier `m` (GOAL-NAV-001).
- Continue condition: the mob has a path that is not done.
- Stop action: none.

The continue condition does not read the damage record. Thus a panic that started runs to the end
of its path. When the path ends while the record is still fresh, the goal starts again in the same
or the next full evaluation, with a new destination. The mob runs until the first path that ends
after the record is no longer fresh. E-G2 shows two paths after one hit: the second starts in the
19th tick after the hit and ends in the 43rd tick.

On fire means that the fire ticks of the mob are more than 0 and the type is not immune to fire. In
`26.4-snapshot-7`, `panic_causes` holds `panic_environmental_causes`, which holds `cactus`,
`freeze`, `hot_floor`, `in_fire`, `lava`, `on_fire` and `lightning_bolt`, among others.

**GOAL-PNC-003** (`source_suggested`, semantic, confidence: medium, fidelity: exact). `Q-WATER`:

- Set: the block positions `(X + dx, Y + dy, Z + dz)` with `|dx| ≤ 5`, `|dy| ≤ 1` and `|dz| ≤ 5`.
  `(X, Y, Z)` is the block position of the mob. Snapshot of the issue tick.
- Filter: the block has water.
- Select: the smallest `|dx| + |dy| + |dz|`.
- Tie: the smaller `dx`, then the smaller `dy`, then a positive `dz` before a negative `dz`.
- Cadence: in each full evaluation in which the panic goal is examined, the record is fresh with a
  type of `panic_causes`, and the mob is on fire.

The result is the block position itself, not its centre. The tie rule is the vanilla order, so this
query has no deviation.

**GOAL-PNC-004** (`design`, semantic, confidence: medium, fidelity: envelope). The panic goal is a
decision (AIT-RFX-006, AIT-DEC-001). The reflex of the hit writes the damage record in the hot tick.
The decision reads the record from the snapshot. The envelope is the allowed delay of AIT-DEC-003
for a panic position.

### 4.3 Breed approach

**GOAL-BRD-001** (`source_suggested_black_box_confirmed`, semantic, confidence: medium, fidelity:
exact). The breed goal holds the flags `move` and `look`. It is an evaluation goal with the speed
modifier 1.0 for each animal of section 5. It keeps a partner and a love count.

- Start condition: the mob is in love, and `Q-PARTNER` has a result. The result is the partner.
- Start action: none. The love count is 0.
- Goal tick: set a look target at the partner with the head turn speed 10 and the pitch limit of the
  type. Request a path to the partner (GOAL-NAV-002). Add 1 to the love count. When the love count
  is 30 or more and the squared feet distance to the partner is less than 9, propose the breed pair
  action.
- Continue condition: the partner is alive and in love, the partner is not in panic, and the love
  count is less than 60.
- Stop action: forget the partner. Set the love count to 0.

Thus two animals in love that stand next to each other breed in the 30th full evaluation after the
start, about 60 ticks. E-G1 shows the child 58 ticks after the summon of two cows in love. When the
partner is not near after 60 evaluation units, the goal stops, and it can start again at once with a
new count.

**GOAL-BRD-002** (`source_suggested`, semantic, confidence: medium, fidelity: exact). `Q-PARTNER`:

- Set: the entities of the same entity type as the mob in the box of the mob grown by 8.
- Filter: all these conditions are true:
  - the entity can mate with the mob (ANIM-FOOD-004): it is another entity, and both are in love;
  - it is not a spectator, and it is alive;
  - it is not in panic;
  - its feet distance is at most `max(8 × v, 2)`, with `v` the visibility factor (GOAL-QRY-002).

  The query does not check the line of sight.
- Select: the smallest feet distance.
- Tie: GOAL-QRY-004.
- Cadence: in each full evaluation in which the goal is examined and the mob is in love.

**GOAL-BRD-003** (`design`, semantic, confidence: medium, fidelity: envelope). The breed goal is a
decision. The breed action is the pair action of AIT-CPL-004, with its check at the grant. ANIM
states its result: the child, the end of the love of both animals and the age of the parents. Two
animals can select different partners. The first grant wins (AIT-DEV-005).

### 4.4 Tempt

**GOAL-TMP-001** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The tempt goal
holds the flags `move` and `look`. It is an evaluation goal. Each tempt goal of a type has an item
test, a speed modifier `m` and a stop distance of 2.5 (section 5). It keeps a player and a
calm-down.

- Start condition: the calm-down is 0, and `Q-TEMPT` has a result. The result is the player.
- Start action: none.
- Goal tick: set a look target at the player with the head turn speed `75 + 20 = 95` and the pitch
  limit of the type. When the squared feet distance to the player is less than `2.5²`, clear the
  path (GOAL-NAV-003). Else request a path to the player (GOAL-NAV-002) with the speed modifier `m`.
- Continue condition: the same as the start condition. The player can change to another player in
  each full evaluation.
- Stop action: forget the player. Clear the path. Start the calm-down (GOAL-TMP-004).

The head turn speed of the tempt goal is the head yaw limit plus 20, so the head of an animal
follows a player that holds its food almost at once.

**GOAL-TMP-002** (`source_suggested`, semantic, confidence: medium, fidelity: exact). `Q-TEMPT`:

- Set: the players in the dimension of the mob, at the snapshot (the player table, AIT-CDN-002).
- Filter: a qualified player; the item in the main hand or in the off hand passes the item test; at
  a feet distance of at most `max(R × v, 2)`. `R` is the value of the attribute `tempt_range` of the
  mob (TRK-TYPE-004). `v` is the visibility factor of the player toward the mob. The query does not check
  the line of sight, and does not check the game mode except the spectator.
- Select: the smallest feet distance.
- Tie: GOAL-QRY-004.
- Cadence: in each full evaluation in which the goal is examined or runs, while the calm-down is 0.

**GOAL-TMP-003** (`source_suggested`, semantic, confidence: medium, fidelity: exact). A player that
rides the mob is not excluded. A player in creative mode tempts an animal. Two tempt goals of one
type with the same priority do not interrupt each other. The first in the list wins when both can
start (GOAL-FRM-002).

**GOAL-TMP-004** (`design`, semantic, confidence: medium, fidelity: envelope). The calm-down starts
at the stop of the goal. The goal does not start in a full evaluation that is less than 100 ticks
after the stop. It can start in the first full evaluation that is 100 ticks or more after the stop.
GOAL-DEV-002 records the vanilla count.

**GOAL-TMP-005** (`design`, semantic, confidence: medium, fidelity: envelope). The tempt goal is a
decision. Its look target follows GOAL-CTL-005. The envelope is the allowed delay of AIT-DEC-003 for
the goal selection.

### 4.5 Follow an adult

**GOAL-FOL-001** (`source_suggested_black_box_confirmed`, semantic, confidence: medium, fidelity:
exact). The follow goal holds no flag (GOAL-FRM-003). It is an evaluation goal. Each animal type
gives its speed modifier `m` (section 5). It keeps an adult and a path timer.

- Start condition: the mob is a baby, `Q-ADULT` has a result, and the squared feet distance to the
  result is 9 or more. The result is the adult.
- Start action: set the path timer to 0.
- Goal tick: decrease the path timer by 1. When it is 0 or less, set it to 5 evaluation units, and
  request a path to the adult (GOAL-NAV-002) with the speed modifier `m`.
- Continue condition: the mob is a baby, the adult is alive, and the squared feet distance to the
  adult is from 9 to 256.
- Stop action: forget the adult. The stop does not clear the path.

Thus a baby asks for a path to the adult every 10 ticks. Its request replaces each other path of
the mob. When the baby comes within 3 blocks, the goal stops, but the baby walks to the end of its
last path. E-G3 shows a baby pig that stops 1.35 blocks from the adult. The adult is any adult of
the same type, not only a parent.

**GOAL-FOL-002** (`source_suggested`, semantic, confidence: medium, fidelity: exact). `Q-ADULT`:

- Set: the entities of the same entity type as the mob in a box. The box is the box of the mob,
  grown by 8 on the `x` and `z` axes and by 4 on the `y` axis.
- Filter: an adult.
- Select: the smallest feet distance.
- Tie: GOAL-QRY-004.
- Cadence: in each full evaluation while the goal does not run and the mob is a baby.

**GOAL-FOL-003** (`design`, semantic, confidence: medium, fidelity: envelope). The follow goal is a
decision. The envelope is the allowed delay of AIT-DEC-003 for the goal selection.

### 4.6 Random stroll that avoids water

**GOAL-STR-001** (`source_suggested_black_box_confirmed`, semantic, confidence: medium, fidelity:
exact). The stroll goal holds the flag `move`. It is an evaluation goal with the speed modifier 1.0
for each animal of section 5.

- Start condition: all these conditions are true:
  - no passenger controls the mob;
  - the idle time is less than 100 (AIT-VAN-020);
  - a random integer uniform in `[0, 60)` is 0;
  - the destination of GOAL-STR-002 exists.
- Start action: request a path to the destination (GOAL-NAV-001).
- Continue condition: the mob has a path that is not done, and no passenger controls the mob.
- Stop action: clear the path.

Thus an animal with no player within 32 blocks starts no stroll when 100 ticks passed after the last
reset of its idle time. A hit resets the idle time (DMG-HIT-002).

**GOAL-STR-002** (`source_suggested`, semantic, confidence: medium, fidelity: distribution). The
destination of the stroll goal is:

- when the mob is in water: the result of `L(15, 7)`; when that search has no result, the result of
  `D(10, 7)`;
- else, with a chance of `0.999`, the result of `L(10, 7)`, and with a chance of `0.001`, the
  result of `D(10, 7)`. The goal does not try the other search when the selected search has no
  result.

The draw of the chance of `0.999` is independent of the draws of the searches.

**GOAL-STR-003** (`design`, semantic, confidence: medium, fidelity: envelope). The stroll goal is a
decision. Its random gate, its idle time test and its search read the snapshot of the issue tick.
The envelope is the allowed delay of AIT-DEC-003 for a random position.

**GOAL-STR-004** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity:
exact). The idle time of an animal of section 5 obeys these rules:

- A new mob has the idle time 0. The save of a mob does not hold the idle time. Thus a mob that
  loads from a save also has the idle time 0.
- Each AI step adds 1 to the idle time before the goal selection of that step. Thus the stroll goal
  is eligible in the AI steps 1 to 99 after the last reset.
- A mob with `NoAI:1b` does no AI step, so its idle time does not change.
- Only two events set the idle time to 0. The first is the despawn check of AIT-VAN-020, step 5,
  when the nearest non-spectator player of the dimension is less than 32 blocks away. The second is
  a hit after the tests of DMG-HIT-001 (DMG-HIT-002, step 1).

These states do not change the idle time and do not stop its increase:

- the flag `PersistenceRequired`, a custom name, a leash and a vehicle;
- the age, the age lock and the love state;
- a player in the game mode `spectator` at each distance, and a player in another dimension;
- each game rule.

A persistent mob only skips the removal steps 3 and 4 of AIT-VAN-020. Thus no key of the `summon`
compound keeps the stroll eligible (`entity-tracking.md`, TRK-SUM-007). E-G13 shows this.

### 4.7 Look at a player

**GOAL-LKP-001** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The look goal
holds the flag `look`. It is an evaluation goal with the range 6 for each animal of section 5. It
keeps a player and a look count.

- Start condition: a random float uniform in `[0, 1)` is less than `0.02`, and `Q-LOOK` has a
  result. The result is the player.
- Start action: set the look count to `ceil((40 + r) / 2)` evaluation units, where `r` is an
  integer uniform in `[0, 40)`.
- Goal tick: when the player is alive, set a look target at the player. Use the head turn speed and
  the pitch limit of the type. Then decrease the look count by 1.
- Continue condition: the player is alive, the squared feet distance to the player is at most
  `6²`, and the look count is more than 0.
- Stop action: forget the player.

The continue condition does not check the line of sight or the visibility factor. The count gives
a look of 20 to 40 evaluation units: 20 and 40 each with the chance 1/40, each value from 21 to 39
with the chance 2/40. This is about 40 to 80 ticks.

**GOAL-LKP-002** (`source_suggested`, semantic, confidence: medium, fidelity: exact). `Q-LOOK`:

- Set: the players in the dimension of the mob, at the snapshot.
- Filter: all these conditions are true:
  - the player is a qualified player;
  - the feet distance is at most `max(6 × v, 2)`, with `v` the visibility factor;
  - the line of sight check from the mob to the player is true (GOAL-QRY-003);
  - the player does not ride the mob, directly or through a chain of vehicles.
- Select: the smallest distance from the point `(x, e, z)` to the feet point of the player. Here
  `x` and `z` are those of the feet point of the mob, and `e` is the `y` of its eye point.
- Tie: GOAL-QRY-004.
- Cadence: in each full evaluation in which the goal is examined and the random gate passes.

**GOAL-LKP-003** (`design`, semantic, confidence: medium, fidelity: envelope). The look goal is a
decision. Its look target follows GOAL-CTL-005. The random gate and the query are independent. An
implementation can draw the gate first and make the query only when it passes.

### 4.8 Random look around

**GOAL-LKR-001** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The look around
goal holds the flags `move` and `look`. It is a per-tick goal. It keeps a direction and a look
time.

- Start condition: a random float uniform in `[0, 1)` is less than `0.02`.
- Start action: draw an angle `θ` uniform in `[0, 2π)`. The direction is `(cos θ, sin θ)` on the
  `x` and `z` axes. Set the look time to `20 + r` ticks, where `r` is an integer uniform in
  `[0, 20)`.
- Goal tick, in each tick: decrease the look time by 1. Set a look target at the point
  `(x + cos θ, e, z + sin θ)`, with the head turn speed and the pitch limit of the type. Here `x` and
  `z` are those of the current feet point, and `e` is the `y` of the current eye point.
- Continue condition: the look time is 0 or more.
- Stop action: none.

The look point is at the height of the eye, so the pitch is 0. The goal runs for `T + 1` ticks when
the look time `T` is odd, and `T + 2` ticks when it is even, because the continue condition runs only
in full evaluations. The goal holds `move`, so it starts only when no goal with a smaller priority
number holds `move` or `look`.

**GOAL-LKR-002** (`design`, semantic, confidence: medium, fidelity: exact). The start of the look
around goal is a decision: the random gate, the angle and the look time. The goal tick of each tick
is a part of the guidance, so it is a reflex. The guidance keeps the direction, and computes the
look point from the current position in each tick. The end of the goal is a due event at the first
full evaluation after the look time is less than 0 (AIT-RFX-008).

### 4.9 Eat grass

**GOAL-EAT-001** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The eat goal
holds the flags `move`, `look` and `jump`. It is an evaluation goal. It keeps an eat timer.

- Start condition: a random integer uniform in `[0, N)` is 0, where `N` is 25 for a baby and 500
  for an adult. Then the block at the block position of the mob is in the tag `edible_for_sheep`,
  or the block below it is `grass_block`.
- Start action: set the eat timer to 20 evaluation units. Send `entity_event` with the event 10 for
  the mob to the clients that track it. Clear the path.
- Goal tick: set the eat timer to `max(0, timer - 1)`. When the timer becomes 2, do the eat of
  GOAL-EAT-002.
- Continue condition: the eat timer is more than 0.
- Stop action: set the eat timer to 0.

Thus the eat occurs in the 18th goal tick, 34 ticks after the start. The goal holds its flags for 40
ticks. The client plays the eat animation for 40 ticks after the event 10. An interrupt, for example
by the panic goal, stops the goal before the eat; the client then still plays the full animation.

**GOAL-EAT-002** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The eat reads
the blocks again at the current block position `p` of the mob:

1. If the block at `p` is in the tag `edible_for_sheep`: when the game rule `mob_griefing` is true,
   destroy the block at `p` with no drops (SIM-SET-004). Then the mob eats (ANIM-TICK-003).
2. Else, if the block below `p` is `grass_block`, do these steps:
   1. When `mob_griefing` is true, send `level_event` 2001 at that position, with the state id of
      the default `grass_block`.
   2. When `mob_griefing` is true, set that position to `dirt` with the flag word 2. This flag word
      holds only *sync-clients* (SIM-SET-002).
   3. The mob eats (ANIM-TICK-003).
3. Else nothing occurs. The mob does not eat.

The mob eats also when `mob_griefing` is false. ANIM-TICK-003 states the result of an eat, for example the
regrowth of the wool and the growth of a baby.

**GOAL-EAT-003** (`design`, semantic, confidence: medium, fidelity: exact). The start of the eat goal
is a decision. The event 10 and the path clear occur at the application. The eat timer counts from
the application (AIT-LAG-010). The eat of GOAL-EAT-002 is a due event at the 18th full evaluation
after the application. It is a reflex (AIT-RFX-008). It reads the blocks of the copy of PHY-CMP-009,
and its block writes follow the rules of block writes.

### 4.10 Sleep and wake

The rules of this section let the AI stage skip the goal selection of a mob in most full
evaluations. A mob that skips only quiet evaluations makes the same decisions as a mob that runs
each full evaluation. GOAL-WAKE-001 to GOAL-WAKE-008 state the goal selection. GOAL-WAKE-010 to
GOAL-WAKE-014 state the goals of the benchmark: float, panic, stroll, look at a player and look
around. GOAL-WAKE-015 to GOAL-WAKE-018 state the other goals of section 5.

#### 4.10.1 The goal selection

**GOAL-WAKE-001** (`design`, semantic, confidence: medium, fidelity: exact). The AI stage can skip
the goal selection of a mob in a quiet evaluation (GOAL-WAKE-002). It must run each other full
evaluation. In a skipped evaluation, the mob is asleep:

- no goal starts or stops, and no evaluation goal ticks;
- the flags, the goal state and the guidance state do not change;
- no wait draws a value, and no wait is due (GOAL-WAKE-003).

The reflexes of the mob run as usual: the guidance, the look control, the body rotation, the goal
tick of the look around goal (GOAL-LKR-002) and the float goal (GOAL-WAKE-010).

**GOAL-WAKE-002** (`design`, semantic, confidence: medium, fidelity: exact). A full evaluation of a
mob is quiet when all these conditions are true:

1. The tick count of the mob is more than 1 (GOAL-FRM-010).
2. The mob is not busy (GOAL-WAKE-004).
3. No wait of the mob is due in this evaluation (GOAL-WAKE-003).
4. No goal timer of the mob is due in this evaluation (GOAL-WAKE-006).
5. No wake event of the mob occurred after its last full evaluation that ran (GOAL-WAKE-005).
6. In its last full evaluation that ran, no goal started and no goal stopped.

Condition 6 lets the next evaluation release the flags of a goal that stopped in step 3 of
GOAL-FRM-002. It also lets each goal that became eligible draw its wait.

**GOAL-WAKE-003** (`design`, semantic, confidence: medium, fidelity: distribution). This server
draws a wait in place of a draw of a gate in each full evaluation:

1. In a full evaluation in which a goal is eligible and has no wait, draw a wait `w`. The law of
   `w` is geometric with the chance `p` of the gate: `P(w = k) = (1 - p)^(k - 1) × p`, for each
   integer `k ≥ 1`.
2. The gate fails in each eligible evaluation before the `w`-th one, counted from the evaluation of
   the draw as the first. The gate passes in the `w`-th one. That evaluation is due.
3. When the goal is not eligible in an evaluation, its wait ends. The gate draws nothing there.
4. When the gate passes, the wait ends. If the rest of the start condition is false, the goal does
   not start. The next eligible evaluation draws a new wait.

The goals with a gate:

| Goal | `p` | Parts before the gate |
|------|-----|-----------------------|
| stroll (GOAL-STR-001) | `1/60` | no passenger controls the mob; the idle time is less than 100 |
| look at a player (GOAL-LKP-001) | `0.02` | none |
| look around (GOAL-LKR-001) | `0.02` | none |
| eat grass (GOAL-EAT-001) | `1/25` for a baby, `1/500` for an adult | none |

The value `w = 1 + floor(ln(u) / ln(1 - p))`, with `u` uniform in `(0, 1]`, has this law. The
random values come from the stream of AIT-LAG-008. The part names the goal and the wait. The issue
tick is the tick of the evaluation of the draw.

The geometric law has no memory. Thus the evaluation in which the gate passes has the law of
independent draws in each eligible evaluation, which GOAL-FRM-007 states. A wait that ends and a new
wait do not change this law. A server that runs each full evaluation uses the same waits. Its runs
and the runs of a server that skips quiet evaluations are equal, tick by tick (AIT-INV-002).
GOAL-DEV-004 records the vanilla draws.

**GOAL-WAKE-004** (`design`, semantic, confidence: medium, fidelity: exact). A mob is busy while one
of these conditions is true:

- the panic goal does not run, and the damage record is fresh with a damage type in
  `panic_causes`;
- a tempt goal of the mob does not run, and a qualified player holds an item of its item test
  within its range (GOAL-WAKE-015);
- the mob is in love;
- the mob is a baby;
- the look goal, a tempt goal, the breed goal, the follow goal or the eat goal runs.

Each of these states reads a world query, or writes the guidance state, in each full evaluation.
Thus the mob cannot skip them. A busy state ends with no wake event: the next evaluation is quiet
when the conditions of GOAL-WAKE-002 are true.

**GOAL-WAKE-005** (`design`, semantic, confidence: medium, fidelity: exact). These are the wake
events of a mob:

1. A hit on the mob that passes the tests of DMG-HIT-001. The hit resets the idle time, and it can
   set the damage record.
2. The path of the mob becomes done, or the mob loses its path. A stop of the follower, a failed
   request and a path clear are examples.
3. A passenger mounts the mob or leaves it, or the controller of the mob changes (GOAL-ANI-007).
4. The disabled flags of the mob change (GOAL-FRM-008).
5. The idle time of the mob resets while it is 100 or more. A non-spectator player that comes less
   than 32 blocks from the mob resets it (AIT-VAN-020).
6. The mob enters love (ANIM-LOVE-001), or its age changes from adult to baby.
7. The mob does an AI step after a tick with no AI step (GOAL-FRM-009). The clear of `NoAI`, the
   end of the control by a player and the first tick of the column of the mob are examples.

The event wakes the first full evaluation whose snapshot holds the event (AIT-LAG-001). The hot
tick must detect each event in the tick in which it occurs. An event that the hot tick detects later
is a delay of the decision, and AIT-LAG-003 limits it.

**GOAL-WAKE-006** (`design`, semantic, confidence: medium, fidelity: exact). The end of a look
around is the only goal timer that wakes a mob (GOAL-WAKE-014). The timers of the other goals run
while the mob is busy. The goal timer counts only AI steps. In a tick with no AI step, the mob ends
each of its waits, and the goal timer does not count. The next AI step is a wake event
(GOAL-WAKE-005, item 7).

**GOAL-WAKE-007** (`design`, semantic, confidence: medium, fidelity: exact). The idle time can end
the eligibility of the stroll goal with no wake event. In the first full evaluation in which the
idle time is 100 or more, the wait of the stroll ends. That evaluation stays quiet when the other
conditions of GOAL-WAKE-002 are true. An implementation can keep the tick of the last reset in place
of a counter. It then computes the idle time at a due evaluation.

**GOAL-WAKE-008** (`design`, semantic, confidence: medium, fidelity: exact). A quiet evaluation
changes nothing when the AI stage runs it in full. This is the reason for each step of
GOAL-FRM-002:

- Step 1: the continue condition of each running goal depends only on a busy state, a due timer
  or a wake event. GOAL-WAKE-010 to GOAL-WAKE-018 list these for each goal.
- Step 3: the examined goals change only in a full evaluation that runs, or by the wake events 3, 4
  and 7. Each eligible goal has a wait that is not due, so each gate fails. Each start condition
  without a gate is false, else the mob is busy.
- Step 4: each evaluation goal that ticks keeps the mob busy. The other running goals have no
  evaluation tick, or tick as a reflex.

Thus the decisions of a sleeping mob are those of GOAL-FRM-002. The deterministic parts are exact.
The gates and the random position searches have the fidelity `distribution`, as in section 4. The
delay of each decision is that of AIT-LAG-002, with no extra delay.

#### 4.10.2 The goals of the benchmark

**GOAL-WAKE-010** (`design`, semantic, confidence: medium, fidelity: exact). Float in a fluid
(GOAL-FLT-001):

- Start and continue: the hot tick tests the fluid height and the lava flag at each full
  evaluation of the mob, also when the mob is asleep. The test reads only the row of the mob
  (GOAL-FLT-002).
- Change: the fluid height of `entity_floatable` at the mob crosses the jump threshold, or the
  lava flag changes. A mob whose box touches no block with a fluid state cannot change it. The hot
  tick can skip the test for such a mob while the goal does not run.
- Target choice: none. The jump draw in each goal tick is a reflex.
- Selection: the float goal holds only `jump`. No other goal of the pig, the cow and the chicken
  holds `jump`, so its start and its stop change no other goal. For a sheep, a start of the float
  goal stops the eat goal. Thus it is a wake event of a sheep.

With GOAL-FLT-003, the float goal never runs, and the test is false for each mob.

**GOAL-WAKE-011** (`design`, semantic, confidence: medium, fidelity: exact). Panic (GOAL-PNC-002):

- Start: the damage record and the destination search. A hit is wake event 1. While the panic goal
  does not run and the record is fresh with a type of `panic_causes`, the mob is busy. Thus the
  goal tries a new destination in each full evaluation until it starts, as GOAL-PNC-002 states. The
  busy state ends in the first full evaluation in which the record is not fresh.
- Continue: the path is not done. The end of the path is wake event 2.
- Target choice: the destination, only at the start. The search has the fidelity
  `distribution` (GOAL-POS-002).
- Interrupt: no goal of section 5 interrupts the panic goal. A disabled `move` stops it (wake
  event 4).

Thus a hit wakes a pig. The pig sleeps while it runs on its path, and wakes at the end of each path.
It runs again while the record is fresh.

**GOAL-WAKE-012** (`design`, semantic, confidence: medium, fidelity: distribution for the gate and
the destination, exact for each other part). Random stroll (GOAL-STR-001):

- Start: a wait with `p = 1/60` (GOAL-WAKE-003). At the due evaluation, the goal searches the
  destination (GOAL-STR-002). When the search gives no result, the goal does not start, and the
  next eligible evaluation draws a new wait.
- Eligible: the stroll goal is examined, no passenger controls the mob, and the idle time is less
  than 100. For a pig, the stroll goal is examined while the panic goal, the breed goal and the
  tempt goals do not run. The look around goal does not block it.
- Changes of the eligibility: a start or a stop of the panic, breed or tempt goals, in a full
  evaluation that runs; wake events 3, 4, 5 and 7; the end by the idle time (GOAL-WAKE-007).
- Continue: the path is not done, and no passenger controls the mob. Wake events 2 and 3 change
  it.
- Target choice: the destination, only at the start.

With no non-spectator player within 32 blocks, the idle time reaches 100 in the 100th AI step after
the summon or the last hit. After that, the pig starts no stroll until a hit or a player resets the
idle time. E-G9 shows this.

**GOAL-WAKE-013** (`design`, semantic, confidence: medium, fidelity: distribution for the gate,
exact for each other part). Look at a player (GOAL-LKP-001):

- Start: a wait with `p = 0.02`. At the due evaluation, the goal makes `Q-LOOK`. When it has no
  result, the goal does not start, and the next eligible evaluation draws a new wait.
- Eligible: the look goal is examined. For a pig, this is while the breed goal and the tempt goals
  do not run. The look around goal does not block it. The rule of GOAL-FRM-002 for a goal that
  stopped in step 3 applies.
- A player that comes within 6 blocks is not a wake event. `Q-LOOK` is read only in the
  evaluation in which the gate passes, and the gate has no memory.
- Continue: while the goal runs, the mob is busy. Each full evaluation tests the player, the
  distance and the look count, and sets the look target again.
- Target choice: the player of `Q-LOOK`, only at the start.

While the dimension of the mob has no qualified player, `Q-LOOK` has no result. Then an
implementation can end the waits of the look goal. Later, a qualified player can enter the
dimension, or a player there can become qualified. Then each mob of the dimension with an eligible
look goal must draw a new wait in its next full evaluation. This event is a wake event of each such
mob.

**GOAL-WAKE-014** (`design`, semantic, confidence: medium, fidelity: distribution for the gate, the
direction and the look time, exact for each other part). Random look around (GOAL-LKR-001):

- Start: a wait with `p = 0.02`. The start action draws the direction and the look time `T`.
- Eligible: the look around goal is examined. For a pig, no goal must hold `move` or `look`. Also,
  no goal with one of these flags stopped in step 3 of the last evaluation.
- Goal tick: a reflex in each tick (GOAL-LKR-002). It does not wake the mob.
- Continue: a goal timer. The look time is `T - 1` after the goal tick of the start evaluation. It
  decreases by 1 in each later AI step. The timer is due in the first full evaluation in which the
  look time is less than 0 before the goal tick. In that evaluation, the goal stops.
- Interrupt: the stroll goal and the look goal can start while the look around goal runs. Their
  waits are due in full evaluations that run.
- Target choice: the direction, only at the start.

#### 4.10.3 The other goals of the animals

**GOAL-WAKE-015** (`design`, semantic, confidence: medium, fidelity: exact). Tempt
(GOAL-TMP-001):

- Start: the calm-down is 0, and `Q-TEMPT` has a result. The goal has no gate.
- Tempt state: in each tick, the hot tick tests each qualified player that holds an item of a
  tempt item test in a hand. It finds each mob with that tempt goal within `max(R × v, 2)` of the
  player, as `Q-TEMPT` states. Each mob that it finds is busy in the next full evaluation. The
  cost of the test depends on the number of such players, not on the number of mobs.
- Eligible: the tempt goal is examined. For a pig, this is while the panic goal, the breed goal
  and the other tempt goal do not run. A start of the panic goal or the breed goal, and its stop,
  occur in full evaluations that run.
- Calm-down: its end needs no wake event. The goal can start only while a player tempts the mob,
  and then the mob is busy.
- Continue: while the goal runs, the mob is busy. The player can change in each full evaluation.
- Target choice: the player of `Q-TEMPT`, in each full evaluation.

**GOAL-WAKE-016** (`design`, semantic, confidence: medium, fidelity: exact). Breed approach
(GOAL-BRD-001): a mob in love is busy. The start of the love is wake event 6. The end of the love by
its timer, by a hit or by a breed needs no wake event. While the goal runs, the mob is busy. The
partner is the result of `Q-PARTNER` at the start.

**GOAL-WAKE-017** (`design`, semantic, confidence: medium, fidelity: exact). Follow an adult
(GOAL-FOL-001): a baby is busy. A change from adult to baby is wake event 6. The growth of a baby
into an adult needs no wake event. The adult is the result of `Q-ADULT` at the start.

**GOAL-WAKE-018** (`design`, semantic, confidence: medium, fidelity: distribution for the gate,
exact for each other part). Eat grass (GOAL-EAT-001):

- Start: a wait with the `p` of GOAL-WAKE-003. At the due evaluation, the goal reads the blocks at
  the mob and below it. When the test is false, the goal does not start.
- Eligible: the eat goal is examined. For a sheep, the float, the panic, the breed, the tempt and
  the eat goals must not run. No flag of the eat goal is disabled.
- A change of the age between baby and adult changes `p`. It ends the wait, and the next
  evaluation draws a new one. A growth into an adult needs no wake event, because a baby is busy.
- Continue: while the goal runs, the mob is busy.

#### 4.10.4 The benchmark scene

Take an adult pig of `summon` on a dry superflat world with a pen of fences. No player tempts it. The
pig runs its full evaluations only in these cases:

- its first two AI steps;
- a wait that is due: the stroll in its first 100 AI steps, the look goal and the look around goal;
- the end of a look around;
- a hit, and each full evaluation in the 40 ticks after a hit while the panic goal does not run;
- the end of a path of a stroll or a panic;
- a full evaluation in which the look goal runs, near a qualified player;
- the full evaluation after each start or stop.

Each other full evaluation is quiet.

**GOAL-WAKE-019** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity:
distribution for the gates and the destinations, exact for each other part). Take adult pigs of
`summon minecraft:pig <x> <y> <z> {}` on a dry superflat world. The only player of the dimension is
in the game mode `spectator`. The pigs then move in two phases:

1. AI steps 1 to 99: the stroll goal is eligible, with `p = 1/60` in each full evaluation
   (GOAL-WAKE-012). The look around goal can start with `p = 0.02`. A stroll that starts in this phase
   continues to the end of its path, also after AI step 99.
2. After the last path ends: the position of each pig stays constant. Only the look around goal
   starts. It turns the head, and the body follows the head (GOAL-LKR-001, GOAL-CTL-006).

In both phases, no pig panics, is tempted, breeds, follows an adult or looks at a player. The
spectator does not reset the idle time (GOAL-QRY-005). Thus wake event 5 never occurs, and no pig
leaves phase 2. The compound `{}` skips the first spawn setup, so no pig is a baby (TRK-SUM-003).
The key `PersistenceRequired:1b` does not change these phases (GOAL-STR-004).

**GOAL-WAKE-020** (`source_suggested_black_box_confirmed`, semantic, confidence: high, fidelity:
exact). A benchmark that needs strolls after AI step 99 must reset the idle time with a hit
(GOAL-STR-004). The vanilla command with the fewest side effects is this command:

```
execute as @e[type=minecraft:pig] run damage @s 0 minecraft:drown
```

The interval of the runs must be at most 99 ticks. A hit of 0 points of `minecraft:drown` has
these effects on each pig:

- The idle time becomes 0. The hit is wake event 1.
- The health does not change. The type is not in `panic_causes`, so the pig does not panic.
- The type has the tags `no_impact` and `no_knockback`. Thus the hit marks no velocity and gives no
  knockback.
- A full hit sets the hurt time and the cooldown, and records the damage source. It sends the
  damage event and plays the hurt sound (DMG-HIT-005).
- Each run sends one success message (CMD-DMG-004). From the console, the message goes to the
  server log.

With an interval of 80 ticks, each pig stays in phase 1 of GOAL-WAKE-019. E-G13 shows this.

## 5. The animals

**GOAL-ANI-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The goal list of
the pig, in this order:

| Priority | Goal | Parameters |
|----------|------|------------|
| 0 | float (GOAL-FLT-001) | |
| 1 | panic (GOAL-PNC-002) | `m = 1.25` |
| 3 | breed (GOAL-BRD-001) | `m = 1.0` |
| 4 | tempt (GOAL-TMP-001) | item test: the item is `carrot_on_a_stick`; `m = 1.2` |
| 4 | tempt (GOAL-TMP-001) | item test: the item is in `pig_food` (ANIM-FOOD-001); `m = 1.2` |
| 5 | follow an adult (GOAL-FOL-001) | `m = 1.1` |
| 6 | stroll (GOAL-STR-001) | `m = 1.0` |
| 7 | look at a player (GOAL-LKP-001) | range 6 |
| 8 | look around (GOAL-LKR-001) | |

**GOAL-ANI-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The goal list of
the cow, in this order:

| Priority | Goal | Parameters |
|----------|------|------------|
| 0 | float | |
| 1 | panic | `m = 2.0` |
| 2 | breed | `m = 1.0` |
| 3 | tempt | item test: the item is in `cow_food` (ANIM-FOOD-001); `m = 1.25` |
| 4 | follow an adult | `m = 1.25` |
| 5 | stroll | `m = 1.0` |
| 6 | look at a player | range 6 |
| 7 | look around | |

**GOAL-ANI-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). The goal list of
the sheep, in this order:

| Priority | Goal | Parameters |
|----------|------|------------|
| 0 | float | |
| 1 | panic | `m = 1.25` |
| 2 | breed | `m = 1.0` |
| 3 | tempt | item test: the item is in `sheep_food` (ANIM-FOOD-001); `m = 1.1` |
| 4 | follow an adult | `m = 1.1` |
| 5 | eat grass (GOAL-EAT-001) | |
| 6 | stroll | `m = 1.0` |
| 7 | look at a player | range 6 |
| 8 | look around | |

**GOAL-ANI-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). The goal list of
the chicken, in this order:

| Priority | Goal | Parameters |
|----------|------|------------|
| 0 | float | |
| 1 | panic | `m = 1.4` |
| 2 | breed | `m = 1.0` |
| 3 | tempt | item test: the item is in `chicken_food` (ANIM-FOOD-001); `m = 1.0` |
| 4 | follow an adult | `m = 1.1` |
| 5 | stroll | `m = 1.0` |
| 6 | look at a player | range 6 |
| 7 | look around | |

The target lists of the four types are empty. In `26.4-snapshot-7`, none of the four types uses a
brain.

These relations follow from the tables and GOAL-FRM-002. A player can see them:

- The panic interrupts the breed, the tempt, the eat, the stroll and the look around. It holds only
  `move`, so a mob in panic can still look at a player.
- The breed interrupts the tempt. An animal in love walks to its partner, not to the player.
- The tempt interrupts the stroll, the look at a player and the look around.
- The eat interrupts the stroll and both look goals. The float blocks the eat, because both need
  `jump`.
- The stroll and the look at a player run together. The look around starts only when the mob does
  not stroll.
- The follow goal of a baby runs beside each other goal. Its path requests replace the path of the
  panic, the stroll and the tempt every 10 ticks.

**GOAL-ANI-005** (`source_suggested`, semantic, confidence: medium, fidelity: exact). These are the
rows of the malus table (PATH-MAL-001, PATH-MAL-002) of the four types. ANIM-TYPE-005 states the
same rows:

| Entity type | Path type | Malus |
|-------------|-----------|-------|
| `pig`, `cow`, `sheep`, `chicken` | `FIRE_IN_NEIGHBOR` | 16 |
| `pig`, `cow`, `sheep`, `chicken` | `FIRE` | -1 |
| `chicken` | `WATER` | 0 |

Thus a chicken can pick a panic destination in water (GOAL-POS-002), but its stroll still avoids
water (GOAL-POS-003, step 3).

**GOAL-ANI-006** (`source_suggested`, semantic, confidence: high, fidelity: exact). The four types
use the defaults: the head turn speed 10, the pitch limit 40 and the head yaw limit 75. The walk
value of the four types is GOAL-POS-004. ANIM-TYPE-005 calls it the stroll target value.

**GOAL-ANI-007** (`source_suggested`, semantic, confidence: medium, fidelity: exact). A pig with a
saddle whose first passenger is a player that holds `carrot_on_a_stick` in either hand has that
player as its controller (ANIM-RIDE-001). Then the client of the player moves the pig, and the server does no AI
step for the pig (GOAL-FRM-009). The goals of the pig keep their state. ANIM-RIDE-002 and the
rules after it state the motion and the boost. A pig with a player passenger that does not control
it runs its AI. Its stroll can start, because a player passenger that does not control the pig is
not a passenger that controls the mob.

**GOAL-ANI-008** (`documentation`, semantic, confidence: medium, fidelity: exact). These values that
the goals read come from other documents:

| Value | Rule |
|-------|------|
| `movement_speed`, `follow_range` and `tempt_range` of each type; a baby has the same values | TRK-TYPE-004, ANIM-FOOD-003 |
| the dimensions and the eye height, adult and baby | TRK-TYPE-002, ANIM-TYPE-001 |
| the food items and the tempt items | ANIM-FOOD-001, ANIM-FOOD-002 |
| the condition "can mate" | ANIM-FOOD-004 |
| the age, adult and baby | ANIM-AGE-001 |
| the love state and its timer; a hit ends the love | ANIM-LOVE-001, ANIM-LOVE-002 |
| the result of the breed action | ANIM-BREED-001 |
| the result of an eat | ANIM-TICK-003 |
| the controller of a pig | ANIM-RIDE-001 |

ANIM-FOOD-003 also lists the priorities and the speed modifiers of section 5. The two documents must
give the same values. This document is the authority for the order of each goal list and for the
flags of each goal.

## 6. Hooks

The headings of this section reserve the place of later rules. Each line states the scope. The
framework of sections 1 to 3 applies to them.

### 6.1 Target selection

The goals of a target list: the nearest attackable target (AIT-VAN-011) and the revenge target. They
hold the flag `target` and write the attack target.

### 6.2 Melee attack

The melee goal: its start check, its path timers and its hit (AIT-VAN-006, AIT-RFX-003).

### 6.3 Anger and alerts

The anger of a zombified piglin and the alert to other mobs of the same type (AIT-VAN-024,
AIT-CPL-004, `gold-farm.md` mechanic 9).

### 6.4 The turtle egg goal

The goal that moves to a `turtle_egg` and breaks it (`gold-farm.md` mechanic 6, PATH-REQ-001).

### 6.5 Strafe

The move operation `strafe` of the move control (PATH-MOV-001), which ranged goals set.

### 6.6 Door goals and the creeper swell

The goal forms of the door interaction (AIT-VAN-008) and the creeper swell (AIT-VAN-007).

### 6.7 Leash and riders

The navigation of a leashed mob toward its holder, the home that a leash sets, and the controls of a
mob that rides a mob.

### 6.8 Mooshroom

The mooshroom uses the goal list of the cow. Its walk value prefers `mycelium`.

## 7. Parallelism notes

This section lists, for each goal, its place, its reads, its writes and its world queries. It does
not give algorithms. "Own" means the row and the guidance state of the mob itself. "Snapshot" means
the state of AIT-LAG-001.

| Goal | Place | Reads (own) | Reads (other entities and world) | Writes | World queries |
|------|-------|-------------|----------------------------------|--------|---------------|
| float | reflex | fluid height, lava flag, eye height, jump flag holder | none | jump request; flag `jump` | none |
| panic | decision | damage record, fire ticks, block position, home, path state | blocks: collision shape at the own position, water, solid render, malus inputs, light | path request; flag `move`; in-panic state | `Q-WATER`; `D(5, 4)` |
| breed | decision, with a pair action | love state, position, box, path state | same-type entities: position, box, love state, in-panic state, alive, spectator, invisibility, armor count, `mob_visibility` items | look target; path request; love count; flags `move`, `look`; breed proposal | `Q-PARTNER` |
| tempt | decision | position, calm-down | player table: position, alive, spectator, held items in both hands, sneak, invisibility, armor count, `mob_visibility` items; the attribute `tempt_range` | look target; path request or path clear; calm-down; flags `move`, `look` | `Q-TEMPT` |
| follow an adult | decision | age, position, box, path timer | same-type entities: position, box, age, alive | path request; path timer | `Q-ADULT` |
| stroll | decision | idle time, the passenger that controls the mob, in-water flag, position, home, path state | blocks: solid render, legacy solid flag, water, malus inputs, light | path request or path clear; flag `move` | `L(15, 7)`, `L(10, 7)`, `D(10, 7)` |
| look at a player | decision | position, eye height, look count | player table: position, eye height, alive, spectator, sneak, invisibility, armor count, `mob_visibility` items, vehicle chain; blocks for one ray to each candidate | look target; look count; flag `look` | `Q-LOOK` |
| look around | start: decision; tick: reflex (guidance) | position, eye height, direction, look time | none | look target in each tick; flags `move`, `look` | none |
| eat grass | start: decision; eat: reflex (due event) | age, block position, eat timer | blocks at the own position and below; the game rule `mob_griefing` | event 10; path clear; eat timer; block write at the eat; the eat of ANIM | none |
| look control, body rotation | reflex (guidance) | rotations, look target, look hold, path state, position change, first passenger | none | pitch, head yaw, body yaw, stable values | none |

These notes follow from the table:

- Each goal writes only its own mob, except the breed proposal (AIT-CPL-004) and the block write of
  the eat (GOAL-EAT-003). Thus the decisions of different mobs do not conflict.
- The goals of one mob conflict only through the shared path, the look target and the flags. Their
  order inside one mob is GOAL-FRM-005.
- A decision reads other entities only through the snapshot. The entity reads are the in-love and
  in-panic states, the age, the position, the box and the visibility inputs. The snapshot must
  publish these values for each animal.
- The player reads come from the player table. GOAL-QRY-005 states its values: the position, the
  eye height, two flags, the shift key state, the two held items and the vehicle chain. The
  invisibility inputs are deferred (section 9, U3).
- Most evaluations of an adult animal that is not in love draw only random gates: `0.02` for each
  look goal, `1/60` for the stroll and `1/500` for the eat. For such an animal, the tempt query is
  the only query in each full evaluation. It reads only the player table. A baby adds `Q-ADULT`,
  and an animal in love adds `Q-PARTNER`.
- The block reads of a random position search are at most 10 candidates, each with a short column
  for GOAL-POS-003, step 2. The light reads are one value for each candidate.

## 8. Intentional deviations

### GOAL-DEV-001: Ties in a nearest selection by the order key

- **Rule:** GOAL-QRY-004, in `Q-PARTNER`, `Q-TEMPT`, `Q-ADULT` and `Q-LOOK`.
- **Reference:** `Q-TEMPT` and `Q-LOOK` take the first player in the player list of the dimension.
  `Q-PARTNER` takes the first entity in the order of the entity storage. `Q-ADULT` takes the last
  entity in that order. These orders depend on the join order and on the storage of the sections.
- **Visible to the client:** rarely. Only two members at exactly the same distance give a
  difference. A different partner or player can win.
- **Test:** a differential test accepts each member at the smallest distance.

### GOAL-DEV-002: The calm-down of the tempt counts ticks

- **Rule:** GOAL-TMP-004.
- **Reference:** the calm-down is 50 evaluation units. It decreases by 1 only in a full evaluation
  in which the goal is examined. Thus a goal with a smaller priority number that holds `move` or
  `look` makes the calm-down longer. The same occurs while the other tempt goal of a pig runs.
- **Visible to the client:** yes, rarely. In the reference, an animal that breeds or panics during
  the calm-down follows the player later than 100 ticks after the stop. This server lets it follow
  100 ticks after the stop.
- **Test:** a differential test accepts a start at 100 ticks or more after the stop, in this case: a
  goal with a smaller priority number held a flag of the tempt goal during the calm-down.

### GOAL-DEV-003: The stable head yaw compares wrapped angles

- **Rule:** GOAL-CTL-006, step 3. This server compares `|wrap(head yaw - stable head yaw)|` with 15.
- **Reference:** the reference compares the difference without a wrap. The head yaw is not wrapped,
  so a head that turned through more than one full turn can give a difference near 360 with no real
  turn. Then the reference resets the stable count.
- **Visible to the client:** almost never. The body can start to follow the head up to 10 ticks
  later after such a turn.
- **Test:** a differential test does not compare the body yaw after a turn of more than 180 degrees
  in total.

### GOAL-DEV-004: Keyed random values for the goals

- **Rule:** AIT-LAG-008. The part of each stream names the goal and the draw, for example "stroll
  gate" or "look around angle".
- **Reference:** each goal draws from the random source of its mob, in the order of the goal list.
  AIT-DEV-002 records this.
- **Visible to the client:** no, as a distribution.
- **Test:** as AIT-DEV-002.

## 9. Unresolved

- **U1**: resolved. PATH-REQ-004, step 1, keeps the current path after a failed request to an
  entity, as GOAL-NAV-002 states.
- **U2**: partly resolved. GOAL-POS-006 gives a walk value without light. It is exact on a world with
  full sky light at each candidate that is not above grass. Deferred: the block light, the sky light
  below a roof, the sky darkness of the time and the weather, and the ambient light. These need a light engine of the server side, which no specification states yet. The block
  corpus must still give the solid render property and the legacy solid flag (PATH section 12.3).
- **U3**: partly resolved. GOAL-QRY-005 states the player table. Deferred: the invisibility of a
  player, its armor count, the items with `mob_visibility`, and the clamp of the factor to
  `[0, 10]`. These need the effects and the equipment of a player.
- **U4**: partly resolved. GOAL-FLT-003 sets the fluid height to 0 on a dry world. Deferred: the
  fluid height of water and lava at the box of a mob, the lava flag and the in-water flag. These
  need the fluid model that `entity-physics.md`, section 12, puts out of scope.
- **U5**: the leash, the mob that rides a mob, and the home are hooks (section 6.7). The random
  position searches read the home now.
- **U6**: ANIM-FOOD-003 repeats the priorities and the speed modifiers of section 5, and
  ANIM-TYPE-005 repeats the malus rows of GOAL-ANI-005. A change must update both documents.
  ANIM-TYPE-005 refers to `pathfinding.md` for the walk value from the light. GOAL-POS-004 states
  that value; `pathfinding.md` does not.
- **U7**: the experiments with a client (E-G4 to E-G7, E-G10, E-G11) did not run. The tempt range,
  the visibility factor and the rotations rest on the source.
- **U8**: the goals of hostile mobs (section 6) wait for step G5 of `gold-farm.md`.

## 10. Experiments

### 10.1 Experiments with a vanilla server

Start a vanilla server of the version in the table at the top. Use a flat world, the game rule
`spawn_mobs` false, and `pause-when-empty-seconds=-1`. Force-load the chunks of the test. Use
`/tick freeze` before each step, and `/tick step 1` for each tick. Read the positions with
`/data get entity`. Read the rotations from the packets of a scripted client
(`tools/vanilla-oracle/`). Repeat each experiment with a random part at least 200 times.

| Experiment | Steps | Expected result | Rules |
|------------|-------|-----------------|-------|
| E-G1 | Summon two cows with `InLove:600`, 1 block apart. Step 1 tick at a time and count the cows. | A child appears 57 to 60 ticks after the summon. | GOAL-BRD-001, AIT-VAN-023 |
| E-G2 | Summon a pig. After 10 ticks, run `damage` with 1 point of `cactus`. Record the position in each tick for 130 ticks. | The pig runs at 0.215 blocks in each tick on paths of up to about 6 blocks. It starts a new path while the record is fresh, and stops after the first path that ends more than 40 ticks after the hit. | GOAL-PNC-001, GOAL-PNC-002, GOAL-POS-002, GOAL-CTL-002 |
| E-G3 | Summon an adult pig with `NoAI:1b`, and a baby pig with `Age:-24000` 8 blocks away. Record the position of the baby. | The first request fails. The baby walks at 0.1666 blocks in each tick, with requests every 10 ticks. It stops near the adult, within 3 blocks. | GOAL-FOL-001, GOAL-NAV-002, GOAL-FRM-003 |
| E-G4 | Put a player with wheat at distances of 9.9, 10.1 and 2.4 blocks from a cow. Repeat while the player sneaks, and while the player is invisible with no armor. | The cow follows at 9.9, not at 10.1. It stops its path at 2.4. A player that sneaks tempts up to 8 blocks; an invisible player up to 2 blocks. | GOAL-TMP-001, GOAL-TMP-002, GOAL-QRY-002 |
| E-G5 | Let a cow follow a player with wheat. Change the item to dirt, and record the tick of the stop. Change it back to wheat each tick. | The cow follows again 100 to 102 ticks after the stop. | GOAL-TMP-004, GOAL-DEV-002 |
| E-G6 | Stand 4 blocks from a pig for 2000 ticks. Record the `rotate_head` and the pitch of each look. | Each look lasts 40 to 80 ticks. The pitch is clamped to 40. The head returns to the body at 10 degrees in each tick. | GOAL-LKP-001, GOAL-CTL-004 |
| E-G7 | Put 100 sheep on grass in a closed pen. Record each `entity_event` 10 and each `block_update` of grass to dirt. | Each change to dirt comes 34 ticks after an event 10 of a sheep on it. The rate is about 1 in 1000 ticks for each adult sheep. | GOAL-EAT-001, GOAL-EAT-002 |
| E-G8 | Put a pig in water 3 blocks deep. Record its `y` in each tick. | The pig stays at the surface and jumps in 80 % of the ticks. | GOAL-FLT-001 |
| E-G9 | Summon 100 pigs with no player in the dimension. Record the ticks in which each pig starts to walk, for 400 ticks. | No pig starts a stroll after its tick 100. Before tick 100, the starts have a chance of about 1 in 120 ticks for each pig. | GOAL-STR-001, AIT-VAN-020 |
| E-G10 | Record the head of a pig with no player near, for 2000 ticks. | Each look around lasts 22 to 40 ticks, at the height of the eye. | GOAL-LKR-001 |
| E-G11 | Turn the head of a pig that stands with a look at a player, then let the look end. Record `rotate_head` and the body of the client view. | The body follows the head 10 to 20 ticks after the head stops. | GOAL-CTL-006 |
| E-G12 | Set a pig on fire 3 blocks from water on stone. Record its path. | The pig walks into the nearest water block of GOAL-PNC-003. | GOAL-PNC-002, GOAL-PNC-003 |
| E-G13 | With no player in the dimension, summon pigs 6 blocks apart with `PersistenceRequired:1b`, with `CustomName`, and with no key. Step 300 ticks and record the positions. Step 600 ticks and record them again. Then run the command of GOAL-WAKE-020 each 80 ticks, and record the positions. | No pig of a group moves in the 600 ticks. After the first run of the command, the pigs stroll again. | GOAL-STR-004, GOAL-WAKE-019, GOAL-WAKE-020 |

### 10.2 Tests for this server

| Test | Steps | Rules |
|------|-------|-------|
| C-G1 | Give a goal list with two goals of the same priority and the same flags. Make both start conditions true. Check that the first in the list runs. | GOAL-FRM-002 |
| C-G2 | Run 50,000 pigs on a flat world with `AiDelay::Fixed(1)`, two times. Compare the world hash in each tick. | GOAL-DEV-004, AIT-INV-002 |
| C-G3 | Compare the start rates of the stroll, the look goals and the eat over 10,000 animals with the chances of section 4. | GOAL-FRM-007, GOAL-STR-001, GOAL-LKP-001, GOAL-LKR-001, GOAL-EAT-001 |
| C-G4 | Run the random position searches on fixed block scenes with fixed draws. Compare the result with the rules. | GOAL-POS-002, GOAL-POS-003, GOAL-POS-004 |
| C-G5 | Run 50,000 pigs with `AiDelay::Fixed(1)` two times: one run evaluates each full evaluation, one run skips each quiet evaluation. Hit a group of pigs with `damage`, and let a player walk near them. Compare the world hash in each tick. | GOAL-WAKE-001 to GOAL-WAKE-014, AIT-INV-002 |
| C-G6 | Count the evaluations in which each gate passes over 10,000 animals, with the waits of GOAL-WAKE-003. Compare the counts with a geometric law of the chance of each gate. | GOAL-WAKE-003 |

## 11. Captured results

The vanilla server of `26.4-snapshot-7` ran on a flat world with the settings of section 10.1. The
records count the steps of `/tick step 1` after the summon.

### E-G1

Two cows with `InLove:600` at `(0.5, -60, 0.5)` and `(1.5, -60, 0.5)`. The count of cows is 2 after
57 steps and 3 after 58 steps. The breed goal starts in the first AI step (tick count 1), and the
30th full evaluation of one cow is at tick count 58. This agrees with GOAL-BRD-001 for a cow whose
full evaluations are at the even tick counts.

### E-G2

A pig at `(0.5, -60, 0.5)`. The `damage` command ran after step 10. The pig did not move in steps 11
and 12, and moved in step 13. This agrees with a failed search in the first full evaluation
(GOAL-POS-002, a chance of about 0.31) and a start in the next one. The pig ran at a steady 0.2150
blocks in each tick. The first path ended at step 27. A second path started at step 29 and ended at
step 53. No panic path started after that. At step 61 the pig started a stroll at 0.1377 blocks in
each tick. The speeds agree with GOAL-CTL-002 for `m = 1.25` and `m = 1.0`.

### E-G3

An adult pig with `NoAI:1b` at `(0.5, -60, 0.5)` and a baby at `(8.5, -60, 0.5)`. The baby did not
move in steps 1 to 9, and moved from step 10 at 0.1666 blocks in each tick. Thus the first request in
the first AI step gave no path, and the next request came 5 evaluation units later. From step 22 the
speed fell to 0.137 (a stroll with `m = 1.0` that replaced the path). From step 30 the speed rose to
0.1666 again (the next request of the follow goal). The baby passed within 3 blocks of the adult at
step 47 and walked on to step 59. It stopped at `(1.822, -60, 0.783)`, 1.35 blocks from the adult.

### E-G13

No player was in the dimension. Each group had 16 pigs on a grid with 6 blocks between pigs:
`{PersistenceRequired:1b}`, `{CustomName:"a"}` and a compound with only a tag. In the first 300
ticks, 31 of the 48 pigs moved more than 0.5 blocks. From tick 300 to tick 900, no pig of a group
moved: each position stayed equal, bit for bit.

A second run summoned 48 pigs with `{}` and stepped 300 ticks. Then the command of GOAL-WAKE-020
ran each 80 ticks. In the next 400 ticks, 43 of the 48 pigs moved more than 0.5 blocks. In the
320 ticks after that, all 48 pigs moved. Each pig stayed at `y = -60`. No run of the command
failed.
