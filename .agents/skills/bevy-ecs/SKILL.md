---
name: bevy-ecs
description: >-
  Use when designing or reviewing Bevy 0.19 ECS state: Components, Resources,
  Messages, observers, markers, caches, Changed<T> usage, sub-app extraction or
  synchronization logic. Separates authoritative truth, cheap derived state,
  materialized projections/caches, and temporal facts. Mutate authoritative
  truth from actions; derive consequences centrally; materialize only when
  recomputation or structural querying justifies it; use messages/events only
  when current state cannot reconstruct the required fact. Includes a
  classification test, 15 design rules, recurring defects in simulation code,
  worked examples, Bevy change-detection cost caveats, and a review checklist.
  Triggers: component design, derived state, source of truth, state duplication,
  resync bug, flag soup, state machine, marker vs enum, cache, projection,
  Changed<T>, Added<T>, RemovedComponents, work queue, dirty set, events vs
  messages, observer, relationships, reverse index, archetype churn, sub-app
  extract, broadcast encoding, save/load state, ECS design rules.
metadata:
  version: "3.1.0"
  bevy: "0.19.1"
  domain: specialized
  scope: design-and-review
  related-skills: bevy-developer
---

# Data-Oriented State & ECS Design Rules (Bevy)

The recurring failure mode this skill targets is **hand-synchronized state**:
one fact changes, several consequences are stored separately, and every action
that changes the fact must remember to update all of them.

The fix is not "never cache" or "everything must be an event." The fix is to
classify state correctly, choose one authoritative representation for each fact,
and make every projection from that truth have exactly one owner.

This skill targets modern Bevy ECS semantics, including Bevy 0.19 behavior for
change detection, deferred `Commands`, `Message`, observer `Event`, and
`RemovedComponents`.

---

## Core mental model

Do not begin with methods such as "damage", "heal", "die", "switch mode", or
"start animation". Begin with the data model.

Every piece of runtime information belongs to one of four categories:

### 1. Authoritative truth

State that cannot be reconstructed from other retained authoritative state
inside the chosen system boundary.

Examples:

- current HP;
- velocity;
- inventory contents;
- selected game mode;
- cooldown end time;
- player input sampled for the current simulation tick.

Authoritative truth is the state actions are allowed to mutate.

### 2. Derived state

A value that can be deterministically reconstructed from authoritative truth and
is cheap enough to compute when needed.

Examples:

- `health.current / health.max`;
- `health.current <= 0.0`;
- whether a cooldown has expired;
- which animation semantic should be active;
- whether an ability is castable.

Derived state should normally be computed on read or by a centralized deriving
system. It is not an independent source of truth.

### 3. Materialized derived state

A derived value intentionally stored because recomputation, indexing, query
selectivity, cross-system reuse, or external synchronization makes storage
worthwhile.

Examples:

- a `Dead` marker derived from `Health` so hot queries can use `With<Dead>`;
- a navigation cache derived from world geometry;
- a visibility set;
- a column or face cache;
- a precomputed animation classification;
- a replicated snapshot generated from authoritative simulation state.

A materialized value is **not truth merely because it is stored**. It is a
projection/cache/index. It must have one authoritative derivation path and a
defined invalidation/update rule.

### 4. Temporal facts

Information about **what happened**, not merely what is true now.

Examples:

- entity A dealt 25 damage to entity B;
- player X killed player Y;
- an item was picked up;
- a sound-worthy impact happened;
- an achievement condition was crossed.

Two different histories may produce the same current state. If the consumer
needs information that current authoritative state cannot reconstruct, the fact
is legitimately event-like.

---

## Actions are transformations, not state

An action is something that changes authoritative truth:

```text
Damage
Heal
Teleport
SwitchGameMode
StartCooldown
```

The preferred flow is:

```text
Action
  |
  v
Authoritative truth
  |
  +--> cheap derived state
  |
  +--> materialized projection/cache/index
  |
  +--> temporal facts, when history itself matters
```

