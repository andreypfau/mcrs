# Entity AI timing: reflexes, decisions and their delay

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

This document states when the AI of a mob acts, but not what the AI does. It divides the AI into two
parts. A reflex acts in the hot tick, in the same tick as in the vanilla server. A decision runs
beside the tick and applies a small number of ticks later. This document states which parts can be
late, and by how much. It also states the rules that keep a late decision deterministic.

The aim has two parts. A player must not see a difference in the behaviour of the mobs. The AI must
not add cost to the hot loop of movement, collision and push (`entity-physics.md`).

Section 1 states the vanilla cadence of each part of the AI. The rules of section 1 come from the
reference implementation. No experiment has confirmed them yet. Sections 2 to 7 state the model of
this server. Section 10 gives the experiments.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **tick**, **entity**, **living entity**, **position**, **box**: as
  `entity-physics.md`, section 0.1, defines them.
- **AI step**, **travel step**, **server-simulated entity**, **external impulse**: as
  `entity-physics.md`, section 0.1, defines them.
- **Exact-parity mode**: as PHY-ORD-010 defines it.
- **Mob**: a living entity that is not a player and that has an AI. A mob with the data value
  `NoAI:1b` has no AI.
- **Hot tick**: the part of the server tick that moves the entities, computes the push and applies
  the writes between entities (`entity-physics.md`, section 9).
- **AI stage**: the part of the server that computes the decisions. It runs beside the hot tick.
- **Part**: one unit of the AI with its own cadence, for example the goal selection or one sensor.
  Section 1 lists the parts.
- **Period**: the number of ticks between two runs of a part in the vanilla server.
- **Phase**: the remainder of the tick number divided by the period, at the ticks on which a part
  runs.
- **Goal**: a unit of behaviour of a mob that has a start condition, a continue condition and a
  tick. A goal holds one or more control flags while it runs.
- **Control flag**: one of the four flags `move`, `look`, `jump` and `target`. Only one goal can
  hold one flag at a time.
- **Goal selection**: the procedure that stops goals, starts goals and ticks goals (AIT-VAN-002).
- **Target selection**: the goal selection for the goals that hold the flag `target`. These goals
  set the attack target.
- **Attack target**: the entity that a mob attacks, or no entity.
- **Full evaluation**: a tick in which the goal selection checks the start conditions and the
  continue conditions (AIT-VAN-002).
- **Evaluation unit**: the interval between two full evaluations of a mob. It is 2 ticks.
- **Brain**: the AI of a mob that uses memories, sensors and behaviours, for example a villager,
  a piglin or a goat. Some mobs also run goals in the same AI step.
- **Memory**: one value in the brain of a mob, with an optional expiry time. The registry
  `memory_module_type` names each kind.
- **Sensor**: a part of the brain that scans the world and writes memories. The registry
  `sensor_type` names each kind.
- **Behaviour**: a unit of the brain with a start condition, a tick and a stop. A one-shot behaviour
  runs its action in the tick in which it starts, and then stops.
- **Path**: a list of block positions (nodes) from the mob to a goal position.
- **Path search**: the procedure that finds a path.
- **Next node**: the node of a path that the mob moves to now.
- **Guidance**: the work of each tick that moves a mob along its current path and turns it to its
  current look target. It contains the path follower, the move control, the look control and the
  jump control.
- **Path follower**: the part of the guidance that advances the next node, detects a stuck mob and
  gives the move control its wanted position (AIT-VAN-012).
- **Move control**, **look control**, **jump control**: three parts of the guidance. From their
  targets, they set the input vector, the head rotation and the jump input of the mob.
- **Look target**: the point that the look control turns the head to.
- **Line of sight**: the result of a ray from the eye of one entity to the eye of another entity.
  The collision shapes of the blocks stop the ray.
- **Idle time**: a counter of a living entity. It increases by 1 in each AI step. It changes to 0
  when the entity takes damage, and when a player is less than 32 blocks away (AIT-VAN-020).
- **Follow range**: the value of the attribute `follow_range` of a mob.
- **Guidance state**: the result of the decisions that the hot tick reads. It contains the attack
  target, the current path and its speed, and the look target. It also contains the control flags of
  each goal that runs, and the state of each reflex.
- **Reflex**: a part of the AI that runs in the hot tick and acts in the same tick as in the vanilla
  server (section 2.1).
- **Decision**: the result of one run of one part in the AI stage, for one mob (section 2.2).
- **Issue tick**: the tick in which the vanilla server runs the part that gives a decision.
- **Snapshot**: the state that a decision reads (AIT-LAG-001).
- **Application**: the step in which the hot tick changes the guidance state from a decision.
- **Tile task**: the unit of work of the entity pass that ticks the entities of one tile
  (`physics-architecture.md`).
- **Delay**: the number of ticks from the issue tick of a decision to its application. The symbol is
  `k`.
- **Allowed delay**: the largest delay of a part for which a player does not see a difference
  (AIT-DEC-002).
- **`max_lag`**: the largest delay of each decision (AIT-LAG-003).
- **Stale decision**: a decision whose validity check fails at its application (AIT-LAG-005).
- **Fact**: a record of an event that the hot tick sends to the brain of a mob, for example damage
  that the mob took. A decision that the hot tick dropped also gives a fact.
- **Pair action**: an action that changes two or more entities, or an entity and shared world state.
  Examples are a breed action and the claim of a point of interest.
- **Proposal**: the form of a pair action in a decision (AIT-CPL-001).
- **Grant**: the result of a proposal that passes its check in the hot tick.
- **Point of interest**: a block that the game records for villagers, for example a job site block
  or a bed. A villager claims it with a ticket.
- **Order key**: the UUID of an entity, as PHY-CMP-013 defines it. It does not depend on the
  division of the world into cells, and the save keeps it.
- **Phase offset**: the value from 0 to 3 that PHY-CMP-016 derives from the UUID.
- **Merge point**: the one global step at the start of a tick in which the proposals apply
  (AIT-CPL-002). The writes between entities that are not claims also apply there (PHY-CMP-015). A
  claim does not apply at the merge point. It enters the arbitration of PHY-CMP-017.
- **Player table**: the positions of all players of one dimension at the end of the previous tick.
  Each cell has the same copy (AIT-CDN-002).
- **Cadence class**: a group of mobs that use one reduced cadence (AIT-CDN-003).
- **`AiDelay::Fixed(k)`**, **`AiDelay::AtLeast(k)`**: the two delay modes of the AI stage
  (AIT-LAG-004).
- **`AiDelay::Inline`**: the mode of the exact-parity mode, with `k = 0` (AIT-LAG-011).

### 0.2 Provenance

- `source_suggested`: taken from the reference implementation, with no captured result. The
  confidence is at most medium. Section 10.1 attaches an experiment.
- `design`: a rule of this server's own design. It does not state vanilla behaviour. A comparison
  with a vanilla server cannot confirm it. A test of section 10.2 confirms it.
- `unresolved`: not decided. Section 9 collects these items.

Each rule states its kind and its confidence, for example
`(source_suggested, semantic, confidence: medium)`. The kinds are those of `docs/spec/README.md`. A
rule with the label `design` that states a number gives a default value. Section 7 lists each
configurable value.

### 0.3 ID prefix

Each rule has an ID with the prefix `AIT`. The second part names the section.

| Prefix | Subject |
|--------|---------|
| `AIT-VAN` | the vanilla cadence of each part |
| `AIT-SPL` | the split into reflexes and decisions |
| `AIT-RFX` | the reflexes |
| `AIT-DEC` | the decisions and their allowed delays |
| `AIT-LAG` | the rules of a delayed decision |
| `AIT-CPL` | pair actions |
| `AIT-CDN` | the cadence of this server |
| `AIT-INV` | invariants |
| `AIT-CFG` | configuration |

### 0.4 Scope

This document states when the parts of the AI act, the split of the AI and the invariants. It does
not state the behaviour of each goal, behaviour or sensor. The rules of section 1 name a behaviour
only to give its cadence. A specification of the AI of each mob type can come later. It must assign
each of its parts to a reflex or to a decision, with the rules of section 2.

## 1. The vanilla cadence

The rules of this section state the vanilla server. They give the period and the random parts of
each part of the AI. The table of AIT-VAN-025 collects them. These periods are the slack that a late
decision can use with no visible difference.

### 1.1 The AI step of a mob

**AIT-VAN-001** (`source_suggested`, semantic, confidence: medium). Step 4 of PHY-ORD-002 runs the
AI of a mob. In the vanilla server, the AI of a mob with goals does these steps in this order:

1. Increase the idle time by 1.
2. Clear the cache of line of sight results (AIT-VAN-004).
3. Do the target selection, then the goal selection (AIT-VAN-002).
4. Do the path follower (AIT-VAN-012).
5. Do the part of the mob type. For a mob with a brain, this is the brain tick (AIT-VAN-017).
6. Do the move control, then the look control, then the jump control (AIT-VAN-013).

A mob with `NoAI:1b` does not do these steps. A mob runs these steps only when it ticks
(PHY-ORD-011).

