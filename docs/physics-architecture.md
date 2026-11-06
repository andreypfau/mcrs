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

Section 14 is the implementation plan.

## 0. Decisions

| Question | Decision | Section |
|----------|----------|---------|
| Hot-loop model | Jacobi tick: each entity reads a frozen start-of-tick state and writes only its own row. The entity pass is one parallel map, with no islands and no locks | §3 |
| Work inside one system | per-tile dependency counters: a tile task starts when the tasks it reads from are done. Bevy stays the scheduler between systems | §3.9 |
| Sequential groups | collider groups (PHY-CMP-018) and explosion groups (PHY-CMP-020), found from the start-of-tick state; both start at the beginning of `Simulate` and overlap the main pass | §3.5 |
| Unit of causality | one entity for one tick; a mount tree is one unit (PHY-ORD-014); a sequential group is one unit | §3.5, §3.6 |
| Order key | the persisted birth key of PHY-CMP-013, never the network id | §4 |
| Writers | one writer per row in each phase; commands before the freeze; the block domain in `WorldWrite` (PHY-BLK-001); a server vehicle places its rider (PHY-SRV-011) | §3.7 |
| Writes and claims | binned by target tile; applied and arbitrated per target tile in parallel; writes at the start of the next tick (PHY-CMP-015), claims after the block domain (PHY-CMP-017) | §3.7 |
| Frozen state | the grid is written inside the pass into a back buffer; the freeze patches only tiles that changed after the pass | §3.4 |
| AI | eight reflexes in the hot tick; decisions in an AI stage on an isolated pool at the vanilla periods; versioned snapshot; late decisions applied per tile; proposals per component; `max_lag` 2; `AiDelay::Inline` in exact-parity mode | §5 |
| Data layout | hybrid: hot state in SoA rows of the kernel crate; cold state in ECS only for mobs; items, orbs and projectiles are rows only; players are rows | §6 |
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
  the Jacobi pass (the cap of §12.6 is the replacement);
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

All numbers are estimates from these assumptions and the kernel targets of §12.4. Benchmarks
replace them (§10.6).

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
| Writes to other entities | start of the next tick, in key order | PHY-CMP-015, PHY-DEV-005 | binned merge point (§3.7) |
| Claims | once per tick after the block domain; `command < entity < block`; several grants to one winner in the order of the target keys | PHY-CMP-017 | binned arbitration (§3.7) |
| Item phase | an offset 0–3 from the birth key | PHY-CMP-016, PHY-DEV-007 | §6.6 |
| Push cap | mutual top-`K`, only with `max_entity_cramming` 0 | PHY-CMP-019, PHY-DEV-008 | §12.6 |
| Explosions | sequential groups in birth-key order; rays of one explosion in parallel with keyed random values; opt-in wave mode for groups larger than `N`; other exploders as writes at `T + 1` | PHY-EXP-001..003, PHY-CMP-020, PHY-CMP-022, PHY-DEV-009..011 | §3.5 |
| Block domain on entities | same tick, after the entity pass, in block order | PHY-BLK-001/002 | §3.7 |
| Riding player | the vehicle writes its position; the client gives only the rotation | PHY-SRV-011 | §3.6 |
| Start-of-tick state of a player | the state after the moves of this tick are accepted | PHY-CMP-021 (PHY-DEV-012 is the alternative, not used) | `predict` before the main pass (§3.9, §8.2) |
| Server copy of a player | jump at move handling; pushes and impulses from the pass; then 0.98 decay, small components to zero, block effects, own push | PHY-CMP-023, PHY-SRV-004, PHY-GRV-009, PHY-PSH-008 | §8.6 |
| Mount trees | one unit, root first | PHY-ORD-014 | §3.6 |
| Rest | exact skip with the push and input rules | PHY-SKIP-001..007, PHY-INP-020 | rest per tile (§6.6) |
| Layout independence | | PHY-INV-014, PHY-INV-015 | same result for any thread count, task split or cell layout |
| Exact-parity mode | one world, sequential, live | ARCH Decision 20, PHY-ORD-010/011, PHY-PSH-006, AIT-LAG-011 | §3.8 |
| AI timing | reflexes, delay, `max_lag` | AIT | §5 |
| Push on the own player | bounded input, superset of pushers, kept positions | MVC-UNC-004..007, MVC-PRED-019 | §8.4 |
| Block work | semantic level; chains whole in one partition; footprint bounds of Tables 1–7 | SIM-MODEL-006, 009, 102(a), 106, SIM-RND-001 | §7.4, §7.5 |
| Raids | after the merge point, before the entity pass | AIT-CPL-006 | `DomainInput` (§3.7) |

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
row'_e, effects_e = tick(row_e, steering_e, F_T + end boxes of the hard colliders)
```

It reads the zones of PHY-RCH-001 and PHY-RCH-006. It writes only `row'_e` and appends effects
(PHY-RCH-002, PHY-CMP-015, PHY-CMP-017). PHY-INV-015 states that the result is then independent of
the order of the entities, the thread count, the task split and the cell layout.

The push follows PHY-CMP-012 with the order key of PHY-CMP-013 (§4). The crowd damage uses the
neighbours of the gather and the random value of PHY-CMP-014, so the pass has no shared random
source.

### 3.2 What is frozen, and what that costs

| State | How it stays frozen | Cost per tick |
|-------|---------------------|---------------|
| Blocks | by order: no block write between the freeze and `WorldWrite`; explosion groups write only into an overlay until `WorldWrite` | zero; a schedule test holds the order |
| Bodies and positions of other entities | the front buffer of `BodyGrid`; the pass writes the back buffer | written in the pass while the rows are hot (§3.4) |
| Hard colliders | `ColliderEnds`, ready per tile | §3.5 |
| Own row | read and written in place; no other entity reads `EntityRows` during the pass | zero |

### 3.3 Jacobi against interaction islands

| Cost at 1M | Jacobi | Islands (connected read zones, tick-list order inside) |
|------------|--------|--------------------------------------------------------|
| Partition work | none | ~2.5M pairs × 5 ns plus union-find at ~10 ns per pair: ~40 core-ms |
| Largest serial unit | one mount tree | the largest island. Zones grow the box by ≥ 1 block (PHY-RCH-001), so they percolate at about one mob per 9 m². A village or a farm is one island: 50k × 0.75 µs = 37 ms on one core |
| Wall time of the pass, 32 cores | ~7 ms | ≥ 37 ms in a dense area |
| Parity gained | none needed (PHY-DEV-002/003/004) | closer to the reference order, which no policy requires |

Decision: Jacobi. Islands stay only in the sequential exact-parity mode (§3.8).

### 3.4 The body grid, built inside the pass

Per tile, the grid holds the public rows sorted by bucket (4 × 4 blocks, full height), then by key.
A query merges the sorted runs of the buckets that it touches, which gives the order of
PHY-CMP-012 step 4 without a sort.

`BodyGrid` has a front and a back buffer per tile. The front buffer is `F_T`. Each tile task of the
main pass writes the back buffer of its tile from its rows right after it steps them, while they
are in cache: a counting sort of ~1k rows into buckets. After the pass, other writers can still
change rows: explosion groups, the block domain (pistons), claim grants, tile moves, the merge point
of the next tick, commands and the life cycle. Each of them marks the tile dirty. The freeze patches
the back buffer of the dirty tiles only, then swaps front and back.

| Option | Serial at the freeze, 1M | Core-ms | Complexity |
|--------|--------------------------|---------|------------|
| Rebuild dirty tiles at the freeze | ~0.7 ms (a phase of its own) | ~5.4 | low |
| Grid inside the pass, patch at the freeze (chosen) | ~0.1 ms (only tiles changed after the pass) | ~5, hidden in the pass | low: one more buffer per tile |
| Incremental, second generation for movers | ~0.1 ms | ~3 | high |

### 3.5 Sequential groups

Two rules make a small set of entities tick in sequence, in birth-key order. The freeze finds the
groups from `F_T`, so the set of groups does not depend on the thread count or the layout. Each
group is one task. Both kinds start at the beginning of `Simulate` and overlap the main pass,
through the per-tile dependency counters of §3.9.

| | Collider group (PHY-CMP-018) | Explosion group (PHY-CMP-020) |
|---|---|---|
| Why | two moving hard colliders that read each other's start box overlap, and PHY-COL-003 then lets them pass | a TNT cannon needs the push of the charges in the move of the projectile in the same tick (PHY-EXP-002) |
| Seeds | every awake hard collider (boats, rafts, minecarts, shulkers, happy ghasts) | every primed TNT with a fuse of 1 or less, and every creeper that reaches its fuse in this tick (AIT-RFX-002); both known from the row at the start of the tick |
| Growth | union of colliders whose read zones intersect, over the per-section list of hard colliders. A resting collider is not stepped: its start box is its end box | every entity whose read zone touches the reach (a cube of edge `4p`, grown by the displacement of the exploder), then every entity that an explosion of the group can ignite or move, until the group stops growing; groups that share an entity, or whose reaches intersect, merge |
| Finding cost | union-find over ~10k colliders at 1M: < 0.5 core-ms | grid and position-table queries per seed: ~2 µs per seed |
| Start | beginning of `Simulate` | beginning of `Simulate` |
| Reads | other colliders of the group live; everything else from `F_T` | other members live; blocks through an overlay `CollisionWorld`: the copy point plus the removals of the earlier explosions of the group; everything else from `F_T` |
| Writes | its rows; `ColliderEnds` for each tile that its zones touch | its rows; removed blocks into the overlay of the group; writes and claims into the bins |
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
rows of a unit are contiguous in one tile. The vehicle reads the input of a controlling passenger
from its start-of-tick row.