The architectural error is:

```text
Action
  |- mutate truth
  |- update animation
  |- toggle several flags
  |- update UI
  |- update cache
  |- emit "state changed" notifications
  `- remember every other consequence
```

That turns every action handler into a synchronization protocol.

---

# The classification test

Before adding a field, component, resource, cache, or event, run these questions
in order.

## Test A — Can it be reconstructed?

> If I delete this value, can I reconstruct it deterministically from state whose
> lifetime I actually guarantee?

If **no**, it is probably authoritative truth or a temporal fact.

If **yes**, continue.

## Test B — Is recomputation cheap enough?

> Is recomputing it at each relevant read cheaper and simpler than storing and
> invalidating it?

If **yes**, keep it derived and compute it.

If **no**, it may deserve materialization.

## Test C — Does structural storage buy something?

> Does storing it as a component/resource materially improve query selectivity,
> indexing, reuse, interoperability, replication, or algorithmic complexity?

If **yes**, materialize it deliberately.

## Test D — Is this information about state or history?

> Can every consumer reconstruct everything it semantically needs from current
> authoritative state and other guaranteed retained data?

If **yes**, prefer derivation/change detection over a semantic event.

If **no**, the event/message represents a legitimate temporal fact.

The important distinction is:

```text
"HP is 75"             -> state
"HP changed"           -> usually derivable notification
"A hit for 25 occurred"-> temporal fact
```

---

# Bevy mapping

| Concept | Typical Bevy mechanism |
|---|---|
| Authoritative truth | minimal `Component` / `Resource` |
| Action | system, command, observer, input/message consumer that writes truth |
| Cheap derived state | pure function or system reading truth |
| Materialized projection | component/resource/cache maintained by one projection system |
| Change observation | `Changed<T>`, `Added<T>`, `Ref<T>`, lifecycle APIs |
| Structural query index | marker/tag component + `With<T>` / `Without<T>` |
| Buffered temporal fact | `Message<T>` |
| Immediate targeted temporal fact | observer `Event` / trigger |
| Removal observation | `RemovedComponents<T>` or lifecycle observer |
| Relationship | Bevy relationships such as `ChildOf` or custom relationships |

These are mechanisms, not categories. A `Component` can hold authoritative truth
or a materialized projection; storage location alone does not determine meaning.

---

# The 15 rules

## Rule 1 — Component granularity follows access patterns

Do not split components because "small is always better", and do not combine them
because they belong to the same domain object.

Group data that is commonly read and mutated together. Split data with different
access frequency, lifetime, storage strategy, or query membership.

Questions to ask:

- which systems read these fields together?
- which systems mutate them together?
- how often do they change?
- how many entities carry them?
- does splitting improve hot-query width or create needless joins?

Data-oriented design optimizes **layout for access**, not struct aesthetics.

---

## Rule 2 — Components may have methods; they must not own world orchestration

The useful boundary is not "components contain zero logic."

Pure helpers, constructors, validation, and derived accessors are fine:

```rust
#[derive(Component)]
struct Health {
    current: f32,
    max: f32,
}

impl Health {
    fn fraction(&self) -> f32 {
        self.current / self.max
    }