**AIT-VAN-002** (`source_suggested`, semantic, confidence: medium). The goal selection of a mob runs
in each tick. Let `t` be the tick count of the mob and `n` its network id. A tick is a full
evaluation when `(t + n)` is even, or when `t ≤ 1`. In a full evaluation, the goal selection does
these steps:

1. Stop each goal that runs and holds a disabled flag, or whose continue condition is false.
2. Release the flags of the goals that stopped.
3. Examine each goal that does not run, in the order in which the mob type added the goals. Start
   the goal when all these conditions are true:
   - it holds no disabled flag;
   - each of its flags is free, or the holder has a larger priority number and permits an interrupt;
   - its start condition is true.

   A goal that starts stops the holders of its flags, and takes the flags.
4. Tick each goal that runs.

In a tick that is not a full evaluation, the goal selection ticks only some goals that run. These
are the goals that need a tick in each tick (AIT-VAN-005). The other goals do nothing in that tick.

Thus the start conditions and the continue conditions have a period of 2 ticks, with a phase from
the network id. A start can occur 0 or 1 tick after its condition becomes true.

**AIT-VAN-003** (`source_suggested`, semantic, confidence: medium). A goal that ticks only in full
evaluations counts its timers in evaluation units. A delay of `d` ticks in the goal is `ceil(d / 2)`
evaluation units, so the real delay is `2 × ceil(d / 2)` ticks. A random chance of "1 in `r` ticks"
is a chance of 1 in `ceil(r / 2)` in each full evaluation. A goal that ticks in each tick counts its
timers in ticks.

**AIT-VAN-004** (`source_suggested`, semantic, confidence: medium). A mob keeps the line of sight
result for each other entity during one tick. The first query in a tick casts the ray. The other
queries in the same tick use the result. Step 2 of AIT-VAN-001 clears the cache. The cache does not
change the result.

**AIT-VAN-005** (`source_suggested`, semantic, confidence: medium). These goals tick in each tick in
the reference. Each other goal ticks only in full evaluations.

- the creeper swell (AIT-VAN-007);
- the melee attack (AIT-VAN-006) and the ocelot attack;
- the ranged attacks: the bow, the crossbow and the generic ranged attack;
- the door interaction: open a door and break a door (AIT-VAN-008);
- float in a fluid, and climb out of powder snow (AIT-VAN-009);
- move to a block, for example a crop or a turtle egg;
- random stand, and random look around;
- some goals of the types bee, squid, blaze, slime, magma cube, ghast, guardian, shulker, vex and
  the raiders.

**AIT-VAN-006** (`source_suggested`, semantic, confidence: medium). The melee attack goal has these
timers:

- Start: the start condition runs at most one time in 20 game ticks. It needs an attack target that
  is alive. It also needs a path to the target, or the target in reach.
- Path: while the goal runs, a timer counts down by 1 in each tick. At 0, the goal searches a new
  path when the mob sees the target and one of these is true:
  - the goal has no last path;
  - the target moved 1 block or more after the last path;
  - a random chance of 0.05 passes.

  The goal that follows a target that it does not see needs no line of sight.
- Path timer: after a new path, the timer is `4 + r` ticks. `r` is a random integer from 0 to 6. The
  timer increases by 10 when the target is more than 32 blocks away. Else it increases by 5 when the
  target is more than 16 blocks away. It increases by 15 when the path search fails.
- Attack: a cooldown counts down by 1 in each tick. The mob attacks when the cooldown is 0, the
  target is in reach and the mob sees the target. Then the cooldown changes to 20 ticks.
- Reach: the box of the mob, expanded horizontally by `sqrt(2.04F) - 0.6F`, intersects the hit box
  of the target. An item with the component `attack_range` changes the reach.

A mob with a brain attacks with a one-shot behaviour. It checks the reach in each tick, and it reads
the visible entities from the last sensor scan. The cooldown is a memory with an expiry.

**AIT-VAN-007** (`source_suggested`, semantic, confidence: medium). The creeper swell has these
steps:

- Start: in a full evaluation, the swell goal starts when the swell direction is more than 0. It
  also starts when the attack target is alive and less than 3 blocks away (squared distance less
  than 9). The start stops the current path.
- Direction: while the goal runs, the direction changes in each tick. It changes to -1 when the
  target is not alive or is more than 7 blocks away (squared distance more than 49). It also changes
  to -1 when the creeper does not see the target. Else it changes to +1.
- Continue: in a full evaluation, the goal continues when the direction is more than 0, or when the
  target is less than 3 blocks away.
- Counter: at the start of the tick of the creeper, before its AI step, the counter adds the
  direction. The counter does not go below 0. When the counter reaches the value `Fuse`, which is 30
  by default, the creeper explodes. A creeper with `ignited:1b` has the direction +1.

Thus the direction that the AI sets in tick `T` changes the counter in tick `T + 1`.

**AIT-VAN-008** (`source_suggested`, semantic, confidence: low). A mob opens a door on its path in
one of two ways.

The goal form, for example a zombie or a vindicator:

- Start: in a full evaluation, the goal starts when the mob has a path on the ground. Its horizontal
  collision flag must also be true. It also needs a wooden door 1 block above a node of the path, in
  the nodes up to the next node plus 1. The horizontal squared distance to that door must be at most
  2.25. The door can also be 1 block above the block of the mob.
- Open: the goal opens the door when it starts.
- Close: in each tick, a timer of 20 ticks counts down. The goal also detects when the mob passes
  the door. When the timer ends, or when the mob passed the door, the goal stops and closes the
  door. Some mob types do not close the door.
- Break: the break form adds 1 to its progress in each tick. The door breaks at 240 ticks of
  progress, on a difficulty that permits it.

The brain form, for example a villager, runs in each tick while the mob has a path that started and
is not done:

1. If the next node is the node of the last check, set the cooldown to 20 and do the check.
2. Else, decrease the cooldown by 1. If the cooldown is still more than 0, do not do the check.
3. The check opens a closed door at the previous node and at the next node, and records it.
4. The check then closes each recorded door that the mob passed. It does not close a door that is
   more than 3 blocks away. It also does not close a door when another mob of the same type moves
   through it. That mob is less than 2 blocks from the door, and the door is its previous node or
   its next node.

Thus, after the next node changes, the check waits for up to 19 ticks. A villager can stop at a
closed door during that time. This is vanilla behaviour, and this server must keep it.

**AIT-VAN-009** (`source_suggested`, semantic, confidence: medium). Two goals react to the block
that the mob is in:

- Float: in a fluid deeper than the limit of the mob type, the mob jumps with a chance of `0.8F` in
  each tick.
- Climb out of powder snow: a mob of the tag `powder_snow_walkable_mobs` in powder snow jumps in
  each tick. The block above must be powder snow or have no collision shape.

The start of each goal is in a full evaluation.

**AIT-VAN-010** (`source_suggested`, semantic, confidence: medium). The panic of a mob with goals
starts in a full evaluation after the mob took damage of a type that causes panic. A mob on fire
searches water in 5 blocks. Else it searches a random position. Thus the panic starts 0 or 1 tick
after the damage.

A mob with a brain reads the damage from a sensor (AIT-VAN-016). It can start to panic up to 19
ticks after the damage.

### 1.2 Target selection

**AIT-VAN-011** (`source_suggested`, semantic, confidence: medium). The target selection has these
timers:

- Nearest attackable target: in each full evaluation, the start condition passes with a chance of 1
  in `ceil(r / 2)`. The default of `r` is 10, so the chance is 1 in 5. Then the mob takes the
  nearest valid target in a box of the follow range around its own box. The distance is from the eye
  of the mob. A goal that must see the target also checks the line of sight.
- Thus the wait from the tick in which a target becomes valid to the start is random. It has a
  geometric distribution with a mean of 5 evaluation units, about 10 ticks.
- Keep: in each full evaluation, the goal continues while the target is in the follow range. The mob
  must be able to attack the target. The target must not be on the team of the mob. A goal that must
  see the target stops when the mob did not see it in 30 evaluation units, about 60 ticks.
- Revenge: the revenge goal starts in a full evaluation when the time of the last damage by a living
  entity changed. Its limit for a target that the mob does not see is 150 evaluation units.
- Reach check: a goal that must reach the target searches a path to it. It keeps the result for
  `ceil((10 + r) / 2)` evaluation units, where `r` is a random integer from 0 to 4.

The alert to other mobs of the same type is a pair action (AIT-VAN-024).

### 1.3 Path search and path follower

**AIT-VAN-012** (`source_suggested`, semantic, confidence: medium). The path follower runs in each
tick (step 4 of AIT-VAN-001). For walking mobs, it does the steps below in the order of
PATH-FOL-001. Steps 2 to 5 require an unfinished path. Steps 3 and 4 also require a mob that can
update a path (PATH-REQ-001, step 2).

1. If a recompute waits (AIT-VAN-015), try it. The end of the path drops a waiting recompute
   (PATH-RCP-002).
2. Advance the next node as PATH-FOL-002 states. A mob that cannot update a path does only the air
   advance of PATH-FOL-001, step 4, and then step 5.