- A server vehicle that carries a player sets the position of the player (PHY-ORD-014 step 3,
  PHY-SRV-011). The pass does not write `PlayerRows`: it emits a `RiderPlaced` message, which the
  player-row writer applies in `Project`, in the same tick, before the output. While the player
  rides, `predict` takes only the rotation from the client and runs no movement check.
- A vehicle that a client controls is written in `ClientInput` from the vehicle moves, before the
  pass; MVC-VEH-001..014 check them. The pass does not step it.

### 3.7 Writers, the merge point and claims

**One writer per row in each phase.**

| Phase | Writer of `EntityRows` | Writer of `PlayerRows` |
|-------|------------------------|------------------------|
| `DomainInput` | commands (write-through); the merge point (writes of `T − 1`, local and mailed; mailed block-domain writes); proposal grants; impulses; the raid tick (AIT-CPL-006); the life cycle; then the freeze | commands, through the teleport of PHY-SRV-010; the merge point |
| `ClientInput` | client-controlled vehicles | — |
| `Simulate` | collider groups, the main pass (late AI decisions first, per tile), explosion groups (each on its own rows) | `predict` before the main pass; the server part of each player after it (§8.6) |
| `WorldWrite` | explosion overlays commit; entity block actions; the block domain (PHY-BLK-001) in block order; claim grants | claim grants |
| `Project` | tile moves of rows that left their tile | `RiderPlaced` |

**Commands before the freeze.** Commands run first in `DomainInput`, as the reference handles the
packets of the clients before the tick (PHY-ORD-010 step 1). A command writes rows through the store
API at once, so `/tp` followed by `execute at @s` in one function reads the new position. Its writes
mark tiles dirty, so the freeze sees them, and the groups found at the freeze are never stale. A
command that moves a player uses the teleport of PHY-SRV-010 and the protocol of TP-CONFIRM, never a
direct write of the accepted state.

**Block domain (PHY-BLK-001).** Pistons, slime and honey blocks that a piston moves (mover kind
`piston`, PHY-BLK-002) and shulker boxes (`shulker_box`) move entities. The block domain runs after
the entity pass and writes the rows directly, in block order. A block in one cell that writes an
entity of another cell sends the write in the cell mail; the owner applies it at the merge point,
before the freeze.

**Bins by target tile.** Every write and claim carries its `WriterKey` and goes into a bin of
(task, target tile). No lock is taken: each task owns its bins. At the merge point, and at the claim
arbitration, each target tile takes its bins from every task, sorts them by
`(target key, writer key, sequence)` and applies them, in parallel over target tiles. Writes from
another cell arrive in the cell mail and join the bins of the same point, so the f64 sum order is the
same in each layout.

```rust
pub enum WriterKey {              // variant order is the order of PHY-CMP-017
    Command { sequence: u64 },
    Entity { key: BirthKey },
    Block { pos: YzxPos },        // compared by y, then z, then x
}
```

**Claims (PHY-CMP-017).** A pickup by a player, a mob or a hopper, a merge of items or orbs, and a
removal are claims. The arbitration runs at the end of `WorldWrite`, after the block domain, per
target tile in parallel: per target, the lowest key wins, and the winner and the target apply the
grant in the same step, before `Output`. When one claimant wins several targets in one tick, the
grants apply in the order of the target keys (PHY-CMP-017 step 5). A claim from another cell arrives
one tick later and joins the arbitration of `T + 1` in the owner (PHY-CMP-017).

**Raids (AIT-CPL-006).** The raid tick runs in `DomainInput`, after the merge point and before the
freeze, so the entity pass sees what it changed; the scheduled ticks that the reference puts before
the raids ran in the previous `WorldWrite`. It is one serial step per raid, in the order of the raid
centres (y, z, x).

**Block actions of entities.** The step, inside and fall actions that the pass starts (PHY-RCH-002)
are effects. They apply in `WorldWrite`, before the scheduled ticks of the block domain, in the order
`(block position y, z, x, writer key)`, against the current state (PHY-CMP-010).

### 3.8 Exact-parity mode

Exact-parity mode is one world with every link `Inline` (ARCH Decision 20). It uses none of
PHY-CMP-008, 009, 012, 014, 015, 017, 018 or 020. One sequential driver runs the level tick in the
order of PHY-ORD-010:

1. The client packets of the tick, in arrival order; a move packet changes its player when it is
   handled (PHY-SRV-003).
2. The dimensions in creation order.
3. In each dimension: the world border, the weather, the time, the scheduled block ticks, the
   scheduled fluid ticks, the raids, the random ticks and precipitation, the client broadcast, the
   block events (SIM-TICK-002).
4. `step_entities_in_order` over the rows in `TickListSeq` order, with a live collider view,
   sequential push (PHY-PSH-006), immediate writes and claims, and AI with `AiDelay::Inline`
   (AIT-LAG-011).
5. The block entities, then the life cycle and the output.
6. The server part of each player, in connection order.

The kernel functions and the decision functions are the same as in the sharded mode.

### 3.9 Schedule of a tick

Bevy schedules the systems of the `TickSet` phases. Inside one system, the work units are tiles,
and a tile task starts when its dependency counter reaches zero: each task that it reads from
decrements the counter when it ends. The counters live in the system for one tick, on the Bevy
compute pool. This is the only scheduling below Bevy (ARCH Decision 10, revised).

```text
DomainInput:  commands (write-through); stage results (light, generation; columns land in
              parallel, one batched structural change); cell mail (halo changes [C1], HaloBodies,
              mailed writes, claims and block-domain writes); merge point per target tile;
              proposals per component; raid tick; life cycle per tile (rows, batched mob spawns)
              -> freeze: patch the back buffer of dirty tiles, swap; find collider and explosion
                 groups; set the dependency counters (players included)
ClientInput:  ClientTick per player (from the network threads, after L0/L1);
              client-controlled vehicles; client block edits only send BlockSetRequest
Simulate:     one system, `entity_tick`, over tiles and players with dependency counters:
                predict per player (accepts the moves, the jump of PHY-CMP-023 step 1) -> its
                  accepted row in the player layer of the front grid (PHY-CMP-021)
                collider groups (waves) -> ColliderEnds ready per tile
                main pass per tile, once the players and colliders near it are ready: late AI
                  decisions of its entities, reflexes, steering, tick_entity / tick_mount_tree,
                  push gather, rest, grid back buffer, bins
                explosion groups (overlay), starting at once
                server part per player, once the tiles near it are done (PHY-CMP-023 steps 2, 3)
WorldWrite:   explosion overlays commit per block tile; entity block actions;
              block phases (§7.5), each driven by dependencies per block tile:
                scheduled block ticks -> fluid ticks -> random ticks and precipitation
                -> broadcast cut -> block events -> block entities
              -> claim arbitration per target tile, grants
              -> exact-parity mode only: the driver of §3.8 replaces this phase and `Simulate`
Project:      tile moves; RiderPlaced; project_section_bricks; trim the kept changes
Output:       tile events from mover lists; AI delta (versioned), block changes, player table and
              decision issues to the AI link; setbacks; cell mail
```

- Inside a task: no `Commands`, no structural change. Structural changes happen only at the life
  cycle, batched.
- `tests/it/schedule_ambiguity.rs` covers the new plugins.

## 4. Keys

| Key | What it is | Persisted | Used for |
|-----|------------|-----------|----------|
| `BirthKey` | PHY-CMP-013: the birth tick, the source key (kind, then the parent key, the block position, the chunk and attempt, the saved list position, the command sequence or the login order), the sequence. Kept across a change of dimension | yes, with the phase offset of PHY-CMP-016 | the push order, the item phase, the crowd damage stream, the groups, the AI random streams, the bins, claims |
| `EntityId` | the global runtime id from the coordinator's ranges; the wire id | no | the slot map of the store, side tables, the protocol |
| `Entity` | the Bevy entity of the cold state of a mob or a player | no | ECS only |

The source key of kind `entity` holds the birth key of the parent, which nests without a bound. The
stored form (PHY-CMP-013, digest form) is:

```rust
pub struct BirthKey {             // 32 bytes; Ord in field order
    tick: u64,
    kind: SourceKind,             // load < player < command < spawn < block < entity
    source: u128,                 // block: y, z, x packed in order; spawn: chunk and attempt;
                                  // load: chunk and list position; command, player: sequence;
                                  // entity: a 128-bit digest of the parent's stored key
    sequence: u32,
}
```

The comparison follows PHY-CMP-024: tick, kind, source value (a digest as an unsigned 128-bit
integer), sequence, then the UUID. The UUID is made at birth from the parent's stored key, the birth
tick and the sequence, with a keyed hash that is independent of the digest function (a different
algorithm and key), and is saved with the entity. Two entities compare equal only when both 128-bit
functions collide, about `2^-256` per pair. The row keeps the UUID in a side table keyed by
`EntityId`; the comparison reads it only on a tie.

## 5. AI: reflexes in the tick, decisions in a pipeline

### 5.1 Reflexes in the hot tick

The eight reflexes of AIT-RFX-001..008 run in the main pass, in the tick in which the reference runs
them (AIT-INV-003). They read the start-of-tick state of AIT-SPL-002 and write the own row, or make
writes and claims.