    fn is_dead(&self) -> bool {
        self.current <= 0.0
    }
}
```

What should not live in a component method is behavior requiring ECS/world
orchestration:

```text
find other entities
spawn/despawn
query the world
send unrelated notifications
update several external projections
```

That behavior belongs in systems, commands, or observers.

---

## Rule 3 — One authoritative representation per fact

Do not represent one semantic fact independently in several mutable places.

Bad:

```text
Health.current
Dead(bool)
CanMove(bool)
AnimationState::Death
```

when all three latter values are independently written from actions.

Better:

```text
Health.current                  authoritative
Health::is_dead()               derived
Dead marker                     optional materialized projection
AnimationState                  optional materialized projection
```

Each projection must derive from the same authoritative source.

---

## Rule 4 — Run the classification test on every stored field

If a field can be reconstructed, do not silently promote it to truth merely
because storing it is convenient.

Choose explicitly:

```text
derive on read
materialize as cache/index
retain as authoritative truth
retain as temporal history
```

The classification should be visible in code structure and ownership.

---

## Rule 5 — Derive cheap consequences instead of synchronizing them

If a value is cheap to compute, compute it.

```rust
fn can_cast(cd: &Cooldown, now: Duration) -> bool {
    now >= cd.ends_at
}
```

Do not introduce a mutable `can_cast: bool` unless measurement or query semantics
justify materialization.

Every independently mutable duplicate creates a consistency obligation.

---

## Rule 6 — Materialized derived state has one projection owner

Caching is allowed. Structural marker components are allowed. Precomputed
classifications are allowed.

The rule is:

> A materialized projection is written from exactly one derivation path.

Do not let damage, healing, resurrection, cheats, loading, and admin commands all
toggle `Dead` themselves.

Instead:

```text
all actions -> Health -> one health-to-dead projection
```

The projection must handle **both directions** and every relevant lifecycle path.

---

## Rule 7 — Actions mutate authoritative truth, not its projections

An action may create a legitimate temporal fact, but it should not manually
synchronize reconstructable consequences.

Bad:

```rust
fn apply_damage(/* ... */) {
    health.current -= damage;
    commands.entity(entity).insert(Dead);
    animation.set(AnimationState::Death);
    health_bar.fill = health.current / health.max;
}
```

Better:

```rust
fn apply_damage(/* ... */) {
    health.current -= damage;
}
```

Other systems derive or materialize consequences from `Health`.

Litmus test:

> If a new action changes this authoritative state, do all existing consequences
> remain correct without editing the new action?

If not, derivation is still distributed.

---

## Rule 8 — Centralize derivation; choose the invalidation mechanism by cost

"Use `Changed<T>`" is not a universal rule.

Possible mechanisms include:

- compute on read;
- `Changed<T>` / `Added<T>`;
- `Ref<T>::is_changed()`;
- observer/lifecycle event;
- buffered `Message<T>`;
- `RemovedComponents<T>`;
- dirty bit inside a value, or a dirty set in a resource (a dirty *marker
  component* is a structural change per mark; avoid it on hot paths);
- explicit work queue/change list;
- periodic recomputation;
- domain-specific incremental index.

Choose based on:

- candidate entity count;
- mutation frequency;
- derivation cost;
- latency requirements;
- whether structural filtering helps;
- whether history is required.

The invariant is centralized ownership of the derivation, not a specific Bevy API.

---

## Rule 9 — `Changed<T>` is change detection, not archetype-level skipping

In Bevy, `Changed<T>` and `Added<T>` are not archetypal filters.

A query such as:

```rust
Query<&Health, Changed<Health>>
```

can still scan all entities matching the structural part of the query and check
their change ticks, even when very few changed.

By contrast:

```rust
Query<&Health, With<Dead>>
```

can filter at the archetype level.

Therefore do not assume:

```text
Changed<T> == process only changed entities at O(changes)
```

For very high cardinality + sparse mutations, an explicit change list, observer,
message, dirty index, or different data layout may be cheaper.

Profile the actual workload.

---

## Rule 10 — Marker component vs enum/value depends on churn and query selectivity

A marker component is valuable when its presence is semantically useful to
structural queries:

```rust
#[derive(Component)]
struct Dead;
```

Benefits:

- `With<Dead>` / `Without<Dead>` are archetypal filters;
- absence/presence naturally represents membership;
- hot systems can avoid unrelated archetypes.

Cost:

- insertion/removal is a structural change;
- frequent toggling can cause archetype churn;
- mutually exclusive states spread across several tags can become invalid.

Use a marker when state changes relatively infrequently and structural filtering
has real value.

Use an enum/value when state changes frequently or consumers normally inspect the
value after already matching the entity.

`#[component(storage = "SparseSet")]` makes insert/remove cheaper, but the
entity still changes archetype. It lowers the cost of a marker; it does not turn
a frequently flipped marker into a value.