3. Then do the stuck check of PATH-FOL-003.
   The acceptance of a changed path starts the window: the navigation has no unfinished path, or
   the ordered node positions differ (PATH-REQ-004). The same path given again keeps it. A
   recompute is a request, so the same rule applies to its path.
   Each advance and each check starts it again.
   Check only after more than 100 follower ticks from the window start.

   A failed check drops the path and marks the mob stuck.
   Without an earlier stop or restart, a motionless mob with positive mob speed drops its path
   101 follower ticks after the window starts.
   The advance comes before the check in the same tick. Thus a mob that advances at most 101
   follower ticks after each window start does not fail this check.
4. If an unfinished path remains, do the timeout check of PATH-FOL-004. The timer starts again at
   each new next node.
5. If an unfinished path remains, give the move control its wanted position and speed modifier
   (PATH-FOL-005).

A failed stuck check sets the stuck state. The stuck state has no fixed expiry. Only the acceptance
of a changed path clears it. A failed check always drops the path, so no other stuck check and no
timeout runs before that acceptance. A request that gives the current path, a stop and idle ticks
do not clear it. `pathfinding.md`, PATH-DEV-006 to PATH-DEV-009, records where the reference
differs.

**AIT-VAN-013** (`source_suggested`, semantic, confidence: medium). The move control, the look
control and the jump control run in each tick, after the path follower and after the goals. A look
target that a goal sets stays for 2 ticks. A goal that ticks only in full evaluations sets it again
in each full evaluation, so the head does not return. The jump control sets the jump input for one
tick.

**AIT-VAN-014** (`source_suggested`, semantic, confidence: medium). The path search has these
limits:

- Path length: the largest length is `L = max(follow range, m)`. `m` is 16 by default. Some types
  have a larger `m`: 32 for a fox, 40 for a llama, and 48 for a bee and an allay. The search does
  not expand a node at a distance of `L` or more from the start.
- Node budget: the search visits at most `floor(16 × L) × f` nodes. `f` is 1 by default. A goal can
  change it, for example a bee sets 0.5 and 10.
- Region: the search reads the blocks in a cube around the start. Its half side is `L + 8` for a
  block target, and `L + 16` for an entity target. For an entity target, the start is the block
  above the mob.
- Reach: the search ends at a node whose Manhattan distance to the target is at most the reach range
  of the request.
- Same target: a request for the target of the current path returns the current path while that path
  is not done.

**AIT-VAN-015** (`source_suggested`, semantic, confidence: medium). The path of a mob
changes for these triggers:

- Block change: a block whose collision shape changes, and that is near the rest of the path. Near
  means that the distance from the block centre to a mid point is less than the number of nodes that
  remain. The mid point is between the mob and the end node. The recompute occurs in the same tick.
  When the last recompute was 20 ticks ago or less, the recompute waits. The path follower tries it
  in each tick until 20 ticks passed.
- Goal: each goal and each behaviour asks for a path on its own timer. AIT-VAN-006 gives the timer
  of the melee attack. A brain asks for a new path when its walk target moved more than 2 blocks
  from the target of the current path. The squared distance must be more than 4. The brain checks
  this in each tick.
- Stuck or timeout: the path follower stops the path (AIT-VAN-012). Brain movement chooses a
  retry pause only when it stops with an unreached walk target and stuck navigation. The pause
  is a uniform integer from 0 to 39 eligible start attempts, not elapsed ticks. Ineligible
  attempts do not spend it. A failed initial route search does not create that pause
  (BRAIN-VAN-035 in `mob-brain.md`).
- Done: the path ends when the mob reaches the last node.

### 1.4 Sensors and brain

**AIT-VAN-016** (`source_suggested`, semantic, confidence: medium). Each sensor of
a brain runs with its own period. At brain creation, each sensor chooses an integer offset O
uniformly from 0 to `P - 1`, where P is its period.
Number brain updates from 1. The first run is at `max(1, O)`; later runs are P updates apart.
Thus offsets 0 and 1 both give a first-update run. No brain update means no countdown advance.
The offset choice has distribution fidelity. The periods are:

| Sensor type | Period (ticks) |
|-------------|----------------|
| `armadillo_scare_detected` | 5 |
| `secondary_pois` | 40 |
| `golem_detected` | 200 |
| each other sensor type | 20 |

The sensor `hurt_by` has the period 20. Thus a mob with a brain sees its own damage up to 19 ticks
late. A sensor whose query uses follow range must read the mob's current value.
BRAIN-VAN-011 through BRAIN-VAN-020 in `mob-brain-sensors.md` define the individual query ranges.
BRAIN-VAN-010 there states phase recreation and its first-run distribution.

**AIT-VAN-017** (`source_suggested`, semantic, confidence: medium). The brain tick
runs in each eligible AI step. It does these steps:

1. Erase each finite memory whose remaining expiry is already at most zero. Otherwise decrease
   its finite expiry by one. A value that reaches zero remains present until the next update.
2. Tick each sensor in the mob's declared order (AIT-VAN-016).
3. Try to start stopped behaviours of active activities, in ascending numeric priority across
   activities. At equal priority, use the activity and declaration order of BRAIN-VAN-027.
4. Tick or stop behaviours that run, including newly started behaviours and old behaviours of
   inactive activities, in that same order. Complete all starts before this running pass.

An ordinary timed start chooses an integer duration uniformly from inclusive minimum and maximum bounds.
That choice has distribution fidelity. A one-shot or group has no generic duration.
Start checks have a period of one brain update. Most inputs are memories that sensors write with periods of five updates or more.
BRAIN-VAN-004 and BRAIN-VAN-028 through BRAIN-VAN-033 in `mob-brain.md` give exact boundaries.

### 1.5 Items, orbs and despawn

**AIT-VAN-018** (`source_suggested`, semantic, confidence: medium). An item entity tries to merge in
this way:

- Period: when the item moved into another block position in this tick, the period is 2. Else the
  period is 40. The item tries when its tick count is a multiple of the period.
- Candidates: the other items whose boxes intersect its own box. Its own box expands by 0.5 on the
  `x` and `z` axes, and does not expand on the `y` axis.
- Conditions: both items are alive. Neither has the pickup delay 32767 or the age -32768. The age is
  less than 6000. The count is less than the stack size. The owner values are equal, and the two
  stacks can merge with a total that is not more than the stack size.
- Direction: the stack with the smaller count goes into the other stack. On equal counts, the item
  that tries goes into the candidate. The item that stays takes the larger pickup delay and the
  smaller age. The server removes an empty item.
- One item can absorb several candidates in one try, one after the other.

An item also tries to merge after it moves to another dimension. An item despawns at the age 6000.

**AIT-VAN-019** (`source_suggested`, semantic, confidence: medium). An experience orb has these
timers:

- Merge: it tries to merge when its tick count `t` satisfies `t mod 20 = 1`. The candidates are the
  orbs in its box expanded by 0.5, with the same value. Their network ids must differ from its own
  by a multiple of 40.
- Attraction: in each tick, when it follows no player within 8 blocks, it searches a player. It
  takes the nearest player within 8 blocks that is not a spectator and is alive. It then accelerates
  to that player in each tick.
- Despawn: at the age 6000.

**AIT-VAN-020** (`source_suggested`, semantic, confidence: medium). The despawn check of a mob runs
in each tick, before the tick of the mob. It runs for each entity of the tick list that the tick
command does not freeze. It runs also when the chunk of the entity does not tick entities. It does
these steps:

1. On the difficulty `peaceful`, remove a mob of a type that cannot exist on that difficulty.
2. Find the nearest player of the dimension that is not a spectator. Use the distance in 3
   dimensions. If the server finds no player, stop.
3. Let `D` be the despawn distance of the category of the mob. `D` is 64 for the category
   `water_ambient`, and 128 for the other categories. Remove a mob that is not persistent and is
   more than `D` blocks away.
4. Remove a mob that is not persistent when all these conditions are true:
   - the idle time is more than 600;
   - the player is more than 32 blocks away;
   - a random chance of 1 in 800 passes.
5. Else, when the player is less than 32 blocks away, set the idle time to 0.

A mob is persistent when it has `PersistenceRequired:1b`, a leash or a vehicle. A name tag sets
`PersistenceRequired:1b`. Some types never despawn far from players, for example a villager. The
random value comes from the random source of the mob.

The idle time also stops a random stroll: a mob with an idle time of 100 or more does not start a
random stroll. Thus a mob more than 32 blocks from each player stops its random stroll after 100
ticks. This server must keep this vanilla behaviour.

### 1.6 Villagers and pair actions

**AIT-VAN-021** (`source_suggested`, semantic, confidence: medium). A villager acquires a point of
interest in this way:

- First try: 0 to 19 ticks after the first start check. The random value comes from the random
  source of the dimension.
- Later tries: each try sets the next try 20 to 39 ticks later.
- Search: the 5 nearest points of interest of the required type within 48 blocks that have space.
- Path: one path search to the set of the 5 positions.
- Claim: when the path reaches a position, the villager takes the ticket of the point in the same
  tick, and records it in a memory.
- Retry: when the path fails, each of the 5 positions waits before the next try. The wait increases
  by a random 40 to 79 ticks for each failure, up to 400 ticks. The villager forgets a position with
  no try in 400 ticks.

