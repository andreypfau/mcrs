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
| Server-simulated entity motion and push | semantic | within 1e-6 per tick; colliders, blocks and pushes from the start of the tick (PHY-CMP-006, 008, 009, 012) |
| AI decisions | semantic | the decisions and their distributions; a decision applies one tick or more after the state it read |

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

Tile
    Fixed spatial square of 2 x 2 columns, full height. The memory segment and
    the work unit of the entity domain, and the replication tile of the Muxer

Cell
    Unit of placement: a part of a world that can run in another thread,
    process or machine (cells.md)
```

Where cells run, and how a player sees and simulates the world, is in `cells.md`: cells, the cell tree, the Muxer, the data layer and stages. How entities tick inside a world is in `physics-architecture.md`.

A tile is a work unit, not a causal unit. A cell is a placement unit; it is not the unit of parallel work inside a process.

The term **region file** is reserved for Anvil storage. Do not use `region` for simulation partitions. A `cell` is only a unit of placement (`cells.md`), never a causal partition.

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

A domain whose step reads only a frozen snapshot and writes only its own item has no causal group larger than one item. Entity physics is such a domain: its causal unit is one entity, or one vehicle with its passengers, for one tick. Its tasks are batches of tiles.

---

# 6. Domain-Specific Sharding

There should not necessarily be one universal spatial shard type.

Different simulation domains have different dependency structures.

Examples:

```text
EntityShard
FluidShard
LightingShard
BlockTickShard
WorldGenShard
```

These shards can overlap spatially. Two independent mechanisms can occupy blocks inside the same 16×16×16 section.

Therefore:

**Shard identity must not be derived solely from spatial bounds.**

`EntityShard` is not a shard type (section 7). The list gives examples of
domains with a causal structure of their own.

---

# 7. The entity domain

This section described an `EntityShard`: a spatial group of entities that interact
through collision, proximity and targeting, which grows and splits with density. The entity
domain no longer has such a shard. The design is in `physics-architecture.md`.

The physics specification lets the tick of a server-simulated entity read the colliders, the
blocks and the pushes from the start of the tick (PHY-CMP-008, PHY-CMP-009, PHY-CMP-012). Then
the tick of each entity reads only frozen state and its own state, and writes only itself and
deferred effects (PHY-INV-015). This gives the entity domain three properties:

- The causal unit is one entity for one tick. A vehicle and its passengers are one unit, because a
  passenger reads the new state of its vehicle.
- The entity pass is one parallel map. It needs no islands, no merge or split, and no locks.
- The result does not depend on the order, the thread count, the tasks or the cells.

What a shard did for entities, other units now do:

| Need | Unit |
|------|------|
| Memory locality and the work unit | the tile: 2 × 2 columns, full height |
| Load balance | tasks: batches of tiles, or batches of `par_iter_mut` |
| Placement in another process | the cell (`cells.md`) |
| Targeting, sensing, pathfinding | the AI stage beside the tick |

Interactions that write another entity (a hit, a knockback, a pickup) are deferred effects. They
apply in one apply step after the pass of the same tick, in a stable order (PHY-CMP-015). Exclusive actions (a pickup,
a merge, a removal) are claims, arbitrated once per tick after the block domain (PHY-CMP-017). The
block domain writes entities in the same tick, after the entity pass (PHY-BLK-001).

Two small sets tick in sequence, in the order of the UUID: groups of colliders whose read
zones touch (PHY-CMP-018), and explosion groups around primed TNT that explodes in the tick
(PHY-CMP-020). The start-of-tick state decides them, and each group is one task.

The order key of every rule above is the persisted UUID of PHY-CMP-013, not the network id,
which an implementation gives per cell and does not save.

The exact-parity mode does not use the start-of-tick rules. It ticks the entities in sequence, in
the order in which they entered the tick list, over the same store and kernel.

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
| Entities: hot motion state (position, velocity, motion flags, rest, steering) | components of the entity (`MotionState`, `MotionKind`); the physics kernel crate reads them through a body grid |
| Entities: cold state (health, equipment, attributes, team, custom data) | Bevy ECS |
| Entities: brain state (memories, goals, plans) | the AI stage beside the tick |
| Players | rows of the body store (position, rotation, pose); cold state in Bevy ECS |
| Block entities | ECS or specialized sparse storage |
| Scheduled block ticks | timing wheel / priority structure |
| Spatial queries | section/chunk-based spatial index |
| Redstone | specialized dependency graph |
| Lighting | specialized propagation data |
| Fluids | specialized active simulation representation |

Bevy is the execution/runtime framework for suitable domains, not necessarily the universal storage format for every Minecraft object.

Entities, mobs included, are Bevy ECS entities with components (issue #1067). A measurement at 1M
entities (#97) compared a tile-segmented store of rows in spatial order with components and a section
cache. Rows take 30.6 ms for each tick. Components take 35.3 ms. The difference does not pay for a second
storage model, for a tile address on each access, or for a second replication path. The hot
loop skips quiet tiles through `StirredTiles`, and the body grid gives the spatial lookup.
Planned: the code still holds rows until the migration of mobs to entities.

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

A driver can run a step beside the tick, on a thread of its own. The tick does not wait for such a step. The writes of the step are input of the first tick after the step ends. That tick applies them in a stable order. World generation and lighting run their steps beside the tick.

AI runs beside the tick in the same way. The AI stage keeps its own replica of the bodies and a navigation projection of the blocks, fed by the output of each tick. A decision that it issues at tick `T` reads the replica at `T` and applies at tick `T + k` or later, with `k ≥ 1`.

## A step as a parallel map

A step can also read only a frozen view and write only its own item. Then the driver runs it as a parallel map over the items, with no shard and no cross-shard message. The entity pass is such a step. Its frozen view is the blocks at the copy point and the body grid of the previous tick; its outputs are the new components of the entities and deferred effects. One serial system applies the effects after the map, in the order of the `EntityUuid` of the target, the writer key, the write tick and the sequence.

## Bevy schedules; no custom scheduler

Inside the tick, the engine's driver is Bevy. There is no task graph, worker scheduler or dependency analyser of our own between systems.

Inside one system, a domain can run its tile tasks with dependency counters: a task starts when the tasks it reads from have ended, and each ending task decrements the counters of its dependents. The counters live for one tick, on the Bevy compute pool. The entity domain uses them to overlap the sequential groups, the movement checks and the main pass, and the block domain to drive its colour phases without global barriers (`physics-architecture.md` §3.9). Bevy stays the scheduler between systems.

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

The cells of one world follow the same rule. `cells.md` section 4 lists the message and the delay of each domain across a cell boundary.

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

The entity domain uses snapshot isolation per tick, without a copy of the world:

```text
Freeze: blocks by order (no block write before the block step),
        bodies = the grid that the previous tick rebuilt
        |
        v