For one mutually exclusive variable, never encode illegal combinations with
several independent booleans.

---

## Rule 11 — Events/messages represent temporal information, not redundant state

Use this smell test:

> Can the receiver reconstruct everything it semantically needs from current
> authoritative state and guaranteed retained data?

If yes, a semantic event is usually redundant.

Example:

```text
Health changed -> UI recomputes fill from Health
```

If no, a temporal fact is legitimate.

Example:

```text
DamageApplied {
    target,
    source,
    amount,
    damage_type,
}
```

Current `Health` cannot reconstruct who caused the hit, its amount, ordering, or
whether multiple hits produced the same final HP.

Use buffered `Message<T>` when batch/pull semantics fit. Use observer `Event`
when immediate targeted reaction fits.

Do not use events merely to keep two mutable representations synchronized.

---

## Rule 12 — System order and deferred visibility are contracts

If one system produces truth or a projection and another consumes it in the same
tick, express the dependency.

Use:

- named `SystemSet`s;
- `.before()` / `.after()`;
- `.chain()`;
- explicit schedule boundaries;
- explicit deferred-command synchronization when required.

Remember that `Commands` are deferred. A component inserted or removed through
`Commands` is not necessarily visible to the immediately following system until
deferred operations have been applied.

Avoid designs whose correctness depends on accidental scheduler order.

---

## Rule 13 — Prefer declarative execution gates when the gate is shared state

If a system should not run at all outside some state, use a run condition,
schedule/state boundary, or a structurally selective query rather than repeating:

```rust
if !enabled {
    return;
}
```

inside many systems.

A local early return is still fine when it is genuinely local control flow.
The rule is about centralized lifecycle/execution policy, not banning `if`.

---

## Rule 14 — Use relationships and required structure when they model the data

Prefer Bevy's relationship facilities for actual entity relationships rather than
maintaining parallel bespoke parent/child graphs without need.

Use Required Components or construction APIs when they establish invariants that
must hold whenever a component exists.

Do not force all domain graphs into hierarchy APIs: choose the representation
that matches traversal, ownership, mutation, and cardinality requirements.

---

## Rule 15 — Optimize materialization only after naming its cost model

Every materialized projection trades recomputation cost for:

- memory;
- invalidation complexity;
- update work;
- possible structural churn;
- schedule dependencies;
- save/load semantics;
- replication semantics.

Before materializing, state why:

```text
recomputation is expensive
hot queries need structural selectivity
many consumers reuse the result
external systems require a stable snapshot
incremental maintenance is asymptotically cheaper
```

Then measure.

"Cache because it feels faster" and "split because ECS likes small components"
are not performance arguments.

---

# Worked example 1 — Animation derives from authoritative movement and health

Do not store independent `walking`, `running`, and `dead` booleans.

```rust
use bevy::prelude::*;

#[derive(Component, Default)]
struct Velocity(Vec3);

#[derive(Component)]
struct Health {
    current: f32,
    max: f32,
}

#[derive(Component, PartialEq, Eq)]
enum AnimState {
    Idle,
    Walk,
    Run,
    Death,
}

fn drive_animation(
    mut q: Query<
        (&Velocity, &Health, &mut AnimState),
        Or<(Changed<Velocity>, Changed<Health>)>,
    >,
) {
    for (velocity, health, mut anim) in &mut q {
        let speed = velocity.0.length();

        let next = if health.current <= 0.0 {
            AnimState::Death
        } else if speed < 0.01 {
            AnimState::Idle
        } else if speed < 4.0 {
            AnimState::Walk
        } else {
            AnimState::Run
        };

        anim.set_if_neq(next);
    }
}
```

Here:

```text
Velocity          authoritative
Health            authoritative
semantic anim     derived
AnimState         materialized derived state
```

`AnimState` is stored because a renderer/animation graph may need a stable,
reusable classification. Only `drive_animation` owns that projection.