**AIT-VAN-022** (`source_suggested`, semantic, confidence: medium). Gossip and the golem summon have
these timers:

- Gossip: two villagers that trade gossip both need 1200 ticks after their last gossip. The gossip
  copies values from the other villager, with the random source of the villager. Both villagers
  record the time.
- Decay: the gossip of a villager decays one time in 24000 ticks.
- Golem summon: a villager wants a golem when it slept in the last 24000 ticks and it has no memory
  `golem_detected_recently`. After a gossip, it counts the villagers in its box expanded by 10 that
  want a golem, up to 5. With 5, it tries to summon an iron golem. A villager that panics needs 3.
- After the summon, each villager in the box gets the memory `golem_detected_recently` for 599
  ticks. The sensor `golem_detected` also writes it, with the period 200.

**AIT-VAN-023** (`source_suggested`, semantic, confidence: medium). An animal breeds in this way:

- Start: in a full evaluation, an animal in love searches the nearest free partner in its box
  expanded by 8. The partner must be able to mate and must not panic.
- Approach: in each full evaluation, it moves to the partner and adds 1 to a counter.
- Breed: when the counter reaches 30 evaluation units, about 60 ticks, and the partner is less than
  3 blocks away, the animal breeds. The breed action makes a child, and ends the love of both
  animals.

Two animals can select different partners. The first animal that reaches the end of its counter
breeds with its own partner. The breed action ends the love of that partner too.

**AIT-VAN-024** (`source_suggested`, semantic, confidence: medium). These pair actions also exist:

- Alert: some types, for example a wolf and a zombie, alert other mobs. When the revenge goal of
  such a mob starts, the mob sets the attack target of other mobs of the same type. The box is the
  follow range horizontally and 10 blocks vertically. Only a mob with no attack target changes, in
  the same tick.
- Fish school: a fish with no leader searches a leader one time in 200 to 219 ticks, in a box
  expanded by 8. The leader adds the fish with no leader as followers, up to its school size. A
  follower searches a path to its leader each 10 ticks.
- Raid: the dimension ticks each raid before the entities (step 3 of PHY-ORD-010). The raid adds
  raiders, starts waves and changes the state of its raiders.

### 1.7 The slack of each part

**AIT-VAN-025** (`source_suggested`, semantic, confidence: medium). This table collects the cadence
of each part. "Phase" names the value that selects the ticks of the part.

| Part | Period | Phase or random part | Rule |
|------|--------|----------------------|------|
| goal selection, start and continue | 2 | `(t + n)` even, or `t ≤ 1` | AIT-VAN-002 |
| goals that tick only in full evaluations | 2 | as the goal selection | AIT-VAN-003 |
| control flags of a ridden mob | 5 | `t mod 5 = 0` | AIT-VAN-026 |
| target selection, start | 2 | chance 1 in 5 for each evaluation | AIT-VAN-011 |
| target selection, keep | 2 | 30 evaluation units unseen | AIT-VAN-011 |
| melee attack, start | 20 | from the last start check | AIT-VAN-006 |
| melee attack, path | 4 to 10, plus 5, 10 or 15 | random | AIT-VAN-006 |
| melee attack, hit | 1 | cooldown of 20 | AIT-VAN-006 |
| creeper swell, start | 2 | as the goal selection | AIT-VAN-007 |
| creeper swell, direction | 1 | none | AIT-VAN-007 |
| door, goal form | 2 for the start, 1 for the close | as the goal selection | AIT-VAN-008 |
| door, brain form | 1, with a wait of up to 19 | node change | AIT-VAN-008 |
| path follower and controls | 1 | none | AIT-VAN-012, AIT-VAN-013 |
| path recompute after a block change | at most 1 in 20 ticks | none | AIT-VAN-015 |
| path recompute of a brain | 1 | walk target moved more than 2 blocks | AIT-VAN-015 |
| sensors | 5, 20, 40 or 200 | random start from 0 to `P - 1` | AIT-VAN-016 |
| brain start checks | 1 | inputs from sensors | AIT-VAN-017 |
| item merge | 2 when it moved, else 40 | `t mod P = 0` | AIT-VAN-018 |
| orb merge | 20 | `t mod 20 = 1` | AIT-VAN-019 |
| orb attraction | 1 | none | AIT-VAN-019 |
| despawn | 1 | chance 1 in 800 | AIT-VAN-020 |
| point of interest | 20 to 39 | random | AIT-VAN-021 |
| fish school leader | 200 to 219 | random | AIT-VAN-024 |
| breed action | 2 | counter of 30 evaluation units | AIT-VAN-023 |

**AIT-VAN-026** (`source_suggested`, semantic, confidence: medium). When the tick count of a mob is
a multiple of 5, the mob updates its control flags. It disables `move` and `look` when a mob
passenger controls it. It disables `jump` in that case and when it rides a boat.

## 2. The split

### 2.1 Reflexes

**AIT-SPL-001** (`design`, semantic, confidence: medium). Each part of the AI is a reflex or a
decision. A part is a reflex when one of these is true:

- a delay of one tick gives an artefact that a player can see, for example a hit, an explosion, a
  door or a motion;
- the part runs in each tick in the vanilla server, and it changes the motion or the health of an
  entity directly.

Each other part is a decision. A part that a later specification adds must use this rule.

**AIT-SPL-002** (`design`, semantic, confidence: medium). A reflex runs in the hot tick, in the tick
in which the vanilla server runs it. It reads these values:

- the state of its own entity at the start of the tick;
- the guidance state of its own entity;
- the state of other entities at the start of the tick (PHY-CMP-008);
- the blocks of the copy of PHY-CMP-009.

A reflex writes only its own entity, except through the writes of PHY-CMP-015 and the claims of
PHY-CMP-017.

**AIT-SPL-003** (`design`, semantic, confidence: medium). A reflex must be cheap. In one tick, a
reflex does a fixed amount of work for its entity. It casts at most one ray for line of sight to its
attack target, with a length of at most the follow range. A reflex does no path search, no search of
entities in a box larger than 8 blocks, and no search of blocks. The cache of AIT-VAN-004 applies to
the rays of one tick.

**AIT-RFX-001** (`design`, semantic, confidence: medium). The guidance is a reflex: the path
follower (AIT-VAN-012), the move control, the look control and the jump control (AIT-VAN-013), and
the body rotation. It moves the mob along its current path and turns it to its current look target.
The reason: a delay moves the mob in a wrong direction, or makes it stop, in each tick of the delay.
The client sees the position in each update.

**AIT-RFX-002** (`design`, semantic, confidence: medium). The creeper swell is a reflex
(AIT-VAN-007). The reflex contains the start condition and the continue condition in full
evaluations, the direction in each tick, the counter and the explosion. The attack target comes from
a decision. The reason: a late direction lets the creeper explode after the player moved more than 7
blocks away, or out of the line of sight. A late start lets the player stand next to the creeper for
longer with no swell.

A creeper whose counter reaches `Fuse` in this tick explodes in this tick. The hot tick knows this
at the start of the tick: the counter plus the direction is `Fuse` or more. Thus the creeper is a
seed of an explosion group (PHY-CMP-020, step 1). A primed TNT with a fuse of 1 or less is a seed in
the same way.

When the swell starts, it takes the flag `move`, and the hot tick stops the current path. The AI
stage reads the state of the swell from the replica of its next snapshot. It must not start a goal
that takes `move` while the swell runs.

**AIT-RFX-003** (`design`, semantic, confidence: medium). The melee attack is a reflex: the
cooldown, the reach test, the line of sight test and the hit (AIT-VAN-006). The look at the attack
target in each tick is a part of this reflex. The start check of the goal and its path requests are
decisions. The reason: players notice a late hit that lands when the target is no longer in reach.
They also notice a late cooldown, because it changes the rate of the hits.

The ranged attacks of AIT-VAN-005 are reflexes in the same way: the time that the mob sees the
target, the draw time and the release. The reason: a late release aims at an old position.

**AIT-RFX-004** (`design`, semantic, confidence: medium). The door on a path is a reflex
(AIT-VAN-008). The reflex contains the start and the close of the goal form, and the progress of the
break form. It also contains the check of the brain form with its cooldown. The reason: a late open
makes the mob walk into a closed door and stop. A late close leaves the door open after the mob
passed.

The open and the close are block writes. They follow the rules of block writes in `cells.md`. The
mob itself sees the open door at the next tick (PHY-DEV-003), as for each block change.

**AIT-RFX-005** (`design`, semantic, confidence: medium). The escape from the block that the mob is
in is a reflex. It contains the float in a fluid and the climb out of powder snow (AIT-VAN-009),
with their start conditions. The reason: they set the jump input in each tick, and a late jump makes
the mob sink. The search of a panic position is a decision (AIT-DEC-001).