| Reflex | Rule | Per-tick work (AIT-SPL-003) |
|--------|------|-----------------------------|
| steering: path follower, move, look, jump control, body rotation | AIT-RFX-001 | fixed; ~50 ns |
| creeper swell: start and continue, direction, counter, explosion | AIT-RFX-002 | fixed, one sight ray when it has a target; a swell that ends this tick seeds an explosion group |
| melee and ranged attack | AIT-RFX-003 | fixed, one sight ray |
| door on the path | AIT-RFX-004 | fixed; the open and close are block writes |
| escape: float in a fluid, climb out of powder snow | AIT-RFX-005 | fixed |
| effects of damage | AIT-RFX-006 | at the merge point; a fact to the brain |
| orb attraction | AIT-RFX-007 | one lookup in the player table and the per-column index |
| due events of the AI | AIT-RFX-008 | O(due), from the timing wheel |

Cost per living mob per tick: ~0.1 µs on average (fixed checks, one ray for the ~10% with a target,
the issue of its due decisions). AIT-INV-006 holds: the hot cost of a mob does not depend on its AI.

Also hot: physics, push, crowd count; the due events of physics; the projectile chunk-load budget per
tile (technique: 10 per tick), which defers a projectile, never deletes it.

### 5.2 Decisions and the AI stage

The decisions of AIT-DEC-001 run in the AI stage, a kernel without Bevy on a link per world.

- **Issue (AIT-CDN-001).** The hot tick issues each part at its vanilla period (AIT-VAN-025), with
  the phase from the birth key (AIT-DEV-003), never by backlog, wall time or load.
- **Versioned snapshot (AIT-LAG-001).** A decision of issue tick `T` reads the bodies and steering
  states at the start of `T`, the block copy of `T`, the player table of `T`, the facts before `T`,
  and the brain state after the earlier decisions of its entity. The tick sends, per tick, the rows
  and steering states that changed. The replica keeps, per entity, the current version and the
  versions of the last `max_lag + 1` ticks; the navigation projection keeps old section versions
  for the same span (copy on write). A decision of `T` processed late still reads the versions of
  `T`.
- **Path search** reads the navigation projection of `T`, the start-of-tick block copy (AIT U7). Its
  slices yield the pool after ~200 µs and resume on the same version.
- **Cheap inputs (technique).** Behaviour conditions re-run only when an input changed, by a change
  counter on the memories; only mobs with a path hear block changes on it (an active-navigator set
  per tile); path types and tag bits per state are precomputed.
- **Random values (AIT-LAG-008)** come from a stream of the world seed, the birth key, the issue
  tick and the part.
- **Output.** A decision writes only the brain and steering state of its entity (AIT-DEC-007). A
  change of another entity or of shared state is a proposal.

### 5.3 Application

- **Decisions per tile (AIT-LAG-002).** A decision applies before the hot step of its entity, at
  the start of the tile task in the main pass of `T + k`, after the merge point changed the entity,
  in the order `(issue tick, birth key)` within the tile (AIT-LAG-007). The merge point keeps only
  the proposals.
- **Revalidation (AIT-LAG-005/006).** The task checks each decision against the current state: the
  entity was removed or changed dimension; the attack target is not alive, not in the dimension,
  cannot be attacked, or is outside the follow range; the mob teleported after the issue tick and
  the decision has a path; the path starts more than `max_lag × s + 1` blocks from the mob; the path
  goes to an entity that is not alive or not in the dimension; a reflex took a control flag that the
  decision needs; the condition of a removal changed. A stale decision drops whole; the steering
  keeps its state; a fact goes to the brain.
- **Proposals (AIT-CPL-002..005)** keep one global point, at the merge point of `DomainInput`. They
  are split into components of proposals that name a common entity (union-find), and the components
  are granted in parallel, each in the order `(issue tick, birth key)`. An item merge grant enters
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
a burst shorter than two ticks hides in the lag. §12.3 sizes the stage.

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

| Criterion at 1M, 25% awake | Plain Bevy components + `par_iter` | Hybrid: SoA rows in the kernel crate, cold state in ECS |
|----------------------------|------------------------------------|---------------------------------------------------------|
| Skip of resting entities | visits every entity: ~15 core-ms | per tile: a quiet tile costs zero |
| Block reads | hash + `Query::get` per section: +37 core-ms | per-task section cache: ~2.4 core-ms |
| Snapshot and neighbours | a separate grid anyway | the grid is part of the store |
| Change detection | `Changed<T>` scans over 1M (1–2 ms each, serial) | mover lists from the pass |
| Structural changes | an ECS spawn per item, serial | items are rows; mobs spawn in batches |
| Estimated hot loop | ~260 core-ms | ~210 core-ms |

Decision: hybrid. Each hot field has one home.

### 6.2 Resources

| Resource | Content | Written in `Simulate` by |
|----------|---------|--------------------------|
| `EntityRows` | the rows of server entities, one segment per tile | the groups and the main pass, each on its own rows |
| `PlayerRows` | the rows of players: the accepted state and the server copy | `predict` and the server part of each player |
| `BodyGrid` | the grid and the position table, front and back buffer per tile | the main pass (back buffer of its tile) |
| `ColliderEnds` | end boxes of hard colliders, with a ready counter per tile | collider groups |
| `TileQueues` | per tile: late AI decisions, bins of writes and claims | the AI link (decisions), the tasks (bins) |

`predict` takes `ResMut<PlayerRows>` and `Res<BodyGrid>` (front); `entity_tick` takes the rest.
Bevy runs them in parallel.

### 6.3 Rows and the life cycle

```rust
pub struct TileRows {                       // SoA; sorted by motion class; mount trees contiguous
    position: Vec<DVec3>,
    velocity: Vec<DVec3>,
    rotation: Vec<Rotation>,                // raw f32 yaw and pitch, head yaw; never wrapped
    pose: Vec<Pose>,
    motion: Vec<MotionState>,               // flags, support, "last search found no block", fall
                                            // distance, jump delay
    class: Vec<MotionClass>,
    size: Vec<BodySize>,
    rest: Vec<Rest>,
    steering: Vec<Steering>,
    key: Vec<BirthKey>,
    id: Vec<EntityId>,
    awake: Vec<u32>,
    movers: Vec<u32>,
}
```

- **Row-only entities.** Items, experience orbs and projectiles have no ECS entity. Their cold state
  (item stack, owner, pickup delay end tick, age end tick, projectile owner) lives in side tables
  keyed by `EntityId`. A spawn or a despawn is a row operation in its tile, in parallel over tiles.
- **Mobs** keep an ECS entity for their cold state. Each tile reserves `EntityId`s from its range in
  parallel; the life cycle then spawns all new mobs of the tick with one `spawn_batch`.
- **Players** are rows (approved decision 3). `Transform` stops being the truth for a player:
  `column_streams`, `movement`, `cell_crossing`, `publish_player_tiles` and persistence read the
  accepted state in `PlayerRows` (MVC-PRED-012).
- Side tables are keyed by `EntityId`, never by row.

### 6.4 Classification

| Data | Class (bevy-ecs) | Home | Writer |
|------|------------------|------|--------|
| position, velocity, rotation, pose, motion state, rest, steering | authoritative | rows | the writer of the phase (§3.7) |
| motion class, birth key | authoritative, immutable | rows | the life cycle |
| body size | derived from class, pose, `scale`, baby; cached per row | rows | one projection at the merge point; `tick_player` derives it from the pose on read (§9) |
| accepted player state, server copy | authoritative | `PlayerRows` + `Prediction` | `predict`, the server part |
| grid rows, position table | materialized, frozen | `BodyGrid` | the pass (back buffer), the freeze patch |
| mover lists | temporal fact of the tick | rows | the pass |
| cold state of items, orbs, projectiles | authoritative | side tables by `EntityId` | the life cycle, claims |
| cold state of mobs and players | authoritative | ECS | their systems |
| brain state | authoritative | AI stage | the AI stage |
| box, `pushable`, `push_targets` | derived on read | — | — |

### 6.5 The grid row and the position table