A resurrection is automatically handled because `Health` changing from `0` to a
positive value reruns the same projection. No special "resurrection animation
sync" code is required.

For very large entity counts with sparse changes, benchmark whether
`Changed<Velocity>` / `Changed<Health>` is the right invalidation mechanism.

---

# Worked example 2 — `Dead` as a materialized structural index

Sometimes `Dead` is worth storing even though death is derivable from `Health`,
because many hot queries benefit from `With<Dead>` / `Without<Dead>`.

That makes `Dead` a **materialized projection**, not authoritative truth.

```rust
use bevy::prelude::*;

#[derive(Component)]
struct Health {
    current: f32,
    max: f32,
}

#[derive(Component)]
struct Dead;

fn project_dead_state(
    mut commands: Commands,
    q: Query<(Entity, &Health, Has<Dead>), Changed<Health>>,
) {
    for (entity, health, has_dead) in &q {
        let should_be_dead = health.current <= 0.0;

        match (should_be_dead, has_dead) {
            (true, false) => {
                commands.entity(entity).insert(Dead);
            }
            (false, true) => {
                commands.entity(entity).remove::<Dead>();
            }
            _ => {}
        }
    }
}
```

The critical property is bidirectionality:

```text
alive -> dead
dead  -> alive
```

Damage, poison, healing, resurrection, loading, cheats, and admin commands only
modify `Health`. None of them own `Dead`.

If another same-tick system must observe the newly inserted/removed marker, its
ordering must also account for deferred `Commands`.

If death membership changes extremely frequently, benchmark whether an enum/bool
value is cheaper than structural churn.

---

# Worked example 3 — State change vs temporal fact

Health-bar state is reconstructable:

```rust
fn sync_health_bar(
    q: Query<&Health, Changed<Health>>,
    // ...
) {
    for health in &q {
        let fill = health.current / health.max;
        // write UI projection
    }
}
```

A combat hit is not necessarily reconstructable:

```rust
#[derive(Message)]
struct DamageApplied {
    target: Entity,
    source: Entity,
    amount: f32,
}
```

These histories can end at the same HP:

```text
100 -> 75
100 -> 90 -> 75
```

Current `Health { current: 75, ... }` cannot tell a combat log, hit sound,
achievement system, or kill-assist system which history occurred.

Therefore:

```text
health bar fill   -> derived from state
"damage happened" -> legitimate temporal fact
```

Do not replace every event with `Changed<T>`, and do not emit an event merely
because some derived state needs refreshing.

---

# Worked example 4 — Cooldown stores the minimum authoritative fact

Avoid synchronizing:

```text
can_cast bool
remaining time
icon fill
timer object
```

when all are consequences of the same deadline.

```rust
use std::time::Duration;

#[derive(Component)]
struct Cooldown {
    ends_at: Duration,
}

fn can_cast(cd: &Cooldown, now: Duration) -> bool {
    now >= cd.ends_at
}

fn remaining(cd: &Cooldown, now: Duration) -> Duration {
    cd.ends_at.saturating_sub(now)
}

fn cast(cd: &mut Cooldown, now: Duration, duration: Duration) {
    cd.ends_at = now + duration;
}

fn interrupt(cd: &mut Cooldown, now: Duration) {
    cd.ends_at = now;
}
```

`ends_at` is authoritative inside this model. Castability and remaining duration
are derived.

If the application retains a complete authoritative command/event log instead,
the system boundary changes: the current cooldown may itself become a projection
of history. "Truth" is always relative to the reconstruction boundary the
application actually guarantees.

---

# Recurring defects in simulation code

These six shapes appear again and again in Bevy code that grows past a few
hundred entities. Each one compiles, passes tests, and costs work proportional
to the population instead of the change.

## D1 — `Changed<T>` used as a work queue

```rust
fn rebuild(touched: Query<&TilePos, Or<(Changed<Terrain>, Changed<Occupant>)>>) { /* ... */ }
```

over every tile in a large map, run every frame. The producer already knows
which tiles it touched and often already emits a message for them.