**AIT-RFX-006** (`design`, semantic, confidence: medium). The effects of damage on the mob that
takes it are a reflex. They are the health, the hurt time, the knockback and the death. They also
are the reset of the idle time, and the time and the source of the last damage. These writes are not
claims, so they apply at the merge point (PHY-CMP-015). The removal of a mob after its death is not
a claim: the pass removes the row when its death time reaches 20 (DMG-DTH-003). The hits of the own
step and fall actions apply in the tick of the mob, and the crowd hit in the crowd fix-up after the
main pass (DMG-ORD-002, DMG-CRW-002).

Then the hot tick sends a fact to the brain. The reaction is a decision: the revenge target, the
panic and the alert. The reason: the health and the knockback are visible in the next update. The
vanilla reaction already has a delay of 0 or 1 tick for goals, and up to 19 ticks for brains
(AIT-VAN-010).

**AIT-RFX-007** (`design`, semantic, confidence: medium). The attraction of an experience orb is a
reflex (AIT-VAN-019). It reads the player table (AIT-CDN-002) for the nearest player within 8
blocks. The reason: the orb moves in each tick, and the client simulates its motion.

**AIT-RFX-008** (`design`, semantic, confidence: medium). The due events of the AI are reflexes. One
due event is the end of a memory expiry that a reflex reads, for example the attack cooldown of a
brain. Another is the end of a timer that a reflex starts. A wheel of timers gives them
(`physics-architecture.md`). The reason: a reflex must see the same value of the timer as in the
vanilla server.

### 2.2 Decisions

**AIT-DEC-001** (`design`, semantic, confidence: medium). These parts are decisions. The AI stage
computes them.

- the goal selection, except the start and the continue of the reflexes of section 2.1;
- the tick of each goal that ticks only in full evaluations, for example a random stroll, a look at
  a player or a breed approach;
- the target selection, with its line of sight checks and its reach checks;
- the control flags of a ridden mob (AIT-VAN-026);
- each sensor scan;
- the start checks and the ticks of the behaviours of a brain, except the reflexes of section 2.1;
- each path search and each path recompute;
- the search of a panic position, and the search of a random position;
- the item merge search and the orb merge search;
- the despawn check;
- the acquisition of a point of interest;
- the selection of a partner for a pair action (section 4).

**AIT-DEC-002** (`design`, semantic, confidence: medium). Each decision has an allowed delay. The
default is the vanilla period of its part (AIT-VAN-025). A delay up to the period keeps the result
inside the jitter that the vanilla server already has. For example, the vanilla start of a goal
occurs 0 or 1 tick after its condition, and a sensor starts at a random phase. Thus a late result
moves by at most one period of its part.

**AIT-DEC-003** (`design`, semantic, confidence: medium). The allowed delay of each decision is in
this table.

| Decision | Allowed delay (ticks) | Reason |
|----------|-----------------------|--------|
| goal selection, and the ticks of goals in full evaluations | 2 | the period |
| target selection | 2 | the period. The vanilla wait is about 10 ticks |
| melee attack start | 2 | the start check of AIT-VAN-006 already waits up to 19 ticks |
| control flags of a ridden mob | 5 | the period |
| sensor scan | its period | the start of a sensor is random in the vanilla server |
| brain start checks and brain ticks | 2 | see AIT-DEC-004 |
| path search and path recompute | 2 | see AIT-DEC-005 |
| panic position, random position | 2 | the start of the goal |
| item merge search | 2 when the item moved, else 40 | the period |
| orb merge search | 20 | the period |
| despawn check | no visible limit | see AIT-DEC-006 |
| point of interest | 20 | the shortest period of the tries |
| pair actions | 2 | the period of the goal or the behaviour that starts them |

**AIT-DEC-004** (`design`, semantic, confidence: low). A brain start check has the period 1 in the
vanilla server (AIT-VAN-017). Its allowed delay is 2 for this reason. A start check changes its
result only when an input changes. The inputs are memories from sensors, with a period of 5 ticks or
more and a random phase. The other inputs are the results of other decisions.

A delay of 2 ticks moves the phase of a sensor, and the vanilla phase is already random. The inputs
that change in each tick, for example a reach or a door on the path, are reflexes (section 2.1). U3
lists the behaviours that need a review.

**AIT-DEC-005** (`design`, semantic, confidence: medium). A path search has an allowed delay of 2.
While a new path waits, the guidance follows the current path (AIT-RFX-001). A vanilla recompute
after a block change already waits for up to 20 ticks (AIT-VAN-015). A melee path waits 4 to 10
ticks. Thus a delay of 2 ticks is inside the vanilla jitter for each trigger.

**AIT-DEC-006** (`design`, semantic, confidence: medium). The despawn check has the period 1, but a
delay is not visible. The random despawn needs each player more than 32 blocks away. The instant
despawn needs each player more than 64 or 128 blocks away. The validity check of AIT-LAG-005 drops a
despawn when a player came near, or the mob became persistent. Thus the despawn check gives no lower
limit to `max_lag`.

**AIT-DEC-007** (`design`, semantic, confidence: medium). A decision writes only the brain state and
the guidance state of its own entity. A change of another entity, or of shared world state, is a
proposal (section 4). A change of a block is a block write at its application, and follows the rules
of block writes.

## 3. Rules for a delayed decision

**AIT-LAG-001** (`design`, semantic, confidence: medium). A decision of issue tick `T` reads this
snapshot:

- its own brain state, after all decisions of the same entity with an earlier issue tick;
- the bodies and the guidance states at the start of tick `T`;
- the blocks of the copy of PHY-CMP-009 for tick `T`;
- the player table of tick `T`;
- the facts of the ticks before `T`.

The snapshot does not change while the AI stage computes the decision. A decision is a function of
its snapshot and of its random values (AIT-LAG-008). It reads nothing else, for example no wall time
and no state of the AI stage.

**AIT-LAG-002** (`design`, semantic, confidence: medium). A decision of issue tick `T` applies in
tick `T + k`, with `1 ≤ k ≤ max_lag`. It applies before the hot step of its entity, after the
changes that the merge point of that tick makes to the entity. The vanilla server uses the decision
in the tick `T` itself, so the extra delay is `k` ticks.

A decision writes only the guidance state of its own entity (AIT-DEC-007). Thus an implementation
can apply it at the start of the tile task of its entity. The result is the same as an application
at the merge point. A tick that applies all decisions at the merge point is one valid
implementation, not a requirement.

A proposal is not such a write. Each proposal goes to the merge point of tick `T + k`, and
AIT-CPL-002 checks it there in one global order. A grant that makes a claim enters the arbitration
of PHY-CMP-017 in the same tick `T + k`.

**AIT-LAG-003** (`design`, semantic, confidence: medium). The default of `max_lag` is 2. The value
comes from the smallest allowed delay in the table of AIT-DEC-003, which is 2 ticks. The goal
selection, the target selection, the paths and the brain start checks have that limit. The despawn
check has no limit (AIT-DEC-006).

The default must not be more than the smallest allowed delay. An operator can set a larger value.
Then each decision with an allowed delay less than `max_lag` is a deviation (AIT-DEV-001). The
server reports the value at start.

**AIT-LAG-004** (`design`, semantic, confidence: medium). The AI stage has two delay modes:

- `AiDelay::AtLeast(1)`, the default: a decision of issue tick `T` applies in the first tick
  `T' ≥ T + 1` that starts after the decision is ready. When `T' = T + max_lag` and the decision is
  not ready, the hot tick waits for it before it applies it.
- `AiDelay::Fixed(k)`, with `1 ≤ k ≤ max_lag`: each decision applies at `T + k`. In tick `T'`, the
  hot tick waits until each decision of issue tick `T' - k` is ready. The tests, the replay tool and
  the determinism checks use this mode.

In both modes, the tick never applies a decision later than `T + max_lag`. A late AI stage makes the
tick slower. It does not make a decision older. The server reports the time that the hot tick waits
as a metric.

**AIT-LAG-005** (`design`, semantic, confidence: medium). The hot tick checks each decision again at
its application, against the current state. The decision is stale when one of these is true:

- its entity no longer exists, or changed dimension;
- the attack target of the decision is not alive, or not in the dimension;
- the mob cannot attack the attack target of the decision, or that target is outside the follow
  range;
- the decision has a path, and the mob teleported after the issue tick;
- a path of the decision starts more than `max_lag × s + 1` blocks from the position of the mob. `s`
  is the largest speed of the mob in one tick;
- a path of the decision goes to an entity that is not alive or not in the dimension;
- a reflex of the entity took a control flag that the decision needs, for example the creeper swell
  took `move`;
- the condition of a removal changed, for example a player came near a mob that the despawn check
  removes (AIT-VAN-020);
- a proposal fails its check (AIT-CPL-002).

A block change on the path does not make the path stale. AIT-VAN-015 already triggers a recompute
for it.

**AIT-LAG-006** (`design`, semantic, confidence: medium). The hot tick does these steps for a stale
decision:

1. Drop the whole decision. Do not apply a part of it.
2. Keep the current guidance state. The mob follows its current path, or stays when it has none.
3. Send a fact to the brain of the mob. The fact names the decision and the reason.

The hot tick does not compute a new decision in its place. The part runs again at its next vanilla
slot (section 5). The fact lets that run see the reason.

