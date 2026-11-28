---
name: bevy-ecs
description: >-
  Use when designing or reviewing Bevy 0.20 ECS state: Components, Resources,
  Messages, observers, markers, caches, Changed<T> usage, sub-app extraction or
  synchronization logic. Separates authoritative truth, cheap derived state,
  materialized projections and temporal facts. Includes a classification test,
  design rules, recurring defects in simulation code and a review checklist.
  Triggers: component design, derived state, source of truth, state duplication,
  resync bug, flag soup, marker vs enum, cache, projection, Changed<T>,
  RemovedComponents, work queue, dirty set, events vs messages, observer,
  relationships, reverse index, archetype churn, sub-app extract, broadcast
  encoding, ECS design rules.
metadata:
  bevy: "0.20.1"
  scope: design-and-review
  related-skills: bevy-developer
---

# ECS state design

The failure this skill targets is hand-synchronized state: one fact changes,
several consequences are stored apart, and every action must remember to update
all of them. Classify each value, keep one authoritative representation per
fact, and give each projection exactly one owner.

## Four kinds of state

1. **Authoritative truth.** Cannot be rebuilt from other retained state:
   current HP, velocity, inventory, cooldown end time, sampled input. Actions
   mutate only this.
2. **Derived state.** Deterministic and cheap to compute from truth: HP
   fraction, "is dead", "cooldown expired". Compute on read or in one central
   system.
3. **Materialized derived state.** Stored on purpose because recomputation,
   indexing, query selectivity, reuse or replication pays for it: a `Dead`
   marker for `With<Dead>` queries, a navigation cache, a visibility set. It is
   a projection with one derivation path and a stated update rule, never truth.
4. **Temporal facts.** What happened, not what is true now: "A hit B for 25".
   Two histories can give the same current state; a consumer that needs the
   history needs a `Message` or an observer event.

Actions (damage, heal, teleport) write truth only. A handler that also toggles
flags, updates animation, UI and caches and emits "changed" notifications is a
synchronization protocol.

## Classification test

Run it on every field, component, resource, cache or event, in order.

- **A.** If I delete the value, can I rebuild it from state whose lifetime I
  guarantee? No: truth or temporal fact. Yes: continue.
- **B.** Is recomputing at each read cheaper and simpler than storing and
  invalidating? Yes: derive it. No: continue.
- **C.** Does storage buy query selectivity, indexing, reuse, replication or a
  better complexity class? Yes: materialize it deliberately. Name the cost
  model: why recomputation is expensive, which consumers reuse it, who owns it.
- **D.** Can every consumer rebuild what it needs from current state? Yes:
  derive or use change detection, not an event. No: the event is a temporal
  fact.

"HP is 75" is state. "HP changed" is usually derivable. "A hit for 25 occurred"
is a temporal fact.

## Rules

1. **Group components by access pattern.** Group data read and written
   together. Split data with different frequency, lifetime, storage or query
   membership. Taxonomy is not a reason.
2. **Components may have pure methods** (constructors, validation, derived
   accessors). World orchestration (find entities, spawn, query, notify) belongs
   in systems, commands or observers.
3. **One authoritative representation per fact.** `Health.current` is truth;
   `is_dead()` is derived; a `Dead` marker or an animation state is an optional
   projection. Never independent mutable `Dead`, `CanMove` and `AnimationState`
   flags written by actions.
4. **Derive cheap consequences.** `now >= cooldown.ends_at` is a function, not
   a mutable `can_cast` bool.
5. **One projection owner.** Damage, healing, resurrection, cheats, loading and
   admin commands all write `Health`; one system projects `Health` to `Dead` in
   both directions and over every lifecycle path.
6. **Litmus test for actions.** If a new action changes the truth, do all
   existing consequences stay correct without editing it? If not, derivation is
   still distributed.
7. **Pick the invalidation mechanism by cost.** Options: compute on read,
   `Changed`/`Added`, `Ref::is_changed`, lifecycle observer, `Message`,
   `RemovedComponents`, a dirty bit in a value or a dirty set in a resource,
   an explicit work queue, periodic recompute, an incremental index. A dirty
   marker component is a structural change per mark; avoid it on hot paths. The
   invariant is central ownership of the derivation, not one API.
8. **`Changed<T>` is not an archetype filter.** `Query<&T, Changed<T>>` visits
   every entity with `T` and checks ticks. `With<Dead>` filters by archetype.
   Sparse changes over a large population take a change list, message, observer
   or dirty set.
9. **Marker versus value.** A marker pays when membership is rare and filtered
   often (`With`/`Without` are archetype filters). Insert and remove are
   structural changes; frequent toggling churns archetypes. State that flips
   often is a value, an enum field. `SparseSet` storage makes the flip cheaper
   but still moves the archetype. Never encode one mutually exclusive variable
   as several independent bools.