Fix: the producer writes the touched ids into a `Message` or a dirty set in a
resource. The consumer iterates those ids (`iter_many`, `get_mut`) and is gated
with `run_if(on_message::<..>)`. Keep `Changed<T>` for small populations.

The same defect without a producer: "re-test every pending entry every frame
because there is no signal". The missing signal is the bug; add it.

## D2 — False change from a `&mut` deref

```rust
for mut bounds in &mut bounds_q {
    bounds.grow_to(point); // derefs &mut even when the bounds already contain it
}
```

Every `&mut T` deref marks `T` changed. Downstream `Changed<T>` consumers then
redo their work, sometimes cloning a whole value into another world.

Fix: compute first, write only on a difference (`set_if_neq`, or
`bypass_change_detection()` followed by `set_changed()` when the value really
moved). Narrow a `Mut` to one field with `map_unchanged`.

## D3 — Exclusive system as a drain or an I/O step

```rust
fn spawn_connections(world: &mut World) { /* drain a channel */ }
fn autosave(world: &mut World) { /* write files synchronously */ }
```

Both stop every other system in the schedule on every run, and the second also
blocks the schedule on disk.

Fix: a normal system holds the receiver in a resource and spawns through
`Commands`. Disk work runs on `IoTaskPool`; another system collects finished
tasks with `check_ready`. Keep `&mut World` for real world-wide access:
extraction between worlds, a queued command, driving a sub-app, scene or save
application, a test fixture.

## D4 — `Old*` twins and hand-maintained reverse indices

```text
Transform        authoritative
OldTransform     copied from Transform for every entity every frame
InRoom           authoritative
OldInRoom        copied to diff against
RoomOccupants    reverse index, rebuilt by scanning every entity
```

The copies are materialized temporal facts with no reason to exist outside the
one system that needs the difference. Writing them every frame also marks every
entity changed.

Fix: the consumer keeps the previous value in its own `Local` or in a field it
alone owns, updated only for changed entities. A reverse index is a Bevy
relationship (`#[relationship]` / `#[relationship_target]`) when edits are
rare, or one resource with one owner fed by messages when they are frequent.