**AIT-LAG-007** (`design`, semantic, confidence: medium). In one tick, the decisions apply in the
order of the pair `(issue tick, UUID)`, from the lowest. The UUID compares as PHY-CMP-013
states. Each decision writes only its own entity, so only the order inside one entity can change the
result. The proposals at the merge point use the same order (AIT-CPL-002).

The decisions of one entity and one issue tick apply in the order of the steps of AIT-VAN-001. A
decision of one entity never applies before a decision of the same entity with an earlier issue
tick.

**AIT-LAG-008** (`design`, semantic, confidence: medium). A decision takes its random values from a
stream. The stream is a function of the world seed, the UUID of the entity (PHY-CMP-013), the
issue tick and the part. The `i`-th value of one decision is the `i`-th value of its stream. Thus
the value does not depend on the order of the entities, on the thread count or on other uses of a
random value. AIT-DEV-002 records the difference from the vanilla server.

A reflex takes its random values from a stream of the same form. The part names the reflex, for
example the chance of `0.8F` of AIT-VAN-009.

**AIT-LAG-009** (`design`, semantic, confidence: medium). A hold time that a periodic decision sets
again, for example the look target of 2 ticks (AIT-VAN-013), ends at the first of these events:

- the next decision of the same part applies;
- `P + max_lag - 1` ticks pass after the application, where `P` is the period of the part.

The delay of each decision can change by up to `max_lag - 1` ticks. Thus the interval between two
applications can be up to `P + max_lag - 1` ticks. This rule keeps the head of the mob on its target
in that interval. A decision that does not set the hold again ends it at the vanilla time after its
own application.

**AIT-LAG-010** (`design`, semantic, confidence: medium). A timer that a decision starts counts from
the application, not from the issue tick. Examples are the duration of a behaviour and the counter
of a breed approach. Thus a late decision does not make a timer shorter.

**AIT-LAG-011** (`design`, semantic, confidence: medium). In the exact-parity mode (PHY-ORD-010),
the AI stage runs inline, with `k = 0`. Each decision of tick `T` applies in the AI step of its
entity in tick `T`, and reads the live state. The rules AIT-LAG-001 to AIT-LAG-010 do not apply in
that mode.

## 4. Pair actions

**AIT-CPL-001** (`design`, semantic, confidence: medium). A decision that wants to change another
entity, or shared world state, gives a proposal. A proposal has these fields:

- the kind of the pair action;
- the proposer, and each other entity or world object that the action changes;
- the issue tick;
- the values of the snapshot that the action needs, for example the gossip of the other villager.

A proposal changes nothing in the AI stage. Only a grant in the hot tick changes the world.

**AIT-CPL-002** (`design`, semantic, confidence: medium). The hot tick checks the proposals at the
merge point, in the order of AIT-LAG-007. For each proposal, it does these steps:

1. Check the conditions of the action against the current state. The current state contains the
   grants before this proposal at the same merge point.
2. If a condition fails, drop the proposal as a stale decision (AIT-LAG-006). Send a fact to the
   proposer.
3. Else, grant it. Apply all changes of the action in one step, to each entity and object that it
   names.

Thus each pair action is atomic: each entity sees all of its changes in the same hot tick, or none.
The result does not depend on the thread count or on the division into cells.

**AIT-CPL-003** (`design`, semantic, confidence: medium). An entity takes part in at most one
granted pair action of one kind at one merge point, unless the kind permits more. The item merge
permits more (AIT-CPL-005).

**AIT-CPL-004** (`design`, semantic, confidence: medium). The pair actions of the vanilla server
have these checks:

| Pair action | Check at the grant | Changes |
|-------------|--------------------|---------|
| breed (AIT-VAN-023) | both animals alive, in love, not in panic; the partner less than 3 blocks away | a child with a new UUID of the source kind `entity` (PHY-CMP-013); the love of both ends |
| alert (AIT-VAN-024) | each receiver alive, of the same type, with no attack target now | the attack target of each receiver that passes; a fact to its brain |
| gossip (AIT-VAN-022) | both villagers alive; 1200 ticks after the last gossip of each | the gossip of the proposer from the snapshot of the other; the time of both |
| golem summon (AIT-VAN-022) | the count of villagers that want a golem, now, is still enough | the iron golem; the memory `golem_detected_recently` for each villager in the box |
| point of interest claim (AIT-VAN-021) | the point still exists and has space | the ticket; the memory of the proposer |
| fish school (AIT-VAN-024) | the leader alive and with space; each follower with no leader now | the leader and the school size |
| raid membership (AIT-VAN-024) | the raid is active and the raider can join | the raid and the raider |
| item merge (AIT-VAN-018) | AIT-CPL-005 | AIT-CPL-005 |

The alert and the golem summon each read a group of entities. The grant counts the group again from
the current state. The first grant can change the count for a later proposal, for example the memory
`golem_detected_recently` stops a second summon at the same merge point.

**AIT-CPL-005** (`design`, semantic, confidence: medium). An item merge proposal names the item that
tries and one candidate. The hot tick grants it when these conditions are true now:

- both items exist, and the conditions of AIT-VAN-018 hold;
- the boxes still intersect, with the expansion of AIT-VAN-018.

The direction of AIT-VAN-018 uses the counts now. On equal counts, the item that tries goes into the
candidate. An item can absorb several candidates at one merge point, in the order of the proposals.
A removed item makes each later proposal with it stale.

The grant of a merge enters the arbitration of PHY-CMP-017 in the same tick, as a claim on each of
the two items. The arbitration decides between it and the other claims on the same item, for example
a pickup or a transfer into a hopper. PHY-CMP-015 does not apply to a merge. The writer key is the
UUID of the item that tries. A despawn is a claim of the removal in the same way.

**AIT-CPL-006** (`design`, semantic, confidence: medium). A raid itself is not a part of the AI. In
the reference, the dimension ticks its raids before its entities in the same tick (step 3 of
PHY-ORD-010). This server keeps that place. In each tick, the raid tick has this position:

1. after the merge point of the tick;
2. before the entity pass of the tick, thus before each tile task;
3. after the block steps that PHY-ORD-010 puts before the raids.

The raid tick reads the entities at their state after the merge point. The raid changes its raiders
directly, before the entity pass, so each raider uses the change in the same tick. A change that is
a claim enters the arbitration of PHY-CMP-017. A raider that the raid adds follows the rules of
PHY-ORD-011 for a new entity. The decisions of a raider, for example its path to the centre of the
raid, follow section 3.

## 5. The cadence

**AIT-CDN-001** (`design`, semantic, confidence: medium). By default, the AI stage issues each part
at its vanilla period (AIT-VAN-025). The hot tick issues a decision of a part for an entity in each
tick in which the vanilla server runs that part. A part whose vanilla phase reads the network id
reads the phase offset of PHY-CMP-016 in its place (AIT-DEV-003). The issue never depends on the
backlog of the AI stage, on wall time or on the load of the server.

The goal selection of a mob is an exception. The AI stage can skip a full evaluation that
`mob-goals.md`, GOAL-WAKE-002, states to be quiet. A quiet evaluation gives no change, so the skip
is not visible.

A reduced rate for far mobs is visible in mob farms and in villages. Other servers removed it for
that reason. Thus this server keeps the vanilla periods by default.

**AIT-CDN-002** (`design`, semantic, confidence: medium). Each cell keeps the player table of each
dimension. The table holds the position of each player at the end of the previous tick, with the
flags spectator and alive. `mob-goals.md`, GOAL-QRY-005, adds the values that the goals read. The coordinator sends each change to each cell at the sync point. Thus
the table is the same in each cell and in each division of the world. The nearest player of the
despawn check, of the idle time reset and of the orb attraction comes from this table.

**AIT-CDN-003** (`design`, semantic, confidence: medium). A server can set reduced cadences for far
mobs, as a recorded deviation (AIT-DEV-006). This is off by default. When it is on, these rules
apply:

- The cadence class of a mob at an issue tick comes only from the distance to the nearest player in
  the player table. It must not depend on the cell, the halo, the thread count or the load.
- The class multiplies the period of each decision by a factor of the class. It does not change a
  reflex.
- The class changes only at a full evaluation of the mob.
- A class never applies to a mob within 32 blocks of a player.

**AIT-CDN-004** (`design`, semantic, confidence: medium). The vanilla server has its own distance
rules: the despawn distances and the idle time (AIT-VAN-020), and the chunks that tick entities.
These rules are not cadence classes. This server keeps them in each mode.

## 6. Invariants

**AIT-INV-001** (`design`, semantic, confidence: medium). The content of each decision does not
depend on the thread count, the division into cells, the load of the AI stage or wall time. In the
mode `AiDelay::AtLeast(1)`, only the tick of the application can change, and it stays in the range
of AIT-LAG-002.

**AIT-INV-002** (`design`, semantic, confidence: medium). With `AiDelay::Fixed(k)`, two runs from
the same world are equal tick by tick. A run with one thread and a run with many threads are also
equal. A run with one cell and a run with many cells are also equal. The exceptions of PHY-INV-014
stay.

**AIT-INV-003** (`design`, semantic, confidence: medium). A reflex is never late. It acts in the
tick in which the vanilla server acts, from the state of AIT-SPL-002.

