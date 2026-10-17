# Minecraft Server Architecture Context

## Purpose

This document captures the current architectural decisions for a high-performance Minecraft-compatible server implemented in Rust using Bevy ECS.

The goal is **not** to reproduce the internal architecture or algorithms of the vanilla Minecraft server.

The goal is to:

- reproduce the required observable Minecraft behavior;
- avoid inheriting implementation mistakes, historical constraints, and accidental complexity from vanilla;
- design the server around Rust, data-oriented architecture, ECS, parallel execution, spatial partitioning, and explicit dependency tracking;
- allow substantially different internal algorithms as long as required gameplay semantics remain compatible;
- make intentional deviations from vanilla explicit rather than accidental.

This is best described as **behavior-driven reimplementation** rather than a literal port.

---

# 1. Core Principle

Treat Minecraft as a behavioral specification, not as an implementation specification.

For every subsystem:

1. Determine what behavior is externally or gameplay-observably required.
2. Separate semantic requirements from vanilla implementation details.
3. Identify ordering, causality, and numerical constraints.
4. Design an independent algorithm optimized for Rust and modern hardware.
5. Verify behavior using differential and property-based testing.
6. Benchmark the independent implementation separately.

The server should preserve **behavioral contracts**, not Java control flow.

---

# 2. Compatibility Levels

Every feature should explicitly declare a compatibility policy.

Suggested modes:

```text
exact
semantic
intentional_deviation
```

## exact

Used when observable behavior must match vanilla very closely.

Examples:

- network protocol behavior;
- player movement rules where clients depend on exact results;
- collision edge cases that would otherwise cause desynchronization;
- serialization formats where compatibility requires exactness.

## semantic

Used when the resulting gameplay behavior must match, but the internal algorithm and execution ordering do not need to.

Examples:

- lighting;
- many AI operations;
- pathfinding;
- some world simulation systems;
- systems where only the stable result matters.

## intentional_deviation

Used when vanilla behavior is considered undesirable, accidental, pathological, or incompatible with the target architecture.

The deviation must be documented explicitly.

A differential test against vanilla must not automatically treat such differences as bugs.

## Policy per subsystem

| Subsystem | Policy | What must match |
|---|---|---|
| Network protocol | exact | every packet the vanilla client reads or sends |
| Serialization (NBT, Anvil, datapack JSON, registry sync) | exact | byte layout and field values |
| Player movement | exact | the arithmetic of the movement model, tick by tick |
| World generation | exact | blocks, biomes and structures for a seed |
| Neighbour and shape updates | semantic | the state after the chain, and the chain limit |
| Scheduled block and fluid ticks | semantic | due tick, priority and what runs; persisted pending ticks |
| Block events | semantic | which events are delivered, and the resulting state |
| Redstone | semantic | delays, pulse lengths and the resulting states |
| Fluids | semantic | tick rates, levels, spread distance and the resulting states |
| Lighting | semantic | the stable light levels |
| Random behaviour (growth, spawning, loot, AI choices) | semantic | distributions, not vanilla's random sequence |

A subsystem not in the table declares its policy when it is built.

## Semantic now, exact later

Redstone, fluids, neighbour updates, scheduled ticks and block events are semantic for now: the gameplay result must match vanilla, but the order of work inside a tick does not have to. They must stay cheap to move to vanilla's tick-for-tick order:

- Every queue keeps the fields vanilla orders by: due tick, priority and insertion sequence.
- The order inside a shard is decided in one replaceable place, the ordering policy of the domain. Switching a domain to vanilla's exact order is a change of that policy, not a redesign.
- Vanilla's tick-for-tick fixtures are still recorded and are marked as the target of the exact policy, so the switch can be tested against them.
- Determinism is required under either policy: the same inputs give the same result regardless of thread count, with ties broken by a stable key. Shard formation is itself a function of the inputs, so the layout is reproducible; where two layouts can give different results (a cross-shard effect applied at the next sync point), the domain's ordering policy states the difference, and thread timing never decides it.

---

# 3. Logical Tick vs Physical Execution

A fundamental architectural distinction:

```text
Logical Simulation Time
!=
Physical CPU Execution Order
```

Minecraft gameplay can still operate in logical ticks while the work belonging to a tick is executed in parallel.

Execution should preserve only the ordering constraints that are semantically required.

If two operations commute and cannot affect each other's observable result, they should be free to execute concurrently.