Two projections of the same fact with different parameters (two "who can see
whom" sets with different radii) are the same defect at a larger scale: pick
one, derive the other or delete it.

## D5 — Clone per recipient

```rust
for recipient in recipients {
    queue_of(recipient).push(payload.clone()); // deep clone, then encoded again
}
```

A broadcast is cloned once per recipient and encoded once per recipient.

Fix: encode once into an immutable byte buffer and give each recipient a handle
to it. Per-recipient work is limited to what really differs per recipient
(compression state, encryption).

## D6 — Extract by re-insert

```rust
sub_app.set_extract(|main, sub| {
    sub.insert_resource(main.resource::<Clocks>().clone()); // every update
});
```

Allocates every update, marks the resource changed every update, and so defeats any
`resource_changed` gate in the sub-app.

Fix: keep a version next to the value in the main app (one writer). Extraction
compares versions and copies only when it differs. A sub-app never advances its
own copy of global state.

---

# Anti-pattern catalog

Watch for:

- one semantic fact independently mutable in several places;
- bool soup encoding mutually exclusive states;
- action handlers that update truth and several consequences;
- cached/materialized state with multiple writers;
- one-way projection code that handles insertion but not removal;
- `Changed<T>` assumed to be O(number of changes);
- events that merely say "recompute state you already have";
- state used to reconstruct temporal history that has already been discarded;
- marker components toggled at high frequency without measuring churn;
- components split by taxonomy instead of access pattern;
- components forbidden from harmless pure methods by dogma;
- unordered projection/consumer systems;
- same-tick logic that forgets `Commands` are deferred;
- custom parallel representations of relationships with no measured need;
- caches with no explicit invalidation owner.

---

# Review checklist

- [ ] For every stored value: authoritative, derived, materialized derived, or temporal fact?
- [ ] If a value is reconstructable, is there a concrete reason to store it?
- [ ] Does every semantic fact have exactly one authoritative representation?
- [ ] Does every materialized projection have exactly one writer/owner?
- [ ] Does the projection handle every direction and relevant lifecycle path?
- [ ] Could a new action mutate the truth without learning about its projections?
- [ ] Are component boundaries based on access patterns rather than domain taxonomy?
- [ ] Are markers used because structural query selectivity is valuable?
- [ ] Is marker churn acceptable for the mutation frequency?
- [ ] Is an enum/value better for frequently changing mutually exclusive state?
- [ ] Is `Changed<T>` being treated correctly as non-archetypal change detection?
- [ ] Would a change list, observer, dirty set, or queue be cheaper at this cardinality?
- [ ] Does any writer deref `&mut` without a real change (D2)?
- [ ] Does any system take `&mut World` without needing world-wide access, or block on I/O (D3)?
- [ ] Is there an `Old*` copy, a hand-built reverse index, or a second projection of the same fact (D4)?
- [ ] Is a broadcast cloned or encoded per recipient (D5)?
- [ ] Does a sub-app extract copy state every update instead of on a version change (D6)?
- [ ] Can every event consumer reconstruct the same information from current state?
- [ ] If not, does the event clearly represent a temporal fact?
- [ ] Are same-tick producer/consumer dependencies ordered explicitly?
- [ ] Does the design account for deferred `Commands` visibility?
- [ ] Are shared execution gates expressed declaratively when useful?
- [ ] Are relationships represented using Bevy facilities when they match the domain?
- [ ] Does every cache/materialization have a stated cost-model reason?
- [ ] Has the relevant hot path been profiled instead of optimized by folklore?

---

# Decision table

| Situation | Default choice |
|---|---|
| Cannot reconstruct from retained state | authoritative truth |
| Reconstructable and cheap | derive on read |
| Reconstructable, expensive, reused | materialized cache |
| Reconstructable but hot systems need membership filtering | materialized marker/index |
| Mutually exclusive state changes frequently | enum/value field |
| Rare membership state used heavily in filters | marker component |
| Need to react to ordinary state mutation | derive/change detection |
| Need O(changes)-like sparse work at huge cardinality | consider queue/observer/change list; benchmark |
| Need historical fact not recoverable from current state | `Message` / observer `Event` |
| Need same-tick structural change visibility | explicit ordering + deferred sync |
| Need a hierarchy/relationship | Bevy relationship if its semantics fit |

---

# Do not over-apply

This skill is not an argument for:

- zero cached data;
- zero events;
- zero methods on components;
- the smallest possible components;
- marker components everywhere;
- `Changed<T>` everywhere;
- eliminating state machines;
- avoiding all denormalization.

The objective is narrower:

```text
one authoritative representation per fact
+
explicit derivation/materialization
+
one owner per projection
+
events only when temporal information matters
+
mechanisms chosen from the actual cost model
```

A deliberately materialized cache with one owner is good design.
An independently mutable duplicate with five writers is not.

---

# Sources and further reading

- Bevy 0.19 release notes — https://bevy.org/news/bevy-0-19/
- Bevy `Changed<T>` documentation — https://docs.rs/bevy/latest/bevy/prelude/struct.Changed.html
- Bevy `ArchetypeFilter` documentation — https://docs.rs/bevy/latest/bevy/ecs/query/trait.ArchetypeFilter.html
- Bevy `RemovedComponents<T>` documentation — https://docs.rs/bevy/latest/bevy/prelude/struct.RemovedComponents.html
- Bevy ECS documentation — https://docs.rs/bevy/latest/bevy/ecs/
- Sander Mertens, ECS FAQ — https://github.com/SanderMertens/ecs-faq
- Sander Mertens, writing on ECS state machines, relationships, and "games as databases"
- Kent C. Dodds, "Don't Sync State. Derive It!" — https://kentcdodds.com/blog/dont-sync-state-derive-it