**AIT-INV-004** (`design`, semantic, confidence: medium). A decision is never older than `max_lag`
at its application. Before the hot step of tick `T`, the hot tick applied or dropped each decision
with an issue tick of `T - max_lag` or earlier.

**AIT-INV-005** (`design`, semantic, confidence: medium). A decision writes only its own entity.
Each change of another entity or of shared state comes from a grant (AIT-CPL-002).

**AIT-INV-006** (`design`, semantic, confidence: medium). The cost of the hot tick for one mob does
not depend on the AI of the mob. It is the cost of the reflexes (AIT-SPL-003), of the issue of its
jobs and of the application of its decisions.

## 7. Configuration

**AIT-CFG-001** (`design`, semantic, confidence: high). Each value in this table is configurable.
The default applies when the operator sets no value.

| Value | Default | Rule |
|-------|---------|------|
| `max_lag` | 2 ticks | AIT-LAG-003 |
| `ai_delay` | `AiDelay::AtLeast(1)` | AIT-LAG-004 |
| `far_cadence` | off | AIT-CDN-003 |
| `far_cadence_classes` | none | AIT-CDN-003 |

## 8. Intentional deviations

These entries record where this server differs from a vanilla server. AIT-DEV-001 to AIT-DEV-005
apply in each mode except the exact-parity mode. AIT-DEV-006 applies only when the operator sets it.

### AIT-DEV-001: Decisions apply 1 to `max_lag` ticks late

- **Rule:** AIT-LAG-002.
- **Reference:** the vanilla server applies each result of the AI in the tick of the part.
- **Visible to the client:** no, within the allowed delays. Examples:
  - Creeper swell: no difference. The swell is a reflex (AIT-RFX-002). Only the attack target can be
    late, by up to 2 ticks. The vanilla target wait is already about 10 ticks.
  - Zombie target: the zombie raises its arms and walks to the player 1 or 2 ticks later. The
    vanilla wait is a geometric distribution with a mean of about 10 ticks, and the melee start
    waits up to 19 ticks more. The mean increases by 1 to 2 ticks.
  - Villager at a door: no difference for the door. The door check is a reflex (AIT-RFX-004), with
    the vanilla cooldown. A new path can arrive 1 or 2 ticks later.
  - Item merge in a stream of items: an item that falls from a farm merges 1 or 2 ticks later. Some
    items reach a hopper or a player before the merge. Then the proposal is stale, and the hopper
    takes two small stacks in place of one. The total count of items stays the same.
  - When an operator sets `max_lag` above 2, the goal start, the target and the path can be late by
    more than one evaluation unit.
- **Test:** a differential test compares the distribution of the times of each decision, not each
  time. For a fixed scenario, it accepts a shift of 1 to `max_lag` ticks for each decision.

### AIT-DEV-002: Keyed random values for the AI

- **Rule:** AIT-LAG-008, with the UUID of PHY-CMP-013. The stream of the AI stays separate from
  the stream of PHY-CMP-014.
