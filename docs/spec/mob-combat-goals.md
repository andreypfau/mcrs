# Mob combat goals: target goals, attacks, avoidance, anger and the goals of tame animals

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-7` (Minecraft Java Edition, 26.4 Snapshot 7) |
| Protocol version | `1073742169` |

This document states the generic goals that a mob uses to select an attack target, to attack it,
to avoid another entity, and to serve an owner. The wolf (`mob-wolf.md`, prefix `WOLF`) and the
llama use these goals. `mob-goals.md` (prefix `GOAL`) states the goal framework and the goals of the
passive animals. This document uses that framework with no change.

The rules state results, not algorithms. Each rule has a fidelity class (GOAL section 0.3). Each
goal has a section "Cost and parallelism". That section states the world queries of the reference,
their cadence and their order. It also states the semantic shortcuts that keep the result equal,
and the fidelity of each shortcut. The aim is a parallel implementation that ticks 50,000 mobs with
a small cost in each tick.

## 0. Conventions

### 0.1 Terms

- The terms of `mob-goals.md`, section 0.1, apply. Examples are **goal list**, **target list**,
  **priority**, **examined**, **evaluation goal**, **per-tick goal**, **evaluation unit**, **gate**,
  **wait**, **qualified player**, **visibility factor**, **line of sight check**, **feet point**,
  **eye point**, **feet distance**, **order key**, **world query**, **keyed random value**.
- **Stored target**: the one entity that a mob keeps as its target, or none.
- **Valid target test**: a test of an entity for one mob. It is false for a player in the game mode
  `creative` or `spectator`. It is false when the mob cannot attack the entity (CGOAL-NEAR-002,
  step 4, first part). Else it is true.
- **Attack target**: the stored target when it passes the valid target test now, else none. A goal
  that "sets the attack target" to an entity stores the entity when it passes the valid target test,
  else stores none.
- **Target goal**: a goal of the target list. Each target goal of this document holds the flag
  `target`, except the reset of universal anger, which holds no flag.
- **Follow distance**: the value of the attribute `follow_range` of the mob, times the follow scale
  of the goal. The follow scale is 1 unless the goal names another value.
- **Hurt-by record**: the attacker and the attack time of a living entity (CGOAL-HURT-001).
- **Last-hit record**: the last entity that a living entity hit, and the time of that hit
  (CGOAL-MELEE-008).
- **Owner**: the living entity that a tame mob belongs to. The owner is "present" when it is loaded
  in a dimension of the server. Else the mob has no present owner.
- **Tame**: the state of a mob that has an owner reference.
- **Ordered to sit**: a server state of a tame mob. A player of the owner sets and clears it.
- **Sitting pose**: an entity data bit of a tame mob that the client shows.
- **Ally**: an entity B is an ally of a mob A when both are on the same team, or A is tame and B is
  its owner or an ally of its owner. Teams are a hook.
- **Within home**: a block position is within home of a mob when the mob has no home, or the
  squared distance from the position to the home block is less than the square of the home radius
  (GOAL-POS-001).
- **Anger state**: the anger end time and the anger target of a neutral mob (CGOAL-RESET-ANGER-001).
- **Index probe**: a test of a spatial index of a set of entity types. It answers "does some entity
  of the set have a box that intersects this box". It does not read the other state of the entity.

### 0.2 Provenance, kind, confidence and fidelity

The labels of `mob-goals.md`, sections 0.2 and 0.3, apply. This document has no captured result
yet. Each `source_suggested` rule has a confidence of at most high, and section 9 attaches an
experiment. Each rule states its fidelity: `exact`, `distribution` or `envelope`.

### 0.3 ID prefix

Each rule has an ID with the prefix `CGOAL`. Other documents can cite the groups of this table.

| Group | Subject |
|-------|---------|
| `CGOAL-NEAR` | the attack target, the combat test, the base of a target goal, and the nearest attackable target goal |
| `CGOAL-HURT` | the hurt-by record, and the hurt-by target goal with the alert of others |
| `CGOAL-MELEE` | the melee attack goal and the melee hit |
| `CGOAL-LEAP` | the leap at the attack target |
| `CGOAL-AVOID` | the avoid entity goal and the search of a position away from a point |
| `CGOAL-RANGED` | the generic ranged attack goal |
| `CGOAL-RESET-ANGER` | the anger of a neutral mob, and the reset of universal anger |
| `CGOAL-OWNER` | the owner-hurt-by and the owner-hurt target goals, follow owner, sit when ordered, and beg |
| `CGOAL-DEV` | intentional deviations |

### 0.4 World queries

Each world query has the form of GOAL section 0.5: set, filter, select, tie and cadence. The IDs of
the queries of this document are `Q-NEAREST`, `Q-NEAREST-PLAYER`, `Q-ALERT`, `Q-AVOID`,
`Q-ANGER-ALERT` and `Q-BEG`.

### 0.5 Scope

In scope:

- the attack target and the tests of a candidate target;
- the target goals: nearest attackable target, hurt-by target, owner-hurt-by target, owner-hurt
  target, and the reset of universal anger;
- the goals melee attack, leap at target, avoid entity, ranged attack, follow owner, sit when
  ordered, and beg;
- the anger state of a neutral mob.

Out of scope:

| Subject | Where |
|---------|-------|
| The goal framework, the controls, the random position searches | `mob-goals.md` |
| The cadence of the AI, reflexes and decisions, pair actions | `entity-ai-timing.md` (AIT) |
| The path search and the path follower | `pathfinding.md` (PATH) |
| The hit procedure and the damage cooldown | `entity-damage.md` (DMG) |
| The knockback of a hit with a causing entity | hook. DMG-HIT-005, step 2, names it |
| Teams, the game rule `pvp` and the ally relation of teams | hook |
| The projectile of a ranged attack | the specification of each mob type |
| The goals of a bow, a crossbow, a trident and a spear | a later specification |
| The goals of mobs with a brain | a later specification |

### 0.6 Artefacts cited

- The damage type tags `no_anger` and `no_wolf_retaliation` of the data pack. In
  `26.4-snapshot-7`, `no_anger` holds `mob_attack_no_aggro`, and `no_wolf_retaliation` holds
  `sulfur_cube_hot`.
- The game rules `universal_anger` and `forgive_dead_players`.
- The packet `swing_animation` (`assets/mcrs/reports/packets.json`).

## 1. Target selection

### 1.1 The attack target and the combat test

**CGOAL-NEAR-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). A mob keeps
one stored target. Each goal of this document reads the attack target, not the stored target, unless
the rule names the stored target. The save does not keep the stored target, except through the anger
state (CGOAL-RESET-ANGER-002). A new mob has no stored target.

**CGOAL-NEAR-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The combat
test of a candidate entity `C` for a mob `M`, with a range `R`, has these steps in this order. The
test is false at the first step that fails.

1. `C` is not `M`.
2. `C` is alive, and `C` is not a spectator.
3. `C` passes the filter of the goal, when the goal has one.
4. `M` can attack `C`, and `C` is not an ally of `M`. `M` can attack `C` when all these conditions
   are true:
   - `C` is not a `ghast`;
   - `C` is not a player while the difficulty is `peaceful`;
   - `C` is not invulnerable. A player with the ability "invulnerable", for example in the game mode
     `creative`, is invulnerable;
   - when `M` is tame, `C` is not the owner of `M`.
5. When `R > 0`: the feet distance from `M` to `C` is at most `max(R × v, 2)`. `v` is the
   visibility factor of `C` toward `M` (GOAL-QRY-002). A test that ignores the visibility uses
   `v = 1`.
6. When the test checks the line of sight: the line of sight check from `M` to `C` is true
   (GOAL-QRY-003).

A combat test checks the line of sight unless the rule of the goal says that it does not.

The non-combat test is the same test without step 4.

**CGOAL-NEAR-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). The base of a
target goal. A target goal has two options: must see and must reach. It keeps a goal target, an
unseen count and an unseen limit `U`. `U` is 30 evaluation units, unless the goal sets another
value. The base gives these parts:

- Start action: set the unseen count to 0. Clear the reach cache (CGOAL-NEAR-008).
- Continue condition: let `T` be the attack target. When it is none, let `T` be the goal target.
  The condition is true when all these conditions are true:
  - `T` exists;
  - `M` can attack `T` (CGOAL-NEAR-002, step 4, first part);
  - `M` and `T` are not on the same team;
  - the feet distance from `M` to `T` is at most the follow distance;
  - when the goal must see: the line of sight check to `T` is true, or the unseen count after an
    increase by 1 is at most `U`. A true check sets the unseen count to 0.

  When the condition is true, it sets the attack target to `T`.
- Stop action: set the attack target to none. Forget the goal target.

The goal target is set only by the hurt-by goal (CGOAL-HURT-003). The other target goals keep no goal
target. Thus their continue condition is false at once when another goal clears the attack target.

### 1.2 The nearest attackable target goal

**CGOAL-NEAR-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). The nearest
attackable target goal holds the flag `target`. It is an evaluation goal. It has these parameters:

| Parameter | Meaning | Default |
|-----------|---------|---------|
| type set | the entity types that it can select. A set can be all players | none |
| random interval `r` | in ticks | 10 |
| must see | an option of the base (CGOAL-NEAR-003) | none |
| must reach | an option of the base | false |
| filter | step 3 of the combat test | none |
| follow scale `k` | the follow distance is `follow_range × k` | 1 |

Let `d` be the follow distance at the time of the test.

- Start condition: when `r > 0`, a random integer uniform in `[0, ceil(r / 2))` is 0. Then the
  query of CGOAL-NEAR-005 has a result. The result is the candidate.
- Start action: set the attack target to the candidate. Then do the start action of the base.
- Continue condition: the continue condition of the base.
- Stop action: the stop action of the base.

Thus the default gate passes with a chance of 1 in 5 in each full evaluation in which the goal is
examined (AIT-VAN-011).

**CGOAL-NEAR-005** (`source_suggested`, semantic, confidence: high, fidelity: exact). `Q-NEAREST`,
when the type set is not the players:

- Set: the entities of the type set whose box intersects the box of the mob grown by `d` on each of
  the three axes. Snapshot of the issue tick.
- Filter: the combat test with the range `d`, with the visibility, with the line of sight.
- Select: the smallest distance from the point `(x, e, z)` to the feet point of the candidate. `x`
  and `z` are those of the feet point of the mob, and `e` is the `y` of its eye point.
- Tie: GOAL-QRY-004.
- Cadence: in each full evaluation in which the goal is examined and the gate passes.

`Q-NEAREST-PLAYER`, when the type set is the players: the same query over the player table of the
dimension (AIT-CDN-002), with no box. The range test of step 5 limits the distance.

The vertical size of the box is `d`, not a smaller value. Thus a wolf with `d = 16` searches 16
blocks up and down.

**CGOAL-NEAR-006** (`source_suggested`, semantic, confidence: high, fidelity: exact). The must-reach
option has no effect on the nearest attackable target goal. The query of CGOAL-NEAR-005 does not use
the reach cache, and the base continue condition does not read it. The query also does not test
that the candidate is within home. CGOAL-DEV-001 records the difference to AIT-VAN-011.

**CGOAL-NEAR-007** (`source_suggested`, semantic, confidence: high, fidelity: exact). The non-tame
variant of the nearest attackable target goal. Only a mob that can be tame uses it. It differs in
these parts:

- Start condition: the mob is not tame. Then the start condition of CGOAL-NEAR-004. The tame test
  comes before the gate.
- Continue condition: the combat test of CGOAL-NEAR-002 of the candidate of the start, with the
  range `d`, the visibility and the line of sight. It does not read the attack target. It does not
  read the tame state. It has no unseen count.
- The continue condition does not set the attack target.

Thus the goal stops in the first full evaluation in which the candidate is not in line of sight,
is out of range, or is dead. The goal can continue while the attack target is none, for example
after an avoid goal cleared it (WOLF-HUNT-004). While it continues, it holds `target`. No target goal
with a larger priority number can start.

**CGOAL-NEAR-008** (`source_suggested`, semantic, confidence: medium, fidelity: distribution). The
single-candidate test of a target goal. The hurt-by goal and the owner goals test one candidate
`C` with a combat test `X`. The test is true when all these conditions are true:

1. `C` exists, and `X` is true for `C`;
2. the block position of `C` is within home of the mob;
3. when the goal must reach: the reach cache says "reach".

The reach cache has three states: empty, reach and no reach. Each test decreases the cache time by
1. When the cache time becomes 0 or less, the cache becomes empty. An empty cache does a path search
to `C` with the reach range 0 (PATH-REQ-001). It sets the cache time to `ceil((10 + r) / 2)`
evaluation units, with `r` uniform in `[0, 5)`. The cache is "reach" when the search gives a path
whose end node is at most 1.5 blocks from the block position of `C`, horizontally. Else it is "no
reach". No vanilla goal of the wolf or the llama uses this option with these goals.

### 1.3 Cost and parallelism of the nearest attackable target goal

**CGOAL-NEAR-009** (`source_suggested`, semantic, confidence: high, fidelity: exact). The reference
does the work of the start condition in this order:

1. the tame test, for the non-tame variant;
2. the draw of the gate;
3. the enumeration of each entity of the type set in the box of CGOAL-NEAR-005. The box spans
   `2d + w` blocks on the horizontal axes and `2d + h` blocks on the vertical axis, where `w` and
   `h` are the width and the height of the mob;
4. for each entity of the box, the steps of the combat test in order. The filter of step 3 is cheap
   and comes before the range test and the ray;
5. one ray of the line of sight check for each candidate that passes steps 1 to 5. The cache of
   AIT-VAN-004 keeps the result for the rest of the tick.

A type set that is a parent kind of several types enumerates all of them. For example, the hunt of
a wolf (WOLF-TGT-001) enumerates each animal in the box and then filters three types by step 3.

**CGOAL-NEAR-010** (`design`, semantic, confidence: high, fidelity: exact). These shortcuts keep
the result of `Q-NEAREST` equal:

- **Type index.** An index of the entities of each type of the type set, after the filter of step 3
  when the filter reads only the type. The query reads only that index. For a wolf hunt the index
  holds `sheep`, `rabbit` and `fox`, not each animal.
- **Empty probe.** When the index probe of the box is empty, the result is none. The goal then
  needs no ray and no distance.
- **Nearest first.** Sort the candidates that pass steps 1 to 5 by the select distance, then by the
  order key. Do the ray for each candidate in this order. The first candidate with a true ray is the
  result. The rays that are not done do not change the result.

**CGOAL-NEAR-011** (`design`, semantic, confidence: medium, fidelity: distribution). The gate of
CGOAL-NEAR-004 is a gate of GOAL-WAKE-003, with `p = 1 / ceil(r / 2)`. The goal is eligible when it
is examined, and, for the non-tame variant, the mob is not tame. These additional rules apply:

- While the index probe of the box is empty, the gate has no effect: a pass gives no result. The
  implementation can end the wait and draw no new wait.
- In the first full evaluation in which the probe is not empty, the goal draws a new wait.

The geometric law has no memory. Thus the evaluation of the first start has the law of the
reference. The probe is a part of each full evaluation of the mob. It is not a wake event.

**CGOAL-NEAR-012** (`design`, semantic, confidence: medium, fidelity: exact). Place of each part:

- the start condition and the query: a decision (AIT-DEC-001). It reads the snapshot;
- the continue condition: a decision. It reads the snapshot and casts at most one ray;
- the start action and the continue write: the attack target of the own mob only.

The start of a target goal and the melee start check of the same full evaluation read the same
attack target. The target list runs before the goal list (GOAL-FRM-001).

## 2. The hurt-by record and the hurt-by target goal

**CGOAL-HURT-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The hurt-by
record of a living entity. A hit that passes the cooldown of DMG-HIT-003, full or partial, sets the
record when all these conditions are true:

- the damage source has a causing entity, and that entity is living;
- the damage type is not in the tag `no_anger`;
- the damage type is not `wind_charge`, or the type of the hit entity is not in the entity type tag
  `no_anger_from_wind_charge`.

The record holds the causing entity and the attack time. The attack time is the tick count of the
hit entity. This rule fills the hook of DMG-HIT-005, step 1, for the anger of mobs.

In each tick, the base entity tick clears the record when the recorded entity is dead, or when the
tick count minus the attack time is more than 100. The save keeps the record as the UUID and the
ticks since the hit.

**CGOAL-HURT-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The hurt-by
target goal holds the flag `target`. It is an evaluation goal. It must see. It does not need to
reach. It has two parameters: a set of ignored attacker types, and an optional alert with a set of
types that do not receive the alert. It keeps the attack time of its last start, at first 0.

- Start condition: all these conditions are true:
  - the hurt-by record has an entity `A`, and its attack time differs from the kept attack time;
  - `A` is not a player while the game rule `universal_anger` is true;
  - the type of `A` is not an ignored type;
  - the single-candidate test (CGOAL-NEAR-008) of `A` is true, with a combat test that has no
    range, that ignores the visibility, and that does not check the line of sight.
- Start action (CGOAL-HURT-003).
- Continue condition and stop action: the base (CGOAL-NEAR-003), with the unseen limit 150
  evaluation units.

**CGOAL-HURT-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). The start
action of the hurt-by goal does these steps in this order:

1. Set the attack target to `A`. Set the goal target to the attack target.
2. Keep the attack time of the record.
3. Set the unseen limit to 150 evaluation units. The limit stays for each later start.
4. When the goal has the alert, alert the others (CGOAL-HURT-004).
5. Do the start action of the base.

**CGOAL-HURT-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). `Q-ALERT`:

- Set: the entities of the same entity type as the mob whose box intersects a box. The box is the
  unit cube whose lower corner is the feet point of the mob, grown by `d` on the `x` and `z` axes
  and by 10 on the `y` axis. Thus the box is not centred on the mob: it spans from `x - d` to
  `x + 1 + d`, and from `y - 10` to `y + 11`.
- Filter: all these conditions are true:
  - the entity is not a spectator, and it is not the mob;
  - it has no attack target;
  - when the mob can be tame: the entity has the same owner as the mob. Two entities with no owner
    have the same owner;
  - it is not an ally of `A`;
  - its type is not in the set of types that do not receive the alert.
- Select: all.
- Cadence: one time, at the start of the hurt-by goal with the alert.

For each member, set its attack target to `A`. The receiver does not start a target goal. Its
attack target stays until a goal of the receiver changes it.

"The same entity type" is exact for the wolf and the llama. A `trader_llama` does not alert a
`llama`, and a `llama` does not alert a `trader_llama`.

**CGOAL-HURT-005** (`design`, semantic, confidence: medium, fidelity: envelope). The alert is the
pair action "alert" of AIT-CPL-004. The proposer is the mob of the hurt-by goal. The grant checks
each receiver again: it is alive, of the same type, and has no attack target now. The envelope: the
receivers see the new attack target in the same tick as the proposer, or at most `max_lag` ticks
later (AIT-LAG-002).

### 2.1 Cost and parallelism of the hurt-by goal

**CGOAL-HURT-006** (`design`, semantic, confidence: high, fidelity: exact). The start condition
reads only the own hurt-by record and the kept attack time, until the attack time differs. Thus it
is cheap in each full evaluation. A change of the record is a hit, which is wake event 1 of
GOAL-WAKE-005. The alert reads one box query of the own type, one time for each start. Its box spans
`2d + 1` blocks horizontally and 21 blocks vertically. An index of the entities of each type
answers it. The continue condition casts at most one ray in each full evaluation.

## 3. Attacks

### 3.1 The melee attack goal

**CGOAL-MELEE-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The melee
goal holds the flags `move` and `look`. It is a per-tick goal (AIT-VAN-005). It has three
parameters: a speed modifier `m`, the option "follow a target that it does not see", and an attack
interval `I`, by default 20 ticks. It keeps a path from its start check, the target point of its
last path request, a path timer, an attack cooldown, and the game time of its last start check.
The target point starts at `(0, 0, 0)`. The last check time starts at 0.

**CGOAL-MELEE-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The start
condition does these steps:

1. When the game time minus the last check time is less than 20, the condition is false. Stop.
2. Set the last check time to the game time.
3. When the attack target is none or is not alive, the condition is false.
4. Search a path to the attack target with the reach range 0 (PATH-REQ-001). Keep it.
5. The condition is true when the search gave a path, or when the attack target is in reach
   (CGOAL-MELEE-006).

Thus the start condition runs at most one time in 20 game ticks, also when the goal is examined in
each full evaluation. AIT-VAN-006 states the same timer.

**CGOAL-MELEE-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). The other
parts of the goal:

- Start action: give the kept path to the navigation with the speed modifier `m`. Set the
  aggressive bit of the mob flags (TRK-DATA-002, entry 15). Set the path timer and the attack
  cooldown to 0.
- Continue condition: the attack target exists and is alive. Then:
  - with the option "follow a target that it does not see": the block position of the target is
    within home, and the target is not a player in the game mode `spectator` or `creative`;
  - without the option: the path is not done.
- Stop action: when the stored target is a player in the game mode `creative` or `spectator`, set
  the attack target to none. Clear the aggressive bit. Clear the path.

**CGOAL-MELEE-004** (`source_suggested`, semantic, confidence: high, fidelity: exact for the
timers, distribution for the random parts). The goal tick runs in each tick while the goal runs and
the attack target exists. It does these steps:

1. Set a look target at the attack target with the head turn speed 30 and the pitch limit 30.
2. Decrease the path timer by 1, to at least 0.
3. When all these conditions are true, request a new path:
   - the goal has the option "follow a target that it does not see", or the line of sight check to
     the target is true;
   - the path timer is 0;
   - one of these is true: the target point is `(0, 0, 0)`; the squared distance from the target
     point to the feet point of the target is 1 or more; a random float uniform in `[0, 1)` is less
     than `0.05`.

   The request: set the target point to the feet point of the target. Set the path timer to
   `4 + q`, with `q` uniform in `[0, 7)`. Add 10 when the squared feet distance to the target is
   more than 1024, else add 5 when it is more than 256. Request a path to the target with the reach
   range 0 and the speed modifier `m`. Add 15 to the path timer when the request gives no path.
4. Decrease the attack cooldown by 1, to at least 0.
5. When the attack cooldown is 0, the target is in reach, and the line of sight check to the target
   is true: set the attack cooldown to `I`, and do the melee hit (CGOAL-MELEE-007).

The timers count ticks, because the goal ticks in each tick. The draw of `0.05` occurs only when the
first two conditions of step 3 are true, and the other two parts of the third condition are false.

**CGOAL-MELEE-005** (`source_suggested`, semantic, confidence: medium, fidelity: exact). A
consequence: the goal does not clear the attack target when it stops for another reason. A goal with
a smaller priority number that takes `move`, for example a leap, stops the melee goal. The melee
goal then waits for its next start check, up to 20 ticks after the last check (WOLF-HUNT-003).

**CGOAL-MELEE-006** (`source_suggested`, semantic, confidence: high, fidelity: exact). The target is
in reach when the box of the mob, grown by `sqrt(2.04F) - 0.6F` on the `x` and `z` axes, intersects
the box of the target. A mob that rides a vehicle uses the horizontal union of its own box and the
box of the vehicle, with its own vertical extent. An item with the component `attack_range` in the
active hand changes the reach. That case is a hook.

**CGOAL-MELEE-007** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The melee
hit of a mob `M` on a target `T` does these steps:

1. Start the swing of the main hand. When a swing starts, send `swing_animation` for `M` with the
   main hand to the players that track `M`, not to `M` itself.
2. Hit `T` with the procedure of DMG section 3. The damage type is `mob_attack`. `M` is the causing
   entity and the direct entity. The amount is the value of the attribute `attack_damage` of `M`.
   An item in the main hand of `M` can change the type and the amount. That case is a hook.
3. When the hit is not rejected: apply the extra knockback of `M`, set the last-hit record of `M`
   (CGOAL-MELEE-008), and play the attack sound of the type, when the type has one.

The hit applies the default knockback of a hit with a causing entity. That knockback is a hook
(section 0.5).

**CGOAL-MELEE-008** (`source_suggested`, semantic, confidence: high, fidelity: exact). The last-hit
record of a living entity holds the entity that it last hit with a melee hit that was not rejected,
and the tick count of that hit. The base entity tick clears the entity when it is dead. The
owner-hurt target goal reads this record (CGOAL-OWNER-002).

#### Cost and parallelism of the melee goal

**CGOAL-MELEE-009** (`source_suggested`, semantic, confidence: high, fidelity: exact). The
reference costs of the goal:

- Start check: at most one path search in 20 game ticks, while the attack target exists.
- Goal tick: one path request each 4 to 10 ticks while the target moves, more when it is far. A
  request to the same target block gives the current path (GOAL-NAV-004). A new search occurs when
  the target changes its block position.
- One look target in each tick.
- One reach test in each tick in which the cooldown is 0. One ray only when the target is in reach,
  or when the goal does not follow an unseen target.

**CGOAL-MELEE-010** (`design`, semantic, confidence: medium, fidelity: exact). Place of each part
(AIT-RFX-003): the cooldown, the reach test, the ray of the hit, the hit and the look target are a
reflex. The start check and each path request are decisions. The path timer is a part of the
decision state: the reflex counts it down, and the decision reads it at its issue tick. A melee hit
changes another entity. It is a hit of DMG section 3, and DMG-ORD-002 states the order of the hits
of one tick.

### 3.2 Leap at the target

**CGOAL-LEAP-001** (`source_suggested`, semantic, confidence: high, fidelity: exact for the
conditions, distribution for the gate). The leap goal holds the flags `jump` and `move`. It is an
evaluation goal with one parameter, the vertical speed `y0`. It keeps a target.

- Start condition: all these conditions are true, in this order:
  - no passenger controls the mob;
  - the attack target exists. It becomes the target of the goal;
  - the squared feet distance to the target is from 4 to 16;
  - the mob is on the ground;
  - a random integer uniform in `[0, 3)` is 0.
- Start action: let `(vx, vy, vz)` be the velocity of the mob. Let `D` be the horizontal offset
  `(tx - x, 0, tz - z)` from the feet point of the mob to the feet point of the target. When
  `|D|² > 1.0E-7`, let `D` be `0.4 × D / |D| + 0.2 × (vx, vy, vz)`. Set the velocity to
  `(D.x, y0, D.z)`.
- Continue condition: the mob is not on the ground.
- Stop action: none. The goal has no goal tick.

The chance 1 in 3 is the chance 1 in 5 ticks of AIT-VAN-003. The goal stops in the first full
evaluation after the mob lands.

**CGOAL-LEAP-002** (`design`, semantic, confidence: medium, fidelity: envelope). The start of the
leap is a decision. Its velocity write applies at the application of the decision (AIT-LAG-002). The
envelope: the velocity write occurs 1 to `max_lag` ticks after the issue tick, and it uses the
velocity of the mob at the application. The leap reads no world query. Its cost is one distance and
one draw in each full evaluation in which the mob has an attack target at 2 to 4 blocks.

### 3.3 The ranged attack goal

**CGOAL-RANGED-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The ranged
goal holds the flags `move` and `look`. It is a per-tick goal. It has four parameters: a speed
modifier `m`, a smallest interval `Imin` and a largest interval `Imax` in ticks, and an attack range
`R`. A type that gives one interval `I` has `Imin = Imax = I`. The goal keeps a target, a see count
and an attack time. The attack time starts at −1. The type gives the ranged attack action.

- Start condition: the attack target exists and is alive. It becomes the target of the goal.
- Continue condition: the start condition is true, or the target of the goal is alive and the path
  is not done.
- Stop action: forget the target. Set the see count to 0 and the attack time to −1.

**CGOAL-RANGED-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The goal
tick does these steps in each tick. Let `s²` be the squared feet distance to the target, and `f`
be `sqrt(s²) / R`.

1. Do the line of sight check to the target. When it is true, add 1 to the see count. Else set the
   see count to 0.
2. When `s² ≤ R²` and the see count is 5 or more, clear the path. Else request a path to the target
   with the speed modifier `m` and the reach range 1.
3. Set a look target at the target with the head turn speed 30 and the pitch limit 30.
4. Decrease the attack time by 1. Then:
   - when it is 0: if the line of sight check of step 1 was false, stop the tick. Else do the
     ranged attack action with the power `clamp(f, 0.1, 1.0)` (f32), and set the attack time to
     `floor(f × (Imax - Imin) + Imin)`;
   - else, when it is less than 0: set the attack time to `floor(lerp(f, Imin, Imax))`.

Thus the first attack occurs `Imin + 1` ticks after the start when `Imin = Imax` and the target stays
in sight. An attack time that reaches 0 without the line of sight becomes −1 in the next tick, and
a new interval starts.

**CGOAL-RANGED-003** (`source_suggested`, semantic, confidence: medium, fidelity: exact). The path
request of step 2 occurs in each tick while the target is out of range or not seen for 5 ticks. A
request to the same target block gives the current path (GOAL-NAV-004). The continue condition lets
the mob finish its path after the attack target becomes none.

**CGOAL-RANGED-004** (`design`, semantic, confidence: medium, fidelity: exact). Place of each part
(AIT-RFX-003): the see count, the attack time, the ray and the attack action are a reflex. The path
request and the path clear are decisions. The cost in each tick is one ray (cache of AIT-VAN-004),
one distance and one look target. A path search occurs only when the target changes its block
position.

## 4. Avoid an entity

**CGOAL-AVOID-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The avoid
goal holds the flag `move`. It is an evaluation goal. It has these parameters: a type set, a
largest distance `D`, a walk speed modifier `m1`, a sprint speed modifier `m2`, and a filter. The
default filter rejects a player in the game mode `creative` or `spectator`. A mob type can add an
extra start test, which runs after the base start condition (CGOAL-AVOID-005). The goal keeps an
entity to avoid and a path.

**CGOAL-AVOID-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). `Q-AVOID`:

- Set: the entities of the type set whose box intersects the box of the mob grown by `D` on the `x`
  and `z` axes and by 3 on the `y` axis. Snapshot of the issue tick.
- Filter: the combat test (CGOAL-NEAR-002) with the range `D`, the visibility and the line of
  sight. The filter of step 3 is the filter of the goal.
- Select: the smallest feet distance.
- Tie: GOAL-QRY-004.
- Cadence: in each full evaluation in which the goal is examined. The goal has no gate.

**CGOAL-AVOID-003** (`source_suggested`, semantic, confidence: medium, fidelity: distribution).
The away search `W(h, v, p)` gives a random position away from a point `p`. Let `(ax, az)` be the
horizontal offset from `p` to the feet point of the mob, and `c = atan2(az, ax) - π/2`. The search
makes 10 tries. Each try does these steps:

1. Draw `u1` uniform in `[0, 1)` (f32) and let the angle be `c + (2 × u1 - 1) × π/2`. Draw `u2`
   uniform in `[0, 1)` and let the length be `h × sqrt(u2) × sqrt(2)`.
2. Let `(dx, dz)` be `(-length × sin(angle), length × cos(angle))`. When `|dx| > h` or `|dz| > h`,
   the try fails.
3. Draw `b`, an integer uniform in `[-v, v]`. The offset is the block position of `(dx, b, dz)`.
4. Do steps 2 to 4 of GOAL-POS-002 with this offset in place of `(a, b, c)`.

The selection of the result is that of GOAL-POS-002: the largest walk value, then the earlier try.
Thus each try draws a direction within 90 degrees of the direction away from `p`.

**CGOAL-AVOID-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). The base
start condition does these steps in this order:

1. Make `Q-AVOID`. When it has no result, the condition is false. Else the result is the entity to
   avoid, `E`.
2. Make the away search `W(16, 7, p)`, with `p` the feet point of `E`. When it has no result, the
   condition is false. Else let `q` be the result.
3. When the squared distance from `E` to `q` is less than the squared feet distance from `E` to the
   mob, the condition is false.
4. Search a path to `q` with the reach range 0. Keep it. The condition is true when the search gives
   a path.

**CGOAL-AVOID-005** (`source_suggested`, semantic, confidence: high, fidelity: exact). The other
parts of the goal:

- Extra start test: when the mob type gives one, the start condition is true only when the base
  condition is true and the extra test is true. The extra test runs after step 4.
- Start action: give the kept path to the navigation with the speed modifier `m1`.
- Goal tick: when the squared feet distance to `E` is less than 49, set the speed modifier of the
  navigation to `m2`. Else set it to `m1`.
- Continue condition: the path is not done.
- Stop action: forget `E`. The stop does not clear the path.

A mob type can add work to the start action and the goal tick (WOLF-GOAL-003).

### 4.1 Cost and parallelism of the avoid goal

**CGOAL-AVOID-006** (`source_suggested`, semantic, confidence: high, fidelity: exact). The
reference cost of one examined full evaluation:

1. one box enumeration of the type set. The box spans `2D + w` blocks horizontally and `6 + h`
   blocks vertically;
2. the combat test of each entity in the box, with one ray for each entity in range;
3. when step 1 of CGOAL-AVOID-004 has a result: up to 10 tries of the away search, each with the
   block reads of GOAL-POS-002, step 4, and one walk value;
4. when step 3 of CGOAL-AVOID-004 passes: one path search;
5. the extra start test.

The goal has no gate. Thus the reference makes `Q-AVOID` in each full evaluation in which the goal
is examined. Steps 3 and 4 run in each such evaluation while an entity to avoid is near, until the
goal starts. An extra start test that fails throws away the work of steps 3 and 4.

**CGOAL-AVOID-007** (`design`, semantic, confidence: high, fidelity: exact). These shortcuts keep
the result equal:

- **Type index and empty probe.** An index of the entities of the type set. When the probe of the
  box is empty, the start condition is false. No other work occurs.
- **Nearest first.** CGOAL-NEAR-010 applies to `Q-AVOID` with the feet distance.
- **Cheap extra test first.** When the extra test reads only the own mob and `E`, and its random
  draw is independent of the draws of steps 2 to 4, the implementation can do the extra test after
  step 1. When the extra test is false, steps 2 to 4 do not occur. The result has the same law.
  WOLF-GOAL-004 applies this rule. A deterministic part of the extra test, for example a tame test,
  can come before step 1.

The order of the random draws changes. With keyed random values (AIT-LAG-008) each draw has its own
key, so the law of each result stays equal. The fidelity of the draws is `distribution`.

**CGOAL-AVOID-008** (`design`, semantic, confidence: medium, fidelity: envelope). The start
condition, the away search and the path search are one decision (AIT-DEC-001). The goal tick is a
decision too. It writes only the speed modifier of the own navigation. The envelope is the allowed
delay of AIT-DEC-003 for a random position.

## 5. Anger of a neutral mob

**CGOAL-RESET-ANGER-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). A
neutral mob has an anger state: an anger end time (a game time, or −1 for none) and an anger target
(a UUID reference, or none). The type gives the entity data entry of the end time, if any, and the
law of the anger duration. The mob is angry when the end time is more than 0 and more than the
current game time. These operations change the state:

- **Start the anger timer**: set the end time to the game time plus a duration from the law of the
  type.
- **Stop the anger**: clear the hurt-by record. Set the anger target to none. Set the attack target
  to none. Set the end time to −1.
- **Refresh universal anger**: stop the anger, then start the anger timer. The mob is then angry
  with no anger target.

**CGOAL-RESET-ANGER-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The
save keeps the end time as `anger_end_time` and the anger target as `angry_at`. A save without
`anger_end_time` and with the legacy key `AngerTime` sets the end time to the game time plus that
value. A save with neither sets −1. At a load in a dimension, the mob sets its attack target to the
entity of `angry_at`, when that entity is loaded.

**CGOAL-RESET-ANGER-003** (`source_suggested`, semantic, confidence: high, fidelity: exact). The
anger update. A type names the cadence and the value of the option "stay angry while a target is
present". The update does these steps:

1. Let `P` be the stored target. When `P` exists, `P` is dead or in the dying state, the anger
   target refers to `P`, and `P` is a mob: stop the anger. The update ends.
2. Let `T` be the attack target. When `T` exists:
   1. When the anger target does not refer to `T`, set the anger target to `T`. This is a new
      target.
   2. When the target is new, or the option is true: start the anger timer.
3. When the anger target existed before step 2, the mob is not angry, and one of these is true:
   `T` is none; `T` is not a valid player target; the option is false: stop the anger.
4. When the anger target of before step 2 refers to a loaded player in the game mode `creative` or
   `spectator`, or the difficulty is `peaceful`: stop the anger.

A valid player target is a player who is not in the game mode `creative` or `spectator`, while the
difficulty is not `peaceful`. Steps 3 and 4 read the anger target of before step 2.

With the option true, step 2 starts the anger timer again in each update while an attack target
exists. Thus the end time moves in each update. Any attack target makes the mob angry: also an
animal or a skeleton.

**CGOAL-RESET-ANGER-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). The
"angry at" test of a neutral mob for an entity `C` is true when the mob can attack `C`, and one of
these is true:

- `C` is a valid player target, the game rule `universal_anger` is true, the mob is angry, and it
  has no anger target;
- the anger target refers to `C`.

When a player dies and the game rule `forgive_dead_players` is true, each neutral mob whose anger
target refers to that player stops the anger.

**CGOAL-RESET-ANGER-005** (`source_suggested`, semantic, confidence: high, fidelity: exact). The
reset of universal anger is a target goal that holds no flag (GOAL-FRM-003). It has the option
"alert others of the same type". It keeps the attack time of its last start, at first 0.

- Start condition: the game rule `universal_anger` is true, the hurt-by record has an entity that
  is a player, and the attack time of the record is more than the kept attack time.
- Start action: keep the attack time. Refresh universal anger of the mob. With the option: make
  `Q-ANGER-ALERT`, and refresh universal anger of each member.
- Continue condition: the start condition. Thus the goal stops in the next full evaluation.

`Q-ANGER-ALERT` is `Q-ALERT` (CGOAL-HURT-004) with only these filters: the entity is not a
spectator and is not the mob. The box uses the follow range with no scale.

**CGOAL-RESET-ANGER-006** (`design`, semantic, confidence: high, fidelity: exact). Cost: with
`universal_anger` false, which is the default, the start condition reads one game rule in each full
evaluation. The goal holds no flag, so it is examined in each full evaluation. A server can test the
game rule one time for each tick and skip the goal in each mob. The anger update (CGOAL-RESET-ANGER-003)
reads only the own mob and the liveness of the stored target. It is a reflex with no world query.

## 6. The goals of a tame mob

### 6.1 Owner-hurt-by and owner-hurt target goals

**CGOAL-OWNER-001** (`source_suggested`, semantic, confidence: high, fidelity: exact). The
owner-hurt-by target goal holds the flag `target`. It is an evaluation goal. It does not need to
see and does not need to reach. It keeps the attack time of its last start, at first 0.

- Start condition: all these conditions are true, in this order:
  - the mob is tame, and it is not ordered to sit;
  - the owner is present;
  - the owner took damage in the last 100 ticks, and the damage type of that damage is not in the
    tag `no_wolf_retaliation`;
  - the hurt-by record of the owner has an attack time that differs from the kept attack time. Let
    `A` be the entity of that record;
  - the single-candidate test (CGOAL-NEAR-008) of `A` is true, with the combat test with no range,
    with the visibility and with the line of sight;
  - the "wants to attack" test of the mob type for `A` and the owner is true. A type with no such
    test uses true.
- Start action: set the attack target to `A`. Keep the attack time of the record of the owner. Do
  the start action of the base.
- Continue condition and stop action: the base (CGOAL-NEAR-003).

**CGOAL-OWNER-002** (`source_suggested`, semantic, confidence: high, fidelity: exact). The
owner-hurt target goal is the same goal with these changes:

- The start condition does not test the last damage of the owner.
- `A` is the entity of the last-hit record of the owner (CGOAL-MELEE-008, or the player attack),
  and the attack time is the time of that record.

**CGOAL-OWNER-003** (`design`, semantic, confidence: high, fidelity: exact). Cost: the first
condition is a test of the own state. A wild mob ends the start condition there. A tame mob reads
two records of its owner. A change of these records is an event of the owner. The single-candidate
test casts at most one ray.

### 6.2 Follow the owner

**CGOAL-OWNER-004** (`source_suggested`, semantic, confidence: high, fidelity: exact). The follow
owner goal holds the flags `move` and `look`. It is an evaluation goal with three parameters: a
speed modifier `m`, a start distance `a` and a stop distance `b`. It keeps the owner, a path timer
and a saved water malus.

A tame mob cannot move to its owner when one of these is true: it is ordered to sit; it is a
passenger; it is leashed; the owner is present and is a spectator.

- Start condition: the owner is present, the mob can move to the owner, and the squared feet
  distance to the owner is `a²` or more.
- Start action: set the path timer to 0. Save the malus of `WATER` and set it to 0 (PATH-MAL-001).
- Goal tick: let "far" be true when the squared feet distance to the owner is 144 or more, in the
  same dimension. When not far, set a look target at the owner with the head turn speed 10 and the
  pitch limit of the mob. Decrease the path timer by 1. When it is 0 or less, set it to 5
  evaluation units. Then, when far, try the teleport (CGOAL-OWNER-005). Else request a path to the
  owner with `m`.
- Continue condition: the path is not done, the mob can move to the owner, and the squared feet
  distance to the owner is more than `b²`.
- Stop action: forget the owner. Clear the path. Restore the malus of `WATER`.

**CGOAL-OWNER-005** (`source_suggested`, semantic, confidence: medium, fidelity: distribution).
The teleport to the owner makes 10 tries around the block position `O` of the owner. Each try draws
`dx` and `dz` uniform in `[-3, 3]`. When both `|dx|` and `|dz|` are less than 2, the try fails.
Else it draws `dy` uniform in `[-1, 1]`, and tests the block position `O + (dx, dy, dz)`:

- its mob path type for the mob is `WALKABLE` (PATH-TYP-004);
- the block below is not a leaves block, unless the type can fly to its owner;
- the box of the mob, moved by the offset from its block position to the tested position, has no
  collision.

The first position that passes is the destination. The mob moves to the bottom centre of that
block with its rotation, and its path clears. When no try passes, nothing occurs. The panic goal of
a tame mob also tries this teleport in its goal tick, while the mob can move to its owner and is far.

### 6.3 Sit when ordered

**CGOAL-OWNER-006** (`source_suggested`, semantic, confidence: high, fidelity: exact). The sit goal
holds the flags `jump` and `move`. It is an evaluation goal.

- Start condition, in this order:
  1. When the mob is not ordered to sit and is not tame: false.
  2. When the mob is in water, or is not on the ground: false.
  3. When the owner is not present, or is in another dimension: true.
  4. When the squared feet distance to the owner is less than 144 and the owner has a hurt-by
     record: false.
  5. Else: the mob is ordered to sit.
- Start action: clear the path. Set the sitting pose.
- Continue condition: the mob is ordered to sit.
- Stop action: clear the sitting pose.

A tame mob that is not ordered to sit and has no present owner starts the goal by step 3. Its
continue condition is false, so the goal stops in the next full evaluation and starts again in
step 3 of that evaluation. The sitting pose stays set at the end of each such evaluation.

### 6.4 Beg

**CGOAL-OWNER-007** (`source_suggested`, semantic, confidence: high, fidelity: exact). The beg goal
holds the flag `look`. It is an evaluation goal with one parameter, the range `L`. The type gives
the item test "interesting". It keeps a player and a look time. It has no gate.

- Start condition: `Q-BEG` has a result, and that player holds an interesting item in the main hand
  or in the off hand.
- Start action: set the interested flag of the type. Set the look time to `ceil((40 + q) / 2)`
  evaluation units, with `q` uniform in `[0, 40)`.
- Goal tick: set a look target at the point `(px, e, pz)`, where `(px, pz)` is the feet point of
  the player and `e` is the `y` of its eye point. Use the head turn speed 10 and the pitch limit of
  the mob. Decrease the look time by 1.
- Continue condition: the player is alive, the squared feet distance to the player is at most `L²`,
  the look time is more than 0, and the player holds an interesting item.
- Stop action: clear the interested flag. Forget the player.

**CGOAL-OWNER-008** (`source_suggested`, semantic, confidence: high, fidelity: exact). `Q-BEG`:

- Set: the player table of the dimension.
- Filter: the non-combat test (CGOAL-NEAR-002) with the range `L`, the visibility and the line of
  sight.
- Select: the smallest feet distance.
- Tie: GOAL-QRY-004.
- Cadence: in each full evaluation in which the goal is examined.

The item test applies only to the selected player. A nearer player with no interesting item blocks
the beg toward a player farther away.

**CGOAL-OWNER-009** (`design`, semantic, confidence: high, fidelity: exact). Cost and shortcuts of
the beg goal. The reference casts one ray for each qualified player within `max(L × v, 2)`, in each
full evaluation in which the goal is examined, before the item test. These shortcuts keep the result
equal:

- When no qualified player of the dimension is within `max(L × v, 2)`, the result is none. This is
  a read of the player table, as for `Q-TEMPT` (GOAL-QRY-005).
- When no qualified player within that range holds an interesting item, the start condition is
  false. No ray is needed.
- Else sort the players in range by the feet distance and do the rays in that order. The first
  player with a true ray is the result. Then do the item test of that player only.

The wake rule of GOAL-WAKE-015 applies: a qualified player with an interesting item in range makes
the mob busy. Else the beg goal does not need a full evaluation.

### 6.5 Parallelism of the owner goals

**CGOAL-OWNER-010** (`design`, semantic, confidence: medium, fidelity: envelope). Each start, tick
and continue of the goals of this section is a decision. The teleport changes the position of the
own mob. It is a move of the own row, not a pair action. It applies at the application of the
decision, and the hot tick checks the collision of CGOAL-OWNER-005 again at the application. When
the check fails at the application, the teleport does not occur. The envelope is the allowed delay
of AIT-DEC-003 for a path search.

## 7. Parallelism notes

| Goal | Place | Reads (own) | Reads (other entities and world) | Writes | World queries |
|------|-------|-------------|----------------------------------|--------|---------------|
| nearest attackable target | decision | follow range, tame state, position, box, eye height | entities of the type set: position, box, type, alive, spectator, invulnerable, team, visibility inputs; blocks for one ray to each candidate | attack target; unseen count | `Q-NEAREST`, `Q-NEAREST-PLAYER` |
| hurt-by target | decision; the alert is a pair action | hurt-by record, kept time, owner | the attacker: alive, type, team; own type in a box: attack target, owner | own attack target; the attack target of each receiver | `Q-ALERT` |
| melee attack | start and path: decision; cooldown, reach, hit, look: reflex | attack target, path state, timers | the target: position, box, alive; blocks for the ray | path; look target; aggressive bit; a hit on the target | none |
| leap | decision | attack target, on-ground flag, velocity, passenger | the target position | velocity | none |
| ranged attack | path: decision; see count, attack time, attack: reflex | target, timers | the target: position, alive; blocks for the ray | path; look target; a projectile | none |
| avoid entity | decision | position, box, home | entities of the type set; blocks of the away search; the path search | path; speed modifier | `Q-AVOID`, `W(16, 7, p)` |
| anger update | reflex | anger state, stored target | the stored target: alive, kind; the anger target: game mode | anger state; attack target; entity data of the end time | none |
| reset universal anger | decision; the alert is a pair action | hurt-by record, kept time | own type in a box | anger state of the mob and of each member | `Q-ANGER-ALERT` |
| owner goals | decision | tame state, ordered to sit, owner reference | the owner: position, records, game mode, dimension | attack target; path; position at a teleport; sitting pose | none |
| beg | decision | position | player table: position, eye height, items in hands, alive, spectator, visibility inputs; blocks for rays | look target; interested flag | `Q-BEG` |

These notes follow from the table:

- Each goal writes only its own mob, except the melee hit, the projectile of a ranged attack, and
  the two alerts. The hit follows DMG-ORD-002. The alerts are pair actions (AIT-CPL-004).
- The snapshot must publish these values of each mob: the attack target, the owner reference, the
  team, the invulnerable flag and the type. A target goal of another mob reads them.
- The expensive work is a box enumeration of a type set, the rays, the away search and the path
  searches. A per-type index makes each empty box query a probe with no ray.

## 8. Intentional deviations

### CGOAL-DEV-001: The must-reach option of the nearest attackable target goal

- **Rule:** CGOAL-NEAR-006.
- **Reference:** the reference has the option, but the nearest attackable target goal does not
  read it. AIT-VAN-011 states a reach check for "a goal that must reach the target". That
  statement applies only to the single-candidate test.
- **Visible to the client:** no. This document keeps the reference behaviour. It records the
  difference to AIT-VAN-011, which a later edit of AIT must fix.
- **Test:** E-C3.

### CGOAL-DEV-002: Shortcuts that change the order of random draws

- **Rule:** CGOAL-NEAR-011, CGOAL-AVOID-007.
- **Reference:** the reference draws each gate in each examined evaluation, and the strength draw of
  an extra start test after the away search and the path search. It draws from one random source
  of the mob in the order of the goal lists.
- **Visible to the client:** no, as a distribution.
- **Test:** as AIT-DEV-002 and C-G6 of `mob-goals.md`.

### CGOAL-DEV-003: Ties by the order key

- **Rule:** GOAL-QRY-004 in `Q-NEAREST`, `Q-NEAREST-PLAYER`, `Q-AVOID` and `Q-BEG`.
- **Reference:** the first entity of the enumeration order wins a tie. GOAL-DEV-001 states the
  same deviation.
- **Visible to the client:** rarely.
- **Test:** accept each member at the smallest distance.

## 9. Unresolved and experiments

### 9.1 Unresolved

- **U1**: the knockback of a melee hit, and the extra knockback of the attribute
  `attack_knockback`, need `player-attack.md` or a mob attack specification.
- **U2**: teams and the ally relation are hooks. The combat test treats each mob as on no team.
- **U3**: the item component `attack_range`, and an item in the main hand of an attacker, are hooks.
- **U4**: AIT-VAN-011 must state that the must-reach option applies only to the single-candidate
  test (CGOAL-DEV-001).
- **U5**: no rule of this document has a captured result. The confidence of the timers rests on the
  source.

### 9.2 Experiments with a vanilla server

The setup of `mob-goals.md`, section 10.1, applies.

| Experiment | Steps | Expected result | Rules |
|------------|-------|-----------------|-------|
| E-C1 | Summon a wolf and a sheep 6 blocks apart, 400 times. Record the tick of the first `set_entity_data` with the aggressive bit. | The wait has a geometric law with a mean of about 10 ticks, plus up to 20 ticks for the melee check. | CGOAL-NEAR-004, CGOAL-MELEE-002 |
| E-C2 | Put a sheep 14 blocks above a wolf on a platform. | The wolf selects the sheep: the box spans 16 blocks up. | CGOAL-NEAR-005 |
| E-C3 | Put a llama behind a wall that blocks the path to a wolf. Record whether the llama targets the wolf. | The llama targets the wolf when it sees it, also with no path. | CGOAL-NEAR-006 |
| E-C4 | Let a wolf hunt a sheep. Record each hit tick and each `swing_animation`. | The hits are at least 20 ticks apart. The sheep dies at the second hit. | CGOAL-MELEE-004, CGOAL-MELEE-007 |
| E-C5 | Hit one wild wolf of a group of 5 with a player. | Each wild wolf within the alert box with no attack target targets the player in the same tick. | CGOAL-HURT-003, CGOAL-HURT-004 |
| E-C6 | Put a wild wolf 10 blocks from a llama with `Strength:1`, 400 times. Record the start of the flight. | The start rate per examined evaluation, given a path, is 2/5. | CGOAL-AVOID-004, CGOAL-AVOID-005 |
| E-C7 | Hold a bone 7 blocks from a wolf. Then let a second player with no bone stand 3 blocks from the wolf. | The wolf begs to the first player only while the second player is out of range or out of sight. | CGOAL-OWNER-007, CGOAL-OWNER-008 |
| E-C8 | Let a llama spit at a wolf. Record the attack time between spits. | The spits are 40 ticks apart. The first spit comes 41 ticks after the start. | CGOAL-RANGED-002 |