The architecture should therefore derive **shards from the dependencies of simulation work**, rather than reproducing a single global sequential vanilla tick loop.

---

# 4. Spatial Discretization

The base spatial discretization unit is the **Chunk Section**:

```text
16 x 16 x 16 blocks
4096 blocks
```

Terminology:

```text
Chunk
    16 x world-height x 16 column

Chunk Section
    16 x 16 x 16 spatial unit

Region file
    Minecraft's existing 32 x 32 chunk Anvil storage file

Shard
    Logical simulation partition of one domain

Kernel
    Plain Rust step of one domain over one shard, without Bevy

Task
    Physical unit of work Bevy runs on its task pool
```

The term **region file** is reserved for Anvil storage. Do not use `region` for simulation partitions, and do not use `cell` for any grouping of chunks.

The only term for a simulation partition is **Shard**.

---

# 5. Section Is Not Necessarily a Scheduling Unit

A section is the minimum common spatial indexing unit.

It is **not necessarily the minimum execution unit**.

Important distinction:

```text
Section = spatial discretization
Shard   = causally independent simulation group
Task    = scheduling / execution unit
```

The system that steps a domain may:

- combine many cheap sections into one task;
- process one expensive section through several specialized tasks;
- process a subsystem whose dependency graph cuts across section boundaries;
- contain multiple independent shards inside one section.

This prevents the 16×16×16 grid from becoming an artificial execution constraint.

---

# 6. Domain-Specific Sharding

There should not necessarily be one universal spatial shard type.

Different simulation domains have different dependency structures.

Examples:

```text
EntityShard
RedstoneShard
FluidShard
LightingShard
BlockTickShard
WorldGenShard
```

These shards can overlap spatially.

Example:

```text
EntityShard E1
└── covers a large area

    RedstoneShard R1
    └── mechanism A

    RedstoneShard R2
    └── mechanism B
```

`R1` and `R2` may exist inside the same `EntityShard`.

They may even occupy blocks inside the same 16×16×16 section if the redstone mechanisms are causally independent.

Therefore:

**Shard identity must not be derived solely from spatial bounds.**

---

# 7. EntityShard

`EntityShard` is primarily spatial.

Its purpose is to group entities whose simulation may interact through:

- collision;
- proximity;
- targeting;
- entity-to-entity interaction;
- player interaction;
- local environmental effects;
- movement across space.

An EntityShard may cover many chunk sections.

Its size should not necessarily be fixed.

It may grow or split depending on:

- entity density;
- colliding entity pairs;
- entities with active AI;
- cross-boundary interactions.

All four are read from simulation state; CPU load and worker availability only decide how shards are grouped into tasks (sections 18, 21).

A shard is not equivalent to a worker thread.

Many shards may execute on one worker.

One machine may execute thousands of logical shards.

---

# 8. RedstoneShard

`RedstoneShard` should primarily be based on **causal connectivity**, not rectangular space.

Conceptually it represents an independent redstone mechanism or dependency component.

Two mechanisms that cannot affect each other should be independently schedulable even when they are physically close.

Example:

```text
Section S

+----------------------+
| Redstone mechanism A |
|                      |
| Redstone mechanism B |
+----------------------+
```

Both can belong to separate `RedstoneShard`s.

A redstone shard can also span many sections.

## Important limitation

Redstone connectivity cannot be determined only from redstone dust adjacency.

Cross-domain causality must be considered.

Potential dependency edges include effects involving:

- pistons;
- observers;
- block updates;
- block replacement;
- entities;
- pressure plates;
- tripwires;
- fluids;
- neighbor updates;
- movable blocks;
- environmental state.

Therefore the redstone dependency graph is better modeled as a **potential causal graph**, not merely an electrical graph.

---

# 9. Dynamic Shard Merge and Split

Shards should be able to change dynamically.

Example:

```text
R1      R2
 \      /
  player connects mechanisms
        |
        v
      R1+R2
```

When a dependency appears, the effect that crosses between the shards is a message, and the domain's system merges the two shards when it applies that message at the next sync point.

When a dependency disappears, the combined graph may later be split again.

Important implementation consideration:

- merging components is usually cheap;
- detecting that a dynamic graph has become safely separable is more expensive.

Therefore splitting can be deferred or amortized, on a budget counted in ticks or work items, never in wall-clock time.

Do not require immediate perfect repartitioning after every block update.

---

# 10. Shared World State

Different shard types should not maintain independent copies of canonical block state.

There should be one authoritative world state.