10. **Events carry temporal facts.** Do not use an event to keep two mutable
    representations in sync or to say "recompute what you already have". Use a
    buffered `Message` for batch pull and an observer `Event` for an immediate
    targeted reaction.
11. **Order and deferral are contracts.** A same-tick producer and consumer get
    a named `SystemSet` or `.before`/`.after`/`.chain`. `Commands` apply later;
    an insert is not visible to the next system until the sync point.
12. **Shared gates are run conditions**, not a repeated `if !enabled { return }`
    in many systems. A local early return is fine for local control flow.
13. **Use relationships and required components** for real entity relations and
    invariants that hold whenever a component exists. Do not force every graph
    into the hierarchy.
14. **Name the cost model before you materialize**: memory, invalidation, update
    work, structural churn, schedule edges, save/load and replication semantics.
    "Feels faster" and "ECS likes small components" are not reasons. The
    measurement goes in the pull request.

## Recurring defects in simulation code

Each compiles and passes tests, and costs work proportional to the population
instead of the change.

- **D1. `Changed<T>` as a work queue.** A rebuild over every tile each frame
  with `Or<(Changed<A>, Changed<B>)>`. The producer knows what it touched: it
  writes ids to a `Message` or a dirty set; the consumer uses `iter_many` or
  `get_mut` and is gated by `on_message`. "Re-test every pending entry each
  frame" lacks a signal: add the signal.
- **D2. False change from a `&mut` deref.** `bounds.grow_to(p)` marks `T`
  changed even when nothing moved, so downstream `Changed<T>` consumers redo
  work or clone a whole value into another world. Compute first and write on a
  difference (`set_if_neq`, or `bypass_change_detection` plus `set_changed`).
  Narrow a `Mut` with `map_unchanged`.
- **D3. Exclusive system as a drain or an I/O step.** `fn(world: &mut World)`
  that drains a channel or writes files stops every other system. Hold the
  receiver in a resource and spawn through `Commands`. Run disk work on
  `IoTaskPool` and collect with `check_ready`. `&mut World` is for seams only.
- **D4. `Old*` twins and hand-kept reverse indices.** `OldTransform` copied
  every frame marks every entity changed. The consumer keeps the previous value
  in its own `Local` and updates only changed entities. A reverse index is a
  Bevy relationship when edits are rare, or one resource with one owner fed by
  messages when frequent. Two projections of one fact with different
  parameters: keep one, derive or delete the other.
- **D5. Clone per recipient.** A broadcast cloned and encoded per recipient.
  Encode once into an immutable buffer and share a handle. Per-recipient work
  is only what differs (compression state, encryption).
- **D6. Extract by re-insert.** `sub.insert_resource(main.clone())` every
  update allocates and marks the resource changed, defeating `resource_changed`
  gates. Keep a version beside the value, one writer in the main app. Extract
  copies on a version change. A sub-app never advances its own copy of global
  state.

## Review checklist

- Is every stored field classified (derive, materialize, truth, history)?
- Is one fact stored once? Any bool soup or independently mutable duplicate?
- Does every projection have one owner, handle both directions and every
  lifecycle path (spawn, load, removal)?
- Could a new action change the truth without learning about projections?
- Are components split by access pattern, markers used for selectivity, and
  frequently flipped state a value?
- Is `Changed<T>` treated as non-archetypal? Would a change list, queue or
  dirty set be cheaper at this cardinality?
- D2: any `&mut` deref without a real change?
- D3: any `&mut World` system without need, or blocking I/O?
- D4: any `Old*` copy, hand-built reverse index or second projection?
- D5: any per-recipient clone or encode of a broadcast?
- D6: any sub-app extract that copies every update?
- Can an event consumer rebuild the same information from state? If not, is the
  event a temporal fact?
- Are same-tick dependencies ordered, deferred `Commands` accounted for, shared
  gates declarative?
- Does every cache have a stated cost-model reason and an invalidation owner?

## Decision table

| Situation | Choice |
|---|---|
| Cannot rebuild from retained state | authoritative truth |
| Rebuildable and cheap | derive on read |
| Rebuildable, expensive, reused | materialized cache |
| Rebuildable, hot systems filter on membership | marker or index |
| Mutually exclusive state flips often | enum field |
| Rare membership used heavily in filters | marker component |
| React to ordinary state mutation | derive or change detection |
| Sparse work at huge cardinality | queue, observer or change list |
| History not recoverable from state | `Message` or observer `Event` |
| Hierarchy or relation | Bevy relationship if semantics fit |

## Do not over-apply

This skill is not an argument for zero caches, zero events, zero component
methods, the smallest components, markers or `Changed<T>` everywhere. A cache
with one owner is good design. An independently mutable duplicate with five
writers is not.