```rust
pub struct GridRow {                        // 80 bytes; bodies only
    pub position: DVec3,
    pub half_width: f32,
    pub height: f32,
    pub key: BirthKey,
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

The position table covers what the hot steps read outside the grid: a look target, the player that
an orb moves to, a leash holder, the target of a homing bullet. A reference to an entity in another
cell reads the halo copy of this table, never a null.

### 6.6 Rest and wake per tile

- Each tile has a stamp: the largest of the block change counters of its sections and their ring,
  the grid revision of the tile and its neighbours, the border revision, and the decisions,
  impulses and block-domain writes that target it. A tile whose stamp did not change skips every
  resting row with no work; in a changed tile each resting row compares its zone stamp (10 ns).
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
  the shape to the step-up (PHY-SHP-005, PHY-STP-004): a carpet has 17 lines on `y`.
- The exact scan visits positions with `x` fastest, then `y`, then `z` (PHY-SHP-007).
- The shape context is built only when a context-dependent state is in the zone; the world border
  is tested only when the zone is near it.
- A task keeps a cache of 64 section references.

### 6.8 In-air batch

Rows of one class are contiguous in a tile, so the in-air rows batch in f64 lanes of 4,
bit-identical to the scalar path.

## 7. Partitioning

### 7.1 Units

| Unit | What it is for | Size |
|------|----------------|------|
| Tile | memory segment, work unit, grid unit, replication tile | 2 × 2 columns, full height |
| Task | a batch of tiles, or a range of rows of one heavy tile, ~50–100 µs | adaptive |
| Block tile | the work unit of the block domain, coloured 2 × 2 | 4 × 4 columns (2 × 2 tiles) |
| Tile group | the work unit of the parallel muxer (REPL §7.12) | 8 × 8 tiles |
| Cell | placement in another process or machine | a k-d area of region boxes |

### 7.2 Cells against one world with tiles, in one process

| Cost at 1M, 16 in-process cells | Cells | One world, tiles |
|---------------------------------|-------|------------------|
| Bodies near a boundary | 60k × 2 × 40 ns = 4.8 core-ms; 3.8 MB of mail per tick | none |
| Halo sections | ~6% more sections; halo changes applied again | none |
| Crossings | ~250 per tick × 2 µs | ~1.2k tile moves × 50 ns |
| Boundary exceptions | C2, C3, C5 | none |
| Serial parts of the schedule | in parallel across cells | once per world; §12.1 removes them |

Decision: one world per dimension in a process. Cells are for placement across processes.

### 7.3 Across processes

| Case | Rule |
|------|------|
| A zone crosses into a neighbour's column | the halo, `HaloBodies` (grid rows) and the halo copy of the position table, at the start of the tick |
| An entity walks into a neighbour's column | hysteresis `N` = 8 blocks; a mount tree crosses as one unit; the brain state goes with it |
| A displacement of 30 blocks or more in one tick that leaves the cell | a teleport at the next tick (technique); bounds C2 |
| Writes and block-domain writes for an entity of another cell | cell mail of `T`, into the bins of the merge point of `T + 1` |
| Claims on an entity of another cell | cell mail of `T`, arbitrated in the owner at `T + 1` (C5, PHY-CMP-017) |
| Split and merge | starting numbers (technique): one ring of buffer sections; merge within one section; split only when the area holds at least 8 sections of 16 × 16 chunks and at least one sixth of it is dead |

Conflicts in a layout of many cells: **C1** halo changes must apply in `DomainInput` (engine fix
E1, §14); **C2** a zone past the halo, bounded by the teleport rule; **C3** a block action in another
cell's column is one tick later (PHY-INV-014); **C5** a claim from another cell joins the
arbitration one tick later; a block chain that leaves the cell is a cross-cell effect, allowed only
at the boundary level, and is reported.

### 7.4 Block shards, first step: random ticks

Random ticks are the first block domain to step in parallel inside one world, because their spec
fixes only a distribution (SIM-MODEL-106). The kernel already draws the picks of a section and the
values of its handlers from a stream keyed by section and step (`StreamKey::Section`).

**Order in the level tick.** The driver runs `random_step` after the scheduled block and fluid ticks
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
from its palette. The kernel reads the list through an accessor of `SimulatedSections`. Memory:
~0.5 GB at 10k players.

**Picks without misses.** For each listed section, `k` times: draw `u` in `0..4096`; read
`n = ticking.len()` again; if `u < n`, take `at = ticking[u]`, read the state at `at`, run the block
handler if the state ticks, then the fluid handler with the state read before the block handler
(SIM-RND-002). This is the distribution of a pick over 4096 positions; no deviation entry.

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
- **Determinism.** Whether a pick overflows depends only on its region and the bounds; the serial
  order is a stable key.
- Each task has its own chain state, scratch and `StepOutput`; scheduled ticks take a calculated
  sequence (P-SEQ: tile key, then task sequence).

**Cost model.** `S × k × c_draw + H × c_handler + P × c_rain + undo`, at 10k players: 2.1M listed
sections × 3 × 3 ns ≈ 19 core-ms; ~185k handler runs × ~0.3 µs ≈ 55 core-ms (range 37–185); ~53k
precipitation runs × 0.2 µs ≈ 11 core-ms; marks and log ≈ 7 core-ms. Total ~92 core-ms parallel, plus
re-runs: only picks whose chain reaches a tripwire line, a structure or command block, an
unmeasured tree or a long chain re-run; ≤ 0.1% of picks, an assumption to measure (≤ 0.3 ms, mostly
in parallel with a doubled region).

### 7.5 Block shards as a whole

| Domain | Global items (A.2) | Plan | Step (§14) |
|--------|--------------------|------|------------|
| Random ticks, precipitation | streams, section order: free | §7.4 | D1–D3 |
| Block entities | claims (PHY-CMP-017); bounds not stated yet (SIM OQ-13) | per block tile once SIM states their bounds; until then one serial step per phase | D4 |
| Scheduled block and fluid ticks | sequence, budget, collect order | per block tile; each tile collects due ticks by the global key; when the budget binds, a merge of the sorted keys finds the cut; calculated sequence | D5 |
| Neighbour and shape updates | the chain | inside the phase that causes them; bounds checked per item; undo and re-run | with each |
| Block events | one FIFO per dimension | per tile FIFO; undo and re-run | D6 |
| Redstone mechanisms, flying machines, long tripwires, structure blocks | chains and ticks that cross regions every tick | causal groups (SIM-MODEL-102(a), second way; ARCH §8, §9): a group whose items overflowed in recent ticks becomes its own partition, the union of the regions it touched, **started first** in each phase; a per-tick split by footprints (union-find over SIM-MODEL-009 bounds) separates parts that no longer touch | D7 |

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

L2 reads `ClientView`, the front grid of the entities, the start-of-tick `ColliderEnds` view and the
pusher history; it writes `Prediction`, `PlayerRows` and the player layer of the front grid.
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

`ClientView` is an overlay over the current `CollisionWorld`:

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
budget. The muxer reports the tier-0 positions of tick `T` in the input of `T + 1`. L2 reads them
through:

```rust
pub trait PusherHistory {
    fn positions(&self, entity: EntityId, ticks: TickRange, out: &mut Vec<SentPosition>);
    fn candidates(&self, near: Aabb, out: &mut Vec<PusherCandidate>);
}
```

### 8.5 Probes and the write queue

`write_queue` reorders packets by priority and drops normal packets above a depth (today:
`muxer/bridge.rs`), and block updates are normal packets. Both break the brackets of the probes.

- L1 inserts its probes at write time, after the order on the wire is final, on the network thread
  of the connection.
- A packet that changes the prediction (block updates, chunk loads and forgets, motion, explosions,
  abilities, effects, attributes, `block_changed_ack`, teleports) is never dropped. Shedding applies
  only to the other packets.

### 8.6 The server copy of a player

The server copy is the state in `PlayerRows`. Its velocity changes in the order of PHY-CMP-023:

1. In `predict`, when an accepted move goes up from the ground, the copy jumps (PHY-INP-005).
2. During the main pass, server entities push the copy and give it impulses. The copy is a push
   neighbour with its state of PHY-CMP-021. These contributions are writes binned to the player's
   target tile; they land in step 3, in key order.
3. The server part of the player runs after the main pass of the tiles near it, in parallel over
   players: it adds the pushes and impulses of step 2; multiplies the velocity by 0.98
   (PHY-GRV-009); sets small components to zero (PHY-GRV-008); applies the block effects without the
   fall distance (PHY-SRV-004, PHY-FALL-003); then pushes the server entities near it and takes the
   opposite push (PHY-PSH-004), a gather over the front grid in key order. The entities already
   count the push of the player in their own gather (PHY-CMP-012).

The copy does no travel step. MVC reads this velocity as the velocity that the server expects, and
an impulse that the server sends to the client (PHY-SRV-007) starts from it.

### 8.7 Crates

| Crate | Bevy | Content |
|-------|------|---------|
| `mcrs_minecraft_physics` (new) | no | numbers and the sine table, shapes, tables (types), masks, world and body views, the query of PHY-SHP-012, the move procedure, step-up, flags, ticks, `PlayerState`, `tick_player`, PHY-SRV-008 and PHY-SRV-010, push, rest, in-air batch, `TileRows`, `BodyGrid`, `EffectBuffer` |
| `mcrs_minecraft_collision_table` (new) | no | builds `CollisionTable` from the block definitions of `mcrs_minecraft_block` |
| `mcrs_minecraft_move_validate` (new) | no | L0, L1 |
| `mcrs_minecraft_move_predict` (new) | no | L2, `ClientView`, `KeptChanges`, `PusherHistory` |
| `mcrs_minecraft_engine_physics` (new) | yes | the resources of §6.2, `entity_tick`, the freeze, the merge point and claims, the life cycle, brick projection, tile events, the AI link |
| `mcrs_minecraft_brain` (new) | no | the AI stage kernel |
| `mcrs_minecraft_engine` (extended) | yes | L0/L1 hooks on the network threads, L2 systems, crossing, `PlayerRows` consumers |
| `mcrs_minecraft_replication` (extended) | no | the codec for entities that are not players (REPL §7.11) |

A foreign-host anticheat links `_physics`, `_collision_table`, `_move_validate` and `_move_predict`,
and implements `CollisionWorld`, `BodyView` and `PusherHistory`.

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

/// An overlay of removed blocks over a base world: explosion groups (§3.5).
pub struct RemovedOverlay<'a, W> { base: &'a W, removed: &'a FxHashSet<BlockPos> }

/// The driver picks the front grid plus `ColliderEnds`, or a live view (exact-parity mode).
pub trait BodyView {
    fn colliders(&self, query: Aabb, mover: BirthKey, out: &mut Vec<EntityCollider>); // PHY-SHP-009
    fn neighbours(&self, of: &GridRow, out: &mut Vec<GridRow>);                     // PHY-RCH-006
    fn position(&self, entity: EntityId) -> Option<PositionRow>;
}

/// The read-only inputs of every kernel function, passed as one value.
pub struct View<'a, W, B> { pub world: &'a W, pub bodies: &'a B, pub tables: &'a Tables }

/// A shape that can stop a box: a table shape at a block position, an entity box, or the space
/// outside the world border. Index-based, so the scratch buffers hold no borrow of the tables.
pub enum Collider { Block { shape: ShapeId, at: BlockPos }, Entity(Aabb), Border(BorderRegion) }

pub struct Tables {
    pub collision: CollisionTable,
    pub sine: SineTable,                    // PHY-NUM-005: 65536 f32 entries
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

/// `input` holds the keys, the rotation of the tick, the push to use, and the facts that S does
/// not hold (spectator, passenger, sleep, food, the sprint window; see §15).
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
/// B0 bins them into the `EffectBuffer`. The keyed stream joins with the push (K4).
pub fn tick_entity<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>, side: Side, row: &MotionRow, entity: &mut EntityState, wake: bool,
    actions: &mut Vec<BlockAction>, scratch: &mut Scratch,
) -> TickOutcome;
/// PHY-CMP-012: the push of the neighbours from the start-of-tick grid, added to `velocity` in
/// the order of their birth keys; the crowd draw of PHY-CMP-014 comes from `stream`; with
/// `max_entity_cramming` 0 only mutual `cap`-nearest pairs push (PHY-CMP-019).
pub fn push_entity<B: BodyView>(
    bodies: &B, of: &GridRow, rules: &PushRules, stream: &KeyedStream, velocity: &mut DVec3,
    scratch: &mut Scratch,
) -> PushOutcome;                           // pushed (PHY-SKIP-006), crowd damage, candidates
/// The in-air batch: four rows of one class whose read zone is in loaded sections of air only
/// (`in_open_air`); the result equals `tick_entity`, bit for bit.
pub fn tick_in_air(row: &MotionRow, lanes: &mut [EntityState; 4]) -> [TickOutcome; 4];

pub fn tick_mount_tree<W: CollisionWorld, B: BodyView>(/* a run of rows, root first */);

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
pub fn apply_teleport(
    sine: &SineTable, state: &mut PlayerState, rotation: &mut PlayerRotation, packet: &Teleport,
);

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

/// Writes and claims, binned by target tile.
pub struct EffectBuffer { bins: SmallVec<[(TileKey, Vec<Keyed<Effect>>); 4]> }
pub struct Keyed<E> { pub target: BirthKey, pub writer: WriterKey, pub sequence: u32, pub effect: E }
```