Conceptually:

```rust
struct WorldState {
    chunks: ChunkStorage,
    entities: EntityStorage,
}
```

Simulation shards describe **which computation owns or operates on which dependency domain**, not necessarily independent physical copies of data.

Possible metadata:

```rust
struct Section {
    blocks: PalettedContainer<BlockState>,
    activity: ActivityMask,
    revision: u64,
}
```

```rust
struct SectionId {
    x: i32,
    y: i32,
    z: i32,
}
```

A section can maintain metadata useful for scheduling without becoming an ECS entity.

---

# 11. Hybrid Storage Model

Do not represent every Minecraft block as a Bevy ECS entity.

That would produce enormous entity counts, poor memory efficiency, scheduler overhead, and bad cache behavior.

Preferred hybrid model:

| Data | Representation |
|---|---|
| Static blocks | palette-compressed chunk/section storage |
| Active blocks | sparse indices / activity structures |
| Entities | Bevy ECS |
| Players | Bevy ECS |
| Block entities | ECS or specialized sparse storage |
| Scheduled block ticks | timing wheel / priority structure |
| Spatial queries | section/chunk-based spatial index |
| Redstone | specialized dependency graph |
| Lighting | specialized propagation data |
| Fluids | specialized active simulation representation |

Bevy is the execution/runtime framework for suitable domains, not necessarily the universal storage format for every Minecraft object.

---

# 12. Activity Tracking

Sections should maintain cheap metadata describing active simulation domains.

Conceptually:

```text
ActivityMask:
    entities
    redstone
    fluid
    lighting
    scheduled_ticks
    block_entities
    other
```

Completely inactive sections should have essentially zero simulation cost.

This allows each domain to avoid scanning large inactive worlds.

Activity metadata must be cheap to update and should serve as an index, not as the actual simulation state.

---

# 13. Kernels, Drivers and Bevy

Shard simulation is a library, independent of Bevy.

## Kernel

Each simulation domain (block updates, redstone, fluids, entities, ...) is a plain Rust kernel in a simulation layer beside the game data, with no Bevy and no tokio.

One step of one shard is a call:

```text
step(shard state, world view, inbound messages, ordering policy)
    -> world writes and outbound messages, into buffers the caller owns
```

The world view is a trait the host implements for its own storage: block state get and set by position, section access, and whatever else the domain needs. Writes made through the view are visible to the step's later reads and are recorded in the write buffer. The engine's section storage is one implementation; a host's own storage is another, for example a mod that drives the redstone kernel through FFI against the game's own chunks.

Given the same state, view, messages and policy, a step returns the same writes and messages. A shard's random source is part of its state.

The ordering policy and the shard layout are inputs, never assumptions. One shard covering the whole world, up to its full extent of 30 million blocks, with the vanilla ordering policy is the exact-parity mode, and it runs through the same code path as the sharded mode.

## Driver

A driver owns the shards of a domain, steps them, delivers cross-shard messages in a stable sorted order at a sync point, and merges and splits shards. Without Bevy a driver is a loop.

## Bevy schedules; no custom scheduler

The engine's driver is Bevy. There is no task graph, worker scheduler or dependency analyser of our own.

- Shards are data: an index or a component per domain.
- One system per domain steps every shard of that domain, in parallel across shards (`par_iter` or similar).
- Cross-shard effects are messages, applied in a stable sorted order at a sync point.
- Order between domains is the order of system sets; the schedule is checked with ambiguity detection set to error.

---

# 14. Causality Matters More Than Distance

Spatial separation is useful but insufficient.

Two nearby objects can be independent.

Two distant objects can be logically connected.

Examples:

- a redstone mechanism spanning multiple sections;
- entities linked through game logic;
- chunk-loading effects;
- scheduled updates;
- long propagation chains;
- world-generation dependencies.

Therefore parallel safety should ultimately be reasoned about using:

```text
reads
writes
events
causal edges
```

rather than only:

```text
distance
chunk boundary
section boundary
```

---

# 15. Cross-Shard Communication

Avoid direct arbitrary mutation of another shard's internal simulation state.

Prefer explicit communication:

```text
Shard A
    |
    | outgoing event
    v
Shard B
```

Possible event examples:

```text
BlockChanged
NeighborChanged
EntityEntered
EntityLeft
CollisionCandidate
SignalChanged
ScheduleBlockTick
SpawnEntity
DespawnEntity
```

