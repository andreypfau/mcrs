# Entity simulation and movement checks: architecture

Design note for the per-tick simulation of entities (collision, gravity, the push between
entities), the work beside the tick (AI and similar), and the movement checks of the player.
Behaviour comes from `docs/spec/entity-physics.md` (PHY), `docs/spec/movement-checks.md` (MVC)
and `docs/spec/entity-ai-timing.md` (AIT). Structure follows
`docs/minecraft-bevy-sharding-architecture.md` (ARCH). `docs/cells.md` (CELLS) and
`docs/replication.md` (REPL) stay valid where this note does not say otherwise; CELLS has a status
note that lists what this note supersedes.

Target load: 10,000 players and 100,000 to 1,000,000 server-simulated entities in one world, at
20 ticks per second.

Where this note names a technique of other servers, it cites it as "technique" and states only
the technique and its numbers.

## 0. Decisions

| Question | Decision | Section |
|----------|----------|---------|
| Hot-loop model | Jacobi tick: each entity reads a frozen start-of-tick state and writes only its own row. The entity pass is one parallel map, with no islands and no locks | §3 |
| Sequential groups | two kinds, found from the start-of-tick state, one task each: collider groups in a pre-pass (PHY-CMP-018), explosion groups after the main pass (PHY-CMP-020) | §3.5 |
| Unit of causality | one entity for one tick; a root vehicle with its passengers is one unit (PHY-ORD-014); a sequential group is one unit | §3.5, §3.6 |
| Order key | the persisted birth key of PHY-CMP-013, not the network id: for the push order, the item phase (PHY-CMP-016), AI random streams, effects and claims | §4 |
| Writers | one writer per row in each phase. The block domain writes rows in `WorldWrite` (PHY-BLK-001). Commands write through outside `Simulate`. A server vehicle places its riding player (PHY-SRV-011) | §3.7 |
| Writes to other entities | one merge point (`DomainInput` of `T + 1`, PHY-CMP-015), local and mailed alike; one claim arbitration in the same tick, after the block domain (PHY-CMP-017) | §3.7 |
| Frozen state | built once, at the freeze at the end of `DomainInput`, after every write that the next pass must see | §3.2 |
| AI | the eight reflexes of AIT in the hot tick; decisions in an AI stage on an isolated pool, at the vanilla periods, applied with revalidation at `T + 1 … T + max_lag` (2); proposals granted atomically at the merge point; `AiDelay::Inline` in the exact-parity mode | §5 |
| Data layout | hybrid: hot state in SoA rows that the kernel crate owns (`EntityRows`, `PlayerRows`, `BodyGrid`); cold state in ECS. Players are rows | §6 |
| Partition | one world per dimension in a process, tiles as work units. A cell is a unit of placement across processes | §7 |
| Players | L0/L1 in the muxer bridge, L2 in the world, in parallel with the entity pass | §8 |

### 0.1 Non-goals

These techniques of other servers change what a vanilla player sees. This design does not use
them:

- collision modes that differ from the reference;
- freezing far mobs (an activation range), and AI or goal throttling by distance by default (the
  reduced cadence of AIT-CDN-003 exists only as an operator's recorded deviation, §5.5);
- a push cap that bumps a counter on both entities of a pair: it depends on the order and breaks
  the Jacobi pass (the cap of §11.6 is the replacement);
- faster experience orbs, an item merge at a fixed long interval, a smaller search radius for
  villager points of interest;
- deleting projectiles after a number of chunk loads (§5.1 defers instead);
- pathfinding that reads the live world from another thread (racy; mobs wait in place);
- ignoring "moved too quickly" while the server lags;
- a null or air value for a reference across a partition (§7.3 reads the start-of-tick state);
- foreign chunks as solid walls (§7.3 uses halos).

## 1. Load model

| Symbol | 100k | 1M | Source |
|--------|------|----|--------|
| awake entities (full physics tick) | 40% | 25% | assumption: most items and idle mobs rest (PHY-SKIP, PHY-SKIP-007) |
| bodies in the grid (pushable or collider) | 60% | 60% | living entities, boats, minecarts, shulkers |
| hard colliders (PHY-SHP-009) | 1% | 1% | boats, minecarts, shulkers, happy ghasts |
| living entities with AI | 60% | 60% | assumption |
| villager-class brains among them | 5% | 5% | assumption |
| players | 10,000 | 10,000 | MVC-LAY-008 |
| kernel cost of one awake entity | 0.75 µs | 0.75 µs | 0.6 µs physics + 0.1 µs push over ~3 neighbours + 0.05 µs steering |
| hot columns | ~850k | ~850k | 10k players, simulation distance 6 (169 columns), half of them shared |
| sections with random-tickable blocks | ~5M | ~5M | ~6 per hot column |

All numbers are estimates from these assumptions and the kernel targets of §11.4. Benchmarks
replace them (§10.6).

## 2. Policies this design relies on

| Domain | Policy | Rule | Consequence here |
|--------|--------|------|------------------|
| Own player tick, re-application, `resolve`, PHY-SRV-008 velocity | exact | PHY-CMP-005 | one scalar f64 path |
| Server-simulated motion | semantic, 1e-6 per tick | PHY-CMP-006, PHY-DEV-001 | optional f32 batch in air |
| Discrete results | exact | PHY-CMP-007 | the batch never decides them |
| Colliders | start of tick; hard colliders from the pre-pass | PHY-CMP-008, PHY-CMP-018, PHY-DEV-002 | §3.5 |
| Explosions | sequential groups in birth-key order; other exploders as writes at `T + 1` | PHY-EXP-001/002, PHY-CMP-020, PHY-DEV-009 | §3.5 |
| Block domain on entities | same tick, after the entity pass, in block order | PHY-BLK-001/002 | §3.7 |
| Claims | once per tick after the block domain; `command < entity < block` | PHY-CMP-017 | §3.7 |
| Push cap | mutual top-`K`, only with `max_entity_cramming` 0 | PHY-CMP-019, PHY-DEV-008 | §11.6 |
| Item phase | an offset 0–3 from the birth key | PHY-CMP-016, PHY-DEV-007 | §6.6 |
| Riding player | the vehicle writes its position; the client gives only the rotation | PHY-SRV-011 | §3.6 |
| Blocks | end of the previous tick | PHY-CMP-009/010/011, PHY-DEV-003 | physics runs before every block write |
| Push | start of tick, sum in key order | PHY-CMP-012, PHY-CMP-013, PHY-DEV-004 | a gather over the grid |
| Crowd damage random value | a stream of (key, tick) | PHY-CMP-014, PHY-DEV-006 | no shared random source in the pass |
| Writes to other entities | at the start of the next tick, in key order | PHY-CMP-015, PHY-DEV-005 | the merge point (§3.7) |
| Mount trees | one unit, root first | PHY-ORD-014 | §3.6 |
| Rest | exact skip; push and input rules | PHY-SKIP-001..007, PHY-INP-020 | rest per tile (§6.6) |
| Layout independence | | PHY-INV-014, PHY-INV-015 | same result for any thread count, task split or cell layout |
| Exact-parity mode | one world, sequential, live | ARCH Decision 20, PHY-ORD-010/011, PHY-PSH-006 | §3.8 |
| AI timing | reflexes, delay, `max_lag` | AIT | §5 |
| Push on the own player | bounded input, superset of pushers, kept positions | MVC-UNC-004..007, MVC-PRED-019 | §8.4 |

## 3. The hot loop

### 3.1 Model

Let `F_T` be the frozen state at the start of tick `T`. The freeze is the last step of
`DomainInput`: after the merge point, the mailed writes of the block domain and the life cycle.
The grid and the position table are rebuilt there, so every write that the next pass must see is in
them, in one cell and in many. `F_T` is:

- the block states at the end of tick `T − 1` (PHY-CMP-009);
- the body grid: for each pushable or collidable entity and each player, its box and the fields of
  PHY-RCH-006 at the start of `T`;
- the position table: the position, rotation and kind of every entity and player (§6.5);
- the world border and the loaded sections.

One tick of one server entity `e` is a function:

```text
row'_e, effects_e = tick(row_e, steering_e, F_T + end boxes of the hard colliders)
```

It reads the zones of PHY-RCH-001 and PHY-RCH-006. It writes only `row'_e` and appends effects
(PHY-RCH-002, PHY-CMP-015). PHY-INV-015 states that the result is then independent of the order of
the entities, the thread count, the task split and the cell layout.

The push follows PHY-CMP-012 with the order key of PHY-CMP-013 (§4). The crowd damage uses the
neighbours of the gather. Its random value comes from a stream of the entity key and the tick
(PHY-CMP-014), so the pass has no shared random source.

### 3.2 What is frozen, and what that costs

| State | How it stays frozen | Cost per tick |
|-------|---------------------|---------------|
| Blocks | by order: no block write between the sync point and `WorldWrite` | zero; a schedule test holds the order |
| Bodies and positions of other entities | `BodyGrid`, rebuilt at the freeze | the rebuild of dirty tiles (§3.4) |
| Hard colliders | `ColliderEnds`, which the pre-pass writes before the main pass | §3.5 |
| Own row | read and written in place; no other entity reads `EntityRows` during the pass | zero |

### 3.3 Jacobi against interaction islands

| Cost at 1M | Jacobi | Islands (connected read zones, tick-list order inside) |
|------------|--------|--------------------------------------------------------|
| Partition work | none | ~2.5M pairs × 5 ns plus union-find at ~10 ns per pair: ~40 core-ms |
| Largest serial unit | one mount tree | the largest island. Zones grow the box by ≥ 1 block (PHY-RCH-001), so they percolate at about one mob per 9 m². A village or a farm is one island: 50k × 0.75 µs = 37 ms on one core |
| Wall time of the pass, 32 cores | ~7 ms | ≥ 37 ms in a dense area |
| Parity gained | none needed (PHY-DEV-002/003/004) | closer to the reference order, which no policy requires |

Decision: Jacobi. Islands stay only in the sequential exact-parity mode (§3.8).

### 3.4 The body grid: rebuild against incremental update

Per tile, the grid holds the public rows sorted by bucket (4 × 4 blocks, full height), then by key.
A query merges the sorted runs of the buckets that it touches, which gives the order of
PHY-CMP-012 step 4 without a sort.

| Option | Write per tick at 1M | Cost | Complexity |
|--------|----------------------|------|------------|
| Full ping-pong of all hot fields | 110 MB | ~11 core-ms | low |
| Rebuild dirty tiles (chosen) | 17 MB plus a counting sort | ~5.4 core-ms | low |
| Incremental, second generation for movers | 12 MB | ~3 core-ms | high |

A tile is dirty when a body in it moved, spawned, despawned or changed a field of PHY-RCH-006. A
clean tile keeps its grid.

### 3.5 Sequential groups

Two rules make a small set of entities tick in sequence, in birth-key order. Each group is one task
item. Groups run in parallel with each other. The freeze finds them from `F_T`, so the set of groups
does not depend on the thread count or the layout.

| | Collider group (PHY-CMP-018) | Explosion group (PHY-CMP-020) |
|---|---|---|
| Why | two moving hard colliders that read each other's start box overlap, and PHY-COL-003 then lets them pass | a TNT cannon needs the push of the charges in the move of the projectile in the same tick (PHY-EXP-002) |
| Seeds | every awake hard collider (boats, rafts, minecarts, shulkers, happy ghasts) | every primed TNT with a fuse of 1 or less, from the timing wheel |
| Growth | union of colliders whose read zones intersect, over the per-section list of hard colliders. A resting collider is not stepped: its start box is its end box | every entity whose read zone touches the reach (a cube of edge `4p`, grown by the displacement of the exploder), then every entity that an explosion of the group can ignite or move, until the group stops growing; groups that share an entity merge. Groups whose reaches intersect also merge, so two groups never remove the same block in parallel (spec request R3) |
| Finding cost | union-find over ~10k colliders at 1M: < 0.5 core-ms | grid and position-table queries per seed: ~2 µs per seed |
| Where | pre-pass in `Simulate`, before the main pass; publishes end boxes in `ColliderEnds` | the first step of `WorldWrite`, after the main pass and before the block step. Members are left out of the main pass. Blocks that an explosion removes are absent for the later members and for nobody else, as PHY-CMP-009 requires |
| Reads | other colliders of the group live; everything else from `F_T` | other members live; blocks live (copy point plus the removals of the group); everything else from `F_T` |
| Typical size | 1–3 | 1 TNT plus the entities in its reach: 1–20 |
| Worst case | a pile of boats on ice: 500 colliders, 0.5 ms; 5,000, 5 ms on one task | a detonation of 1,000 primed TNT in one area: 1,000 explosions in one group |
| Cost of one step | ~1 µs | ~0.3 ms: 1,352 rays × ~17 steps × ~10 ns, plus the exposure of each entity in reach |
| Tail bound | serial; acceptable to ~5,000 colliders per group | serial chain of explosions, but the rays and the exposures of one explosion are independent and fan out over the pool: ~20 µs per explosion on 16 cores, so 1,000 explosions take ~30 ms wall instead of 300 ms |

The main pass reads `ColliderEnds` for the collision with a hard collider, and the start-of-tick
grid for the push. A boat is both: its collision box is its end box, its push box its start box.

An exploder that does not show at the start of the tick (a fireball that hits a block) forms no
group; its impulse and damage are writes at `T + 1` (PHY-DEV-009).

### 3.6 Mount trees

PHY-ORD-014: a root vehicle and its passengers are one unit, and one unit is one task item. The
rows of a unit are contiguous in one tile. The vehicle reads the input of a controlling passenger
from its start-of-tick row.

- A server vehicle that carries a player sets the position of the player (PHY-ORD-014 step 3,
  PHY-SRV-011). The pass does not write `PlayerRows`: it emits a `RiderPlaced` message, which the
  player-row writer applies in `Project`, in the same tick, before the output. While the player
  rides, `predict` takes only the rotation from the client and runs no movement check.
- A vehicle that a client controls is written in `ClientInput` from the vehicle moves, before the
  pass. The pass does not step it. Vehicle models are out of scope of PHY.

### 3.7 Writers, the merge point and claims

**One writer per row in each phase.**

| Phase | Writer of `EntityRows` | Writer of `PlayerRows` |
|-------|------------------------|------------------------|
| `DomainInput` | the merge point: writes of `T − 1` from other entities (local and mailed), mailed block-domain writes, AI results, impulses, the life cycle (spawn, despawn); then the freeze | the merge point (writes on the server copy) |
| `ClientInput` | commands (write-through), client-controlled vehicles | — |
| `Simulate` | the collider pre-pass, then the main pass | `predict` |
| `WorldWrite` | explosion groups; the block domain (PHY-BLK-001) in block order; the grants of the claim arbitration | — |
| `Project` | tile moves of rows that left their tile | `RiderPlaced` |

**Block domain (approved decision 1, PHY-BLK-001).** Pistons, slime and honey blocks that a piston
moves (mover kind `piston`, PHY-BLK-002) and shulker boxes (`shulker_box`) move entities. The block
domain runs after the entity pass. It reads `EntityRows` as the pass and the explosion groups left
them and writes them directly, in block order. A piston thus moves a mob in the same tick, and the
mob does not trail into a `moving_piston`. Pressure plates, tripwire and detector rails only read
rows; a hopper makes a claim. A block in one cell that writes an entity of another cell sends the
write in the cell mail; the owner applies it at the merge point, before the freeze, so the next
pass sees it in either layout. Only the output of tick `T` (the tile event) and the readers later in
the same block step differ: the exception of PHY-INV-014, bounded to that entity.

**Commands write through.** A command runs in `ClientInput`, outside `Simulate`. It writes rows
through the store API at once, so `/tp` followed by `execute at @s` in one function reads the new
position. The grid keeps the start-of-tick row for other entities, and the tile becomes dirty.

**The merge point.** A write from one entity to another (damage, knockback, removal, PHY-CMP-015)
goes into the effect buffers of the tick. Every such write applies at one place: `DomainInput` of
`T + 1`, before the freeze. Writes from another cell arrive in the cell mail of `T` and apply at
the same place, so the f64 sum order is the same in each layout (finding M4). The order is
`(target key, writer key, sequence)`.

**Claims (PHY-CMP-017).** An exclusive action on an entity (a pickup by a player, a mob or a
hopper, a merge of items or orbs, a removal) is a claim, not a write. Each writer appends claims to
its buffer during the tick. One arbitration step runs at the end of `WorldWrite`, after the block
domain, with one total writer key:

```rust
pub enum WriterKey {              // variant order is the order of PHY-CMP-017
    Command { sequence: u64 },
    Entity { key: BirthKey },
    Block { pos: YzxPos },        // compared by y, then z, then x
}
```

Per target, the lowest key wins. The winner and the target apply the grant in the same step (the
item leaves, the inventory changes), before `Output`. Losers get nothing. Arbitration is a sort of
the claims by `(target, writer key)` and one pass: ~100 ns per claim. This removes the item
duplication between a player pickup, a hopper, a mob pickup and a merge.

A claim from another cell arrives one tick late and joins the arbitration of `T + 1` in the owner.
In one cell it would have joined the arbitration of `T`. Spec request R2 asks PHY to state that
case.

### 3.8 Exact-parity mode

Exact-parity mode is one world with every link `Inline` (ARCH Decision 20). It uses none of
PHY-CMP-008, 009, 012, 014, 015, 017, 018 or 020. `WorldWrite` then runs, in this order (PHY-ORD-010 steps 3 to 5):
the scheduled block and fluid ticks and the block events; then `step_entities_in_order` over the
rows in `TickListSeq` order, with a live collider view, sequential push (PHY-PSH-006) and immediate
writes to other entities; then the block entities. The sequential driver is the only writer of
`EntityRows` in that phase. AI runs with `AiDelay::Inline` (AIT-LAG-011): each decision runs in the
AI step of its entity, inside this pass, against the live state, with `k = 0`. The kernel functions
and the decision functions are the same as in the sharded mode.

### 3.9 Schedule of a tick

All systems are plugins of the world's sub-app, in the `TickSet` phases of `FixedUpdate`:

```text
DomainInput:  stage results (light, generation);
              cell mail (halo changes [C1], HaloBodies, mailed writes and block-domain writes);
              merge point: writes of T-1 by (target key, writer key, sequence); impulses;
              AI decisions due at T by (issue tick, BirthKey), revalidated; proposals granted;
              life cycle (row + ECS entity together); body-size and attribute projections
              -> freeze: tile moves, grid and position table rebuild of dirty tiles,
                 collider groups and explosion groups found from this state
ClientInput:  ClientTick per player; commands (write-through); client-controlled vehicles;
              client block edits only send BlockSetRequest
Simulate:     collider groups (pre-pass) -> ColliderEnds
              -> main pass (tasks over tiles; members of explosion groups left out)
              || predict (par over players; PlayerRows)
WorldWrite:   explosion groups (one task each; rays fan out)
              -> block step (PHY-BLK-001: pistons, shulker boxes; hoppers claim)
              -> claim arbitration and grants (PHY-CMP-017)
              -> exact-parity mode only: block ticks, sequential entity pass, block entities
Project:      RiderPlaced; project_section_bricks; trim_change_log
Output:       tile events from mover lists; AI delta, block changes and player table to the AI
              link; setbacks; cell mail (HaloBodies, writes, claims, crossings)
```

- Inside a task: no `Commands`, no structural change. The passes use `ComputeTaskPool` through
  `par_chunk_map_mut`, as `column_streams` does. There is no scheduler of our own.
- The entity domain has four barriers per tick (freeze, pre-pass, main pass, explosion groups);
  each costs tens of microseconds.
- `tests/it/schedule_ambiguity.rs` covers the new plugins.

## 4. Keys

| Key | What it is | Persisted | Used for |
|-----|------------|-----------|----------|
| `BirthKey` | PHY-CMP-013: the birth tick, the source key (kind, then the parent key, the block position, the chunk and attempt, the saved list position, the command sequence or the login order), the sequence. Kept across a change of dimension | yes, with the phase offset of PHY-CMP-016 | the push order, the item phase, the crowd damage stream (PHY-CMP-014), the collider and explosion groups, the AI random streams, the effect merge, claims |
| `EntityId` | the global runtime id from the coordinator's ranges; the wire id | no | the slot map of the store, side tables, the protocol |
| `Entity` | the Bevy entity of the cold state | no | ECS only |

The network id comes from per-cell ranges and is not saved, so it is no order key (finding C1).

The source key of kind `entity` holds the birth key of the parent, which holds the key of its own
parent, with no bound on the depth. A row needs a fixed size. The stored form is:

```rust
pub struct BirthKey {             // 32 bytes; derives Ord in field order
    tick: u64,
    kind: SourceKind,             // load < player < command < spawn < block < entity
    source: u128,                 // block: y, z, x packed in order; spawn: chunk and attempt;
                                  // load: chunk and list position; command, player: sequence;
                                  // entity: a 128-bit digest of the parent's stored key
    sequence: u32,
}
```

Every kind except `entity` keeps the exact order of PHY-CMP-013. For `entity`, two children of
different parents born in the same tick compare by the digest, not by the parent keys. The order is
still total, deterministic and the same in every layout and after a reload; only the rank between
such siblings differs from the rule. A digest collision has a chance of about `10^-26` at 1M
entities. Spec request R1 asks PHY to allow the digest.

Grid rows carry the key, so the grid row grows to 80 bytes (§6.5).

## 5. AI: reflexes in the tick, decisions in a pipeline

AIT splits each part of the AI into a reflex or a decision (AIT-SPL-001). This section maps that
split onto the tick and the AI stage.

### 5.1 Reflexes in the hot tick

The eight reflexes of AIT-RFX-001..008 run in the main pass, in the tick in which the reference
runs them (AIT-INV-003). They read the start-of-tick state of AIT-SPL-002: the own row and steering
state, the grid and the position table, the block copy of PHY-CMP-009. They write the own row, or
make writes and claims of PHY-CMP-015/017.

| Reflex | Rule | Per-tick work (AIT-SPL-003) |
|--------|------|-----------------------------|
| steering: path follower, move, look, jump control, body rotation | AIT-RFX-001 | fixed; ~50 ns |
| creeper swell: start and continue, direction, counter, explosion | AIT-RFX-002 | fixed, one sight ray when it has a target |
| melee and ranged attack: cooldown, reach, sight, hit, look, draw, release | AIT-RFX-003 | fixed, one sight ray |
| door on the path | AIT-RFX-004 | fixed; the open and close are block writes |
| escape: float in a fluid, climb out of powder snow | AIT-RFX-005 | fixed |
| effects of damage | AIT-RFX-006 | at the merge point; a fact to the brain |
| orb attraction | AIT-RFX-007 | one lookup in the player table and the per-column index |
| due events of the AI | AIT-RFX-008 | O(due), from the timing wheel |

Each reflex does at most one line-of-sight ray per tick (cached per tick, AIT-VAN-004), no path
search, no entity search in a box larger than 8 blocks and no block search. Cost per living mob per
tick: ~50 ns of fixed checks, plus a ray of up to the follow range (~250 ns) for the ~10% of mobs
with an attack target, plus ~30 ns to issue its due decisions: ~0.1 µs on average, on top of the
steering already in the 0.75 µs of an awake entity. AIT-INV-006: the hot cost of a mob does not
depend on its AI. §11.1 counts it.

Also hot, outside AIT: physics, push, crowd count; the due events of physics (fuse end, pickup delay,
effect expiry); the projectile chunk-load budget per tile (technique: 10 per tick), which defers a
projectile, never deletes it.

### 5.2 Decisions and the AI stage

The decisions of AIT-DEC-001 (goal and target selection, sensors, brain checks, path searches,
panic and random positions, merge searches, despawn checks, points of interest, partner selection)
run in the AI stage, a kernel without Bevy on a link per world.

- **Issue (AIT-CDN-001).** The hot tick issues each part for each entity at its vanilla period
  (AIT-VAN-025), with the phase from the birth key (AIT-DEV-003). The issue never depends on the
  backlog, wall time or load. The job messages go out in `Output`.
- **Snapshot (AIT-LAG-001).** A decision of issue tick `T` reads the bodies and steering states at
  the start of `T`, the block copy of `T`, the player table of `T`, the facts before `T`, and the
  brain state after the earlier decisions of its entity. The stage keeps a replica of the bodies
  (~40 B each, from the mover lists) and a navigation projection: per block the path type byte and
  tag bits (technique), 2 KB per section, built only near entities with AI.
- **Path search on the copy (AIT U7).** The path search reads the navigation projection of `T`,
  which is the start-of-tick block copy, not the live world. A change later in tick `T` reaches the
  search of `T + 1`, as PHY-CMP-009 states for physics.
- **Slices without a newer view.** A search must read only the projection of its issue tick, even
  when its slices run over two stage ticks. The projection keeps the old version of each section
  that changed in the last `max_lag + 1` ticks (copy on write; block changes touch few sections).
  A slice yields the pool after ~200 µs and resumes on the same version. This answers AIT U2 for
  the pool, not for the throughput (§11.3).
- **Cheap inputs (technique).** A behaviour condition re-runs only when one of its inputs changed,
  by a change counter on the memories. Only mobs with a path hear the block changes on it (an
  active-navigator set per tile).
- **Random values (AIT-LAG-008).** A stream of the world seed, the birth key, the issue tick and the
  part.
- **Output.** A decision writes only the brain and steering state of its entity (AIT-DEC-007,
  AIT-INV-005). A change of another entity or of shared state is a proposal.

### 5.3 Application at the merge point

At the merge point of `T + k`, after the writes of PHY-CMP-015 (AIT-LAG-002):

1. Decisions apply in the order `(issue tick, birth key)` (AIT-LAG-007).
2. Each decision is checked again against the current state (AIT-LAG-005): a removed entity, a dead
   or far target, a teleport, a path that starts too far from the mob, a control flag that a reflex
   took. A stale decision drops whole, the steering keeps its current state, and a fact goes to the
   brain (AIT-LAG-006). No new decision is computed in its place.
3. Proposals are checked and granted in the same order (AIT-CPL-002). A grant changes every named
   entity and object at once, so a pair action (breed, alert, gossip, golem summon, point of interest
   claim, fish school, raid membership) is atomic. An entity takes part in at most one granted pair
   action of a kind per merge point (AIT-CPL-003).
4. An item merge grant (AIT-CPL-005) consumes items. It enters the claim arbitration of PHY-CMP-017
   in this tick, with the writer key of the item that tries, next to the pickups by players and
   hoppers. One item thus has one arbitration point per tick.
5. Timers that a decision starts count from the application (AIT-LAG-010); a hold time ends by
   AIT-LAG-009.

### 5.4 Delay modes and `max_lag`

| Mode | `k` | Where | Rule |
|------|-----|-------|------|
| `AiDelay::AtLeast(1)` | `1 ≤ k ≤ max_lag` | production, default | a decision applies at the first merge point after it is ready; at `T + max_lag` the tick waits for it (AIT-LAG-004) |
| `AiDelay::Fixed(k)` | `1 ≤ k ≤ max_lag` | determinism tests, replays, one cell against many cells, in the sharded mode | the tick always waits for the decisions of `T' − k` (AIT-INV-002) |
| `AiDelay::Inline` | `0` | the exact-parity mode only (AIT-LAG-011) | no AI stage: each decision runs in the AI step of its entity, inside the sequential entity pass, in vanilla order, against the live state. The same decision functions run over a live view. AIT-LAG-001..010 do not apply |

`max_lag` defaults to 2 (AIT-LAG-003). A decision is never older than `max_lag` (AIT-INV-004).

### 5.5 The AI stage is a pipeline

With `max_lag` = 2, the stage has two ticks of latency, but no more. A tick waits once a decision
is two ticks late. Latency headroom therefore does not help a stage that falls behind: the stage
must finish one tick of AI work for each tick, on average and in each burst longer than two ticks.
When the AI work of a tick needs more core time than the stage gets in 50 ms, the AI cost is on the
critical path and the tick slows down.

"Beside the tick" therefore means only this: AI work overlaps the tick and the time between ticks
on other cores, and a short burst hides in the two ticks of lag. It does not mean that AI cost is
free. §11.3 sizes the AI stage for throughput.

When AI work exceeds the cores of the stage, the operator can, in this order:

1. accept the slower tick (the server reports the wait at the merge point as a metric);
2. lower the mob caps or the entity-ticking range of the world;
3. enable the reduced cadence for far mobs (AIT-CDN-003), a recorded deviation, off by default
   (AIT-DEV-006); it never applies within 32 blocks of a player, and never changes a reflex;
4. raise `max_lag`, which makes each decision with a smaller allowed delay a deviation
   (AIT-DEV-001) and only absorbs longer bursts, not a lack of throughput;
5. add cores, or place cells in other processes.

### 5.6 Pool isolation

The stage runs on its own pool, not on the compute pool. Its threads run at a lower OS priority,
or on cores that the compute pool does not use, so the OS never preempts a hot task at a barrier
for an AI job. A job yields after ~200 µs.

### 5.7 Player table

Each cell keeps the player table of each dimension (AIT-CDN-002): the position at the end of the
previous tick, spectator and alive flags; 32 B per player, 320 KB at 10k. The coordinator sends each
change to each cell at the sync point, so a "nearest player" query gives the same answer in each
layout. A per-column reverse index of near players at fixed radii, updated when a player changes
column (technique), serves the despawn distances (AIT-VAN-020, AIT-CDN-004), the idle time, orb
attraction, spawning, entity tracking and the player push.

### 5.8 What the hot loop never waits for

The tick never waits for the light and generation stages, the data layer or the disk, the muxer,
encryption or the sockets. It waits for the AI stage only at `T + max_lag`, or always in
`AiDelay::Fixed`.

## 6. Data layout

### 6.1 Options

| Criterion at 1M, 25% awake | Plain Bevy components + `par_iter` | Hybrid: SoA rows in the kernel crate, cold state in ECS |
|----------------------------|------------------------------------|---------------------------------------------------------|
| Skip of resting entities | visits every entity: ~15 core-ms | per tile: a quiet tile costs zero (~2.3 core-ms in changed tiles) |
| Block reads | hash + `Query::get` per section: +37 core-ms | per-task section cache: ~2.4 core-ms |
| Snapshot and neighbours | a separate grid anyway | the grid is part of the store |
| Change detection | `Changed<T>` scans over 1M (1–2 ms each) | mover lists from the pass |
| Archetypes | hundreds of tables, small tasks | not in the hot path |
| Locality and SIMD | spawn order | sorted by tile and class |
| Estimated hot loop | ~260 core-ms | ~210 core-ms |

Decision: hybrid. Each hot field has one home.

### 6.2 Resources

Bevy runs two systems in parallel only when their accesses do not conflict, so the store is
several resources, one per writer:

| Resource | Content | Written in `Simulate` by |
|----------|---------|--------------------------|
| `EntityRows` | the rows of server entities, one segment per tile | the collider pre-pass, then the entity pass |
| `PlayerRows` | the rows of players, same columns | `predict` |
| `BodyGrid` | the frozen grid and the position table | nobody (written at the freeze) |
| `ColliderEnds` | end boxes of hard colliders | the pre-pass |

`predict` takes `ResMut<PlayerRows>` and `Res<BodyGrid>`; the entity pass takes
`ResMut<EntityRows>`, `Res<BodyGrid>` and `Res<ColliderEnds>`. Bevy runs them in parallel.

### 6.3 Rows

```rust
pub struct TileRows {                       // SoA; sorted by motion class; mount trees contiguous
    position: Vec<DVec3>,
    velocity: Vec<DVec3>,
    rotation: Vec<Rotation>,                // yaw, pitch, head yaw
    pose: Vec<Pose>,
    motion: Vec<MotionState>,               // flags, support, fall distance, jump delay
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

- Side tables (stuck multiplier, client vehicle state, rare effects) are keyed by `EntityId`, never
  by row, so a tile move does not break them.
- One life cycle place: the system at the merge point creates the row and the ECS entity together,
  and removes both together. No other system spawns or despawns an entity.
- Players are rows (approved decision 3). `Transform` stops being the truth for a player: the
  consumers that read it today (`column_streams`, `movement`, `cell_crossing`, persistence) read
  `PlayerRows`. The muxer and the view get the position from the row.

### 6.4 Classification

| Data | Class (bevy-ecs) | Home | Writer |
|------|------------------|------|--------|
| position, velocity, rotation, pose, motion state, rest, steering | authoritative | rows | the writer of the phase (§3.7) |
| motion class, birth key | authoritative, immutable | rows | the life cycle |
| body size | materialized from class, pose, `scale`, baby | rows | one projection at the merge point |
| accepted player state | authoritative (L2) | `PlayerRows` + `Prediction` | `predict` |
| grid rows, position table | materialized, frozen | `BodyGrid` | the rebuild at the freeze |
| mover lists | temporal fact of the tick | rows | the pass; read by tile events, the AI replica, the pusher history |
| health, equipment, attributes, effects, team, custom data | authoritative | ECS | their systems |
| attribute and effect values for motion | materialized | rows | one projection at the merge point |
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
    pub kind: KindSlot,                     // u16
    pub flags: u16,
    pub id: EntityId,
}
```

The position table covers what the hot steps read outside the grid: a look target, the player that
an orb moves to, a leash holder, the target of a homing bullet, and the reflexes. A reference to an
entity in another cell reads the halo copy of this table, never a null.

### 6.6 Rest and wake per tile

- Each tile has a stamp: the largest of the block change counters of its sections and their ring,
  the grid revision of the tile and its neighbours, the border revision, and the intents, impulses
  and block-domain writes that target it.
- A tile whose stamp did not change skips every resting row with no work.
- In a changed tile, each resting row compares its zone stamp (10 ns). PHY-SKIP-005 allows extra
  wakes.
- The cheap exact path (technique): an entity on the ground, with zero horizontal velocity, only
  gravity, and an unchanged floor section stays at rest without a full tick.
- Enter rest: PHY-SKIP-001, 004, 006 and 007: no push with `m ≥ 0.01F`, a crowd count not more than
  `n − 1`, an input with a squared length below `1.0E-7`.
- The item phase is the offset of PHY-CMP-016, saved with the birth key; a skipped item writes
  nothing.

### 6.7 Block access and shapes

The block storage stays as it is. Physics adds per section (technique):

- the brick masks of the earlier version (`solid`, `full`, `large`, `action`, `coarse`, revision);
- an air-only flag and a count of special states (shape larger than one block or context
  dependent). With no special state, the scan margin shrinks and the edge and corner cells are
  skipped.

Per state, the `CollisionTable` holds the shape class (empty, full cube, one box, constant
compound, context dependent) and bit flags (fluid, climbable, path type byte, tag bits). A
one-box shape collides as a box list per axis, a full cube as a plain overlap; only compound shapes
take the grid path. The shape context is built only when a context-dependent state is in the zone.
The world border is tested only when the zone is near it. A task keeps a cache of 64 section
references (~2.4 core-ms of lookups per tick at 1M).

### 6.8 In-air batch

Rows of one class are contiguous in a tile, so the in-air rows batch in f64 lanes of 4,
bit-identical to the scalar path. The f32 variant of PHY-DEV-001 is a later lever.

## 7. Partitioning

### 7.1 Units

| Unit | What it is for | Size |
|------|----------------|------|
| Tile | memory segment, work unit, grid unit, replication tile | 2 × 2 columns, full height |
| Task | a batch of tiles, or a range of rows of one heavy tile, ~50–100 µs | adaptive |
| Tile group | the work unit of the parallel muxer (REPL §7.12) | 8 × 8 tiles |
| Cell | placement in another process or machine | a k-d area of region boxes |

### 7.2 Cells against one world with tiles, in one process

| Cost at 1M, 16 in-process cells | Cells | One world, tiles |
|---------------------------------|-------|------------------|
| Bodies near a boundary | 60k × 2 × 40 ns = 4.8 core-ms; 3.8 MB of mail per tick | none |
| Halo sections | ~6% more sections; halo changes applied again | none |
| Crossings | ~250 per tick × 2 µs | ~1.2k tile moves × 50 ns |
| Boundary exceptions | C2, C3, C5 | none |
| Serial parts of the schedule | in parallel across cells | once per world: the block kernel must step shards (§11.8) |

Decision: one world per dimension in a process. Cells are for placement across processes. While
the block kernel is serial, more cells in one process are a stopgap.

### 7.3 Across processes

| Case | Rule |
|------|------|
| A zone crosses into a neighbour's column | read the halo, `HaloBodies` (grid rows) and the halo copy of the position table, at the start of the tick |
| An entity walks into a neighbour's column | hysteresis `N` = 8 blocks; a mount tree crosses as one unit; the brain state goes with it |
| A displacement of 30 blocks or more in one tick that leaves the cell | it becomes a teleport at the next tick (technique). This bounds C2 |
| Writes and block-domain writes for an entity of another cell | cell mail of `T`, applied at the merge point of `T + 1`, before the freeze, as local ones |
| Claims on an entity of another cell | cell mail of `T`, arbitrated in the owner at `T + 1` (C5) |
| Split and merge | starting numbers (technique): one ring of buffer sections; merge within one section; split only when the area holds at least 8 sections of 16 × 16 chunks and at least one sixth of it is dead |

Conflicts in a layout of many cells:

- **C1. Halo changes land after physics.** They must apply in `DomainInput` (CELLS §12 step 5).
- **C2. A zone past the halo.** Bounded by the teleport rule above; reported as a metric.
- **C3. A block action in another cell's column.** One tick later (PHY-INV-014).
- **C4.** Removed: a block-domain write to an entity of another cell applies before the next pass
  (PHY-BLK-001), because the freeze follows the merge point.
- **C5. A claim from another cell.** It joins the arbitration one tick later. Spec request R2.

## 8. Players and the movement checks

### 8.1 Layers

| Layer | Hook | Notes |
|-------|------|-------|
| L0 | `bridge_inbound`, after `PacketDispatcher::dispatch` | MVC-CHK-023 |
| L1 | `bridge_inbound` and `write_queue`, per connection | `MoveGuard`; probes in wire order |
| L1 → L2 | `DimInputs` | one `ClientTick` per client tick |
| L2 | the world, `Simulate`, parallel over players | in parallel with the entity pass |

`bridge_inbound` must become a parallel loop over connections (~5 ms serial at 50k packets per
tick, ~0.4 ms after). The packet limiter defaults of other servers are inputs for the MVC authors
(technique: all packets 500 per second over 7 s; place recipe 5 per second over 4 s; place or use
item 8 per 300 ms).

### 8.2 L2 next to the entity pass

L2 reads `ClientView`, the grid, `ColliderEnds` and the pusher history; it writes `Prediction` and
`PlayerRows`. Its accepted state enters the grid at the next freeze, so entities see the player at
`T + 1`. A fast path (technique, where MVC allows it): with no collision in the tick and an
unchanged box, the collision test of the new box is skipped.

### 8.3 Pushed, pushing and riding players

- A server entity pushes only the server copy of the player (PHY-PSH-008). L2 treats it as one
  bounded input (MVC-UNC-004/005, MVC-PRED-019).
- A player pushes server entities: its grid row is a pusher in their gather.
- A player on a server vehicle: the vehicle places it through `RiderPlaced` (§3.6). A player that
  controls a vehicle: the vehicle row is written in `ClientInput`. Vehicle checks need their own
  specification (spec request R5).

### 8.4 Pusher history

MVC-UNC-006 lets L2 use a superset of pushers; MVC-UNC-007 states the kept positions: each position
as the client decodes it, with its server tick (28 B), shared by all trackers, for `max_hull_age`
= 40 ticks, plus the probe of each server tick per player (8 B). The muxer is the authority of what
it sent, so it reports the tier-0 positions of tick `T` in the input of tick `T + 1`. L2 reads them
through a trait:

```rust
pub trait PusherHistory {
    fn positions(&self, entity: EntityId, ticks: TickRange, out: &mut Vec<SentPosition>);
    fn candidates(&self, near: Aabb, out: &mut Vec<PusherCandidate>);
}
```

The engine implements it over the muxer reports; a foreign host over its own tracker. Memory: ~100k
entities near players × 20 entries × 28 B ≈ 56 MB.

### 8.5 Crates

| Crate | Bevy | Content |
|-------|------|---------|
| `mcrs_minecraft_physics` (new) | no | shapes, tables (types), brick masks, world and body views, query, move procedure, step-up, flags, ticks, push, rest, in-air batch, `TileRows`, `BodyGrid` and its rebuild |
| `mcrs_minecraft_collision_table` (new) | no | builds `CollisionTable` from the block definitions of `mcrs_minecraft_block` (default features, no Bevy) |
| `mcrs_minecraft_move_validate` (new) | no | L0, L1 |
| `mcrs_minecraft_move_predict` (new) | no | L2, `ClientView`, `PusherHistory` |
| `mcrs_minecraft_engine_physics` (new) | yes | the resources of §6.2, the passes, rest and wake, the merge point, claims, brick projection, tile events, the AI link |
| `mcrs_minecraft_brain` (new, later) | no | the AI stage kernel |
| `mcrs_minecraft_engine` (extended) | yes | L0/L1 hooks, L2 systems, crossing, `PlayerRows` consumers |
| `mcrs_minecraft_replication` (extended) | no | the codec for entities that are not players (REPL §7.11) |

A foreign-host anticheat links `_physics`, `_collision_table`, `_move_validate` and
`_move_predict`, and implements `CollisionWorld`, `BodyView` and `PusherHistory`.

## 9. Kernel API

```rust
pub trait CollisionWorld {
    type Section<'a>: CollisionSection where Self: 'a;
    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>>;
    fn border(&self) -> Option<&WorldBorder>;
}

pub enum SectionLookup<S> { Outside, Unloaded, Loaded(S) }

pub trait CollisionSection {
    fn masks(&self) -> &SectionMasks;            // bricks, air-only flag, special count
    fn state(&self, at: LocalPos) -> BlockStateId;
}

/// The driver picks the frozen grid plus `ColliderEnds`, or a live view (exact-parity mode).
pub trait BodyView {
    fn colliders(&self, query: Aabb, mover: BirthKey, out: &mut Vec<Collider>);    // PHY-SHP-009
    fn neighbours(&self, of: &GridRow, out: &mut Vec<GridRow>);                     // PHY-RCH-006, key order
    fn position(&self, entity: EntityId) -> Option<PositionRow>;                    // the position table
}

pub fn tick_entity<W: CollisionWorld, B: BodyView>(
    world: &W, bodies: &B, tables: &Tables, row: &MotionRow,
    body: &mut Body, living: Option<&LivingInput>, stream: &KeyedStream,
    effects: &mut EffectBuffer, scratch: &mut Scratch,
) -> TickOutcome;

pub fn tick_mount_tree<W: CollisionWorld, B: BodyView>(/* a run of rows, root first */);

pub fn tick_player<W: CollisionWorld, B: BodyView>(
    world: &W, bodies: &B, tables: &Tables,
    start: &PlayerState, keys: Keys, rotation: Rotation, push: PushInput,
    out: &mut PlayerState, effects: &mut EffectBuffer, scratch: &mut Scratch,
);

pub fn move_body<W: CollisionWorld, B: BodyView>(/* PHY-MOV-001; mover kinds include piston */);
#[must_use] pub fn resolve<W: CollisionWorld, B: BodyView>(/* PHY-SRV-006 */) -> Resolved;
#[must_use] pub fn read_zone(body: &Body, row: &MotionRow, displacement: DVec3) -> Aabb;
```

`KeyedStream` gives the value of PHY-CMP-014 from `(BirthKey, tick)`. `EffectBuffer` holds writes
and claims with their `WriterKey`. Tables and brick masks are as in the earlier version.

## 10. Verification

### 10.1 Kernel tests without Bevy

- PHY-GRV-007 factor, PHY-SKIP-003 rest values, PHY-SHP-005 slab lines.
- PHY-PSH-004/005 pairs, bit for bit; PHY-CMP-012 crowds equal for any row order and task split.
- Two boats driven into each other do not overlap after the collider pre-pass.
- A TNT cannon: the projectile uses the push of the charges in the same tick (PHY-EXP-002), for any
  task split and with the rays traced serially or fanned out.
- Group finding: the same `F_T` gives the same collider and explosion groups for any row order.
- A piston moves a mob in the same tick; the mob never overlaps the `moving_piston` (PHY-BLK-002).
- `CollisionWorld`, `BodyView` and `PusherHistory` test doubles.

### 10.2 Property tests

- PHY-INV-001..013; masks and shape classes give the same result as reading every block; the f64
  batch equals scalar.
- Rest equals no rest under random wakes, push neighbours included.
- Claims: for random claims of one item by players, hoppers, mobs, merges and commands, exactly
  one claimant gets it, and the winner is the lowest `WriterKey`.

### 10.3 Thread count, layout, keys

- `tests/it/tick_determinism.rs` with entities, AI (`AiDelay::Fixed(1)`), players and pistons:
  `Inline` against `Thread` gives equal world hashes and tile deltas.
- One world against four cells: entity results equal, except C2, C3 and C5 cases, which the test
  reports.
- Save and load in the middle of a run: the run after the load equals the run without it (birth
  keys, item phases, AI streams).
- AI content: a slowed stage with `AtLeast(1)` gives the same sequence of results per entity as
  `Fixed(1)`; no result applies later than `T + max_lag` (AIT-INV-001, AIT-INV-004).
- Revalidation: a decision whose target died or whose mob teleported drops whole and sends a fact
  (AIT-LAG-005/006). A breed grant changes both animals or neither (AIT-CPL-002).
- Exact-parity mode runs with `AiDelay::Inline`.

### 10.4 Regression tests for references across a boundary

Each test runs with the two entities in one cell and in two cells (technique: the border bugs of
other servers):

- a pet teleports to its owner in another cell;
- a leash whose holder is in another cell;
- a fishing hook on an entity of another cell;
- a selector whose box crosses a boundary;
- a deflected projectile keeps its owner;
- a creaking whose home block is in another cell;
- block-break progress reaches every player in range.

### 10.5 Vanilla parity

- Server entities: PHY E1–E8, E15–E20, E28–E41 within 1e-6, flags exact; push and writes to other
  entities within PHY-DEV-004/005.
- Own player: PHY E9–E14, E21–E23, MVC C1–C7: bit for bit, no violation.

### 10.6 Benchmarks

- `crates/mcrs_minecraft_physics/benches/`: every row of §11.4; CI fails at 1.5× the target.
- `tick_load` and `bot_load`: 10k players with 100k and 1M entities. They measure every row of
  §11.1, the AI lag and the host time between ticks.

## 11. Budget

### 11.1 Whole tick

Core-ms per tick. "In tick" rows share the compute pool inside the 50 ms tick. "Beside" rows run on
their own threads and overlap the tick.

| Work | Where | 100k | 1M | Note |
|------|-------|------|----|------|
| entity pass, awake | in tick | 30 | 187.5 | 0.75 µs × awake |
| collider groups: finding and stepping | in tick | 0.4 | 3 | union-find over awake colliders; ~1 µs per step |
| rest checks, due events, AI results applied (~10% of living per tick), life cycle, merge point | in tick | 1 | 9 | |
| claim arbitration | in tick | 0.2 | 2 | ~20k claims at 1M × 100 ns |
| grid and position table rebuild, section lookups, mover lists | in tick | 2 | 17 | |
| game events and their listeners | in tick | 0.5 | 5 | 20% of movers emit a step event; listener lookup ~100 ns |
| tile events | in tick | 2 | 12.5 | |
| `predict` and the push input | in tick | 27 | 27 | 10k players |
| block random ticks | in tick | 130 | 130 | 5M sections × 3 picks × ~9 ns; grows with players, not entities |
| scheduled ticks, neighbour updates, block events | in tick | 50 | 50 | assumption |
| block entities | in tick | 40 | 40 | ~200k × 0.2 µs, assumption |
| explosion groups: finding and stepping | in tick | 3 | 3 | 10 explosions per tick × ~0.3 ms (rays and exposure), plus ~2 µs per seed to find the groups |
| `Changed<T>` scans over cold ECS | in tick | 0.2 per scan | 1–2 per scan, serial | target zero at 1M: change lists instead |
| sync point and serial parts | in tick, serial | 0.3 ms | 1 ms | wall time |
| reflexes (AIT-RFX-001..008) and the issue of decisions, every living mob | in tick | 6 | 60 | ~0.1 µs per mob: fixed checks, one sight ray for ~10%, the job issue (§5.1) |
| **In-tick total** | | **~293** | **~547** | |
| muxer, parallel per tile group | beside | 57 | 95 | encode 3 tiers × 60 ns per mover + 5 µs per viewer |
| encryption (CFB8, ~125 MB/s per core) | beside, network threads | 10–40 | 10–40 | 24–95 MB/s spread; 125–205 with all players in one crowd (315–512 MB/s) |
| L0 + L1 | beside, host | 5 | 5 | parallel `bridge_inbound` |
| AI stage, decisions | AI pool, pipelined (§5.5) | 144 | 1440 | 5% at 10 µs, 95% at 2 µs per mob per tick, averaged over the vanilla periods |
| AI stage, path search | AI pool, pipelined | 30 | 300 | 0.5% of living per tick × 100 µs |

The AI cost is calibrated on one data point of another server: about 23–31 µs per villager per tick
in a region of ~585 villagers. This design assumes 10 µs for a villager-class brain and 2 µs for a
simple mob, with conditions cached on a change counter and precomputed path types. At the observed
cost (×3), the AI rows become ~520 core-ms at 100k and ~5.2 core-s at 1M.

### 11.2 Wall time in the tick

At 80% parallel efficiency, with block shards and the parallel muxer in place:

| Cores on the compute pool | 100k | 1M |
|---------------------------|------|----|
| 16 | ~23 ms | ~43 ms |
| 32 | ~11 ms | ~21 ms |
| 48 | ~8 ms | ~14 ms |

Without block shards, the block rows (~220 core-ms) run on one core: 220 ms, four ticks. This holds
at 100k as well: random ticks grow with the hot sections of 10k players.

Sequential groups add a tail: the wall time of a phase is at least its longest group.

| Group | Typical tail | Worst case considered |
|-------|--------------|-----------------------|
| collider group | < 10 µs | 5,000 boats in one pile: ~5 ms |
| explosion group | ~0.3 ms (one TNT) | a cannon of 100 charges: ~2 ms with fanned-out rays (30 ms without); 1,000 TNT: ~30 ms with fan-out |

The collider tail sits before the main pass, the explosion tail after it. Both add to the tick, not
to the core count.

### 11.3 Machine fit

`max_lag` = 2 makes the AI stage a pipeline (§5.5): it must sustain one tick of AI work per tick.
At 80% efficiency a core gives 40 core-ms of work per 50 ms tick. Cores that the AI stage needs to
hold 20 ticks per second:

| AI cost | 100k | 1M |
|---------|------|----|
| target (2 µs simple mob, 10 µs villager-class) | 174 core-ms → **5 cores** | 1,740 core-ms → **44 cores** |
| observed on another server (×3: 23–31 µs per villager) | 522 core-ms → **13 cores** | 5,220 core-ms → **131 cores** |

The other work per tick: in the tick 293 (100k) or 547 (1M) core-ms; beside the tick the muxer
(57 / 95), encryption (10–40) and L0/L1 (5).

| Case | Dedicated pools (compute + AI + network) | Shared: AI at low priority fills the idle compute cores | Fits one machine |
|------|------------------------------------------|---------------------------------------------------------|------------------|
| 100k, AI at target | 16 + 5 + 3 = 24 cores | (293 + 102 + 174) / 40 → 16 cores | yes |
| 100k, AI observed | 16 + 13 + 3 = 32 cores | (293 + 102 + 522) / 40 → 24 cores | yes |
| 1M, AI at target | 32 + 44 + 4 = 80 cores | (547 + 140 + 1,740) / 40 → 61, so 64 cores, with no margin | barely |
| 1M, AI observed | 32 + 131 + 4 = 167 cores | (547 + 140 + 5,220) / 40 → 148 cores | no |

In the shared layout, the AI stage gets few cores while the compute pool runs (~21 ms of each tick
at 1M on 32 cores) and all of them in the rest; the two ticks of lag absorb that swing, not a lack
of throughput. When the AI stage does not keep up, AI cost is on the critical path: the tick waits
at `T + max_lag`, and the operator follows §5.5.

Locality and NUMA limit a 64-core machine before memory bandwidth does: the hot loop reads ~200 MB
of rows and grid per tick, ~4 GB/s, far below the bandwidth. Tasks must keep tiles on one NUMA node.

### 11.4 Kernel targets (median, one core, warm cache)

| Benchmark | Target |
|-----------|--------|
| `resolve`, inert air / flat ground / step-up onto a slab | ≤ 60 ns / ≤ 300 ns / ≤ 1.2 µs |
| `tick_entity`, walker on ground, no neighbour | ≤ 600 ns |
| push gather, 3 / 24 neighbours | ≤ 100 ns / ≤ 600 ns |
| item in air, scalar / f64 batch | ≤ 80 ns / ≤ 30 ns |
| grid rebuild per body; rest check in a changed tile | ≤ 15 ns; ≤ 10 ns |
| claim arbitration per claim | ≤ 100 ns |
| `tick_player`, one candidate; `predict`, fast path | ≤ 1.5 µs; ≤ 2.5 µs |
| L0 / L1 per packet | ≤ 30 ns / ≤ 100 ns |
| random tick pick | ≤ 10 ns |
| AI step, simple mob / villager-class | ≤ 2 µs / ≤ 10 µs |

### 11.5 Memory at 1M

| Part | Size |
|------|------|
| rows (~150 B with rotation, pose and key) | ~150 MB |
| grid (80 B × 600k, two buffers during a rebuild) and position table (32 B × 1M) | ~130 MB |
| pusher history | ~56 MB |
| AI replica, brains, navigation projection | ~40 + 120 + 400 MB |

### 11.6 Crowds and the push cap

The push costs `k²` pair tests in a crowd of `k`. With crowd damage on (`max_entity_cramming` >
0), the damage bounds a stable crowd to about `n` (576 pairs at 24, ~6 µs). With crowd damage off,
a farm of 5,000 entities in one block costs 25M pairs, ~125 ms. Approved decision 4: an optional
cap, active only when crowd damage is off. Each entity uses only its `K` nearest neighbours, by the
distance `m` of PHY-PSH-004, with ties broken by the pair `(min key, max key)`, which is the same
from both sides. Each entity still writes only itself, so the pass stays order independent. The cap
is an intentional deviation (PHY-CMP-019, PHY-DEV-008). Jacobi also spreads the rows of a heavy tile over
several tasks.

### 11.7 Levers

| Lever | Gain |
|-------|------|
| Jacobi pass over tiles | no serial island |
| Rest per tile, cheap exact rest path | quiet tiles cost zero |
| Shape classes, section masks, special count | 3–5× fewer block reads |
| Rebuild only dirty tiles | ~40% of the rebuild |
| Behaviour conditions on change counters, active-navigator set | the AI target cost of §11.1 |
| One flush per connection per tick, none when empty (technique) | fewer socket writes |
| Spread player-list latency and other all-to-all updates over buckets (technique) | removes bursts of ~10^8 entries at 10k players |
| Tracker candidates from the per-column "players within view" list, full pass only when its change counter moved (technique) | muxer work proportional to changes |
| In-air batch f64 / f32 | 2–4× / 2× more on that subset |

### 11.8 Preconditions for 1M

1. **Block shards.** Random ticks, scheduled ticks and block entities step in parallel inside one
   world. This is also a precondition for 10k players at 100k entities.
2. **A parallel muxer** per tile group, and encryption on the network threads (REPL §7.12).
3. **AI throughput**: 44 cores of AI work at the target cost (§11.3), so a machine of ~64 cores
   shared or ~80 dedicated; at the observed cost, several processes or fewer AI mobs.
4. **No `Changed<T>` scan** over a cold population larger than ~10k; change lists instead.
5. **A parallel `bridge_inbound`.**
6. **Correctness:** the persisted birth key (§4) and the claim arbitration (§3.7), before any
   determinism or dupe test can pass.

## 12. Spec requests

Resolved: PHY-SKIP-006/007, PHY-INP-020, PHY-ORD-014, PHY-CMP-013 to 020, PHY-BLK-001/002,
PHY-EXP-001/002, PHY-SRV-011, PHY-DEV-002 and 005 to 009, PHY-INV-015, MVC-UNC-006/007, and
REPL §7.11 for the client-simulated categories, and the AI timing request (now AIT, with
AIT-RFX, AIT-LAG, AIT-CPL and AIT-CDN). Open requests:

| # | Rule | Change | Reason |
|---|------|--------|--------|
| R1 | PHY-CMP-013 | Allow a fixed-size stored form of the source key of kind `entity`: a 128-bit digest of the parent key in place of the parent key itself | the parent key nests without a bound on the depth; a row needs a fixed size (§4) |
| R2 | PHY-CMP-017 | State the case of a claim from another cell: it reaches the owner one tick later and joins the arbitration of the next tick, as an exception of PHY-INV-014 | the arbitration of one tick cannot hold a claim that is still in the cell mail |
| R3 | PHY-CMP-020 | State the order of two explosion groups whose reaches intersect but share no entity, for example by merging them into one group | both can remove the same block or ignite the same TNT; in parallel the result would depend on timing |
| R4 | PHY-CMP-020 | State that the rays of one explosion read the state at the step of that explosion, so an implementation can trace them in parallel | bounds the wall time of a large explosion group |
| R5 | MVC, vehicle spec | State the checks of a client-controlled vehicle | PHY-SRV-011 leaves them to the vehicle model |
| R6 | AIT-CPL-005 | Cite PHY-CMP-017, not PHY-CMP-015, and state that a granted item merge enters the claim arbitration of its tick, after the block domain | AIT-CPL-005 resolves the claims "in the order of the merge point", but PHY-CMP-017 arbitrates claims once per tick after the block domain; one item must have one arbitration point |
| R7 | AIT U1, U6 | Close U1 (PHY-CMP-013 now defines the birth key) and U6 (this note now uses `AiDelay::Inline` with `k = 0` in the exact-parity mode) | both items are resolved |
| R8 | AIT U2 | State that a path search may run in slices over at most `max_lag` stage ticks, provided that each slice reads the projection of the issue tick | the search reads only its snapshot (AIT-LAG-001); slicing over a versioned projection keeps that |

## 13. Open questions

1. Block shards: the shard key (tile, or causal component) and the rule for a random tick that
   writes a block in another shard.
2. Brain state: persistence and the move to another cell.
3. The muxer cost of 250k movers per tick, measured.
4. The worst case of an explosion group (1,000 TNT) is ~30 ms wall even with fanned-out rays.
5. U4/U6 in PHY: `OFF_GRID` shapes force coarse mode near them.
6. Projectiles: the ray move and the hit detection read the grid; their spec will state the reach.