The kernel functions take one `View` in place of the world, the bodies and the tables, and a
query returns a `SourcedCollider` in place of a tuple. A function with more than seven inputs
takes a struct (`Probe`, `Mover`). The behaviour is the same; the change keeps each signature
short.

**Boundaries that must be fixed now**, because the serial proposals depend on them (§14, step B0):

| Boundary | Why now |
|----------|---------|
| `EffectBuffer` bins by target tile, each entry with a `WriterKey` | the merge point and the claims run per target tile; a flat buffer forces a serial sort |
| `RemovedOverlay` as a `CollisionWorld` | explosion groups run during `Simulate` and must not write blocks |
| `ColliderEnds` ready per tile | a main-pass tile waits only for the collider groups that touch it |
| `BodyGrid` back buffer per tile | the pass writes the grid of its tile |
| Row-only life cycle (`spawn_row`, `despawn_row`, `reserve_ids`) | items, orbs and projectiles never touch ECS |
| Per-tile decision queue in `TileQueues` | late AI decisions apply in the tile task |
| `masks()` returns `Option` | the client view and the overlays must force the per-block path |
| `Side` and `PlayerState` | `tick_player` cannot be bit-exact without them |

## 10. Verification

### 10.1 Kernel tests without Bevy

- PHY-GRV-007 factor `0.546000063419342`; PHY-SKIP-003 rest values; PHY-SHP-005 slab lines; a carpet
  gives 17 lines on `y` through the one-box fast path; PHY-SHP-007 scan order.
- PHY-NUM-005: sine table entries 0, 16384, 32768, 49152; raw yaw values above 360 and below −360.
- PHY-PSH-004/005 pairs, bit for bit; PHY-CMP-012 crowds equal for any row order and task split.
- PHY-SHP-012: tagged sources; `feet` of the old position gives the scaffolding shape from before the
  move; the placement variant gives no border and empty scaffolding.
- PHY-SRV-008: three candidates for one horizontal flag; PHY-SRV-010: relative flags, rotated
  velocity in f32.
- Two boats driven into each other do not overlap; waves give the same result as the sequential key
  order.
- A TNT cannon: the projectile uses the push of the charges in the same tick, for any task split and
  rays traced serially or fanned out; the overlay hides removed blocks from later members only.
- Group finding: the same `F_T` gives the same groups for any row order.
- A piston moves a mob in the same tick; the mob never overlaps the `moving_piston`.
- The ticking list equals a palette scan after random writes; the binomial group draw has the
  distribution of `k` independent gates per column.

### 10.2 Property tests

- PHY-INV-001..013; masks and shape classes give the same result as reading every block; the f64
  batch equals scalar.
- Rest equals no rest under random wakes, push neighbours included.
- Claims: for random claims of one item by players, hoppers, mobs, merges and commands, exactly one
  claimant gets it, the lowest `WriterKey`, for 1 and N threads.
- The grid built in the pass and patched at the freeze equals a full rebuild.
- Undo and re-run: random-tick results equal for 1 and N threads, and equal to a run with every pick
  serial in the order (pass, then re-runs).
- MVC-INV-001..009 over random packet streams through L0–L2; AIT-INV-003, 005 and 006.

### 10.3 Thread count, layout, keys

- `tests/it/tick_determinism.rs` with entities, AI (`AiDelay::Fixed(1)`), players, pistons and TNT:
  `Inline` against `Thread` gives equal world hashes and tile deltas.
- One world against four cells: equal, except C2, C3 and C5 cases, which the test reports.
- Save and load in the middle of a run: the run after the load equals the run without it.
- AI: a slowed stage with `AtLeast(1)` gives the same results per entity as `Fixed(1)`; none applies
  later than `T + max_lag`; per-tile application equals one merge point; revalidation drops whole.
- Exact-parity mode runs with `AiDelay::Inline`.

### 10.4 Regression tests for references across a boundary

Each test runs in one cell and in two cells (technique: the border bugs of other servers): a pet
teleports to its owner; a leash whose holder is in another cell; a fishing hook on an entity of
another cell; a selector whose box crosses a boundary; a deflected projectile keeps its owner; a
creaking whose home block is in another cell; block-break progress reaches every player in range.

### 10.5 Vanilla parity

- Server entities: PHY E1–E8, E15–E20, E28–E41 within 1e-6, flags exact; push and writes within
  PHY-DEV-004/005.
- Own player: PHY E9–E14, E21–E23, MVC C1–C7, bit for bit, no violation; they need the recorder of a
  vanilla client (U10, step T1).

### 10.6 Benchmarks

- `crates/mcrs_minecraft_physics/benches/`: every row of §12.4; CI fails at 1.5× the target.
- `tick_load` and `bot_load`: 10k players with 100k and 1M entities; they measure every row of
  §12.1, the serial inventory of §11, the AI lag and the host time between ticks.

## 11. Serial paths and the critical path

At 1M on 32 cores, the parallel work of the tick is ~509 core-ms, so its floor is
509 / (32 × 0.8) ≈ 20 ms. The serial paths add to that floor. Values are wall time per tick.

| # | Serial path | Before (design as first written, or code today) | After | How |
|---|-------------|------------------------------------------------|-------|-----|
| S1 | L0/L1 in `bridge_inbound` | 5 ms (today) | 0 | network threads (§8.1) |
| S2 | stage results landing; sections are ECS entities | 1–4 ms | ~0.3 ms | columns land in parallel; one batched structural change |
| S3 | merge point, AI apply, proposals, ECS spawn and despawn | ~1.5 ms (2 in a mob farm) | ~0.3 ms | bins per target tile; decisions per tile; proposal components; row-only entities; batched mob spawns |
| S4 | freeze grid rebuild | ~0.7 ms | ~0.1 ms | grid in the pass, patch at the freeze |
| S5 | finding the groups | < 0.5 ms (1k TNT ~2) | < 0.5 ms | unchanged; hidden behind the patch per tile |
| S6 | commands; raid tick | 0.1–0.3 ms | 0.1–0.3 ms | serial by nature (command order, raid order) |
| S7 | collider groups | 5 ms for a pile of 5k boats, before the pass | hidden while it does not touch the busiest tiles; waves for sparse groups | start at once, per-tile counters |
| S8 | mount trees | small | small | one task item each |
| S9 | explosion groups | ~30 ms for 1k TNT, after the pass | hidden behind the pass up to its length; with the opt-in wave mode (PHY-CMP-022) ~1 ms | start at once, overlay, counters |
| S10 | block colour phases: 5 phases × (4 colours + serial step) ≈ 25 barriers, and imbalance | ~2.3 ms | ~0.5 ms | dependency-driven phases |
| S11 | serial re-runs of overflowed picks | 1–3 ms (most block-setting picks, while shape bounds were unknown) | ~0.1 ms | Tables 4–7 keep grass and crop chains within ~3 blocks; re-run first with a doubled region; only tripwire lines, structure and command blocks, unmeasured trees and long chains re-run serially |
| S12 | large redstone machines as serial groups | 5–15 ms | 5–15 ms of one task, started first, overlapping the other tiles | causal groups started first; per-tick split |
| S13 | scheduled-tick budget cut | 0.5–2 ms when it binds | 0.5–2 ms when it binds | the merge of sorted keys; rare |
| S15 | claim arbitration | ~2 ms (5 in a hopper farm) | ~0.2 ms | bins per target tile |
| S16 | the wait at `max_lag` | 0 when the AI stage keeps up | same | sizing (§12.3) |
| S18 | `predict` before the main pass (PHY-CMP-021) | 0 (it ran beside the pass) | ~0.3 ms on the critical path: tiles near players wait ~1.1 ms, others start at once | per-tile counters on the players near each tile |
| S17 | tier-0 positions for L2 | a dependency between ticks near 40 ms ticks | same | the muxer publishes tier 0 first (later) |
| S19 | Bevy barriers; `Changed<T>` scans | 0.5–1 ms; 1–2 ms per scan | 0.5 ms; none | change lists instead of scans |
| | **Typical serial total** | **~13 ms** | **~1.8 ms** | |
| | **Tick at 1M, 32 cores** | **~33 ms** | **~21.5 ms** | floor 20 ms |
| | **Worst case** (1k TNT, 5k boats, 15 ms redstone) | **~83 ms** | **~34 ms** | floor (509 + 300) / 25.6 ≈ 32 ms |