- **Reference:** the goals take random values from the random source of the mob.
  Section 9 of [mob-brain.md](mob-brain.md#9-parallelism-notes) gives the reference random sources for brain choices.
- **Visible to the client:** no, as a distribution. Each chance and each range stays the same.
- **Test:** a differential test compares distributions over many runs. It does not compare one
  sequence of random values.

### AIT-DEV-003: The phase of a part from the UUID

- **Rule:** AIT-CDN-001, PHY-CMP-016 and PHY-CMP-013.
- **Reference:** the phase of the goal selection uses the network id (AIT-VAN-002). The merge
  classes of the experience orbs use the network id modulo 40 (AIT-VAN-019).
- **This server:** a full evaluation occurs when `(t + o)` is even, or when `t ≤ 1`. `o` is the
  phase offset of PHY-CMP-016. This is the rule of PHY-CMP-016: the phase offset replaces the
  network id in the choice of a tick. The merge class of an orb is not the choice of a tick, so
  PHY-CMP-016 does not give it. The merge class is a fixed function of the UUID, from 0 to 39.
  Two orbs can merge only when their merge classes are equal.
- **Visible to the client:** no. The distribution of the phases and of the merge classes stays the
  same. An implementation can give network ids in a way that depends on the layout, so this server
  does not read them (PHY-CMP-013).
- **Test:** a differential test does not compare the phase of one mob. It compares the share of mobs
  on each phase. A vanilla parity test sets the phase offset to the remainder of the vanilla network
  id divided by 4, as PHY-CMP-016 states. Then the phase of the goal selection is equal to the
  vanilla phase.

### AIT-DEV-004: Decisions read a snapshot

- **Rule:** AIT-LAG-001.
- **Reference:** the AI of a mob reads the live world in its own tick. It sees the entities that
  ticked before it in the same tick at their new positions. It sees each block change before its own
  tick.
- **Visible to the client:** no, as a separate effect. The difference is one tick of motion of the
  other entities. AIT-DEV-001 already contains it.
- **Test:** as AIT-DEV-001.

### AIT-DEV-005: Pair actions in the order of the merge point

- **Rule:** AIT-CPL-002 and AIT-CPL-004.
- **Reference:** the mob that ticks first does the pair action in its own tick. Examples: the first
  animal breeds, the first villager claims the point of interest, and the alert changes the other
  mobs in the same tick.
- **Visible to the client:** yes, rarely. When two mobs want the same partner or the same point of
  interest in one tick, a different mob can win. An alert reaches the pack 1 or 2 ticks later. The
  total of children, golems and claims stays the same.
- **Test:** a differential test accepts each winner of a conflict. It accepts a delay of 1 to
  `max_lag` ticks for each pair action.

### AIT-DEV-006: Reduced cadence for far mobs

- **Rule:** AIT-CDN-003.
- **Reference:** the vanilla server has no cadence class.
- **Visible to the client:** yes. A far mob reacts later, and a farm far from players can change its
  rate. This is the reason why the default is off.
- **Test:** a differential test runs with this deviation off.

## 9. Unresolved

- **U1**: closed. PHY-CMP-013 defines the order key (the UUID), and PHY-CMP-016 defines the phase
  offset.
- **U2**: nobody measured the throughput of the AI stage with `max_lag` 2. A path search costs 50 to
  500 microseconds. The AI stage can split one path search over several stage ticks. The search
  reads a versioned snapshot of its issue tick. The stage keeps the old versions of the changed
  sections for `max_lag + 1` ticks. Thus the split search obeys AIT-LAG-001. The risk that stays: a
  burst of path searches can make the hot tick wait. A test must measure the share of the ticks that
  wait.
- **U3**: AIT-DEC-004 assumes that each input of a brain start check changes only at a sensor scan
  or at a decision. A review must check each behaviour. A behaviour that reads the position of the
  mob or of its target in each tick can need to become a reflex.
- **U4**: the list of reflexes of section 2.1 is not complete for each mob type. Examples to
  classify are the sonic boom of the warden, the fangs of the evoker and the beam of the guardian.
  Others are the fireball of the ghast and the bullet of the shulker. AIT-SPL-001 gives the rule.
- **U5**: the brain state lives in the AI stage. The save must read it, and it must move with an
  entity to another cell. No rule states this yet.
- **U6**: closed. The design adds the mode `AiDelay::Inline` with `k = 0` for the exact-parity mode,
  as AIT-LAG-011 requires.
- **U7**: the path search of the AI stage reads the blocks of the copy of PHY-CMP-009. The vanilla
  path search reads the live world. A block change in the same tick can give a different path.
- **U8**: the stale check of a path in AIT-LAG-005 uses a distance limit. The value comes from no
  experiment.

## 10. Experiments

### 10.1 Experiments with a vanilla server

Start a vanilla server with `tools/vanilla-server/run.sh serve`. Use the game version in the table
at the top of this document. Set `online-mode=false` and `level-type=minecraft:flat`. Set the game
rule `spawn_mobs` to false. Connect one scripted client as an operator, and record the clientbound
packets.

Use `/tick freeze` before each step, and `/tick step 1` to run one tick. The attack target and the
memories are not in the saved data. Read them from the packets that the server sends. Use the flag
"aggressive" of a mob, the swell direction of a creeper, and the block updates of a door. Repeat
each experiment that has a random part at least 200 times, and record the distribution.

| Experiment | Steps | Rules |
|------------|-------|-------|
| E1 | Summon a creeper and put the player 2.5 blocks away. Step until the swell direction changes. Then move the player to 7.5 blocks, and step 1 tick at a time. Record the tick of each change of the direction and of the explosion. Repeat with a block between the creeper and the player. | AIT-VAN-007, AIT-RFX-002 |
| E2 | Summon a zombie and put the player 10 blocks away in survival mode. Record the ticks from the summon to the flag "aggressive". | AIT-VAN-011, AIT-VAN-006 |
| E3 | Let a zombie attack an armor stand in survival mode. Record the ticks of each hit, and move the armor stand out of reach between two hits. | AIT-VAN-006, AIT-RFX-003 |
| E4 | Build a house with a wooden door. Give a villager a bed behind the door. Record the ticks of the door updates and the positions of the villager. Repeat with a zombie and the difficulty `hard`. | AIT-VAN-008, AIT-RFX-004 |
| E5 | Summon two items of the same kind at rest 0.3 blocks apart. Record the tick of the merge. Repeat with items that fall through 5 blocks. | AIT-VAN-018 |
| E6 | Summon one item each tick at the same point for 100 ticks, over a hopper and over stone. Record the count of item entities in each tick, and the stacks in the hopper. | AIT-VAN-018, AIT-CPL-005, AIT-DEV-001 |
| E7 | Summon experience orbs of the same value next to each other. Record the ticks of the merges. Put the player 7 blocks away and record the motion of the orbs. | AIT-VAN-019 |
| E8 | Put a zombie 130 blocks from the player and step 1 tick. Put a zombie 40 blocks from the player, step 600 ticks, then step until it despawns. Record the ticks. | AIT-VAN-020 |
| E9 | Put a zombie 40 blocks from the player and record its positions for 400 ticks. | AIT-VAN-020 |
| E10 | Put a villager with no job next to a lectern. Record the ticks to the claim. Put two villagers at the same distance from one lectern. | AIT-VAN-021, AIT-DEV-005 |
| E11 | Hit one villager with no damage source from a player, then put a zombie near a villager. Record the ticks from the damage to the start of the panic. | AIT-VAN-010, AIT-VAN-016 |
| E12 | Put a path for a zombie to the player. Place and break a block on the path in each tick. Record the ticks of the path changes with a debug view. | AIT-VAN-015 |
| E13 | Feed two cows, and feed three cows next to each other. Record the ticks to the child, and which cows end their love. | AIT-VAN-023, AIT-DEV-005 |
| E14 | Put five tamed wolves of one player near a zombie. Let the zombie hit one wolf. Record the tick in which each wolf gets the flag "aggressive". | AIT-VAN-024, AIT-CPL-004 |
| E15 | Put ten cod in water. Record the leader of each cod for 600 ticks. | AIT-VAN-024 |
| E16 | Put five villagers that slept near a bell, with no iron golem. Record the tick of each golem summon. | AIT-VAN-022 |
| E17 | Use commands only, with no player or scripted client. Set `pause-when-empty-seconds=-1`, force-load the area, disable natural mob spawning, and freeze ticks. Clear the air around an ordinary ice floor from `(0, 63, 0)` through `(11, 63, 0)`, one block wide, above the flat world surface. Keep block light low. Put a cobweb at `(0, 64, 0)`. Summon a persistent husk with `NoAI:1b` at `(11.5, 64, 0.5)`; do not make it invulnerable. Summon a persistent snow golem at `(0.5, 64, 0.5)` with `OnGround:1b`, `Invulnerable:1b` and `Rotation:[-90.0f,0.0f]`. Run `/tick step 1` and record full-precision `Pos` of both mobs and the golem's `OnGround` after each tick. Record the first snowball with `data get entity`. Set `t0` to the first tick of eastward pursuit, not the summon tick. Keep the acceptance checks through `t0 + 110`: straight, grounded pursuit from near the initial block centre, with the target fixed, visible and more than 10 blocks away. Reject an ambiguous or ungrounded start, a block change, or displacement of at least `1.5625` blocks through `t0 + 101`. For the calibrated comparison, accept only a first snowball at `t0 + 20`; record other offsets as rejected runs. Record horizontal position on every tick through `t0 + 110`, and any later pauses and exit from the cobweb if continuing to `t0 + 320`. Repeat from a rebuilt scene. **Expected result:** `Pos[0]` at `t0 + 101` equals its value at `t0 + 100`, and increases again at `t0 + 102`. The contrasting prediction is an increase of about `0.003586` blocks at `t0 + 101`. | AIT-VAN-012, PATH-FOL-003 |

### 10.2 Tests for this server

| Test | Steps | Rules |
|------|-------|-------|
| C1 | Run a world with mobs for 2000 ticks with `AiDelay::Fixed(1)`, two times. Compare the world hash in each tick. | AIT-INV-002 |
| C2 | Repeat C1 with 1, 2 and 8 threads, and with 1 and 4 cells. Compare the world hash in each tick. | AIT-INV-001, AIT-INV-002 |
| C3 | Run C1 with `AiDelay::AtLeast(1)`, and slow the AI stage at random. Compare the sequence of decisions of each entity with C1. Only the ticks of the applications can differ. | AIT-INV-001, AIT-LAG-004 |
| C4 | In C3, check in each tick that no decision is older than `max_lag`. | AIT-INV-004 |
| C5 | Slow the AI stage by 10 ticks in one tick. Check that the creeper swell, the melee hit and the door of E1, E3 and E4 occur in the same ticks as with no slowdown. | AIT-INV-003 |
| C6 | Kill the target of a mob while its path decision waits. Check that the hot tick drops the decision and the mob keeps its old guidance state. | AIT-LAG-005, AIT-LAG-006 |
| C7 | Give two animals the same partner in one issue tick. Run with 1 and 4 cells. Check that the same pair breeds in both runs. | AIT-CPL-002, AIT-CPL-003 |
| C8 | Measure the time of the hot tick with 10000 mobs, with the AI on and off. The difference must be only the cost of AIT-INV-006. | AIT-INV-006, AIT-SPL-003 |

## 11. Captured results

### E17

The vanilla server of `26.4-snapshot-7` ran without a player. The start log reports
`26.4 Snapshot 7`. Ticks were frozen and stepped singly, natural mob spawning was disabled,
and the route was force-loaded. Time stayed at 18000 with clear weather. Before each run, the
mobs were killed and the scene rebuilt; no other mob was present.

The scene used the proposed coordinates on a platform above the flat world's surface: ordinary
ice at `(0, 63, 0)` through `(11, 63, 0)`, cobweb at `(0, 64, 0)`, husk at
`(11.5, 64, 0.5)` and snow golem at `(0.5, 64, 0.5)`. Both mobs were persistent. The husk had
`NoAI:1b`; the golem had `OnGround:1b`, `Invulnerable:1b` and `Rotation:[-90.0f,0.0f]`.
The volume from `(-3, 62, -3)` through `(15, 70, 3)` was cleared before building.

Seven runs reached `t0 + 320`. Runs 2, 3, 4, 6 and 7 were accepted. Here, `t0` and the first
snowball are single-step counts after the summon. `x` is the golem's horizontal coordinate;
`dx(+102)` is its change from `t0 + 101` to `t0 + 102`, in blocks.

| Run | `t0` | First snowball | `x(+100) = x(+101)` | `dx(+102)` |
|-----|------|----------------|---------------------|------------|
| 1 | 1 | 21 | 1.0057772867504262 | 0.0035858782939612865 |
| 2 | 3 | 23 | 0.8621737076900899 | 0.0035858782939612865 |
| 3 | 21 | 41 | 0.8621737076900899 | 0.0035858782939612865 |
| 4 | 6 | 26 | 0.8621737076900899 | 0.0035858782939612865 |
| 5 | 3 | 22 | 0.8621737076900899 | 0.0035858782939612865 |
| 6 | 4 | 24 | 0.8621737076900899 | 0.0035858782939612865 |
| 7 | 18 | 38 | 0.8621737076900899 | 0.0035858782939612865 |

The world game ticks at `t0` were 5870, 6193, 6534, 6860, 7183, 7507 and 7845, respectively.
Run 1 was rejected: `OnGround` was `0b` at `t0`, although the feet stayed at height 64. Run 5
was rejected: its first snowball was at `t0 + 19`, outside the required `+20` calibration.
Every run saved `OnGround:0b` at summon step 1 and `1b` at every later step.
The five accepted runs started pursuit after that first step, with no preceding movement.

In every accepted run, the husk stayed exactly at `(11.5, 64, 0.5)`, and the golem stayed at
`y = 64`, `z = 0.5`. Travel from the summon position through `t0 + 101` was
`0.36217370769008994` blocks. The target remained more than 10.60 blocks away through
`t0 + 110`. The route's ice, cobweb and air above it stayed unchanged in every run, including
the continuations. The horizontal change from `t0 + 100` to `t0 + 101` was exactly zero in
every run, accepted or rejected.

The pause at `+101` lasted one tick. Later continuous rests began at `+229` in run 1 and at
`+270` in runs 2 through 7: gaps of 128 and 169 ticks from the first pause. Those rests lasted
through `+320`, at `x = 1.9892319575955655` and `x = 1.992649388587642`, respectively. By then
the golem had left the web and was less than 10 blocks from the husk. These later rests do not
measure another stuck window under the acceptance conditions.

The accepted captures confirm the predicted motion pause at `+101`, with movement at `+100`
and again at `+102`. This agrees with the stop boundary of AIT-VAN-012 for this slow pursuit
and separates its prediction from a continued `0.003586`-block step at `+101`. The commands
do not expose the path or the stuck state directly. Only `26.4-snapshot-7` was run; the older
version was not measured. The window restart triggers, the displacement threshold and speed
formula, height and liquid sampling, zero-speed case, stuck-state clearing, node timeout and
wanted position rules remain unconfirmed.