This makes cross-shard causality explicit. Messages are applied in a stable sorted order at a sync point, so the result does not depend on which shard finished first.

---

# 16. Mutation and Commit Model

One possible execution model is:

```text
Read authoritative state
        |
        v
Compute shard-local changes
        |
        v
Mutation / event buffers
        |
        v
Dependency resolution
        |
        v
Commit
```

Do not implement this as a full copy-on-write world snapshot every tick.

That would be unnecessarily expensive.

Instead consider:

- immutable or versioned read views;
- per-task mutation buffers;
- localized write ownership;
- per-section revisions;
- optimistic conflict detection where useful;
- local barriers rather than global barriers;
- deterministic commit rules where semantics require determinism.

This model remains a design direction, not a mandatory implementation detail.

---

# 17. Avoid Global Tick Barriers Where Possible

A naive architecture could parallelize work but still introduce a full-world barrier after every simulation phase.

That may limit scaling.

Prefer synchronization at the smallest required dependency scope.

Example:

```text
Shard A ──┐
          ├─ local dependency group
Shard B ──┘

Shard C ───── executes independently
Shard D ───── executes independently
```

A global barrier is acceptable only where the semantics actually require it or where benchmarking proves it cheaper than fine-grained synchronization.

---

# 18. Determinism

Determinism is desirable for:

- debugging;
- reproduction;
- tests;
- replay;
- differential verification;
- distributed simulation;
- identifying parallelism bugs.

Parallel execution must not introduce uncontrolled nondeterministic gameplay behavior.

When multiple valid execution orders exist, choose one of:

1. define deterministic commit ordering;
2. prove operations commute;
3. explicitly allow nondeterministic semantic equivalence.

Do not accidentally allow thread scheduling to become gameplay semantics.

The same inputs give the same result regardless of thread count. Shard layout is derived from the inputs, so it is reproducible as well; a result that may differ between layouts, such as the sync point at which a cross-shard effect applies, is stated by the domain's ordering policy. Ties are broken by a stable key, such as a shard key and an insertion sequence, never by arrival from a thread.

---

# 19. Behavior Extraction

When studying vanilla Minecraft, extract:

- inputs;
- outputs;
- invariants;
- equations;
- state transitions;
- event ordering constraints;
- boundary conditions;
- numerical tolerances;
- observable intermediate states;
- causal relationships.

Do **not** treat vanilla control flow as an architecture requirement.

Bad specification:

```text
call method A
then method B
then method C
```

Better specification:

```text
Given state S
and input I
after logical tick T
the following observable state must hold:
...
```

If ordering itself affects observable gameplay, then ordering becomes part of the behavioral contract.

Otherwise it should remain free for optimization.

---

# 20. Specialized Algorithms Are Preferred

Each subsystem should be free to use the most appropriate data structure and algorithm.

Examples:

## Lighting

Possible architecture:

- incremental propagation;
- parallel wavefronts;
- section-local propagation;
- boundary exchange;
- batched updates;
- SIMD-friendly processing.

Do not reproduce vanilla's lighting algorithm merely because vanilla uses it.

Preserve the required lighting semantics.

## Redstone

Possible architecture:

- explicit dependency graph;
- event-driven evaluation;
- strongly connected component analysis;
- incremental connectivity;
- graph-based scheduling.

## Entities

Possible architecture:

- ECS archetypes;
- spatial indexing;
- batched collision;
- vectorized broadphase;
- parallel AI where safe.

## Fluids

Possible architecture:

- sparse active frontier;
- local propagation tasks;
- section boundary events.

Different domains do not need to share the same internal architecture.

---

# 21. Performance Model

Parallelism is not automatically faster.

Avoid excessive:

- tiny tasks;
- atomic operations;
- lock contention;
- synchronization barriers;
- cross-thread messaging;
- ECS command buffering;
- cache-line bouncing.

Each domain's system should use **adaptive task granularity**.

Examples:

- merge many cheap sections into one task;
- isolate expensive redstone mechanisms;
- run an entity-heavy shard as a task of its own;
- batch low-load neighboring shards into one task.

Shard count should not be optimized directly.

Optimize:

```text
useful work / synchronization cost
cache locality
load balance
latency
throughput
```

---

# 22. Shard != Thread != Server

These concepts must remain independent.

```text
Shard
    logical simulation partition

Task
    unit of scheduled work

Worker
    execution thread

Process
    one server process

Node
    physical/virtual machine
```

This allows the same architecture to scale through:

```text
single-thread
    ->
multi-thread
    ->
multi-process
    ->
multi-machine
```

without redesigning the simulation model.

Distributed execution is not an immediate requirement, but the ownership and messaging model should not make it impossible.

---

# 23. Verification Strategy

Use differential testing against vanilla, but classify differences according to compatibility policy.

Pipeline:

```text
Scenario
   |
   +--> Vanilla reference
   |
   +--> Rust server
             |
             v
         Comparator
             |
             v
      Minimal divergence
```

Compare where relevant:

- world state;
- entity state;
- block state;
- velocities;
- collisions;
- event sequences;
- packet-visible state;
- redstone outputs;
- lighting;
- scheduled updates.

The comparator should identify the earliest meaningful divergence.

---

# 24. Property-Based Testing

Not every correctness property should be defined as equality with vanilla.

Use invariants.

Examples:

```text
entities must not penetrate solid collision volumes
```

```text
inactive sections must not change without an external event
```

```text
independent shards must produce the same result regardless of execution order
```

```text
merging two causally independent tasks must not alter semantics
```

```text
splitting a shard must preserve behavior if no cross-boundary dependency exists
```

This is particularly important because the architecture intentionally differs from vanilla.

---

# 25. Parallelism Correctness Tests

The server should specifically test that thread count does not change results, and that shard layout changes them only as the domain's ordering policy states.

For suitable scenarios run the same simulation with:

```text
1 thread
2 threads
4 threads
8 threads
N threads
one shard per domain over the whole world
shards forced to merge or split at different points
```

Then compare semantic results.

For deterministic systems, results should match exactly across thread counts; across layouts they match exactly where the policy promises it (for example a mechanism inside one shard) and otherwise differ only as the policy states.

For semantically nondeterministic systems, required invariants should match.

This helps detect hidden dependencies that the shard formation failed to represent.

---

# 26. Data Locality

Sections provide a useful locality boundary.

Where possible:

- keep section block storage contiguous;
- avoid random pointer-heavy block representations;
- batch operations over sections;
- keep hot metadata compact;
- separate hot simulation data from cold metadata;
- avoid loading full block-state objects for simple queries.

The architecture should be designed for cache behavior, not merely thread count.

---

# 27. Important Non-Goals

Do not:

- port Mojang classes one-to-one;
- reproduce Java object hierarchies;
- make every block an ECS entity;
- reproduce vanilla's global sequential tick architecture;
- assume chunk boundaries define causal independence;
- assume section boundaries define causal independence;
- map one shard to one thread;
- add a dedicated shard abstraction to every subsystem without measured benefit;
- build a task graph or scheduler beside Bevy's;
- make a domain's kernel depend on Bevy or tokio;
- preserve vanilla bugs by default;
- treat every differential mismatch as a defect in the new server.

---

# 28. Current Architectural Decisions

The following are considered current decisions unless explicitly revised.

## Decision 1

Use **behavior-driven reimplementation**, not implementation-level cloning.

## Decision 2

Use **Chunk Sections (16×16×16)** as the fundamental spatial discretization/indexing unit.

## Decision 3

Use the term **Shard** for simulation partitions.

Do not use `Region`, because Minecraft already defines 32×32 chunk region files. The term **region file** is reserved for Anvil storage.

## Decision 4

Shards are **domain-specific** rather than necessarily universal.

Primary examples:

```text
EntityShard
RedstoneShard
```

Additional shard types are allowed only when justified.

## Decision 5

Different shard types may overlap spatially.

## Decision 6

Multiple independent shards of the same type may exist inside one section.

Example:

```text
one section
    -> RedstoneShard A
    -> RedstoneShard B
```

## Decision 7

A shard is a logical dependency/ownership unit, not a thread.

## Decision 8

Sections are indexing units, not necessarily execution units.

## Decision 9

The world uses a hybrid data model.

Bevy ECS is primarily suited to dynamic entities and selected sparse dynamic state.

Static blocks remain in compact section/chunk storage.

## Decision 10

Bevy schedules; there is no custom scheduler.

Shards are data. One system per domain steps every shard of that domain in parallel, and cross-shard messages are applied in a stable sorted order at a sync point.

## Decision 11

Preserve semantic ordering only where behavior requires it.

Parallelize commuting work.

## Decision 12

Use explicit cross-shard events/messages rather than unrestricted cross-shard mutation.

## Decision 13

Allow dynamic shard merge/split.