Parallel map: each entity reads the frozen state, writes its own components,
              appends deferred effects
        |
        v
Collider groups, main pass, explosion groups;
writes to other entities applied in one apply step after the pass of T
by (target key, writer key, write tick, sequence);
a write from another cell joins the apply step of T+1;
claims arbitrated after the block step
        |
        v
Block step applies block actions; rebuild the grid of dirty tiles
```

Only the public part of a body (its box and push fields, 48 bytes) is double-buffered, because only that part is read by another entity. Commit is deterministic and global for this domain.

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

For a step beside the tick, thread timing decides only the tick that applies its writes, never their content or order. This is the third choice above. World generation and lighting accept it, and vanilla also computes both on worker threads.

AI accepts it too, within a bound. The AI step of tick `T` is a function of the brain state, the replica at `T` and a random stream keyed by the UUID and `T`. A path search advances by a budget of nodes per stage tick, never by time. Results apply in the order of `T` and the UUID, at a tick in `[T + k, T + max_lag]`: past `max_lag` the tick waits for the stage (`entity-ai-timing.md`). A mode with a fixed delay of `k` ticks makes the tick always wait, so tests and replays apply each result at the same tick in every run.

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

## Entities

Chosen architecture (`physics-architecture.md`):

- a Jacobi tick: a parallel map over a frozen start-of-tick state;
- components for the hot and the cold fields, with a section cache for the hot loop;
- one body grid with one owner system, updated for the movers, the spawns and the despawns;
- rest per tile: a quiet tile costs no work;
- a batched path for entities in air;
- AI, sensing and pathfinding beside the tick.

## Fluids

Possible architecture:

- sparse active frontier;
- local propagation tasks;
- section boundary events.

Different domains do not need to share the same internal architecture.

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

Thread timing decides when a step beside the tick ends. A test of the whole tick therefore runs in lockstep: after each tick, it waits until the generation and light steps of the tick end. It applies nothing while it waits. Every run then receives the same writes at the same ticks. The test compares the blocks, light, heightmaps and entities after each tick between `Placement::Inline` and `Placement::Thread` of the worlds (`crates/mcrs_minecraft_server/tests/it/tick_determinism.rs`). It fails when the tick applies the writes by arrival, or at a moment between ticks.

The same test runs the AI stage with a fixed delay of one tick. A second test slows the AI stage at random with no fixed delay, and checks that each entity gets the same sequence of decisions; only the ticks that apply them differ. The entity pass is also run with one, two and eight tasks per heavy tile, and the results must be equal.

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
- build a task graph or scheduler beside Bevy's between systems (dependency counters over tiles inside one system are allowed);
- make a domain's kernel depend on Bevy or tokio;
- preserve vanilla bugs by default;
- treat every differential mismatch as a defect in the new server.