**Amdahl.** The serial fraction of the tick's work goes from 13 / (13 + 509) ≈ 2.5% to
1.8 / (1.8 + 509) ≈ 0.35%. The bound on the speedup goes from ~40× to ~300×, so the tick keeps
scaling with cores well past 32.

**The critical path after the changes**, at 1M on 32 cores, typical:

```text
commands + landing + merge point + raids + life cycle  ~0.7 ms   (serial parts, per tile where possible)
freeze patch, group finding                        ~0.5 ms
entity_tick (predict, main pass, groups overlapped) ~9 ms    (~238 core-ms / 25.6; tiles near
                                                              players start ~1 ms later)
WorldWrite block phases, dependency-driven         ~9 ms     (~230 core-ms / 25.6), plus re-runs
claim arbitration per target tile                  ~0.2 ms
Project, Output                                    ~1.5 ms   (mostly parallel)
                                                   ≈ 21.5 ms
```

The worst case stays bound by a single long chain: an explosion group of 1,000 TNT or a redstone
machine of 15 ms. The opt-in wave mode of PHY-CMP-022 removes the first; Block-STM style
speculation for redstone is a later option.

## 12. Budget

### 12.1 Whole tick

Core-ms per tick. "In tick" rows share the compute pool inside the 50 ms tick. "Beside" rows run on
their own threads.

| Work | Where | 100k | 1M | Note |
|------|-------|------|----|------|
| entity pass, awake | in tick | 30 | 187.5 | 0.75 µs × awake |
| grid back buffer in the pass, freeze patch | in tick | 0.5 | 5 | |
| collider groups | in tick | 0.4 | 3 | |
| rest checks, due events, decisions per tile, life cycle, merge point | in tick | 1 | 9 | |
| claim arbitration | in tick | 0.2 | 2 | |
| position table, section lookups, mover lists | in tick | 1.5 | 12 | |
| game events and their listeners | in tick | 0.5 | 5 | |
| tile events | in tick | 2 | 12.5 | |
| `predict`, push input, server part of players | in tick | 28 | 28 | 10k players |
| random ticks and precipitation | in tick | 92 | 92 | §7.4; range 77–222 |
| scheduled ticks, neighbour updates, block events | in tick | 50 | 50 | assumption |
| block entities | in tick | 40 | 40 | assumption |
| explosion groups | in tick | 3 | 3 | 10 explosions per tick |
| reflexes and the issue of decisions | in tick | 6 | 60 | ~0.1 µs per living mob |
| **In-tick total** | | **~255** | **~509** | |
| serial paths (§11) | in tick, serial | ~1 ms | ~1.8 ms | wall time |
| muxer, parallel per tile group | beside | 57 | 95 | |
| encryption (CFB8, ~125 MB/s per core) | network threads | 10–40 | 10–40 | 125–205 with all players in one crowd |
| L0 + L1 | network threads | 5 | 5 | |
| AI stage, decisions | AI pool, pipelined | 144 | 1,440 | 5% at 10 µs, 95% at 2 µs per mob per tick |
| AI stage, path search | AI pool, pipelined | 30 | 300 | 0.5% of living per tick × 100 µs |

The AI cost is calibrated on one data point of another server: about 23–31 µs per villager per tick.
At that cost (×3), the AI rows become ~520 core-ms at 100k and ~5.2 core-s at 1M.

### 12.2 Wall time in the tick

At 80% parallel efficiency, with the serial paths of §11 after the changes:

| Cores on the compute pool | 100k | 1M |
|---------------------------|------|----|
| 16 | ~21 ms | ~42 ms |
| 32 | ~11 ms | ~21.5 ms |
| 48 | ~8 ms | ~15 ms |

Without block shards, the block rows (~175 core-ms) run on one core: three and a half ticks, also
at 100k.

### 12.3 Machine fit

`max_lag` = 2 makes the AI stage a pipeline (§5.5). Cores that it needs for 20 ticks per second, at
40 core-ms of work per core per tick:

| AI cost | 100k | 1M |
|---------|------|----|
| target | 174 core-ms → **5 cores** | 1,740 core-ms → **44 cores** |
| observed on another server (×3) | 522 core-ms → **13 cores** | 5,220 core-ms → **131 cores** |

| Case | Dedicated pools | Shared (AI at low priority fills idle compute cores) | Fits one machine |
|------|-----------------|------------------------------------------------------|------------------|
| 100k, AI at target | 16 + 5 + 3 = 24 | (255 + 102 + 174) / 40 → 16 | yes |
| 100k, AI observed | 16 + 13 + 3 = 32 | (255 + 102 + 522) / 40 → 24 | yes |
| 1M, AI at target | 32 + 44 + 4 = 80 | (509 + 140 + 1,740) / 40 → 64, little margin | barely |
| 1M, AI observed | 32 + 131 + 4 = 167 | 147 | no |

### 12.4 Kernel targets (median, one core, warm cache)

| Benchmark | Target |
|-----------|--------|
| `resolve`, inert air / flat ground / step-up onto a slab | ≤ 60 ns / ≤ 300 ns / ≤ 1.2 µs |
| `tick_entity`, walker on ground, no neighbour | ≤ 600 ns |
| push gather, 3 / 24 neighbours | ≤ 100 ns / ≤ 600 ns |
| item in air, scalar / f64 batch | ≤ 80 ns / ≤ 30 ns |
| grid write per body in the pass; rest check in a changed tile | ≤ 15 ns; ≤ 10 ns |
| claim and write per entry in a bin | ≤ 100 ns |
| `tick_player`, one candidate; `predict`, fast path | ≤ 1.5 µs; ≤ 2.5 µs |
| L0 / L1 per packet | ≤ 30 ns / ≤ 100 ns |
| random tick draw | ≤ 3 ns |
| AI step, simple mob / villager-class | ≤ 2 µs / ≤ 10 µs |

### 12.5 Memory at 1M

| Part | Size |
|------|------|
| rows (~150 B) | ~150 MB |
| grid (80 B × 600k, two buffers) and position table (32 B × 1M) | ~130 MB |
| pusher history | ~56 MB |
| random-tick lists | ~0.5 GB |
| AI replica with versions, brains, navigation projection | ~120 + 120 + 400 MB |

### 12.6 Crowds and the push cap

The push costs `k²` pair tests in a crowd of `k`. With crowd damage on, the damage bounds a stable
crowd to about `n` (576 pairs at 24, ~6 µs). With crowd damage off, the mutual top-`K` cap of
PHY-CMP-019 applies (PHY-DEV-008). Jacobi also spreads the rows of a heavy tile over several tasks.

### 12.7 Preconditions for 1M

1. Block shards (§7.4, §7.5); block entities wait for their bounds (SIM OQ-13).
2. A parallel muxer per tile group, encryption and L0/L1 on the network threads.
3. AI throughput: 44 cores of AI work at the target cost.
4. No `Changed<T>` scan over a cold population larger than ~10k.
5. The serial-path changes of §11.
6. Correctness: the birth key and the claim arbitration before any determinism or dupe test.

## 13. Spec requests

Resolved earlier: R1 (PHY-CMP-013 digest form), R2 (PHY-CMP-017 cross-cell claims), R3 (PHY-CMP-020
step 4), R4 (PHY-EXP-003, PHY-DEV-010), R5 (MVC-VEH-001..014), R6 (AIT-CPL-005), R7 (AIT U1, U6),
R8 (AIT U2), R9 (SIM-MODEL-009), R10 (SIM-MODEL-102(a)), R11 (SIM-RND-004), and the earlier PHY,
MVC, REPL and AIT requests.

The requests of this revision:

| # | Rule | Change | Status |
|---|------|--------|--------|
| R12 | SIM-MODEL-009, SIM OQ-13 | Footprint bounds of shape functions and of the side effects of placement and removal | Resolved: SIM-MODEL-009 Tables 4–7 (shape functions, side effects, scheduled-tick handlers, block-event handlers). Open part: the bounds of block entities (SIM OQ-13) |
| R13 | PHY-CMP-008 | The moves of a player and the entity pass | Resolved: PHY-CMP-021 (moves before the pass conform); PHY-DEV-012 is the alternative, not used |
| R14 | PHY-CMP-017 | The order of several grants to one winner | Resolved: PHY-CMP-017 step 5 (target birth keys) |
| R15 | SIM-RND-001, SIM-MODEL-102(a) | A re-run uses the drawn position; picks of sections interleave | Resolved: SIM-RND-001 |
| R16 | PHY-CMP-020 | Opt-in wave mode | Resolved: PHY-CMP-022, PHY-DEV-011 |
| R17 | AIT-LAG-002 | Decisions at the start of the tile task | Resolved: AIT-LAG-002 |
| R18 | PHY-ORD-013 / MVC-MODE-003 | Coarse triggers as data | Resolved: MVC-MODE-003 (off-grid coordinates, potent sulfur up to 25 blocks below) |
| R19 | PHY-CMP-020 step 1 | A creeper that reaches its fuse seeds a group | Resolved: AIT-RFX-002 |
| R20 | REPL §7.4, MVC-UNC-007 | An entity in the near zone of a viewer is always tier 0 | Resolved in REPL §7.4 (this design's own document) |
| R21 | PHY-CMP-024 | The order of two keys after a digest collision | Resolved: PHY-CMP-024 (UUID from an independent function) |
| R22 | AIT-CPL-006 | The place of the raid tick | Resolved: AIT-CPL-006 (after the merge point, before the pass) |

No request of this note is open. The open spec question that this design depends on is SIM OQ-13
(block entities).

## 14. Implementation plan

Each step ships alone: it builds, passes its tests and the existing tests, and leaves the server
working. Steps in one track run in sequence; tracks marked ∥ run in parallel with each other.

### Track K — kernel (∥ with E, T, B1)

| Step | Crate | Scope | Rules | Tests | Exit |
|------|-------|-------|-------|-------|------|
| K0 | this note | §9 amended: `PlayerState`, raw yaw, sine table, `Side`, the three functions, the boundaries of B0 | PHY-ORD-007, PHY-NUM-005/007, PHY-SHP-012, PHY-SRV-008/010 | review | done in this revision |
| K1 | `mcrs_minecraft_physics` | numbers (f64 with the f32 list), sine table, f64 boxes, shapes with grid lines, scan order, the collision algorithm, step-up, the move procedure, flags, bounce, `query_colliders`, `resolve`, `intersects` | PHY-NUM-001..007, PHY-BOX-001..006, PHY-SHP-001..013, PHY-COL-001..005, PHY-STP-001..005, PHY-MOV-001..004, PHY-FLG-001..005, PHY-RST-001..004 | property PHY-INV-001..013 (10^6 cases each); kernel PHY-COL-005, PHY-STP-005, PHY-NUM-005 entries, slab and carpet grid lines, scan order; `HashMap` test world | all tests pass; benches within 1.5× of §12.4 |
| K2 | `mcrs_minecraft_collision_table` | build `CollisionTable` from the block definitions: f32 → f64 on the pixel grid, shape classes, grid lines, flags, coarse triggers, the motion rows | PHY-SHP-002, PHY-SHP-005, PHY-GRV-017, PHY-ORD-013 | every corpus state round-trips; the 90,186 corpus coordinates widen exactly; carpet 17 lines; default friction 0.6F | table built at startup in < 100 ms |
| K3 | `mcrs_minecraft_physics` | `tick_entity` for walker, item, primed TNT, falling block, orb; rest; in-air batch | PHY-ORD-002, 004, 005, 009, PHY-GRV-001..020, PHY-INP-003, 020, PHY-SKIP-001..007, PHY-RCH-001..005, PHY-FALL-001..003 | parity E1–E8, E15–E20, E28, E32 from `tools/vanilla-oracle` within 1e-6; property: rest equals no rest; batch equals scalar | parity fixtures pass |
| K4 | `mcrs_minecraft_physics` | push and crowd count; the cap | PHY-PSH-001..007, PHY-CMP-012, 014, 019 | kernel PSH-004/005 bit for bit; property: any row order and task split give equal velocities | E35 within PHY-DEV-004 |
| K5 | `mcrs_minecraft_physics` | `PlayerState`, `tick_player` with `Side`, `observed_velocity`, `apply_teleport`, the server part in the order of PHY-CMP-023 | PHY-ORD-001, 007, PHY-INP-001..019, PHY-SRV-003, 004, 008, 010, PHY-GRV-009, PHY-CMP-023, PHY-BOX-006 | kernel tests per rule; parity E9–E14, E21–E23 once T1 records them | bit-exact on the recorded ticks |

### Track T — tools (∥)

| Step | Crate | Scope | Rules | Tests | Exit |
|------|-------|-------|-------|-------|------|
| T1 | `tools/` | recorder of a vanilla client's serverbound moves behind a proxy | PHY U10 | records E9–E14 | fixtures checked in |

### Track E — engine fixes (each ∥ with every other step)

| Step | Crate | Scope | Rules | Tests | Exit |
|------|-------|-------|-------|-------|------|
| E1 | `mcrs_minecraft_engine` | apply halo changes in `DomainInput`, not in the block step (`block_simulation.rs`) | PHY-INV-014, PHY-CMP-009 | a change at `T` in one cell is in the copy of every cell at `T + 1`; one cell against four | test passes |
| E2 | `mcrs_minecraft_engine` | teleport ids start at 1; an accept checks the id and the position; a move during a pending teleport resends it (`teleport.rs`, `movement.rs`) | TP-CONFIRM-ID-001, ACC-001..003, RES-002, MVC-LAT-008 | unit tests per rule; fix the test that expects id 0 | tests pass |
| E3 | `mcrs_minecraft_network`, `mcrs_minecraft_engine` | the inbound rate rule; no disconnect on a burst of 100 packets or a stall of 3 s (`inbound_rate.rs`, `bridge.rs`) | MVC-INV-008/009 | burst and stall tests | tests pass |
| E4 | `mcrs_minecraft_engine`, `mcrs_minecraft_engine_world` | L0 before `raise_arrived`; NaN or infinite disconnects; raw yaw in `Rotation`; `player_position` with relative flags and real rotation | MVC-CHK-023, PHY-NUM-005, MVC-RSP-001, PHY-SRV-010 | L0 fuzz; a yaw of 720 survives; setback packet fields | tests pass |
| E5 | `mcrs_minecraft_engine` | broadcast cut after random ticks; at most one block packet per section per broadcast; block events to players within 64 blocks | SIM-MODEL-107(a), SIM-TICK-002, SIM-SET-005, SIM-BEV-004 | packet order and count tests | tests pass |
| E6 | `mcrs_minecraft_block` | default friction 0.6F | PHY-GRV-017 | definition test | test passes |
| E7 | `mcrs_minecraft_simulation` | precipitation picks with the group draw | SIM-RND-004 | distribution test (binomial tolerance) | test passes |
| E8 | `mcrs_minecraft_engine` | probes at write time; prediction changes never dropped; block updates not shed | MVC-LAT-001, MVC-TICK-002 | wire-order test with shedding forced | test passes |

### Track B — store, keys and the pass (B1 ∥ with K; B2 after K3)

| Step | Crate | Scope | Rules | Tests | Exit |
|------|-------|-------|-------|-------|------|
| B0 | `mcrs_minecraft_physics` | the boundaries of §9: `EffectBuffer` bins with `WriterKey`, `RemovedOverlay`, `ColliderEnds` per tile, grid back buffer, row-only life cycle, `TileQueues`, `masks()` as `Option` | — | compile-level and unit tests | types stable; later steps only fill them |
| B1 | `mcrs_minecraft_engine`, save | birth key, its digest form, the UUID made at birth, the phase offset, persistence | PHY-CMP-013, 016, 024 | save and load equality; one cell against four give equal keys | before any determinism test |
| B2 | `mcrs_minecraft_engine_physics` | `EntityRows`, `BodyGrid` (grid in the pass, patch at the freeze), `entity_tick` over tiles with counters, rest per tile, bins, merge point per target tile, row-only items and orbs | PHY-CMP-008, 009, 012, 015, PHY-INV-015 | tick determinism for 1, 2, 8 threads and task splits; grid patch equals rebuild | 100k items and walkers at ≤ 5 ms on 16 cores |
| B3 | `mcrs_minecraft_engine_physics` | claims per target tile; pickups, hoppers, merges, command kills | PHY-CMP-017 | property: one winner, lowest key, any thread count | dupe tests pass |
| B4 | `mcrs_minecraft_engine_physics` | collider groups with waves; explosion groups with the overlay, creeper seeds, the opt-in wave mode; counters | PHY-CMP-018, 020, 022, PHY-EXP-001..003, AIT-RFX-002 | boats do not tunnel; TNT cannon; group finding determinism | E29–E31 within PHY-DEV-002 |
| B5 | `mcrs_minecraft_engine_physics` | block-domain writes to rows; pistons; entity block actions | PHY-BLK-001/002, PHY-RCH-002, PHY-CMP-010 | piston moves a mob in the same tick | test passes |
| B6 | `mcrs_minecraft_engine`, `mcrs_minecraft_engine_physics` | players as rows; `Transform` no longer truth; `predict` before the main pass with per-tile counters; player layer of the front grid; server copy | PHY-CMP-021, 023, PHY-SRV-004, PHY-GRV-009, PHY-PSH-008, MVC-PRED-012 | an entity sees a player move in the same tick; the view, crossing and replication read the accepted state | existing tests pass |
| B7 | `mcrs_minecraft_engine_physics` | mobs with batched spawns; mount trees; the riding player; the raid tick in `DomainInput` | PHY-ORD-014, PHY-SRV-011, AIT-CPL-006 | passenger follows vehicle in the same tick | E30 within policy |

### Track C — movement checks (after K5, E2, E4, E8)

| Step | Crate | Scope | Rules | Tests | Exit |
|------|-------|-------|-------|-------|------|
| C1 | `mcrs_minecraft_move_validate` | L0 on the network threads | MVC-CHK-023, MVC-LAY-002 | fuzz; value table | ≤ 30 ns per packet |
| C2 | `mcrs_minecraft_move_validate` | L1 per connection, in stream order; probe planning | MVC-LAY-003, MVC-TICK-*, MVC-LAT-001..005, MVC-CHK-010 | MVC-INV-001..009 property tests | ≤ 100 ns per packet |
| C3 | `mcrs_minecraft_move_predict` | L2: candidates, fast path, replay, `ClientView` overlay, `KeptChanges`, push input, `PusherHistory`, setbacks | MVC-PRED-001..019, MVC-LAT-004..008, MVC-UNC-001..007, MVC-RSP-001..004 | parity C1–C7 with zero violations; replay sees the original blocks | 10k scripted players at ≤ 28 core-ms |

### Track D — block shards (D1 ∥ with B)

| Step | Crate | Scope | Rules | Tests | Exit |
|------|-------|-------|-------|-------|------|
| D1 | `mcrs_minecraft_simulation`, `mcrs_minecraft_engine` | ticking list, index per block tile, picks without misses, one task | SIM-RND-001..003, SIM-MODEL-106 | list equals palette scan; mean 7.32 ticks over 10,000 steps | random ticks at 10k players ≤ 92 core-ms |
| D2 | same | colour phases, region-only access, bounds of Tables 1–7, undo and re-run with the same pick, dependency-driven phases | SIM-MODEL-009, 102(a), SIM-RND-001 | 1 and N threads equal; equal to the serial order | re-runs ≤ 0.1% of picks |
| D3 | same | precipitation per block tile | SIM-RND-004 | distribution | — |
| D4 | same | block entities: one serial step per phase now; per block tile once SIM OQ-13 gives their bounds | SIM-TICK-002 step 12, PHY-CMP-017 | hopper claims | — |
| D5 | same | scheduled ticks per block tile, budget cut, calculated sequence | SIM-SCH-001..012 | budget cut equals the global order | — |
| D6 | same | block events per tile | SIM-BEV-001..004 | event order tests | — |
| D7 | same | causal groups started first; per-tick split | SIM-MODEL-102(a) | redstone fixtures equal to one task | 15 ms machine does not delay other tiles |

### Track A — AI (after B2)

| Step | Crate | Scope | Rules | Tests | Exit |
|------|-------|-------|-------|-------|------|
| A1 | `mcrs_minecraft_brain` | stage skeleton: versioned replica, issue at vanilla periods, delay modes, `max_lag` | AIT-LAG-001..004, 008, AIT-CDN-001/002 | AIT-INV-001, 002, 004 | `Fixed(1)` runs equal |
| A2 | `mcrs_minecraft_engine_physics` | reflexes in the pass | AIT-RFX-001..008, AIT-SPL-002/003 | AIT-INV-003, 006 | — |
| A3 | both | per-tile application, revalidation, proposals per component | AIT-LAG-002, 005..007, 009, 010, AIT-CPL-001..005 | AIT-INV-005; breed atomic | — |
| A4 | `mcrs_minecraft_brain` | decisions of the vanilla mobs, as their specs land | AIT-DEC-*, AIT-VAN-* | parity per mob | AI cost near target |

### Track N — replication (∥ after B2)

| Step | Crate | Scope | Rules | Tests | Exit |
|------|-------|-------|-------|-------|------|
| N1 | `mcrs_minecraft_replication` | codec for entities that are not players; update rule of client-simulated categories | REPL §7.11, PHY-CMP-001/002 | client model decodes; items at rest cost 0 bytes | — |
| N2 | `mcrs_engine_muxer` | parallel muxer per tile group; tier 0 for the near zone | REPL §7.4, §7.12 | 250k movers ≤ 10 ms wall | — |

### Track X — exact parity (after B7, A1)

| Step | Crate | Scope | Rules | Tests | Exit |
|------|-------|-------|-------|-------|------|
| X1 | `mcrs_minecraft_engine_physics` | sequential driver of §3.8 with `AiDelay::Inline` | PHY-ORD-010/011, PHY-PSH-006, AIT-LAG-011 | vanilla fixtures bit for bit, batch off | all PHY E-fixtures pass |

### Order

```text
K0 ─ K1 ─ K2 ─ K3 ─ K4 ─ K5 ─────────── C1 ─ C2 ─ C3
 └ B0 ┘          └ B2 ─ B3 ─ B4 ─ B5 ─ B6 ─ B7 ─ X1
B1 ───────────────┘   └ A1 ─ A2 ─ A3 ─ A4    N1 ─ N2
E1..E8, T1 (any time)   D1 ─ D2 ─ D3 ─ D4 ─ D5 ─ D6 ─ D7
```

## 15. Open questions

1. Block shards: the share of picks that re-run (measure it); the rule that makes a group a causal
   partition; the bounds of block entities (SIM OQ-13).
2. Brain state: persistence and the move to another cell.
3. The muxer cost of 250k movers per tick, measured.
4. A single long chain bounds the worst case: 1,000 TNT (~30 ms) or a 15 ms redstone machine. The
   opt-in wave mode and speculation are later options.
5. U4/U6 in PHY: `OFF_GRID` shapes force coarse mode near them.
6. Projectiles: the ray move and the hit detection read the grid; their spec will state the reach.
7. PHY-SHP-005 against §6.7 and §14 (K1, K2): the rule tries `n` from 0 to 3, so a carpet (one
   pixel high) gives the two `y` lines 0 and `1/16`. §6.7 and the K2 test state 17 lines, which
   needs `n` up to 4. The kernel follows the rule; one constant (`MAX_GRID_BITS`) changes it.
   Which value does the reference use?
8. PHY-SHP-005: "two values nearer than `1.0E-7` are one grid line" does not say which value
   stays. The kernel keeps the lowest.
9. PHY-FLG-004: the rule does not say what happens to "the last search found no block" while the
   on-ground flag is false. The kernel keeps the flag as it is.
10. PHY-MOV-002 against PHY-BLK-002: MOV-002 skips only the product for a `piston` move and still
    clears the multiplier and the velocity; BLK-002 says a piston move "does not apply the stuck
    multiplier". The kernel follows MOV-002.
11. PHY-COL-002 step 1: the rule says a zero component resolves to zero, but not whether a `-0.0`
    request gives `+0.0` or `-0.0`. The kernel gives `+0.0`. The sign shows only in a position of
    exactly `-0.0`.
12. PHY-FLG-003: the rule does not state how the client computes the angle between the two
    vectors. The kernel compares `acos(a·b / sqrt(|a|²|b|²))` in f64 with the widened `0.13962634F`.
13. PHY-SHP-010: "the distance to the nearest border line in `x` and `z`" and "inside the border
    grown by `m`" do not state the sign of the distance outside the border, or whether the test is
    strict. The kernel uses the signed distance (negative outside) and strict comparisons.

14. PHY-RCH-002 lists the blocks with an inside action only as examples. The table marks those
    examples and the tags `pressure_plates`, `buttons`, `crops`, `cauldrons` and `fire`. A rest skip
    (PHY-SKIP-004) needs the full set. Which blocks have an inside action?
15. PHY-ORD-005 gives the walker category the `step_height` attribute with the default 0.6. The
    vanilla server (E4, E5 fixtures) shows an armor stand with the default attributes that stops
    at a carpet and a bottom slab, and steps with an explicit attribute of 0.6 or 1.0. The kernel
    gives the armor stand a default of 0 as type data. Is the default of the attribute per type?
16. PHY-ORD-001 step 14 puts the fall distance after the blocks inside the box. The E7, E8 and E17
    fixtures show this order instead: the reset of `fall_damage_resetting` for the centre path, then
    the fall of the tick (PHY-FALL-001), then the stuck multipliers with their reset. A falling
    armor stand ends the tick in which it enters a cobweb at 0, and keeps 0.15 while it descends a
    ladder. The kernel follows the fixtures.
17. PHY-FALL-002: an item that falls through a sweet berry bush, which is in the tag
    `fall_damage_resetting`, keeps its fall distance (E7). The kernel applies the reset of the
    centre path to a living entity only. Which entities does the rule cover?
18. PHY-ORD-005, item: in the three ticks of four that skip `M` and `D`, the rest values of
    PHY-SKIP-003 need the bounce to be skipped too. The kernel skips it; the E28 and E32 fixtures
    agree.
19. PHY-GRV-002: the rule does not say which `v.y` selects the effective gravity of
    `slow_falling`. The kernel reads `v.y` at the start of the travel step, after the jump.
20. PHY-INP-002 step 2: "normalize in f64" does not say whether the components are divided by the
    length or multiplied by its reciprocal. The kernel divides.
21. PHY-CMP-016: the console of the server does not print the network id, so the parity test fits
    the phase offset of each item (0 to 3) to the recorded rest cycle.
22. PHY-ORD-007: S holds no jump key of the previous tick, which PHY-INP-018 needs for "the jump
    key changes to down", and no spectator, passenger, sleep or food state, which PHY-INP-008,
    PHY-INP-010 and PHY-BOX-006 read. The kernel takes them outside `PlayerState`: `Keys`
    carries the press of the jump key, and `PlayerFacts` the rest.
23. PHY-INP-015: "when the flight state starts on the ground, the player jumps" does not say
    whether this is the jump of PHY-INP-005 or the impulse of PHY-INP-013. The kernel does not
    add it yet.
24. PHY-INP-011 needs the blocks that cause suffocation. No rule and no corpus component gives
    that property, so the kernel does not push the player out of blocks yet.
25. PHY-INP-001 step 2: "normalize in f32" does not state the operation. The kernel multiplies by
    `f32(1 / length)`, as step 6 does.
26. PHY-SRV-008: the observed displacement comes from two positions, and `new - old` can differ
    from the resolved move of the client by one unit in the last place. Then the candidate
    differs by the same amount. The exact policy of PHY-CMP-005 needs a statement for this case.
27. PHY-SRV-010 step 7 changes the old position and rotation, which `PlayerState` does not hold.
    The kernel leaves them to the caller.
28. PHY-PSH-008: the own player "uses the push in its next travel step", but the rule does not
    say where in the step. The kernel adds `PlayerInput::push` before the input acceleration.