## Decision 14

Prefer local synchronization over full-world barriers.

## Decision 15

Design for determinism where feasible.

Thread scheduling must not accidentally define gameplay behavior.

## Decision 16

Differential tests against vanilla are evidence, not an absolute oracle.

Compatibility policy determines whether a difference is a bug.

## Decision 17

Performance decisions must be benchmark-driven.

More shards, more tasks, or more parallelism are not inherently better.

## Decision 18

Shard simulation is a library, independent of Bevy.

Each domain is a kernel without Bevy or tokio. One step of one shard takes the shard's state, a world view through a trait the host implements, the shard's inbound messages and the ordering policy, and returns world writes and outbound messages through caller-owned buffers. Bevy is one driver among several.

## Decision 19

Every subsystem declares a compatibility policy (section 2).

Protocol, serialization, player movement and world generation are exact. Redstone, fluids, neighbour updates, scheduled ticks and block events are semantic, with queues that keep vanilla's ordering fields and an ordering policy per domain, so the exact order is a later change of policy.

## Decision 20

One shard per domain over the whole world, with the vanilla ordering policy, is the exact-parity mode, and runs through the same code path as the sharded mode.

---

# 29. Open Design Questions

These are intentionally unresolved.

## Ownership

Determine the exact write-ownership model:

- per shard;
- per section;
- per domain;
- transactional;
- versioned/optimistic hybrid.

## Cross-domain dependency graph

Determine how dependencies are represented between:

```text
EntityShard
RedstoneShard
FluidShard
BlockTickShard
...
```

## Scheduling

Resolved: Bevy schedules; there is no custom scheduler (section 13).

## Shard splitting

Determine efficient methods for detecting when a dynamic causal graph can be split.

## Redstone semantics

Resolved: redstone is semantic now and migratable to exact (section 2). Delays, pulse lengths and resulting states match vanilla; the order inside a tick is the redstone ordering policy. Behaviour that depends only on that order becomes exact when the policy switches to vanilla's, tested against the recorded fixtures. Anything left out on purpose is an intentional deviation and is listed.

## Entity boundaries

Determine whether EntityShard boundaries are:

- section-aligned;
- chunk-aligned;
- arbitrary spatial volumes;
- dynamically clustered.

## Commit semantics

Determine whether mutation buffering and deterministic commit are required globally or only for selected domains.

## Distributed simulation

Not currently required, but avoid architecture that fundamentally prevents moving shards between processes or machines later.

---

# 30. Implementation checklist

When implementing a feature:

1. Start from the behavioral contract.
2. Identify reads, writes, events, and causal dependencies.
3. Determine the appropriate simulation domain and its compatibility policy.
4. Determine whether the feature belongs in ECS or specialized storage.
5. Write the domain's step as a kernel without Bevy, over the world view trait.
6. Determine the minimum required ordering, and keep it in the domain's ordering policy.
7. Keep the fields vanilla orders by in every queue.
8. Express independent work as separate shards.
9. Avoid global synchronization.
10. Add conformance tests, and a kernel test without Bevy.
11. Add thread-count and shard-layout tests.
12. Benchmark before introducing additional complexity.

When the vanilla implementation appears sequential, do not assume the new implementation must be sequential.

When the vanilla implementation appears to contain a bug, do not automatically reproduce it.

Classify the behavior first.

---

# 31. Architectural Summary

The intended server architecture can be summarized as:

```text
                   Minecraft Behavioral Contracts
                              |
                              v
                        World State
                              |
              +---------------+---------------+
              |               |               |
              v               v               v
         Entity Domain   Redstone Domain   Other Domains
              |               |               |
              v               v               v
         EntityShards    RedstoneShards      Shards
              |               |               |
              v               v               v
            Kernels: one step per shard, no Bevy
            (state, world view, messages, policy)
                              |
                              v
            Driver: step shards, deliver messages
            in stable order, merge and split
                              |
              +---------------+---------------+
              |               |               |
              v               v               v
      Bevy systems,     plain loop        host via FFI
      par_iter per      (tests, tools)    (its own storage)
      domain
```

Spatial indexing is based on:

```text
16×16×16 Chunk Sections
```

but simulation partitioning is based on:

```text
causal independence
domain-specific dependencies
work held, counted from state
locality
```

The core principle is:

> Preserve Minecraft semantics where they matter, discard vanilla implementation constraints where they do not, and make causal independence explicit enough to run shards in parallel safely.
