# Entity simulation and movement checks: architecture

Design note for the per-tick simulation of entities (collision, gravity, the push between
entities), the block domain inside one world, the work beside the tick (AI), and the movement
checks of the player. Behaviour comes from `docs/spec/entity-physics.md` (PHY),
`docs/spec/movement-checks.md` (MVC), `docs/spec/entity-ai-timing.md` (AIT) and
`docs/spec/block-simulation.md` (SIM). Structure follows
`docs/minecraft-bevy-sharding-architecture.md` (ARCH). `docs/cells.md` (CELLS) and
`docs/replication.md` (REPL) stay valid where this note does not say otherwise; CELLS has a status
note that lists what this note supersedes.

Target load: 10,000 players and 100,000 to 1,000,000 server-simulated entities in one world, at
20 ticks per second.

Where this note names a technique of other servers, it cites it as "technique" and states only
the technique and its numbers.

## 0. Decisions

Status: the decision "mobs are ECS entities, entity rows are deleted" (#1067) is done. The
migration (step 3 of #1067) moved the mobs to components and deleted the entity rows crate. A name that does not exist yet is marked "planned".
The decision of #1067 does not change the player part (`PlayerRows`, the player layer of `BodyGrid`). It keeps its
current path, and its own issue decides who writes it.

| Question | Decision | Section |
|----------|----------|---------|
| Hot-loop model | Jacobi tick: each entity reads a frozen start-of-tick state and writes only its own components. The entity pass is one parallel map (`MotionStep` with `par_iter_mut`), with no islands and no locks | §3 |
| Work inside one system | per-tile dependency counters for the groups and the players: a task starts when the tasks it reads from are done. Bevy stays the scheduler between systems | §3.9 |
| Sequential groups | collider groups (PHY-CMP-018) and explosion groups (PHY-CMP-020), found from the start-of-tick state; both start at the beginning of `Simulate` and overlap the main pass | §3.5 |
| Unit of causality | one entity for one tick; a mount tree is one unit (PHY-ORD-014); a sequential group is one unit | §3.5, §3.6 |
| Order key | the persisted UUID of PHY-CMP-013, never the network id | §4 |
| Writers | one writer per component in each phase; commands before the grid update; the block domain in `WorldWrite` (PHY-BLK-001); a server vehicle places its rider (PHY-SRV-011) | §3.7 |
| Writes and claims | queue entries from per-thread queues; one serial system applies them in the order (`EntityUuid` of the target, `WriterKey`, write tick, sequence); writes in the one apply step after the pass of the same tick, one tick later from another cell (PHY-CMP-015), claims after the block domain (PHY-CMP-017) | §3.7 |
| Frozen state | `BodyGrid` is one resource with one owner system (planned). The owner system updates the tiles of the movers, the spawns and the despawns before the pass. The pass reads the grid of the previous tick | §3.4 |
| AI | eight reflexes in the hot tick; decisions in an AI stage on an isolated pool at the vanilla periods; versioned snapshot; late decisions applied per tile; proposals per component; `max_lag` 2; `AiDelay::Inline` in exact-parity mode | §5 |
| Data layout | every moving entity that is not a player (items, falling blocks, primed TNT, orbs, projectiles, mobs) is an ECS entity with components, which one parallel pass steps with a section window per entity; players are rows. Entity rows are deleted (planned: #1067) | §6, §6.9 |
| Partition | one world per dimension in a process; tiles as work units; cells for placement across processes | §7 |
| Block domain | block tiles of 4 × 4 columns, 2 × 2 colours, dependency-driven phases, region-only access, undo and re-run for chains that leave the region | §7.4, §7.5 |
| Players | L0 and L1 on the network threads, per connection in stream order; L2 in the world before the entity pass, so entities see the moves of this tick (PHY-CMP-021); the server copy follows PHY-CMP-023 | §8 |

### 0.1 Non-goals

These techniques of other servers change what a vanilla player sees. This design does not use
them:

- collision modes that differ from the reference;
- freezing far mobs (an activation range), and AI or goal throttling by distance by default (the
  reduced cadence of AIT-CDN-003 exists only as an operator's recorded deviation, §5.5);
- a push cap that bumps a counter on both entities of a pair: it depends on the order and breaks
  the Jacobi pass (the push cap of PHY-CMP-019 is the replacement);
- faster experience orbs, an item merge at a fixed long interval, a smaller search radius for
  villager points of interest;
- deleting projectiles after a number of chunk loads (§5.1 defers instead);
- pathfinding that reads the live world from another thread (racy; mobs wait in place);
- ignoring "moved too quickly" while the server lags;
- a null or air value for a reference across a partition (§7.3 reads the start-of-tick state);
- foreign chunks as solid walls (§7.3 uses halos);
- double-buffered blocks (about 1 ms gained for about 10 core-ms per tick) and speculative ray
  tracing for explosions.

## 1. Load model

| Symbol | 100k | 1M | Source |
|--------|------|----|--------|
| awake entities (full physics tick) | 40% | 25% | assumption: most items and idle mobs rest (PHY-SKIP-001..007) |
| bodies in the grid (pushable or collider) | 60% | 60% | living entities, boats, minecarts, shulkers |
| hard colliders (PHY-SHP-009) | 1% | 1% | boats, minecarts, shulkers, happy ghasts |
| living entities with AI | 60% | 60% | assumption |
| villager-class brains among them | 5% | 5% | assumption |
| players | 10,000 | 10,000 | MVC-LAY-008 |
| kernel cost of one awake entity | 0.75 µs | 0.75 µs | 0.6 µs physics + 0.1 µs push over ~3 neighbours + 0.05 µs steering |
| hot columns | ~850k | ~850k | 10k players, simulation distance 6 (169 columns), half of them shared |

All numbers are estimates from these assumptions and the kernel targets of the benchmarks (§10.6).

## 2. Policies this design relies on

| Domain | Policy | Rule | Consequence here |
|--------|--------|------|------------------|
| Own player tick, re-application, the query of PHY-SHP-012, the velocity of PHY-SRV-008, the teleport of PHY-SRV-010 | exact | PHY-CMP-005, PHY-NUM-001..007 | f64 operations with the listed f32 operations of PHY-NUM-003 and PHY-NUM-007, the sine table of PHY-NUM-005 and a raw f32 yaw; no tolerance |
| Server-simulated motion | semantic, 1e-6 per tick | PHY-CMP-006, PHY-DEV-001 | optional f32 batch in air |
| Discrete results | exact | PHY-CMP-007 | the batch never decides them |
| Colliders | start of tick; hard colliders after the pre-pass | PHY-CMP-008, PHY-CMP-018, PHY-DEV-002 | §3.5 |
| Blocks | end of the previous tick | PHY-CMP-009/010/011, PHY-DEV-003 | physics runs before every block write |
| Push | start of tick, sum in key order | PHY-CMP-012, PHY-CMP-013, PHY-CMP-024, PHY-DEV-004 | a gather over the grid |
| Crowd damage random value | a stream of (key, tick) | PHY-CMP-014, PHY-DEV-006 | no shared random source in the pass |
| Writes to other entities | apply step after the pass of the same tick, in key order; one tick later from another cell | PHY-CMP-015, PHY-DEV-005 | sorted queue (§3.7) |
| Claims | once per tick after the block domain; `command < entity < block`; several grants to one winner in the order of the target keys | PHY-CMP-017 | binned arbitration (§3.7) |
| Item phase | an offset 0–3 from the UUID | PHY-CMP-016, PHY-DEV-007 | §6.6 |
| Push cap | mutual top-`K`, only with `max_entity_cramming` 0 | PHY-CMP-019, PHY-DEV-008 | §3.3 |
| Explosions | sequential groups in UUID order; rays of one explosion in parallel with keyed random values; opt-in wave mode for groups larger than `N`; other exploders as writes at `T + 1` | PHY-EXP-001..003, PHY-CMP-020, PHY-CMP-022, PHY-DEV-009..011 | §3.5 |
| Block domain on entities | same tick, after the entity pass, in block order | PHY-BLK-001/002 | §3.7 |
| Riding player | the vehicle writes its position; the client gives only the rotation | PHY-SRV-011 | §3.6 |
| Start-of-tick state of a player | the state after the moves of this tick are accepted | PHY-CMP-021 (PHY-DEV-012 is the alternative, not used) | `predict` before the main pass (§3.9, §8.2) |
| Server copy of a player | jump at move handling; pushes and impulses from the pass; then 0.98 decay, small components to zero, block effects, own push | PHY-CMP-023, PHY-SRV-004, PHY-GRV-009, PHY-PSH-008 | §8.6 |
| Mount trees | one unit, root first | PHY-ORD-014 | §3.6 |
| Rest | exact skip with the push and input rules | PHY-SKIP-001..007, PHY-INP-020 | rest per tile (§6.6) |
| Layout independence | | PHY-INV-014, PHY-INV-015 | same result for any thread count, task split or cell layout |
| Exact-parity mode | one world, sequential, live | PHY-ORD-010/011, PHY-PSH-006, AIT-LAG-011 | §3.8 |
| AI timing | reflexes, delay, `max_lag` | AIT | §5 |
| Push on the own player | bounded input, superset of pushers, kept positions | MVC-UNC-004..007, MVC-PRED-019 | §8.4 |
| Block work | semantic level; chains whole in one partition; footprint bounds of Tables 1–7 | SIM-MODEL-006, 009, 102(a), 106, SIM-RND-001 | §7.4, §7.5 |
| Raids | in `DomainInput`, after the proposal grants, before the entity pass | AIT-CPL-006 | `DomainInput` (§3.7) |

## 3. The hot loop

### 3.1 Model

Let `F_T` be the frozen state at the start of tick `T`:

- the block states at the end of tick `T − 1` (PHY-CMP-009);
- the body grid: for each pushable or collidable entity, its box and the fields of PHY-RCH-006 at
  the start of `T`; for each player, its state after the moves of this tick are accepted
  (PHY-CMP-021), which `predict` writes before the main pass reads it;
- the position table: the position, rotation and kind of every entity and player (§6.5);
- the world border and the loaded sections.

One tick of one server entity `e` is a function:

```text
state'_e, effects_e = tick(state_e, steering_e, F_T + end boxes of the hard colliders)
```

It reads the zones of PHY-RCH-001 and PHY-RCH-006. It writes only `state'_e` and appends effects
(PHY-RCH-002, PHY-CMP-015, PHY-CMP-017). PHY-INV-015 states that the result is then independent of
the order of the entities, the thread count, the task split and the cell layout.

The push follows PHY-CMP-012 with the order key of PHY-CMP-013 (§4). The push of step 9 of tick
`T − 1` reads the boxes after the moves of `T − 1`, and only `F_T` holds them. Thus the pass adds
that push at the start of `T`, before the travel step, and the travel step of `T` uses it, as in
the reference (PHY-PSH-006). A push from `F_T` after the move of `T` lags one more tick. Then each
pair keeps the push for one tick after the boxes part, and a dense crowd does not settle. The crowd damage uses the
neighbours of the gather and the random value of PHY-CMP-014, so the pass has no shared random
source.

### 3.2 What is frozen, and what that costs

| State | How it stays frozen | Cost per tick |
|-------|---------------------|---------------|
| Blocks | by order: no block write between the freeze and `WorldWrite`; explosion groups write only into an overlay until `WorldWrite` | zero; a schedule test holds the order |
| Bodies and positions of other entities | `BodyGrid` of the previous tick; its owner system updates it before the pass, and the pass does not write it | one update for each mover, spawn and despawn (§3.4) |
| Hard colliders | `ColliderEnds`, ready per tile | §3.5 |
| Own components | read and written in place; no other entity reads them during the pass | zero |

### 3.3 Jacobi against interaction islands

| Cost at 1M | Jacobi | Islands (connected read zones, tick-list order inside) |
|------------|--------|--------------------------------------------------------|
| Partition work | none | ~2.5M pairs × 5 ns plus union-find at ~10 ns per pair: ~40 core-ms |
| Largest serial unit | one mount tree | the largest island. Zones grow the box by ≥ 1 block (PHY-RCH-001), so they percolate at about one mob per 9 m². A village or a farm is one island: 50k × 0.75 µs = 37 ms on one core |
| Wall time of the pass, 32 cores | ~7 ms | ≥ 37 ms in a dense area |
| Parity gained | none needed (PHY-DEV-002/003/004) | closer to the reference order, which no policy requires |

Decision: Jacobi. Islands stay only in the sequential exact-parity mode (§3.8).

### 3.4 The body grid and its owner system

Per tile, the grid holds the public bodies sorted by bucket (4 × 4 blocks, full height), then by
`EntityUuid`. A query merges the sorted runs of the buckets that it touches, which gives the order
of PHY-CMP-012 step 4 without a sort.

`BodyGrid` (planned: one resource in `mcrs_minecraft_physics`) holds `F_T` for each tile: the
bodies in bucket order and the index of the first body of each bucket, so a query takes the bodies
of a bucket as one slice. The grid has one owner system. Before `MotionStep`, the owner system
updates the tiles of the movers, of the spawns and of the despawns. It also updates the rows of
the movers of the pass before, so the previous position of an entity that stopped becomes its
position (PHY-CMP-027). The step reads the grid of the previous tick. The step does not write the grid, so it needs no back buffer and no swap.

Other writers can change a body after the pass: explosion groups, the block domain (pistons), claim
grants, the apply step, commands and the life cycle. Each of them changes
the components of the entity and appends the entity to one dirty list. The owner system drains the
dirty list. No other system writes the entity layer; the player layer keeps its current path (§0).
The cost of the update is proportional to the movers,
not to the bodies; the migration pull request measures it with the bench `entity_pass`.

Measured on the row design with the bench `entity_pass` (50k walkers, half of them walking, and 50k items at rest on
32 × 32 tiles) on the amd64 host: AMD EPYC 9454, 16 logical CPUs on 8 cores, shared with other
load. Callgrind counts one tick on one thread with `tools/callgrind.sh`.

| Version | Criterion, 16 threads | Ir per tick |
|---------|-----------------------|-------------|
| Before: masks only on sections of one state, grid installed on the main thread | 6.36 ms | 717M |
| Palette masks with `large` and `inside` (§6.7), grid written by the tile tasks, bucket starts | 3.77 ms | 382M |

The time per tick does not follow the instruction count alone. Before the change in that table, about 1.4 ms of
the tick ran on one thread: the install of the tile grids (~0.9 ms), the serial part of the freeze
and the tile moves of the row design. In a tick of the bench, ~26k walkers move, ~2k standing walkers are awake,
~23k walkers rest, and of the 50k items 12.5k skip their tick at rest and 37.5k run the rest tick
of PHY-ORD-005 without a move. Each living entity runs the push in each tick.

### 3.5 Sequential groups

Two rules make a small set of entities tick in sequence, in UUID order. The grid update finds the
groups from `F_T`, so the set of groups does not depend on the thread count or the layout. Each
group is one task. Both kinds start at the beginning of `Simulate` and overlap the main pass,
through the per-tile dependency counters of §3.9.

| | Collider group (PHY-CMP-018) | Explosion group (PHY-CMP-020) |
|---|---|---|
| Why | two moving hard colliders that read each other's start box overlap, and PHY-COL-003 then lets them pass | a TNT cannon needs the push of the charges in the move of the projectile in the same tick (PHY-EXP-002) |
| Seeds | every awake hard collider (boats, rafts, minecarts, shulkers, happy ghasts) | every primed TNT with a fuse of 1 or less, and every creeper that reaches its fuse in this tick (AIT-RFX-002); the TNT and the creeper are known from their components at the start of the tick |
| Growth | union of colliders whose read zones intersect, over the per-section list of hard colliders. A resting collider is not stepped: its start box is its end box | every entity whose read zone touches the reach (a cube of edge `4p`, grown by the displacement of the exploder), then every entity that an explosion of the group can ignite or move, until the group stops growing; groups that share an entity, or whose reaches intersect, merge |
| Finding cost | union-find over ~10k colliders at 1M: < 0.5 core-ms | grid and position-table queries per seed: ~2 µs per seed |
| Start | beginning of `Simulate` | beginning of `Simulate` |
| Reads | other colliders of the group live; everything else from `F_T` | other members live; blocks through an overlay `CollisionWorld`: the copy point plus the removals of the earlier explosions of the group; everything else from `F_T` |
| Writes | its entities; `ColliderEnds` for each tile that its zones touch | its entities; removed blocks into the overlay of the group; writes and claims into the queue of its task |
| Who waits | a main-pass tile waits only for the collider groups whose zones touch it | the block step of a block tile waits only for the explosion groups whose reach touches it. The overlays commit at the start of `WorldWrite`, in group-key order |
| Members | stepped by the group, left out of the main pass | stepped by the group, left out of the main pass |
| Typical size | 1–3 | 1 TNT plus the entities in its reach: 1–20 |
| Worst case | a pile of 5,000 boats: ~5 ms on one task, hidden while it does not touch the busiest tiles | 1,000 primed TNT: a true chain, ~30 ms with fanned-out rays, hidden behind the main pass up to the length of the pass |

**Waves inside a collider group.** A collider reads only the colliders whose read zones touch its
own. Inside a group, order the colliders by key and draw an edge from each to every later collider
whose zone touches it. A collider runs when all its predecessors ran. This gives the result of the
sequential key order, exactly, and costs one sort and one zone query per collider. It helps a line
of boats on ice (depth ~1) and does nothing for a tight pile, where every zone touches every other.
Adopted, because it is cheap.

**Explosions.** The rays and the exposures of one explosion read the state at the step of that
explosion (PHY-EXP-003), with keyed random values (PHY-DEV-010), so they fan out over the pool;
the chain of explosions stays sequential. The opt-in wave mode of PHY-CMP-022 (PHY-DEV-011) removes
the chain for a group with more than `N` members: every explosion traces from the start state of the
group, the impulses and damage add per member in the order of the exploders' keys, the members then
move in key order, and the removed blocks are the union. All rays of all explosions of the group then
run in parallel. The mode changes TNT cannons, so it is off by default; an operator sets `N`.

An exploder that does not show at the start of the tick (a fireball that hits a block) forms no
group; its impulse and damage are writes at `T + 1` (PHY-DEV-009).

The main pass reads `ColliderEnds` for the collision with a hard collider, and the start-of-tick
grid for the push. A boat is both: its collision box is its end box, its push box its start box.

### 3.6 Mount trees

PHY-ORD-014: a root vehicle and its passengers are one unit, and one unit is one task item. The
members of a unit come from the root through the vehicle and passenger components, and one task
steps them, root first. The vehicle reads the input of a controlling passenger from its
start-of-tick state.

- A server vehicle that carries a player sets the position of the player (PHY-ORD-014 step 3,
  PHY-SRV-011). The pass does not write `PlayerRows`: it emits a `RiderPlaced` message, which the
  player-row writer applies in `Project`, in the same tick, before the output. While the player
  rides, `predict` takes only the rotation from the client and runs no movement check.
- A vehicle that a client controls is written in `ClientInput` from the vehicle moves, before the
  pass; MVC-VEH-001..014 check them. The pass does not step it.

### 3.7 Writers, the apply step and claims

**One writer per component in each phase.**

| Phase | Writer of the components of mobs and other moving entities | Writer of `PlayerRows` |
|-------|------------------------|------------------------|
| `DomainInput` | commands (write-through); mailed claims of `T − 1` from other cells; proposal grants; the raid tick (AIT-CPL-006); the life cycle; then the grid update | commands, through the teleport of PHY-SRV-010 |
| `ClientInput` | client-controlled vehicles | — |
| `Simulate` | collider groups, `MotionStep` (the brain writes `MobIntent` first), explosion groups (each on its own entities) | `predict` before the main pass; the server part of each player after it (§8.6) |
| `WorldWrite` | first the apply step of the queues (hits, knockback, impulses, crowd hits; `DamageSet` of `docs/damage.md` §4); explosion overlays commit; entity block actions; the block domain (PHY-BLK-001) in block order; claim grants | the apply step; claim grants |
| `Project` | none: the publisher derives the tile crossing from the position (`Assigned`, `PublishedTile`, `Departures`) | `RiderPlaced` |

**Commands before the grid update.** Commands run first in `DomainInput`, as the reference handles the
packets of the clients before the tick (PHY-ORD-010 step 1). A command writes components at
once, so `/tp` followed by `execute at @s` in one function reads the new position. The owner system
of the grid sees its writes, and the groups found at the grid update are never stale. A
command that moves a player uses the teleport of PHY-SRV-010 and the protocol of TP-CONFIRM, never a
direct write of the accepted state.

**Block domain (PHY-BLK-001).** Pistons, slime and honey blocks that a piston moves (mover kind
`piston`, PHY-BLK-002) and shulker boxes (`shulker_box`) move entities. The block domain runs after
the entity pass and writes the components directly, in block order. A block in one cell that writes an
entity of another cell sends the write in the cell mail. The write joins the apply step of `T + 1`.

**The sorted queue and claims.** Every write and claim carries its `WriterKey`. A task appends it to the queue
of its own thread. No lock is taken: each thread owns its queue. The apply step runs once per tick, after the
motion pass of the same tick, and the claim arbitration runs later in `WorldWrite`. At each of them, one serial system takes the queues of every thread. It sorts the entries by
(`EntityUuid` of the target, `WriterKey`, write tick, sequence) and applies them. The order never uses the Bevy
`Entity`. A write from another cell arrives in the cell mail and joins the apply step of `T + 1`. A claim from another cell arrives one tick late in the same way. For one layout, the order and the f64 sum order do not depend on the thread count or the task split. The writes from the cell of the target keep the same order in each layout. A write from another cell can apply one tick later than in a layout of one cell, so a target can see another order there (PHY-DEV-005). No write waits for the start of
the next tick, and `DomainInput` holds no write from the own cell. `WriterKey` and `YzxPos` move to
`mcrs_minecraft_entity_key` (planned).

```rust
pub enum WriterKey {              // variant order is the order of PHY-CMP-017
    Command { sequence: u64 },
    Entity { key: EntityUuid },
    Block { pos: YzxPos },        // compared by y, then z, then x
}
```

**Claims (PHY-CMP-017).** A pickup by a player, a mob or a hopper, a merge of items or orbs, and a
removal are claims. The arbitration runs at the end of `WorldWrite`, after the block domain, in one
serial system over the sorted queue: per target, the lowest key wins, and the winner and the
target apply the grant in the same step, before `Output`. When one claimant wins several targets in one tick, the
grants apply in the order of the target keys (PHY-CMP-017 step 5). A claim from another cell arrives
one tick later and joins the arbitration of `T + 1` in the owner (PHY-CMP-017).

**Raids (AIT-CPL-006).** The raid tick runs in `DomainInput`, after the proposal grants and before the
grid update, so the entity pass sees what it changed; the scheduled ticks that the reference puts before
the raids ran in the previous `WorldWrite`. It is one serial step per raid, in the order of the raid
centres (y, z, x).

**Block actions of entities.** The step, inside and fall actions that the pass starts (PHY-RCH-002)
are effects. They apply in `WorldWrite`, before the scheduled ticks of the block domain, in the order
`(block position y, z, x, writer key)`, against the current state (PHY-CMP-010). A part of an action
that writes only the own entity is not an effect: it applies in the pass, at the step of the
reference, with the block of the copy. The damage of the step and fall actions is not such a part:
it is an own hit of the sorted queue, applied after the pass (DMG-ORD-002).

### 3.8 Exact-parity mode

Exact-parity mode is one world with every link `Inline`. It uses none of
PHY-CMP-008, 009, 012, 014, 015, 017, 018 or 020. One sequential driver runs the dimension tick in the
order of PHY-ORD-010:

1. The client packets of the tick, in arrival order; a move packet changes its player when it is
   handled (PHY-SRV-003).
2. The dimensions in creation order.
3. In each dimension: the world border, the weather, the time, the scheduled block ticks, the
   scheduled fluid ticks, the raids, the random ticks and precipitation, the client broadcast, the
   block events (SIM-TICK-002).
4. `step_entities_in_order` over the entities in `TickListSeq` order, with a live collider view,
   sequential push (PHY-PSH-006), immediate writes and claims, and AI with `AiDelay::Inline`
   (AIT-LAG-011).
5. The block entities, then the life cycle and the output.
6. The server part of each player, in connection order.

The kernel functions and the decision functions are the same as in the sharded mode.

### 3.9 Schedule of a tick

Bevy schedules the systems of the `TickSet` phases. Inside one system, the work units are tiles
or groups, and a task starts when its dependency counter reaches zero: each task that it reads from
decrements the counter when it ends. The counters live in the system for one tick, on the Bevy
compute pool. This is the only scheduling below Bevy.

```text
DomainInput:  commands (write-through); stage results (light, generation; columns land in
              parallel, one batched structural change); cell mail (halo changes [C1], HaloBodies,
              mailed claims);
              proposals per component; raid tick; life cycle (batched mob spawns and despawns);
              -> grid update: the owner system of `BodyGrid` updates the tiles of the movers, the
                 spawns and the despawns; find collider and explosion groups; set the dependency
                 counters (players included)
ClientInput:  ClientTick per player (from the network threads, after L0/L1);
              client-controlled vehicles; client block edits only send BlockSetRequest
Simulate:     `entity_tick`, over entities, groups and players with dependency counters:
                predict per player (accepts the moves, the jump of PHY-CMP-023 step 1) -> its
                  accepted state in the player layer of `BodyGrid` (PHY-CMP-021)
                collider groups (waves) -> ColliderEnds ready per tile
                `MotionStep` (`par_iter_mut`, once the players and colliders near an entity are
                  ready): `MobIntent` of the brain, reflexes, steering, push gather over the
                  grid of the previous tick, tick_entity / tick_mount_tree, rest; the effects go
                  to the per-thread queues
                explosion groups (overlay), starting at once
                server part per player, once the entities near it are done (PHY-CMP-023 steps 2, 3)
WorldWrite:   apply step of the queues, after the motion pass (one serial system, sorted by
                (`EntityUuid` of the target, `WriterKey`, write tick, sequence)): hits, knockback, impulses,
                crowd hits; writes from other cells arrive one tick late
              explosion overlays commit per block tile; entity block actions;
              block phases (§7.5), each driven by dependencies per block tile:
                scheduled block ticks -> fluid ticks -> random ticks and precipitation
                -> broadcast cut -> block events -> block entities
              -> claim arbitration (one serial system over the sorted queue), grants
              -> exact-parity mode only: the driver of §3.8 replaces this phase and `Simulate`
Project:      the publisher derives tile crossings; RiderPlaced; project_section_bricks; trim the kept changes
Output:       tile events from mover lists; AI delta (versioned), block changes, player table and
              decision issues to the AI link; setbacks; cell mail
```

- Inside a task: no `Commands`, no structural change. Structural changes happen only at the life
  cycle, batched.
- `tests/it/schedule_ambiguity.rs` covers the new plugins.

## 4. Keys

| Key | What it is | Persisted | Used for |
|-----|------------|-----------|----------|
| `EntityUuid` | PHY-CMP-013: the UUID of the entity, compared as an unsigned 128-bit integer. Kept across a change of dimension | yes, as the UUID of the entity | the push order, the item phase, the crowd damage stream, the groups, the AI random streams, the queue order, claims |
| `EntityId` | the global runtime id from the coordinator's ranges; the wire id | no | the protocol |
| `Entity` | the Bevy entity of a mob or of a moving entity that is not living, or of the cold state of a player. One index `EntityUuid -> Entity` has one owner | no | ECS only; never an order key |

The order key is the UUID. An entity without a UUID gets a new one from its birth inputs:

```rust
pub struct EntityUuid(pub Uuid);   // Ord equals the order of the u128 value

impl EntityUuid {
    // A fixed 128-bit hash of the world seed, the dimension key (its length, then its
    // bytes), the birth tick, the source kind number (load 0, player 1, command 2,
    // spawn 3, block 4, entity 5), the 128-bit source key (block: y, z, x; spawn: chunk
    // and attempt; load: chunk and list position; command: the block of the spawn point,
    // then the block of the source; entity: the UUID of the parent) and the sequence,
    // with the version 4 and the variant of RFC 9562.
    pub fn born(
        world_seed: u64,
        dimension: &str,
        tick: u64,
        source: &BirthSource,
        sequence: u32,
    ) -> Self;
}
```

The birth inputs of two new UUIDs never repeat in one server: the dimension key separates the
dimensions, and a command runs in the cell that owns the column of its source block. No input is a
cell index, so one cell and many cells give the same UUIDs. A spawn with a given UUID is refused when
an entity of the world that applies it has that UUID. Thus two entities of one world never compare
equal (PHY-CMP-024). `EntityBirths` in `mcrs_minecraft_engine` reads the inputs of a world. A UUID from a save, a load or a
summon `UUID` stays as it is. The phase offset of PHY-CMP-016 is the two lowest bits of the UUID,
so the save does not keep it.

A player has the UUID of its profile, so it never gets a new UUID. The player file keeps no key of
this implementation. A read drops the `mcrs:birth` compound of an older file.

The birth tick reads the birth clock of the coordinator. The file `data/mcrs/birth_clock.dat` holds
the value that the clock reaches at the next save, one autosave interval ahead. After a crash, the
clock thus continues above every tick that a saved UUID can hold as a birth input.

## 5. AI: reflexes in the tick, decisions in a pipeline

### 5.1 Reflexes in the hot tick

The eight reflexes of AIT-RFX-001..008 run in the main pass, in the tick in which the reference runs
them (AIT-INV-003). They read the start-of-tick state of AIT-SPL-002 and write the own components, or make
writes and claims.

| Reflex | Rule | Per-tick work (AIT-SPL-003) |
|--------|------|-----------------------------|
| steering: path follower, move, look, jump control, body rotation | AIT-RFX-001 | fixed; ~50 ns |
| creeper swell: start and continue, direction, counter, explosion | AIT-RFX-002 | fixed, one sight ray when it has a target; a swell that ends this tick seeds an explosion group |
| melee and ranged attack | AIT-RFX-003 | fixed, one sight ray |
| door on the path | AIT-RFX-004 | fixed; the open and close are block writes |
| escape: float in a fluid, climb out of powder snow | AIT-RFX-005 | fixed |
| effects of damage | AIT-RFX-006 | in the apply step of the same tick (from another cell: of the next tick); a fact to the brain |
| orb attraction | AIT-RFX-007 | one lookup in the player table and the per-column index |
| due events of the AI | AIT-RFX-008 | O(due), from the timing wheel |

Cost per living mob per tick: ~0.1 µs on average (fixed checks, one ray for the ~10% with a target,
the issue of its due decisions). AIT-INV-006 holds: the hot cost of a mob does not depend on its AI.

Also hot: physics, push, crowd count; the due events of physics; the projectile chunk-load budget per
tile (technique: 10 per tick), which defers a projectile, never deletes it.

### 5.2 Decisions and the AI stage

The decisions of AIT-DEC-001 run in the AI stage, a kernel without Bevy on a link per world.

- **Issue (AIT-CDN-001).** The hot tick issues each part at its vanilla period (AIT-VAN-025), with
  the phase from the UUID (AIT-DEV-003), never by backlog, wall time or load.
- **Versioned snapshot (AIT-LAG-001).** A decision of issue tick `T` reads the bodies and steering
  states at the start of `T`, the block copy of `T`, the player table of `T`, the facts before `T`,
  and the brain state after the earlier decisions of its entity. The tick sends, per tick, the bodies
  and steering states that changed. The replica keeps, per entity, the current version and the
  versions of the last `max_lag + 1` ticks; the navigation projection keeps old section versions
  for the same span (copy on write). A decision of `T` processed late still reads the versions of
  `T`.
- **Path search** reads the navigation projection of `T`, the start-of-tick block copy (AIT U7). Its
  slices yield the pool after ~200 µs and resume on the same version.
- **Cheap inputs (technique).** Behaviour conditions re-run only when an input changed, by a change
  counter on the memories; only mobs with a path hear block changes on it (an active-navigator set
  per tile); path types and tag bits per state are precomputed.
- **Random values (AIT-LAG-008)** come from a stream of the world seed, the UUID, the issue
  tick and the part.
- **Output.** A decision writes only the brain and steering state of its entity (AIT-DEC-007). A
  change of another entity or of shared state is a proposal.

### 5.3 Application

- **Decisions per entity (AIT-LAG-002).** A decision applies before the hot step of its entity: the
  brain writes the component `MobIntent`, and `MotionStep` of `T + k` reads it, after the proposal grants
  changed the entity. The order is `(issue tick, UUID)` (AIT-LAG-007). The grant step of `DomainInput` keeps only
  the proposals.
- **Revalidation (AIT-LAG-005/006).** The step checks each decision against the current state: the
  entity was removed or changed dimension; the attack target is not alive, not in the dimension,
  cannot be attacked, or is outside the follow range; the mob teleported after the issue tick and
  the decision has a path; the path starts more than `max_lag × s + 1` blocks from the mob; the path
  goes to an entity that is not alive or not in the dimension; a reflex took a control flag that the
  decision needs; the condition of a removal changed. A stale decision drops whole; the steering
  keeps its state; a fact goes to the brain.
- **Proposals (AIT-CPL-002..005)** keep one global point, in `DomainInput`. They
  are split into components of proposals that name a common entity (union-find), and the components
  are granted in parallel, each in the order `(issue tick, UUID)`. An item merge grant enters
  the claim arbitration of its tick (AIT-CPL-005, PHY-CMP-017).
- **Timers** that a decision starts count from the application (AIT-LAG-010); hold times end by
  AIT-LAG-009.

### 5.4 Delay modes and `max_lag`

| Mode | `k` | Where | Rule |
|------|-----|-------|------|
| `AiDelay::AtLeast(1)` | `1 ≤ k ≤ max_lag` | production, default | applies at the first tick after it is ready; at `T + max_lag` the tick waits (AIT-LAG-004) |
| `AiDelay::Fixed(k)` | `1 ≤ k ≤ max_lag` | determinism tests, replays, one cell against many cells | the tick always waits for the decisions of `T' − k` (AIT-INV-002) |
| `AiDelay::Inline` | `0` | the exact-parity mode only (AIT-LAG-011) | no AI stage: each decision runs in the AI step of its entity in the sequential pass, against the live state |

`max_lag` defaults to 2 (AIT-LAG-003). A decision is never older than `max_lag` (AIT-INV-004).

### 5.5 The AI stage is a pipeline

With `max_lag` = 2, the stage must finish one tick of AI work for each tick. When it needs more core
time than it gets in 50 ms, the AI cost is on the critical path and the tick waits at
`T + max_lag`. "Beside the tick" means only that AI work overlaps the tick on other cores, and that
a burst shorter than two ticks hides in the lag.

When AI work exceeds the cores of the stage, the operator can, in this order: accept the slower tick
(the wait is a metric); lower the mob caps or the entity-ticking range; enable the reduced cadence
for far mobs (AIT-CDN-003, AIT-DEV-006, off by default); raise `max_lag` (AIT-DEV-001; absorbs
bursts, not a lack of throughput); add cores or place cells in other processes.

### 5.6 Pool isolation

The stage runs on its own pool, at a lower OS priority or on cores that the compute pool does not
use, so the OS never preempts a hot task at a barrier for an AI job.

### 5.7 Player table

Each cell keeps the player table of each dimension (AIT-CDN-002): the position at the end of the
previous tick, spectator and alive flags; 32 B per player. A per-column reverse index of near
players at fixed radii (technique) serves the despawn distances, the idle time, orb attraction,
spawning, entity tracking and the player push.

### 5.8 What the hot loop never waits for

The light and generation stages, the data layer or the disk, the muxer, encryption, the sockets and
L0/L1. It waits for the AI stage only at `T + max_lag`, or always in `AiDelay::Fixed`.

## 6. Data layout

### 6.1 Options

Every moving entity that is not a player is an ECS entity with components (§6.3): items, falling blocks, primed TNT, orbs, projectiles and mobs. Entities that push nothing read no body grid. A kind from a vanilla plugin or a third-party plugin is a set of components with no store of its own (§6.9). Each hot field has one home.

Entity rows are deleted (planned: #1067). The code still has mob rows until the migration pull request. A row attribute side table becomes an attribute component, and a despawn releases it. Players stay rows (§6.3).

### 6.2 Resources

| Resource | Content | Written by |
|----------|---------|--------------------------|
| `Movers` | the moving entities that moved in the last motion pass | the motion pass (§6.3) |
| `PlayerRows` | the rows of players: the accepted state and the server copy | `predict` and the server part of each player |
| `BodyGrid` | the grid and the position table; one resource (planned) | its owner system, before `MotionStep` |
| `ColliderEnds` | end boxes of hard colliders, with a ready counter per tile | collider groups |
| per-thread queues (planned) | per thread: crowd candidates, impulses, damage, claims, expiry | `MotionStep` and the groups; one serial system applies them (§3.7) |

`predict` takes `ResMut<PlayerRows>` and `Res<BodyGrid>`; `entity_tick` takes the rest.
Bevy runs them in parallel.

### 6.3 Entities and the life cycle

Players: see §0.

A mob is a Bevy entity (planned). It has these components:

| Component | Content |
|-----------|---------|
| `Mob` | identity; the index `EntityUuid -> Entity` has one owner |
| `EntityUuid` | the order key (§4) |
| `MotionState` | the body, the rest and the age, as for the items |
| `MotionKind` | the `MotionRow` of the kind |
| `MobIntent` | the intent of the brain; the brain writes it, the step reads it |
| `MobBrain` | the brain state (`mcrs_minecraft_engine_brain`) |
| a component for each side column | for example the steering state and the attributes |

The kernel steps one `EntityState` with its `MotionRow` (`tick_entity`). The step reads both from the
components of the entity.

- **Moving entities that are not players.** Items, falling blocks, primed TNT, experience orbs,
  projectiles and mobs are ECS entities with components, not rows. One pass in `Simulate`
  (`MotionPass`) steps each of them with `tick_entity`, through `par_iter_mut().for_each_init`. Each
  batch has a section cache, a scratch and an action buffer. Each entity writes only itself. A mob
  pushes, so the step of a mob reads `BodyGrid` of the previous tick (§3.4); items push nothing. The cold
  state (item stack, owner, pickup delay, projectile owner) lives in the components of the kind
  plugin. §6.9 gives the steps of a kind plugin.
- **Cross-entity effects.** The step appends crowd candidates, impulses, damage and the expiry of an
  entity to the queue of its thread. One serial system applies the queues in the order
  (`EntityUuid` of the target, `WriterKey`, write tick, sequence). A system that despawns on expiry also runs in
  sorted order (§3.7).
- **Life cycle.** The life cycle spawns and despawns mobs with batched commands. The replication
  plugin gives the network id as for any moving entity (§6.9). `reserve_ids` and `spawn_row` are
  deleted.
- **Tile crossing.** No row moves. The publisher derives the crossing from the position with
  `Assigned`, `PublishedTile` and `Departures`.
- **Players** are rows (approved decision 3). `Transform` stops being the truth for a player:
  `column_streams`, `movement`, `cell_crossing`, `publish_player_tiles` and persistence read the
  accepted state in `PlayerRows` (MVC-PRED-012).
- A side column is a component. It never has a side table keyed by `EntityId`.

**Open part of decision 24.** The player position still lives in the `Transform` component. The
server has no `PlayerRows` store yet. A copy of `Transform` in rows would give a second home for
the position, and its tile could disagree with the tile of the replication for one tick. The
movement check must write the player row first. Then `Transform` becomes a projection of the row,
and the rows become the one home of the player position. The player state already holds a `Body`
and the shared `Orientation`, and players and server entities use the same `TileKey::at`.

### 6.4 Classification

| Data | Class (bevy-ecs) | Home | Writer |
|------|------------------|------|--------|
| position, velocity, rotation, pose, motion state, rest, steering of an entity | authoritative | components (`MotionState` and the side components); players: `PlayerRows` | the writer of the phase (§3.7) |
| motion class, UUID | authoritative, immutable | components (`MotionKind`, `EntityUuid`) | the life cycle |
| body size | derived from class, pose, `scale`, baby; cached per entity | component | one projection in the apply step; `tick_player` derives it from the pose on read (§9) |
| accepted player state, server copy | authoritative | `PlayerRows` + `Prediction` | `predict`, the server part |
| grid rows, position table | materialized, frozen | `BodyGrid` | its owner system, before the pass |
| `MotionState` of a moving entity that is not a player (key, body, rest, age) | authoritative | ECS component | the motion pass in `Simulate`; outside `Simulate`, an impulse of a plugin (§6.9) |
| `MotionKind` | authoritative, constant while the entity moves | ECS component | the kind plugin |
| `Dropped.pos` of an item | projection of `MotionState` | ECS component | one writer: the item plugin, after the motion pass |
| `Movers` | temporal fact of the tick | resource | the motion pass; it holds from the end of one pass to the start of the next |
| `PublishedTile` | materialized: the tile in which the viewers know the entity | ECS component | the publisher, in `MotionPublish` |
| cold state of items, falling blocks, TNT, orbs, projectiles | authoritative | ECS components | the kind plugin, claims |
| cold state of mobs and players | authoritative | ECS | their systems |
| brain state | authoritative | `MobBrain` component | the AI stage |
| box, `pushable`, `push_targets` | derived on read | — | — |

### 6.5 The grid row and the position table

```rust
pub struct GridRow {                        // 104 bytes; bodies only
    pub position: DVec3,
    pub previous: DVec3,                    // at the start of the last tick (PHY-CMP-027)
    pub half_width: f32,
    pub height: f32,
    pub key: EntityUuid,
    pub root_vehicle: EntityId,
    pub team: TeamSlot,
    pub flags: GridFlags,                   // pushable, push_targets, collider kind, spectator,
                                            // no physics, carries passenger, is passenger
}

pub struct PositionRow {                    // 32 bytes; every entity and player
    pub position: DVec3,
    pub kind: KindSlot,
    pub flags: u16,
    pub id: EntityId,
}
```

The same owner system updates the position table. It covers what the hot steps read outside the grid: a look target, the player that
an orb moves to, a leash holder, the target of a homing bullet. A reference to an entity in another
cell reads the halo copy of this table, never a null.

### 6.6 Rest and wake per tile

- Each tile has a stamp: the largest of the block change counters of its sections and their ring,
  the grid revision of the tile and its neighbours, the border revision, and the decisions,
  impulses and block-domain writes that target it. `StirredTiles` lists the tiles whose stamp
  changed. A changed `MotionState` wakes its entity. In a stirred tile each resting entity compares
  its zone stamp (10 ns).
- The cheap exact path (technique): an entity on the ground, with zero horizontal velocity, only
  gravity, and an unchanged floor section stays at rest without a full tick.
- Enter rest: PHY-SKIP-001, 004, 006 and 007.
- The item phase is the offset of PHY-CMP-016; a skipped item writes nothing.

### 6.7 Block access and shapes

Per section, physics adds brick masks (`solid`, `full`, `large`, `action`, `coarse`, revision), an
air-only flag and a count of special states (technique). Per state, `CollisionTable` holds the shape
class (empty, full cube, one box, constant compound, context dependent), the grid lines of the shape
(PHY-SHP-005) and bit flags: fluid, climbable, path type byte, tag bits, and the coarse triggers of
PHY-ORD-008/013 (a state with a coordinate off the 1/16 grid; potent sulfur, which reaches 25 blocks
up).

- The one-box fast path tests the box as a box list per axis, but it still gives the grid lines of
  the shape to the step-up (PHY-SHP-005, PHY-STP-004): a carpet has the two lines 0 and `1/16` on
  `y`, and 0 and 1 on `x` and `z`. A box off the `1/8` grid on one axis keeps only its faces on all
  three axes.
- The exact scan visits positions with `x` fastest, then `y`, then `z` (PHY-SHP-007).
- Each loaded section has masks. A section of one state, or a section from the palette of its
  states, has the bricks of all solid states set. Two flags come from the palette: `large`, a state
  that can reach out of its cell (a large shape or a moving piston), and `inside`, a state that acts
  on a box inside it. Without `large` in the sections of a scan region, the scan reads only the
  inner blocks of the region, because a block on an outer face counts only with a large shape. For
  a walker on flat ground this is about 8 blocks instead of 64 for the supporting block, and about
  36 instead of 150 for a move. Without `inside`, the inside step of the block effects reads no block.
- The shape context is built only when a context-dependent state is in the zone; the world border
  is tested only when the zone is near it.
- A task keeps a cache of 64 section references.

### 6.8 In-air batch

`MotionKind` is a value, so entities of several classes share an archetype. The in-air entities batch
in f64 lanes of 4, bit-identical to the scalar path. The batch path needs a sort by class or a marker
per class. This is planned and not measured; the migration pull request measures the batch.

### 6.9 A moving entity kind from a plugin

`mcrs_minecraft_engine_entity_motion` moves the entities that are not players, and
`mcrs_minecraft_engine_entity_motion_replication` shows them to viewers. The motion crate does not
depend on the replication crate, on `mcrs_minecraft_engine_physics` or on
`mcrs_minecraft_engine_world`. A host adds `MotionPassPlugin` and places `MotionPass` in its tick.
In `MotionSteps`, a system of the host calls `MotionStep::run`. `MotionSteps` is the part of
`MotionPass` after `MotionRelease`. The call takes the block source, the kernel tables, the `Side`
and the tick of the host. After `MotionSteps`, the plugin replaces the list of `Movers`. Before the pass, the host gives the tiles of the changed blocks to
`StirredTiles::replace`. On a server,
`EntityMotionPlugin` of `mcrs_minecraft_engine_physics` does these steps over `BlockWorld`.
A plugin adds a kind in these steps:

1. Spawn the entity with a `MotionState` and a `MotionKind` that holds the `MotionRow` of the kind.
   An entity that gets them through commands before `MotionPass` moves in the same tick.
2. Add an `Appearance` when viewers see the kind. The replication plugin holds each new
   `MotionState` with `MotionHold`. In `MotionRelease`, it gives a `MotionId` to each held entity
   with an `Appearance`, and releases it. Thus, with the replication plugin, an entity without an
   `Appearance` does not move.
3. To see an entity before it moves, insert a `MotionHold` and remove it in `MotionRelease`.
4. Read the result after `MotionPass`. `StepOutcome` holds the start position, the collision flags
   and the outcome of the kernel, for example a landing. `Movers` lists the entities whose motion
   changed in a way that a viewer sees.
5. Give an impulse as a write to `MotionState` in a system outside `Simulate`. The write wakes a
   resting entity. Inside `MotionPass`, only the pass writes `MotionState`.
6. End the entity: despawn it, or remove its `MotionState`. The pass does not end an entity at the
   lifetime of its `MotionRow`. The replication plugin then publishes the removal and removes the
   `MotionId`. A later `MotionState` gets a new id and a new spawn.
7. Show the entity with `Appearance { kind, look }`. `kind` is the type data of the entity, with its
   `ClientMotion` and its `PreciseSync`. For a kind that `mcrs_minecraft_replication` does not know,
   `look` is `SpawnLook::Custom` with the data of `add_entity` and the entries that differ from
   their defaults.
8. Change the entity data with a `MobReport::Entries` event. The next tracker tick sends the
   entries that changed (TRK-DATA-005).
9. Push the own tile events of the entity after `MotionPublish`, to the tile of its
   `PublishedTile`. Thus the viewers get the spawn before the events of the plugin, and the events
   reach the viewers that know the entity.

`mcrs_minecraft_vanilla_item_entity` and the falling block plugin
`mcrs_minecraft_vanilla_falling_block` follow these steps. The falling block plugin gives two review
points: `FallingBlockSet::StartReview` with a `PendingFall`, and `FallingBlockSet::LandingReview`
with a `Landed`. A system in these sets changes or cancels a start or a landing.

| Consumer | What it takes |
|----------|---------------|
| a bot | the kernel alone: `tick_entity` with its own block source |
| a headless client, a replay viewer, an online map | the motion pass without the replication crate |
| a minigame server | its own kinds and `MotionRow` values; a cancel of a fall or a landing in the review sets of the falling block |
| a proxy, another server | the tracker; it is not public yet (#784) |

Open points:

- The tracker is not public (#784).
- Block behaviours from plugins and the automatic start of a fall are open (#785).
- The order of block packets against entity packets, and `add_transient_block`, are open (#787).
- A network id does not return to the allocator of the cell.

## 7. Partitioning

### 7.1 Units

| Unit | What it is for | Size |
|------|----------------|------|
| Tile | grid unit, replication tile, wake unit (`StirredTiles`) | 2 × 2 columns, full height |
| Task | a batch of entities of `par_iter_mut`, or a batch of tiles, ~50–100 µs | adaptive |
| Block tile | the work unit of the block domain, coloured 2 × 2 | 4 × 4 columns (2 × 2 tiles) |
| Tile group | the work unit of the parallel muxer (REPL §7.12) | 8 × 8 tiles |
| Cell | placement in another process or machine | a k-d area of region boxes |

### 7.2 Cells against one world with tiles, in one process

| Cost at 1M, 16 in-process cells | Cells | One world, tiles |
|---------------------------------|-------|------------------|
| Bodies near a boundary | 60k × 2 × 40 ns = 4.8 core-ms; 3.8 MB of mail per tick | none |
| Halo sections | ~6% more sections; halo changes applied again | none |
| Crossings | ~250 per tick × 2 µs | ~1.2k tile crossings, derived by the publisher; no move |
| Boundary exceptions | C2, C3, C5, C6 | none |
| Serial parts of the schedule | in parallel across cells | once per world |

Decision: one world per dimension in a process. Cells are for placement across processes.

### 7.3 Across processes

| Case | Rule |
|------|------|
| A zone crosses into a neighbour's column | the halo, `HaloBodies` (grid rows) and the halo copy of the position table, at the start of the tick |
| An entity walks into a neighbour's column | hysteresis `N` = 8 blocks; a mount tree crosses as one unit; the brain state goes with it |
| A displacement of 30 blocks or more in one tick that leaves the cell | a teleport at the next tick (technique); bounds C2 |
| Writes and block-domain writes for an entity of another cell | cell mail of `T`, into the queue of the apply step of `T + 1` |
| Claims on an entity of another cell | cell mail of `T`, arbitrated in the owner at `T + 1` (C5, PHY-CMP-017) |
| Split and merge | starting numbers (technique): one ring of buffer sections; merge within one section; split only when the area holds at least 8 sections of 16 × 16 chunks and at least one sixth of it is dead |

Conflicts in a layout of many cells: **C1** halo changes must apply in `DomainInput` (engine fix
E1); **C2** a zone past the halo, bounded by the teleport rule; **C3** a block action in another
cell's column is one tick later (PHY-INV-014); **C5** a claim from another cell joins the
arbitration one tick later; **C6** a death in the halo counts in the crowd fix-up (DMG-CRW-002) one
tick later; a block chain that leaves the cell is a cross-cell effect, allowed only
at the boundary level, and is reported.

### 7.4 Block shards, first step: random ticks

Random ticks are the first block domain to step in parallel inside one world, because their spec
fixes only a distribution (SIM-MODEL-106). The kernel already draws the picks of a section and the
values of its handlers from a stream keyed by section and step (`StreamKey::Section`).

**Order in the dimension tick.** The driver runs `random_step` after the scheduled block and fluid ticks
and before the block events (SIM-TICK-002, A.4). The colour phases and the serial step stay inside
that slot. The client broadcast comes after the random ticks and before the block events
(SIM-MODEL-107(a)); today the block step publishes all writes at the end, block events included
(engine fix E5).

**Area as a maintained index.** A section takes random ticks when its simulation level is at most
31 and its column reached block-ticking status (SIM-TICK-005). Each block tile keeps the list of such
sections that hold at least one position whose state ticks randomly; the list changes on a ticket
change and when a count of ticking positions crosses zero.

**Positions that tick.** Each section keeps `ticking: Vec<u16>`, the local indices whose state has a
non-empty `RandomTickTable::kinds` (block, fluid, or both). Every write goes through one wrapper of
`SectionBlocksMut::set` that compares the kinds of the old and new state: a push, or a scan of at most
`n` entries and a swap-remove. `apply_write` uses the same wrapper; a landing section builds its list
from its palette. Memory: ~0.5 GB at 10k players.

**Picks drawn at the start of the step.** The index walks its tiles once in each step. For each
listed section of an owned column, `k` times, it draws `u` in `0..4096` from the stream of the
section; a `u` below `n = ticking.len()` is a hit at `ticking[u]`. Each hit then forks the stream of
its handlers from the stream of the section (`draw_random_picks`). The walk also gives the columns
that take precipitation: each tile keeps a bit for each column that holds a random-ticked section.
The kernel gets only the hits and the columns. A hit reads the state at its position, runs the block
handler if the state ticks, then the fluid handler with the state read before the block handler
(SIM-RND-002). Each pick is uniform over 4096 positions. A position that starts to tick during the
step takes picks from the next step (DEV-08).

**Precipitation picks (SIM-RND-004).** Each random-ticked column takes `k` gates of 1/48 in every
tick, with or without rain and weather. A hit runs at a uniform `(x, z)` of the column, at the height
of the motion-blocking heightmap. Freezing happens without rain; snow and cauldrons fill only during
rain. A block tile draws `m` from Binomial(`C × k`, 1/48) and picks `m` distinct (column, pick) slots,
from a stream keyed by block tile and step.

**Reach (SIM-MODEL-009).** The driver keeps the footprint bound per block state. Block tiles cover the
whole height, so only the horizontal extent matters.

| Handler (SIM-MODEL-009) | Horizontal bound | Fits a 32-block margin |
|-------------------------|------------------|------------------------|
| random ticks of Table 1 | ≤ ±5 | yes |
| sapling, vanilla tree features | ±10 (−13..+34 vertically) | yes |
| sapling whose tree feature the server cannot measure | none | no: serial |
| sugar cane, cactus, nether portal | ±1, reads down the column | yes |
| neighbour-changed: experimental wire ±17, piston ±14, powered and activator rail ±10 | ≤ ±17 | yes |
| neighbour-changed: structure block | ±96 | no: serial |
| shape functions (Table 4): default ±2; `nether_portal` ±22 | ≤ ±22 | yes |
| side effects of placement and removal (Table 5): default ±2; fire that builds a portal ±22; experimental wire ±17; piston placement ±14 | ≤ ±22 | yes |
| side effects (Table 5): `tripwire` ±42, `tripwire_hook` 42 along its line | ±42 | no: serial |
| scheduled-tick handlers (Table 6), default ±2 | ±2 | yes |
| block-event handlers (Table 7), default ±1 | ±1 | yes |
| command block (Table 3) | none | no: serial |
| block entities | not stated yet (SIM OQ-13) | until stated: serial |
| groups of Table 3 (wire networks, piston contraptions, water and lava contact, shape cascades) | none as a group | each item checks its own bound |

Every set without the keep-shape flag makes shape updates (SIM-SET-002 step 3). With the default
bounds of Tables 4 and 5, a grass or crop set and its shape updates and side effects stay within
about 3 blocks of the pick, far inside the 32-block margin, so they never take the serial path. Only
tripwire lines, structure blocks, command blocks, unmeasured trees and long chains of Table 3 can
leave a region.

**Parallel phases by tile colour.** Block tiles of 4 × 4 columns are coloured 2 × 2 on the global
grid. In one colour, each tile owns its **region**: the tile plus a margin of 32 blocks. Regions of
one colour are disjoint and cover the plane.

- A task **reads and writes only its region**. An item whose bound, placed at its target, leaves the
  region is an overflow, for a read as for a write.
- **Undo.** At the start of each pick the task records marks: its write log (position and old state
  of each set), its output buffers (writes, scheduled ticks, block events, drops, spawns, entity
  writes) and its chain state, plus a copy of the random stream. On an overflow the task restores the
  old states in reverse, truncates the buffers, and queues the pick with **the position it picked**.
- **Re-run.** Overflowed picks re-run with a region twice as wide, in parallel where those regions are
  disjoint; a pick that overflows again re-runs in the global serial step, in the order
  (tile key, section, pick index). A re-run uses the stored position and the stored stream state. It
  reads the section as it is then; if the state no longer ticks, the pick does nothing. This is the
  sequential order in which the pick ran after the other picks (SIM-MODEL-106(b)), with the chain as
  one unit (SIM-MODEL-102(a)). SIM-RND-001 lets the picks of different sections interleave, and a
  re-run is the same pick, with the same position and random values.
- **Dependency-driven phases.** A block tile starts phase `p`, colour `c` when the tiles whose regions
  overlap its region finished `(p, c − 1)` and it finished `(p − 1)`. This is the same partial order
  as global barriers, so the result is identical, without the idle time of a barrier.
  Decision for random ticks: the colours use global barriers. A commit of an overlay needs
  exclusive access to the world, and a start by dependency needs in-place writes or layered
  overlays. Change to dependency-driven phases only when a profile shows that the barriers cost
  more than the cost of the barriers.
- **Determinism.** Whether a pick overflows depends only on its region and the bounds; the serial
  order is a stable key.
- Each task has its own chain state, scratch and `StepOutput`; scheduled ticks take a calculated
  sequence (P-SEQ: tile key, then task sequence).

**Walk on threads.** The walk runs in batches of 32 block tiles on the threads of the tile tasks
(`Spread`). Each section has its own stream, so the hits and their order do not depend on the
threads. Each batch keeps its own buffers, and the walk joins the hits and columns of the batches
in the order of the tiles. A tile checks its listed sections in a pass of its own before the first
draw, so the cache misses of the sections overlap: with one pass, the walk is ~40% slower. After
the walk, the calling thread removes the slots of sections that are no longer listed.


### 7.5 Block shards as a whole

| Domain | Global items (A.2) | Plan |
|--------|--------------------|------|
| Random ticks, precipitation | streams, section order: free | §7.4 |
| Block entities | claims (PHY-CMP-017); bounds not stated yet (SIM OQ-13) | per block tile once SIM states their bounds; until then one serial step per phase |
| Scheduled block and fluid ticks | sequence, budget, collect order | per block tile; each tile collects due ticks by the global key; when the budget binds, a merge of the sorted keys finds the cut; calculated sequence |
| Neighbour and shape updates | the chain | inside the phase that causes them; bounds checked per item; undo and re-run |
| Block events | one FIFO per dimension | per tile FIFO; undo and re-run |
| Redstone mechanisms, flying machines, long tripwires, structure blocks | chains and ticks that cross regions every tick | causal groups (SIM-MODEL-102(a), second way; ARCH §9): a group whose items overflowed in recent ticks becomes its own partition, the union of the regions it touched, **started first** in each phase; a per-tick split by footprints (union-find over SIM-MODEL-009 bounds) separates parts that no longer touch |

The phases keep their order (A.4): scheduled block ticks, fluid ticks, random ticks and
precipitation, the broadcast cut, block events, block entities. Raids run in `DomainInput`
(AIT-CPL-006). Each phase ends when its tiles and
its serial step end; the next phase starts per tile by dependency, not by a global barrier. Messages
between tiles exist only in a boundary-level instance.

## 8. Players and the movement checks

### 8.1 Layers

| Layer | Where | Notes |
|-------|-------|-------|
| L0 | network threads, per connection, after the frame decode | MVC-CHK-023; a NaN or infinite value disconnects (engine fix E4) |
| L1 | network threads, per connection, in stream order (MVC-LAY-003) | `MoveGuard`; probes inserted at write time (§8.5) |
| L1 → L2 | `DimInputs` | one `ClientTick` per client tick |
| L2 | the world, `Simulate`, parallel over players | in parallel with `entity_tick` |

L0 and L1 leave the serial `bridge_inbound` loop (~5 ms at 50k packets per tick today).

### 8.2 L2 next to the entity pass

L2 reads a `BlockOverlay` of the client blocks, `BodyGrid`, the start-of-tick `ColliderEnds` view and the
pusher history; it writes `Prediction`, `PlayerRows` and the player layer of `BodyGrid`.
PHY-CMP-021 defines the start-of-tick state of a player as its state after the moves of this tick, so
`predict` runs before the main pass: a main-pass tile starts when the players whose old position is
in the tile or its neighbour tiles are predicted. A player that moves farther in one tick (a fast
move or a teleport) marks its new tile, which waits for it too. Entities thus see a move in the same
tick, as in the reference; PHY-DEV-012 is not used.

`predict` costs ~28 core-ms at 10k players, ~1.1 ms on 32 cores; its tasks overlap the collider
groups, the explosion groups and the tiles with no player near them.

In a layout of many cells, the copy of each cell must hold these player states before the pass
(PHY-CMP-021, PHY-INV-014). The owner sends the accepted rows of the players near a boundary to the
neighbour in a short exchange between `predict` and the pass, a few hundred rows per boundary. A
server that does not want this exchange sets PHY-DEV-012 for all cells, so one cell and many cells
still agree.

A fast path (technique, where MVC allows it): with no collision in the tick and an unchanged box, the
collision test of the new box is skipped.

### 8.3 The client world view and kept changes

The client view is a `BlockOverlay` over the current `CollisionWorld`:

- `uncertain: FxHashMap<BlockPos, (old, new)>`, the changes in a window (MVC-LAT-004, LAT-006);
- `undone: FxHashMap<BlockPos, BlockStateId>`, the old states for a replayed tick (MVC-PRED-017);
- `unloaded`, the columns the client does not have.

A section that holds an overlaid block does not trust its masks: its lookups take the per-block
path and check the overlay first. MVC-LAT-006 runs the step twice, with the old and with the new
states of the uncertain blocks.

The kept changes (MVC-PRED-018) live in a store per world plus an index per player:

```rust
pub struct KeptChanges {                    // per world: block states once, shared by players
    entries: Vec<KeptChange>,               // pos, old, new, published tick
}
pub struct PlayerKept {                     // per player: the probes differ per player
    refs: VecDeque<KeptRef>,                // entry index, probe before, probe after, applied tick
}
```

A player's entry leaves when `replay_depth` client ticks passed after its first applied client
tick, or when no recorded box, grown by the near zone, can touch it. A world entry leaves when no
player refers to it.

### 8.4 Pusher history

MVC-UNC-006 lets L2 use a superset of pushers; MVC-UNC-007 states the kept positions. A pusher inside
the near zone of a viewer is always in tier 0 for that viewer: tier 0 covers 32 blocks (REPL §7.4),
and the near zone is smaller; the muxer must never lower such a tile below tier 0 by the cap or the
budget. The muxer reports the tier-0 positions of tick `T` in the input of `T + 1`. L2 takes them
as a slice of `Pusher`: the hull of the positions of each pusher over the server ticks that the
client can show, and the size of its box.

### 8.5 Probes and the write queue

A packet that is reordered or dropped on its way to the wire breaks the brackets of the probes.

- L1 inserts its probes at write time, after the order on the wire is final, on the network thread
  of the connection.
- No packet is dropped. A client that cannot take the packets is disconnected, as vanilla does.

### 8.6 The server copy of a player

The server copy is the state in `PlayerRows`. Its velocity changes in the order of PHY-CMP-023:

1. In `predict`, when an accepted move goes up from the ground, the copy jumps (PHY-INP-005).
2. During the main pass, server entities push the copy and give it impulses. The copy is a push
   neighbour with its state of PHY-CMP-021. These contributions do not go through the apply step
   of §3.7, because the player body layer keeps its own path; they land in step 3, in key order.
3. The server part of the player runs after the main pass of the tiles near it, in parallel over
   players: it adds the pushes and impulses of step 2; multiplies the velocity by 0.98
   (PHY-GRV-009); sets small components to zero (PHY-GRV-008); applies the block effects without the
   fall distance (PHY-SRV-004, PHY-FALL-003); then pushes the server entities near it and takes the
   opposite push (PHY-PSH-004), a gather over `BodyGrid` in key order. The entities already
   count the push of the player in their own gather (PHY-CMP-012).

The copy does no travel step. MVC reads this velocity as the velocity that the server expects, and
an impulse that the server sends to the client (PHY-SRV-007) starts from it.

### 8.7 Crates

| Crate | Bevy | Content |
|-------|------|---------|
| `mcrs_minecraft_physics` (new) | no | numbers and the sine table, shapes, tables (types), masks, world and body views, the query of PHY-SHP-012, the move procedure, step-up, flags, ticks, `PlayerState`, `tick_player`, PHY-SRV-008 and PHY-SRV-010, push, rest, in-air batch, `BodyGrid` |
| `mcrs_minecraft_collision_table` (new) | no | builds `CollisionTable` from the block definitions of `mcrs_minecraft_block` |
| `mcrs_minecraft_entity_key` | no | `EntityUuid`, `WriterKey`, `YzxPos` (planned: `WriterKey` and `YzxPos` move here) |
| `mcrs_minecraft_move_validate` (new) | no | L0, L1 |
| `mcrs_minecraft_move_predict` (new) | no | L2, `KeptChanges`, `Pusher` |
| `mcrs_minecraft_engine_physics` (new) | yes | the resources of §6.2, `entity_tick`, the grid update, the apply step of the queues and claims, the life cycle, brick projection, tile events, the AI link |
| `mcrs_minecraft_brain` (new) | no | the AI stage kernel |
| `mcrs_minecraft_engine` (extended) | yes | L0/L1 hooks on the network threads, L2 systems, crossing, `PlayerRows` consumers |
| `mcrs_minecraft_replication` (extended) | no | the codec for entities that are not players (REPL §7.11) |

A foreign-host anticheat links `_physics`, `_collision_table`, `_move_validate` and `_move_predict`,
implements `CollisionWorld` and `BodyView`, and gives the `Pusher` list of each player.

## 9. Kernel API

```rust
pub trait CollisionWorld {
    type Section<'a>: CollisionSection where Self: 'a;
    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>>;
    fn border(&self) -> Option<&WorldBorder>;
}

pub enum SectionLookup<S> { Outside, Unloaded, Loaded(S) }

pub trait CollisionSection {
    /// `None` when the section has an overlay: the caller takes the per-block path.
    fn masks(&self) -> Option<&SectionMasks>;
    fn state(&self, at: LocalPos) -> BlockStateId;
}

/// An overlay of block states over a base world: explosion groups (§3.5) and the client view (§8.3).
pub struct BlockOverlay<'a, W> { base: &'a W, blocks: &'a FxHashMap<BlockPos, BlockStateId> }

/// The driver picks `BodyGrid` plus `ColliderEnds`, or a live view (exact-parity mode).
pub trait BodyView {
    fn colliders(&self, query: Aabb, mover: EntityUuid, out: &mut Vec<EntityCollider>); // PHY-SHP-009
    fn neighbours(&self, of: &GridRow, out: &mut Vec<GridRow>);                     // PHY-RCH-006
    fn position(&self, entity: EntityId) -> Option<PositionRow>;
}

/// The read-only inputs of every kernel function, passed as one value.
pub struct View<'a, W, B> { pub world: &'a W, pub bodies: &'a B, pub tables: &'a Tables }

/// A shape that can stop a box: a table shape at a block position, an entity box, or the space
/// outside the world border. Index-based, so the scratch buffers hold no borrow of the tables.
pub enum Collider { Block { shape: ShapeId, at: BlockPos }, Entity(Aabb), Border(BorderRegion) }

pub struct Tables {                         // PHY-NUM-005 reads the sine table of mcrs_minecraft_numeric
    pub collision: CollisionTable,
}                                           // a `MotionRow` (PHY-ORD-005, ORD-009) goes with each call

/// Which side runs the step: the flags, gravity and the bounce differ (PHY-FLG-002/003,
/// PHY-GRV-004, PHY-RST-001).
pub enum Side { OwnClient, Server { authority: bool } }

/// The state `S` of PHY-ORD-007, exactly.
pub struct PlayerState {
    pub position: DVec3,
    pub velocity: DVec3,
    pub pose: Pose,
    pub flags: MoveFlags,                   // on ground, horizontal, vertical, below, minor
    pub support: Option<BlockPos>,
    pub support_missed: bool,               // "the last search found no block" (PHY-FLG-004)
    pub fall_distance: f64,
    pub stuck: DVec3,
    pub jump_delay: u8,
    pub sprint_timer: u8,
    pub jump_timer: u8,
    pub shift0: bool,
    pub jump0: bool,                        // PHY-INP-018: the press of the jump key
    pub forward0: f32,
    pub sprint0: bool,
    pub flight0: bool,
    pub crouching: bool,
    pub swimming: bool,
    pub using_item: bool,
    pub attributes: MotionAttributes,
    pub effects: MotionEffects,
    pub may_fly: bool,
    pub flight_speed: f32,
    pub in_powder_snow0: bool,
}

pub struct PlayerRotation { pub yaw: f32, pub pitch: f32 }   // raw, never wrapped (PHY-NUM-005)

/// `input` holds the keys `K` (with the auto-jump flag), the rotation `R` and the facts `F` of
/// PHY-ORD-007 (spectator, passenger and a vehicle that takes a jump, sleep, food, the sprint
/// window). `input.push` (PHY-PSH-008) adds to the velocity before step 1 of PHY-ORD-001. The
/// old position and rotation stay with the caller (PHY-ORD-007, PHY-SRV-010 step 7).
pub fn tick_player<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>, side: Side, start: &PlayerState, input: &PlayerInput,
    out: &mut PlayerState, actions: &mut Vec<BlockAction>, scratch: &mut Scratch,
);                                          // derives the box from the pose on read
/// PHY-CMP-023 step 3, on the server copy; step 1 is `server_jump`.
pub fn server_part<B: BodyView, W: CollisionWorld>(
    view: View<'_, W, B>, state: &mut PlayerState, copy: &ServerCopy<'_>,
    actions: &mut Vec<BlockAction>, scratch: &mut Scratch,
) -> PushOutcome;

/// `entity` holds the body, the living state (`None` for items, orbs, TNT and falling blocks),
/// the age, the phase offset and the rest state. `wake` is true when the read zone or the entity
/// changed since the last tick. The block actions (step, inside, fall, bounce) go to `actions`;
/// The step appends them to the queue of its thread. The keyed stream joins with the push (K4).
pub fn tick_entity<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>, side: Side, row: &MotionRow, entity: &mut EntityState, wake: bool,
    actions: &mut Vec<BlockAction>, scratch: &mut Scratch,
) -> TickOutcome;
/// PHY-CMP-012: the push of the neighbours from the start-of-tick grid, added to `velocity` in
/// the order of their UUIDs; the crowd draw of PHY-CMP-014 comes from `stream`; with
/// `max_entity_cramming` 0 only mutual `cap`-nearest pairs push (PHY-CMP-019); a pair that is
/// too near gets the push of the entity with the lower key (PHY-CMP-027).
pub fn push_entity<B: BodyView>(
    bodies: &B, of: &GridRow, rules: &PushRules, stream: &KeyedStream, velocity: &mut DVec3,
    scratch: &mut Scratch,
) -> PushOutcome;                           // pushed (PHY-SKIP-006), crowd damage, candidates
/// The in-air batch: four entities of one class whose read zone is in loaded sections of air only
/// (`in_open_air`); the result equals `tick_entity`, bit for bit.
pub fn tick_in_air(row: &MotionRow, lanes: &mut [EntityState; 4]) -> [TickOutcome; 4];

pub fn tick_mount_tree<W: CollisionWorld, B: BodyView>(/* a root and its passengers, root first */);

/// PHY-SHP-012: each collider with its source.
pub enum QueryVariant { Collision, Placement }      // placement: empty scaffolding and powder
                                                    // snow, no border source
pub enum ColliderSource { Block { pos: BlockPos, state: BlockStateId }, Entity(EntityId), Border }
pub struct SourcedCollider { pub collider: Collider, pub source: ColliderSource }
pub fn query_colliders<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>, entity: &QueryEntity, query: Aabb,
    feet: Option<f64>, variant: QueryVariant, out: &mut Vec<SourcedCollider>,
);

/// PHY-SRV-008: the velocity at the end of an observed player tick. With one horizontal collision
/// flag, the candidates are `x` clipped, `z` clipped and both clipped.
pub fn observed_velocity<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>, start: &PlayerState, observation: &Observation,
) -> VelocityCandidates;                    // up to three, and whether `y` is known

/// PHY-SRV-010: apply a `player_position` packet; rotation values are f32.
pub fn apply_teleport(state: &mut PlayerState, rotation: &mut PlayerRotation, packet: &Teleport);

/// PHY-MOV-001. A `piston` move first passes `limit_piston` (PHY-BLK-002), which the block
/// domain calls with the sums of the entity for the game tick.
pub fn move_body<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>, mover: &Mover, body: &mut Body, kind: MoverKind, displacement: DVec3,
    scratch: &mut Scratch,
) -> MoveOutcome;
/// PHY-SRV-006: resolves `probe.displacement` for `probe.bounds`, with step-up; changes no state.
#[must_use] pub fn resolve<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>, probe: &Probe, scratch: &mut Scratch,
) -> Resolved;                              // the step-up result keeps the plain displacement
/// True when a block shape, an entity collider or the border overlaps the box (PHY-BOX-006,
/// PHY-INP-010, PHY-INP-012); the caller shrinks the box as its rule states.
#[must_use] pub fn intersects<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>, entity: &QueryEntity, bounds: Aabb, entities: &mut Vec<EntityCollider>,
) -> bool;
#[must_use] pub fn read_zone(body: &Body, row: &MotionRow, displacement: DVec3) -> Aabb;

/// End boxes of the hard colliders per tile; `ready(tile)` is `None` until every collider group
/// that touches the tile has finished.
pub struct ColliderEnds { tiles: BTreeMap<TileKey, TileEnds> }
/// One resource with one owner system; the step reads the grid of the previous tick.
pub struct BodyGrid { tiles: BTreeMap<TileKey, GridTile> }
```

The per-thread queues and the sorted queue (planned) are generic: the write, claim and expiry types come with the steps
that fill them, and the kernel never reads them. An entry carries the target `EntityUuid`, a
`WriterKey`, the write tick, a sequence and the effect; the order never reads the Bevy `Entity`. `BlockOverlay` returns
no masks for a section with an overlay block, because the masks of the base do not know the overlay.

The kernel functions take one `View` in place of the world, the bodies and the tables, and a
query returns a `SourcedCollider` in place of a tuple. A function with more than seven inputs
takes a struct (`Probe`, `Mover`). The behaviour is the same; the change keeps each signature
short.

**Boundaries that must be fixed now**, because the serial proposals depend on them:

| Boundary | Why now |
|----------|---------|
| Queue entries with the target `EntityUuid`, a `WriterKey`, the write tick and a sequence | one serial system applies them in a fixed order, for any thread count |
| `BlockOverlay` as a `CollisionWorld` | explosion groups run during `Simulate` and must not write blocks |
| `ColliderEnds` ready per tile | a main-pass tile waits only for the collider groups that touch it |
| `BodyGrid` with one owner system | the step reads the grid of the previous tick, and no other system writes it |
| Mobs as ECS entities | the life cycle spawns and despawns them in batches; no row store (§6.3) |
| `MobIntent` component | the brain writes it and the step reads it (§5.3) |
| `masks()` returns `Option` | the client view and the overlays must force the per-block path |
| `Side` and `PlayerState` | `tick_player` cannot be bit-exact without them |

## 10. Verification

### 10.1 Kernel tests without Bevy

- PHY-GRV-007 factor `0.546000063419342`; PHY-SKIP-003 rest values; PHY-SHP-005 slab lines; a carpet
  gives the lines 0 and `1/16` on `y` through the one-box fast path; a box from `x = 0.25` to
  `0.75` and `y = 0` to `1/16` gives only 0.25 and 0.75 on `x`; PHY-SHP-007 scan order.
- PHY-NUM-005: the parity test of `mcrs_minecraft_numeric` checks the sine table entries and the
  sine and cosine of angles above one turn and below minus one turn.
- PHY-PSH-004/005 pairs, bit for bit; PHY-CMP-012 crowds equal for any entity order and task split.
- PHY-SHP-012: tagged sources; `feet` of the old position gives the scaffolding shape from before the
  move; the placement variant gives no border and empty scaffolding.
- PHY-SRV-008: three candidates for one horizontal flag; PHY-SRV-010: relative flags, rotated
  velocity in f32.
- Two boats driven into each other do not overlap; waves give the same result as the sequential key
  order.
- A TNT cannon: the projectile uses the push of the charges in the same tick, for any task split and
  rays traced serially or fanned out; the overlay hides removed blocks from later members only.
- Group finding: the same `F_T` gives the same groups for any entity order.
- A piston moves a mob in the same tick; the mob never overlaps the `moving_piston`.
- The ticking list equals a palette scan after random writes; the binomial group draw has the
  distribution of `k` independent gates per column.

### 10.2 Property tests

- PHY-INV-001..013; masks and shape classes give the same result as reading every block; the f64
  batch equals scalar.
- Rest equals no rest under random wakes, push neighbours included.
- Claims: for random claims of one item by players, hoppers, mobs, merges and commands, exactly one
  claimant gets it, the lowest `WriterKey`, for 1 and N threads.
- The grid that the owner system updates equals a full rebuild.
- Undo and re-run: random-tick results equal for 1 and N threads, and equal to a run with every pick
  serial in the order (pass, then re-runs).
- MVC-INV-001..009 over random packet streams through L0–L2; AIT-INV-003, 005 and 006.

### 10.3 Thread count, layout, keys

- `tests/it/tick_determinism.rs` with entities, AI (`AiDelay::Fixed(1)`), players, pistons and TNT:
  `Inline` against `Thread` gives equal world hashes and tile deltas.
- One world against four cells: equal, except C2, C3 and C5 cases, which the test reports.
- Save and load in the middle of a run: the run after the load equals the run without it.
- AI: a slowed stage with `AtLeast(1)` gives the same results per entity as `Fixed(1)`; none applies
  later than `T + max_lag`; the application per entity equals one grant step; revalidation drops whole.
- Exact-parity mode runs with `AiDelay::Inline`.

### 10.4 Regression tests for references across a boundary

Each test runs in one cell and in two cells (technique: the border bugs of other servers): a pet
teleports to its owner; a leash whose holder is in another cell; a fishing hook on an entity of
another cell; a selector whose box crosses a boundary; a deflected projectile keeps its owner; a
creaking whose home block is in another cell; block-break progress reaches every player in range.

### 10.5 Vanilla parity

- Server entities: PHY E1–E8, E15–E20, E28–E41 within 1e-6, flags exact; push and writes within PHY-DEV-004/005.
- Own player: PHY E9–E14, E21–E23, MVC C1–C7, bit for bit, no violation; they need the recorder of a
  vanilla client (U10).

### 10.6 Benchmarks

- `crates/mcrs_minecraft_physics/benches/`: one benchmark for each kernel entry point.
- `tick_load` and `bot_load`: 10k players with 100k and 1M entities; they measure the AI lag and the host time between ticks.

## 15. Open questions

1. Block shards: the share of picks that re-run (measure it); the rule that makes a group a causal
   partition; the bounds of block entities (SIM OQ-13).
2. Brain state: persistence and the move to another cell.
3. The muxer cost of 250k movers per tick, measured.
4. A single long chain bounds the worst case: 1,000 TNT (~30 ms) or a 15 ms redstone machine. The
   opt-in wave mode and speculation are later options.
5. U4/U6 in PHY: `OFF_GRID` shapes force coarse mode near them. Still open (design). PHY-SHP-005
   now states that a box off the `1/8` grid on one axis keeps only its faces on all three axes.
6. Projectiles: the ray move and the hit detection read the grid; their spec will state the reach.
   Still open (no spec yet).
35. Open: the density functions `upscale` and `domain_warp` of `26.4-snapshot-7`. We read
    `upscale` and evaluate it only when each axis is `interpolate_linear` and the X and Z cell
    sizes are equal. The vanilla data uses only this form. The other modes and `domain_warp`
    need a specification.
46. MVC-LAY-003 and §8.1, the thread of layer 1. Open (design). The order of the clientbound
    packets is final only when `dispatch_encode` writes them, and both directions must meet in
    one state without a lock. Layer 1 therefore runs in `bridge_inbound` and in the write of the
    queues; layer 0 runs on the reader of each connection. Moving layer 1 to the network threads
    needs the writer to pass the probe plan to the reader in the order of the wire.
57. `entity-tracking.md` TRK-RNG-001, TRK-RNG-003, the radius of the subscriptions. Decided for
    G1: `VIEW_RADIUS` becomes 4 tiles, which covers 128 blocks from any point of the own tile;
    TRK-DEV-004 records the square. Still open (design): with 3 tiles a subject can be outside the
    subscriptions at 97 to 128 blocks, and the muxer does not know the view distance of a viewer. The tracking range of a zombified piglin is
    128 blocks, and of a player 160 blocks at the view distance 10. The radius must cover the
    largest tracking range of the dimension, capped by the view distance of the viewer, and the
    codec must pair each subject by the range of its type.
59. SIM-FIRE-012 and SIM-SCH-005, the loaded footprint of a scheduled tick. Open (design). A fire
    tick reads and sets positions up to 2 blocks to the side and from 2 below to 5 above its
    position. Vanilla runs scheduled ticks in a column only when its eight neighbour columns are
    full, so each such read finds a loaded section. Here `BlockTicking` covers one ring past the
    simulation area, and the simulation area can have a vertical reach shorter than the column.
    Does the engine load each section within one column and the footprint height of a section
    that runs scheduled ticks? If not, which rule holds: the section does not run ticks, or the
    tick waits?
65. PATH-PRF-002, PATH-DET-001, the version of the path type table for each snapshot. Deferred to
    track A1. The type of a state reads the `pathfinding/*` tags and `fire`, `campfires`, `fences`,
    `walls` and `fence_gates`, so a data-pack reload changes it. `build_tables` builds
    the tables again in `DomainInput` when `CurrentRegistries` changes, before `Simulate`, and the
    inline runner of PATH-DET-004 reads the table of the same tick, so no version is necessary. The
    AI stage of A1 runs a search after its issue tick; then the snapshot of each tick must name the
    table (for example an `Arc`), and a search keeps the table of its issue tick.
66. Mobs on entities (#1067), what the decision leaves open. Open (design). Who writes the player
    layer of `BodyGrid` after `predict`, when the grid has one owner system. Whether one serial
    system keeps the apply step within budget at 1M entities. The migration pull request measures
    the second.
