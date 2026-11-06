# Physics and movement checks: architecture

Design note for collision, gravity, the one-tick step of every entity, and the movement checks of
the player. Behaviour comes from `docs/spec/entity-physics.md` (PHY) and
`docs/spec/movement-checks.md` (MVC). Structure follows
`docs/minecraft-bevy-sharding-architecture.md` (ARCH), `docs/cells.md` (CELLS) and
`docs/replication.md` (REPL). Physics is a domain inside a cell; it adds no partition, no
scheduler and no transport of its own.

Users, each linking only what it needs:

| User | Needs | Links |
|------|-------|-------|
| L7 proxy | L0 + L1 checks (MVC-LAY-002, MVC-LAY-003, MVC-LAY-007) | `mcrs_minecraft_move_validate` |
| Anticheat on a foreign host | L0–L2 over the host's own block storage | `_move_validate`, `_move_predict`, `_physics` |
| mcrs server | everything: physics in each cell, L0/L1 in the muxer bridge, L2 in the cell of the player | all four new crates and `mcrs_minecraft_engine` |
| mcrs client | own-player tick (PHY-ORD-001) for local prediction | `_physics` |
| Parity tools, `tools/vanilla-oracle` | the kernel alone | `_physics` |

## 1. Policies this design relies on

| Domain | Policy | Rule | Consequence here |
|--------|--------|------|------------------|
| Own player tick, re-application, `resolve` query, PHY-SRV-008 velocity | exact, bit for bit | PHY-CMP-005 | one scalar f64 path, no tolerance, no reordering |
| Server-simulated motion | semantic, 1e-6 per axis per tick | PHY-CMP-006, PHY-DEV-001 | optional local-frame f32 batch path |
| Discrete results of server entities | exact | PHY-CMP-007 | batch path never decides these |
| Tick order of server entities | semantic; colliders from start of tick | PHY-CMP-008, PHY-DEV-002 | every entity of a cell is an independent unit of work |
| Skip of a fixed-point tick | exact (observably identical) | PHY-SKIP-001..005 | rest/wake machinery |
| Exact-parity mode | one cell per world, every link `Inline` | ARCH Decision 20, CELLS §10, PHY-ORD-010/011 | the same systems, sequential in tick-list order |
| L0/L1/L2 | layered, lower never reads higher | MVC-LAY-001..009 | L0/L1 in the muxer bridge, L2 in the cell |
| Replication | semantic, rate may differ | REPL §4.7 | outbound positions go through tile deltas, never a packet of the cell |

## 2. Crate split

### 2.1 Crates

| Crate | Bevy | Content | Deps |
|-------|------|---------|------|
| `mcrs_minecraft_physics` (new) | no, no tokio | f64 boxes; collision shapes with grid lines; per-state collision and motion tables; section brick masks (types only); world-view traits; collision query; move procedure; step-up; flags; one-tick step per category; sine table; rest detection; in-air batch | `mcrs_voxel`, `glam` |
| `mcrs_minecraft_move_validate` (new) | no, no tokio | L0: value checks of MVC-CHK-023 over decoded fields. L1: per-connection state, client-tick assembly, probe planning and windows, tick-rate balance, packet check, L1 speed bound | `mcrs_minecraft_wire`, `glam` |
| `mcrs_minecraft_move_predict` (new) | no, no tokio | L2: candidates, try order, fast path, replay, client world overlay, modes, checks, offset buffer, setback decision | `_physics`, `_move_validate` |
| `mcrs_minecraft_engine_physics` (new) | yes | dimension plugin: tables as resources, section brick projection, `CollisionWorld` over `DimensionView` (owned + halo sections), collider snapshot, server entity tick, rest/wake, effect messages | `_physics`, `mcrs_minecraft_engine_world`, `mcrs_minecraft_block`, bevy |
| `mcrs_minecraft_engine` (extended) | yes | L0/L1 hook in `muxer/bridge.rs`; L2 systems in `movement.rs` (now a set of `movement_*.rs` files); `Prediction` in `cell_crossing::Crossing`; entity crossing; tile events of server entities | adds `_move_validate`, `_move_predict`, `_engine_physics` |
| `mcrs_minecraft_replication` (extended) | no | a codec for entities that are not players: on-ground, velocity, precise sync (§7) | — |

The first three go in `NO_BEVY_PACKAGES`. They need `std` only for `f64::sqrt`
(`// std: f64::sqrt`, correctly rounded as PHY-NUM-001 needs).

There is no `mcrs_minecraft_engine_movement` crate. The L1 hook sits inside `bridge_inbound` and
`write_queue`, and L2 needs `TeleportState`, `Crossing` and `DimInputs`. All of them are
`pub(crate)` parts of `mcrs_minecraft_engine`, so a crate above it would have to reach back into
it (a cycle). `mcrs_minecraft_engine_physics` sits below `mcrs_minecraft_engine`: it needs only the
section storage and `OwnedColumns` of `mcrs_minecraft_engine_world`.

The kernel is one crate, because both halves (shapes/query/move procedure and the tick model) have
the same dependencies. The seam stays clean, so a split costs one move when a user needs only
collision.

The vanilla rows (PHY-ORD-005, PHY-GRV-017) are data the app supplies, never a default of the
kernel (rust-style): a corpus file under `assets/mcrs/` once U2/U3 close, a const table in
`mcrs_minecraft_server` until then.

### 2.2 Dependency graph

```text
mcrs_voxel  glam                     mcrs_minecraft_wire
     \      /                                |
  mcrs_minecraft_physics          mcrs_minecraft_move_validate     <- proxy stops here
      |          \                    /
      |         mcrs_minecraft_move_predict                        <- foreign-host anticheat stops here
      |                    |
mcrs_minecraft_engine_physics          |
  (+ engine_world, block, bevy)        |
           \                           |
            mcrs_minecraft_engine (muxer bridge, cells, movement)  <- server
```

## 3. Kernel API

### 3.1 World view

```rust
pub trait CollisionWorld {
    type Section<'a>: CollisionSection where Self: 'a;
    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>>;
    fn border(&self) -> Option<&WorldBorder>;
}

pub enum SectionLookup<S> { Outside, Unloaded, Loaded(S) }   // PHY-SHP-007 step 6, PHY-GRV-020

pub trait CollisionSection {
    fn bricks(&self) -> &BrickMasks;
    fn state(&self, at: LocalPos) -> BlockStateId;
}

/// Boxes of collidable entities (PHY-SHP-008, PHY-SHP-009). The driver chooses a start-of-tick
/// snapshot (PHY-CMP-008) or a live view (exact-parity mode); the kernel does not know which.
pub trait EntityColliders {
    fn colliders(&self, query: Aabb, mover: ColliderKey, out: &mut Vec<Collider>);
}
```

The cell's implementation (`CellCollisionWorld`, in `mcrs_minecraft_engine_physics`) wraps the
existing `DimensionView` of `mcrs_minecraft_engine_world` plus a `Query<&SectionBricks>`. The halo
is not a separate store: halo sections are ordinary loaded sections of the cell's world, kept
current by `apply_halo_changes`, and `OwnedColumns` only marks them read-only (CELLS §3, the
`CellVolume` of the block kernels does the same for writes). Physics only reads, so it needs no
access distinction: owned and halo sections are both `Loaded`, a column outside the halo or not
yet landed is `Unloaded`. A foreign host implements the trait over its own chunks (ARCH §13). L2
wraps any implementation in `ClientView` (§5.4).

### 3.2 Tables

```rust
pub struct CollisionTable {
    states: Box<[StateCollision]>,      // indexed by BlockStateId, 4 bytes each
    shapes: Box<[Shape]>,               // interned; 0 = empty, 1 = full cube
    motion: Box<[BlockMotion]>,         // a few distinct rows (PHY-GRV-017)
}

#[derive(Clone, Copy)]
pub struct StateCollision {
    pub shape: ShapeId,                 // u16
    pub flags: StateFlags,              // u8: LARGE, CONTEXTUAL, INSIDE_ACTION, STEP_ACTION, COARSE, OFF_GRID
    pub motion: MotionId,               // u8
}

pub enum ShapeClass { Empty, Full, Cell(Aabb), Grid(GridShapeId), Contextual(ContextualKind) }

pub struct GridShape {                  // PHY-SHP-005: per-axis grid lines, solid-cell bitset
    lines: [SmallVec<[f64; 9]>; 3],
    solid: BitBox,
}

pub enum ContextualKind { Scaffolding, PowderSnow, FluidSource }   // PHY-SHP-003
```

- `Cell(Aabb)` is a fast path valid only when no grid line of the shape falls strictly inside the
  box on any axis. A box that spans several grid cells (for example `y` 0..0.75 on the 1/4 grid)
  stops a penetrating mover at an interior line (PHY-COL-003); it must take the `Grid` path.
- `Full` is the full cube: grid lines 0 and 1 only, so face clipping is exact.
- `LARGE` (PHY-SHP-007 step 3) is precomputed per state.
- `COARSE` comes from the block list of PHY-ORD-013, resolved to state ids when the table is built.
  Slime and honey blocks are coarse only while a piston moves them, which is a `moving_piston`
  state, so they need no flag of their own.
- Shapes are built in f64 from the corpus boxes (PHY-SHP-002). `BlockDefinitions` stores f32
  `Aabb`; the builder converts each value back to the pixel grid and recomputes f64, exact only for
  dyadic values. U4/U6 shapes get `OFF_GRID` and feed MVC-MODE-003 row 9.
- `mcrs_minecraft_core::voxel_shape` is an occlusion shape (f32, clipped to the unit cube). It is
  not reused for collision.

Size: ~30k states × 4 B = 120 KB plus a few hundred shapes; it stays in L2 cache on every core.
The table is immutable after startup and every cell holds the same one.

### 3.3 Per-section brick masks

A section is 4×4×4 bricks of 4³ blocks. One `u64` per class, one bit per brick:

```rust
#[derive(Clone, Copy, Default, PartialEq)]
pub struct BrickMasks {
    pub solid: u64,       // brick has a non-empty or contextual collision shape
    pub full: u64,        // every block in the brick is a full cube
    pub large: u64,       // brick has a LARGE state (outer scan shell, PHY-SHP-007)
    pub action: u64,      // brick has an inside action or a step action (PHY-RCH-002, PHY-SKIP-004)
    pub coarse: u64,      // brick has a COARSE or OFF_GRID state (PHY-ORD-013, MVC-MODE-003 row 9)
    pub revision: u32,    // bumped on any change; wake test of PHY-SKIP-002
}
```

48 bytes per section. A query reads the masks of the bricks that its scan region touches:

- brick not `solid` → no block reads for that brick;
- brick `full` → every cell is `Full` without a block read;
- outer shell of the scan region (PHY-SHP-007 steps 3–5) → read only bricks with `large`.

Projection owner: one system, `project_section_bricks`, fed by the existing `BlockPlaced`
messages. `apply_halo_changes` and `apply_write` both write `BlockPlaced`, so one input covers
owned writes, writes from other cells and halo changes. A newly landed section computes all 64
bricks once through its palette (~10 µs). A changed block recomputes its brick (~200 ns).

### 3.4 Query and move procedure

```rust
pub struct Body {
    pub position: DVec3,
    pub velocity: DVec3,
    pub size: BodySize,                 // f32 half width, height (PHY-BOX-001)
    pub motion: MotionState,
}

pub struct MotionState {
    pub flags: MoveFlags,               // on_ground, horizontal, vertical, below, minor (PHY-FLG-001..003)
    pub support: Support,               // supporting block + "last search found nothing" (PHY-FLG-004)
    pub fall_distance: f64,
    pub stuck: DVec3,                   // PHY-MOV-002
    pub jump_delay: u8,
    pub rest: Rest,                     // §6.3
}

pub struct ShapeContext { pub feet_y: f64, pub fall_distance: f64, pub shift: bool, pub kind: ContextFlags }

/// PHY-SRV-006 / PHY-SHP-012: no state change. Used by L2, by MVC-CHK-005 and by the pose tests.
#[must_use]
pub fn resolve<W: CollisionWorld, E: EntityColliders>(
    world: &W, others: &E, table: &CollisionTable,
    request: &ResolveRequest, scratch: &mut Scratch,
) -> Resolved;

/// PHY-MOV-001 steps 1–9.
pub fn move_body<W: CollisionWorld, E: EntityColliders>(
    world: &W, others: &E, table: &CollisionTable,
    body: &mut Body, mover: MoverKind, displacement: DVec3,
    effects: &mut Vec<PhysicsEffect>, scratch: &mut Scratch,
) -> MoveReport;

pub fn intersects<W: CollisionWorld, E: EntityColliders>(       // PHY-SHP-013, PHY-BOX-006, MVC-CHK-005
    world: &W, others: &E, table: &CollisionTable, boxed: Aabb, context: &ShapeContext,
) -> bool;

/// PHY-RCH-001 / PHY-RCH-005: the columns a tick can read, for the halo test of §6.2.
#[must_use]
pub fn read_zone(body: &Body, row: &MotionRow, displacement: DVec3) -> Aabb;
```

`Scratch` holds reused buffers; a step never allocates once warm. The box is never stored:
PHY-BOX-002 rebuilds it from the position after every change. Collider order is PHY-SHP-011
exactly.

### 3.5 One tick

```rust
pub struct MotionRow {                   // one row of PHY-ORD-005, plus PHY-ORD-009 flags
    pub order: Order,                    // Player | Living | Omnidirectional | Flier | Item | FallingBlock
                                         // | PrimedTnt | ExperienceOrb | Arrow | Thrown | Spit | Accelerated
    pub gravity: f64,
    pub horizontal_drag: f32,
    pub vertical_drag: f32,
    pub step_height: StepHeight,         // Attribute | Fixed(f32)
    pub dimensions: PoseDimensions,
    pub stuck_ignore: StuckSet,
    pub powder_snow: PowderSnowFlags,
    pub block_bounce_factor: f32,
    pub travel_gate: TravelGate,
    pub pushable: Pushable,              // PHY-ORD-009/012: Never | Always | Default
    pub policy: Policy,                  // Exact | Semantic (PHY-CMP-005/006)
}

/// PHY-ORD-007: step(S, K, R, W) -> S'. Pure; the caller owns S and S'.
pub fn tick_player<W: CollisionWorld, E: EntityColliders>(
    world: &W, others: &E, tables: &Tables,
    start: &PlayerState, keys: Keys, rotation: Rotation,
    out: &mut PlayerState, effects: &mut Vec<PhysicsEffect>, scratch: &mut Scratch,
);

/// PHY-ORD-002 / PHY-ORD-004 and the non-living orders of PHY-ORD-005.
pub fn tick_entity<W: CollisionWorld, E: EntityColliders>(
    world: &W, others: &E, tables: &Tables, row: &MotionRow,
    body: &mut Body, living: Option<&LivingInput>, phase: TickPhase,
    effects: &mut Vec<PhysicsEffect>, scratch: &mut Scratch,
) -> TickOutcome;                        // Moved | Rested | Unchanged
```

`PhysicsEffect` is the deferred write of PHY-RCH-002 (step, inside and fall actions, bounce, game
events). The driver delivers it as a `Message`; a write into a column of another cell becomes a
`CellBlockWrite` in the cell mail, as every other domain does (CELLS §4). Dispatch is a `match` on
`Order`.

## 4. Data model (server, Bevy)

### 4.1 Components of a server-simulated entity

Each entity belongs to the cell that owns the column it stands in, as players already do
(CELLS §3). Its global id is the existing `EntityId` from the coordinator's ranges
(`entity_ids.rs`).

| Component | Content | Class (bevy-ecs) | Writer |
|-----------|---------|------------------|--------|
| `Transform` (existing) | position f64, rotation | authoritative | physics step (`set_if_neq`) |
| `Velocity` (existing) | DVec3 | authoritative | physics step, impulse sources |
| `MotionState` | flags, support, fall distance, stuck, jump delay, rest | authoritative | physics step only |
| `MotionClass(u16)` | index into `MotionTable` | authoritative, immutable after spawn | spawn |
| `BodySize` | f32 half width, height, eye height | materialized from row + pose + `scale` + baby | `project_body_size` |
| `LivingInput` | input vector f32, jump, speed | authoritative per tick | AI |
| `MotionAttributes` | the ten attributes of PHY-SRV-005 | materialized | attribute projection |
| `MotionEffects` | amplifiers of `slow_falling`, `levitation`, `jump_boost`, `weaving` | materialized | effect projection |
| `TickListSeq(u64)` | position in the cell's tick list (PHY-ORD-010 step 4) | authoritative | the system that adds an entity to the tick list; re-entry and arrival from another cell take a new, larger value |
| box | — | derived on read, never stored | — |
| `pushable` | — | derived on read from `MotionRow::pushable` and state (PHY-ORD-012) | — |

`MotionClass` has `#[require(Velocity, MotionState, BodySize)]`. Rows live in a `MotionTable`
resource. The hot query is `(&mut Transform, &mut Velocity, &mut MotionState, &BodySize,
&MotionClass)` plus `Option<&LivingInput>`; Bevy columns make it SoA by component, and non-living
entities sit in narrow archetypes. Motion state is values, not markers: marker churn at 1% of 100k
entities is ~1000 archetype moves per tick.

`OldTransform` and `SectionEntities` in `mcrs_minecraft_engine_world/src/entity/` stay for now.
The step emits `SectionCrossed { entity, from, to }`, so a later change can retire the
`OldTransform` scan (bevy-ecs D1, D4).

### 4.2 Player components (in the cell of the player)

| Component | Content | Class | Writer |
|-----------|---------|-------|--------|
| `Prediction` | `Predictor` (§5.3) | authoritative (L2) | `predict` |
| `Transform`, `Velocity` | first candidate's position and velocity | materialized (MVC-PRED-012) | `project_accepted_state` |
| `TeleportState` (existing) | pending teleport ids and the synced transform | authoritative | setbacks and accepts (TP-CONFIRM) |

`project_accepted_state` writes `Transform` and calls `TeleportState::sync`, as `process_movement`
does today, so the existing `teleport` system sends no packet for an accepted move. A setback goes
through `TeleportState::sent`; the existing `player_position` writer gains the velocity and the
relative-rotation bits of MVC-RSP-001.

`Prediction` crosses cells with the player: `cell_crossing::Crossing` gains a
`prediction: Option<Box<Predictor>>` field, carried like `replicated` and `view` (CELLS §4, row 1).
L1 state does not cross: it lives on the connection in the host.

### 4.3 Change detection

- Physics writes `Transform` and `Velocity` through `set_if_neq`. Resting entities do not mark
  them, so tile events (§7) cost O(movers), as REPL §4.6 requires ("a subject that does not
  change costs zero bytes").
- Nothing filters on `Changed<MotionState>`.
- Wake tests use `Ref<Velocity>::is_changed()`, `Ref<Transform>::is_changed()` and the same on the
  attribute and effect projections.

## 5. L0, L1, L2

### 5.1 Where the hooks are

The muxer side of the host owns the connections (CELLS §6). Every serverbound play packet passes
`bridge_inbound` (`crates/mcrs_minecraft_engine/src/muxer/bridge.rs`) with its arrival
`timestamp`, the `InboundRateBucket` and the `PacketDispatcher` frame check, and goes to the owner
dimension through `DimInputs::try_send_serverbound`. Every clientbound packet leaves through
`write_queue`, which fixes the final order on the wire (critical, then high, then normal).

| Layer | Hook | Reuses | Adds |
|-------|------|--------|------|
| L0 | `bridge_inbound`, after `PacketDispatcher::dispatch` | the dispatcher's size and trailing-byte checks (MVC-CHK-023 size table, "bytes after the last field") and its disconnect path | `move_validate::check`: the value rows of MVC-CHK-023 (NaN, infinite, pitch, coordinate range, flag bits, command action) |
| L1 inbound | `bridge_inbound`, per connection | `pkt.timestamp` as the arrival time (MVC-CHK-010); `InboundRateBucket` stays the flood limit | `MoveGuard` component on the connection: `move_validate::Connection` |
| L1 outbound | `write_queue`, per connection, after the priority merge | `QueuedPacket::kind()` to classify | probe planning around predicted-state changes in wire order (MVC-LAT-001) |
| L1 → L2 | `DimInputs` | the routing to the owner dimension, the hold while a session is unattached | move packets, `player_input`, `player_command` 1–2, `player_abilities`, `client_tick_end` and the answers to probes are folded into one `ClientTick` per client tick; other packets route as today |
| L2 | the cell of the player, `TickSet::ClientInput` / `Simulate` | `TeleportState`, `Crossing` | `Prediction`, §5.3 |

The keep-alive latency (`muxer/latency.rs`) stays the player-list latency. L1 uses its own
`ping` probes, because MVC needs an acknowledgement at a known place in the client tick stream
(MVC-TICK-002); a keep-alive answer has no such place.

A proxy runs the same `check` and `Connection` in its own forwarding loop and inserts its probes
itself (MVC-LAY-007). `ProtocolIds` comes from `assets/mcrs/reports/registries.json` for the
protocol version of `intention` (MVC-LAY-009); without the report, CHK-024 is off.

### 5.2 L0 and L1 API

```rust
#[must_use]
pub fn check(direction: Direction, packet: &Decoded, body_len: usize) -> L0Output;

pub struct Connection { /* MVC-LAY-003 state; bounded by max_ack_delay */ }

impl Connection {
    pub fn inbound(&mut self, at: Millis, packet: &L0Output, out: &mut L1Output);
    pub fn outbound(&mut self, at: Millis, packet: &OutboundKind, out: &mut ProbePlan);
}

pub struct ClientTick {
    pub index: u64,
    pub moves: MoveSlot,
    pub keys: KeyBits,
    pub sprint: Option<SprintChange>,
    pub fly_request: Option<bool>,
    pub acknowledged: ProbeRange,        // MVC-TICK-002
    pub changes: ChangeAcks,             // which predicted-state changes are applied / in window (MVC-LAT-004)
    pub silent_run: u8,
    pub reports: ReportSet,              // MVC-LAY-005
}
```

`decode` stays where the frame is decoded: the server passes the fields that
`mcrs_minecraft_protocol_game` already decodes; a proxy uses a small raw decoder in the crate for
the dozen fixed-size packets. L1 never reads a clock (MVC-INV-003).

The predicted-state changes on the wire come from two places: packets of the cell (motion,
explode impulse, abilities, effects, attributes, `block_changed_ack`) and the column changes that
the muxer stage serves from `ChunkStream` (block and section updates, chunk loads and forgets).
Both meet in `write_queue`, so L1 sees one ordered stream. Each queued change carries a tag that
L1 records with its probe pair: `ChangeTag::Column { column, tick }` for a column change (the
tick that published it, CELLS §6), `ChangeTag::Player(kind)` otherwise.

### 5.3 L2 (`mcrs_minecraft_move_predict`)

```rust
pub struct Predictor {
    accepted: SmallVec<[Candidate; 2]>,
    record: ReplayRing,                  // replay_depth = 20 (MVC-PRED-015)
    windows: WindowQueue,                // non-block change contents keyed by probe (MVC-LAT-004/005)
    columns: ColumnAcks,                 // per near column: newest applied tick, ticks in window (from ChangeAcks)
    buffer: OffsetBuffer,                // MVC-RSP-004
    levels: ViolationLevels,
}

pub fn predict<W: CollisionWorld, E: EntityColliders>(
    predictor: &mut Predictor, tick: &ClientTick, view: &ClientView<'_, W>, near: &E,
    tables: &Tables, config: &CheckConfig, scratch: &mut PredictScratch,
) -> Verdict;

pub enum Verdict { Accepted(AcceptedTick), Refused(Setback), Coarse(AcceptedTick), Exempt }
```

Per client tick:

1. Mode (MVC-MODE-002/003): block triggers read only the `coarse` brick masks of the near zone.
   Entity triggers (rows 10–11) query the collider and pushable snapshot of §6.1, which includes
   the entities that neighbour cells sent (§6.2).
2. Fast path (MVC-PRED-013/014): `tick_player` for the primary candidate only; ~1.05 steps per tick
   for a legit client.
3. On a miss, the full set, then replay (MVC-PRED-015/017), capped by `max_steps_per_tick`.

### 5.4 Client world view and kept changes

```rust
pub struct ClientView<'a, W> {
    base: &'a W,                         // CellCollisionWorld: owned + halo sections, current state
    overrides: &'a [BlockOverride],      // old states of kept changes not applied, or in window
    unloaded: &'a ColumnSet,             // columns not loaded on the client, or in a window
}

#[derive(Resource)]
pub struct ChangeLog {                   // per cell
    entries: VecDeque<LoggedChange>,     // tick, BlockPos, old, new
}
```

- `ChangeLog` records every `BlockPlaced` of the cell's sections, owned and halo alike, with the
  tick of the change: the owner's tick for an owned write, the publishing tick for a halo change.
  These ticks are the ticks that tag the column changes of the muxer (§5.2), so the `(column,
  tick)` acknowledgement from L1 selects the old or new state of a logged change.
- For each tick, L2 classifies the logged changes in the read zone by MVC-LAT-004 from
  `Predictor::columns`: not applied gives the old state, window gives both runs (MVC-LAT-006).
  A replay (MVC-PRED-017) classifies for the replayed tick.
- Retention (MVC-PRED-018): an entry stays while a player of the cell whose recorded boxes, grown
  by the near zone, can touch it has not applied it for `replay_depth` client ticks.
  `trim_change_log` is the one writer. The block states are stored once per cell, the probes once
  per connection in L1: no copy per recipient (bevy-ecs D5).
- A player that crosses cells carries `Predictor` with its `columns`; the new cell's `ChangeLog`
  holds the same changes for the shared halo columns, because both cells logged the same owner
  ticks.
- `ClientView` reports a brick that holds an override as `solid` and not `full`.

## 6. Server entity tick in a cell

### 6.1 Units of work

A cell is the unit of placement (CELLS §1). Inside a cell, PHY-RCH-004 and PHY-CMP-008 make each
entity its own unit of work for physics: it reads the cell's sections and a start-of-tick
snapshot of colliders, and writes only itself and deferred effects (PHY-RCH-002). No new shard
type is needed. The cell holds many such units, as CELLS §1 says of shards.

```rust
#[derive(Resource)]
pub struct ColliderSnapshot {
    colliders: FxHashMap<SectionPos, SmallVec<[SnapshotBody; 2]>>,   // PHY-SHP-009 entities
    pushables: FxHashMap<SectionPos, SmallVec<[SnapshotBody; 4]>>,   // near player zones only, for MVC-MODE-003 row 11
    border: Option<WorldBorder>,
}
```

`snapshot_colliders` builds it at the start of `Simulate` from the cell's own entities plus the
`HaloBodies` that neighbour cells sent (§6.2). Collidable entities are rare (~2k inserts, ~60 µs).
Pushables are collected only for the sections near the cell's players.

Exact-parity mode is one cell with every link `Inline` (CELLS §10). The driver then runs
`step_entities_in_order`: one sequential pass in `TickListSeq` order, with `LiveColliders`,
passengers right after their vehicle, entities added in the pass waiting for the next tick
(PHY-ORD-011), and the server part of the players after the pass (PHY-ORD-010 step 6). Client
ticks are already applied before `Simulate` (step 1), and the host ticks dimensions in creation
order (step 2).

### 6.2 Cell boundaries

| Case | Rule here | CELLS rule it follows |
|------|-----------|-----------------------|
| Read zone (PHY-RCH-001) crosses into a neighbour's column | read the halo; no message | "Collision, box queries: read the halo" |
| Read zone leaves the halo | the halo is 2 columns (`HALO`, 32 blocks) past the owned boxes. A horizontal reach of `|d| + 1 + width/2` stays inside for any `|d|` below ~30 blocks per tick. PHY-RCH-003 bounds natural motion far below that; only an impulse, a command or a teleport can exceed it. Such a move sees `Unloaded` past the halo, which can differ from a one-cell world. `read_zone` detects it; the step counts it in a metric. See conflict C2 | none yet |
| A deferred effect (PHY-RCH-002) in a neighbour's column | `CellBlockWrite` in the cell mail, applied by the owner at T+1 | "Block write into a column of another cell" |
| A collidable or pushable entity in a neighbour's column inside the read zone | the owner sends `HaloBodies` (box, kind, pushable, `EntityId`) for its entities within `HALO` of the boundary in its cell mail; the reader sees them at T+1, so one tick older than its own colliders. See conflict C1 | new row |
| An entity walks into a neighbour's column | `EntityCrossing` in the cell mail at the end of the tick, like `Crossing` for players: components, `EntityId`, `Replicated`. The new owner gives it a new `TickListSeq` at the end of its list (PHY-ORD-010: re-entry goes to the end) | extends "Player that walks across" |
| The vertical axis | a cell owns whole columns (`Leaf::Whole` on Y), so falls never cross a boundary | — |

Vertical reach (PHY-RCH-003 terminal falls of 3.92 blocks per tick, the 25-block potent-sulfur
probe of MVC-MODE-003) never touches the halo test, because no boundary is horizontal.

### 6.3 Rest and wake

```rust
pub enum Rest {
    Awake,
    Resting { since: GameTick, zone: ZoneStamp },   // max brick revision over the rest read zone
}
```

- Enter: after a full tick with `S' == S` bit for bit (PHY-SKIP-001), no `action` brick in the
  read zone (PHY-SKIP-004), zero stuck multiplier, zero input. Items take `v.y` from the phase
  `(t + n) mod 4` while skipped (PHY-SKIP-003, PHY-CMP-007).
- Read zone at rest: PHY-RCH-005 (box grown by 1 on `x` and `z`, `1 + h` up, `1 + h + |d.y|` down).
- Wake (PHY-SKIP-002): a brick revision in a zone section above `zone` (halo changes bump it too,
  through `BlockPlaced`), a section landing or leaving, a snapshot body in the zone (own or
  `HaloBodies`), a border change, a changed `Ref<Velocity>`/`Ref<Transform>`/projection, non-zero
  `LivingInput`. Extra wakes are allowed (PHY-SKIP-005).
- The tick count advances every tick, skipped or not (PHY-SKIP-005); the skip path never writes it.
- A cell with no ticket skips its schedules entirely (CELLS §4, `is_idle`). Its entities are in
  cold space and do not tick in the reference either.

### 6.4 In-air batch

An awake entity whose swept read zone has no `solid`, `action` or `coarse` brick and no snapshot
body moves with `r = d`. Its tick is integration only, batched per task in f64 lanes
(bit-identical to scalar, no deviation). An f32 local-frame variant is allowed by PHY-DEV-001;
REPL quantises moves to 1/4096 block, so a 1e-6 difference never reaches a relative move. Use it
only if a benchmark shows the f64 batch is the bottleneck.

## 7. Outbound positions

Server entities replicate through the existing tile path, never through a packet of the cell
(REPL §7.1):

- A system next to `publish_player_tiles` (after `VoxelUpdateSet::NetworkSync`) pushes
  `EntityEvent::Move` into `TileEvents` for each non-player entity whose `Transform` changed, and
  `Enter`/`Leave` when it changes tile. Work is O(movers).
- `mcrs_minecraft_replication` quantises to 1/4096 per tier delta base (`delta_base.rs`), sends a
  keyframe (`entity_position_sync`) on overflow, on entering a tile and every
  `KEYFRAME_INTERVAL_TICKS`.
- What is missing for entities that are not players (CELLS §12 step 4 already names "a codec for
  entities that are not players"): the codec writes `on_ground: true` for every move today. A
  server entity needs its on-ground flag, `set_entity_motion` when the velocity changes by more
  than `1.0E-7` squared, and a precise sync when PHY-CMP-002 asks for one (on-ground change;
  falling block and TNT after a collision tick). `Pose` gains `on_ground` and a `precise` bit, and
  a `Data` event carries the velocity. See conflict C3 for tiers against client-simulated
  categories.

## 8. Schedule inside a cell

All systems are dimension plugins in the cell's sub-app. `TickSet` phases in `FixedUpdate`:

```text
DomainInput:  cell mail applies (halo changes -> BlockPlaced, crossings arrive incl. Prediction and EntityCrossing, HaloBodies)
ClientInput:  drain ClientTick (from DimInputs)
Simulate:     snapshot_colliders
              -> predict              (par_iter over players)
              -> [entity AI]          (outside this note; writes LivingInput)
              -> step_entities        (par_iter over awake entities; batch in-air)
                 | step_entities_in_order (exact-parity mode, run condition)
              -> write_effects        (merge per-thread buffers, sort by (TickListSeq, seq))
WorldWrite:   own block actions land; actions for other cells go to the cell mail
Project:      project_section_bricks (BlockPlaced) || project_accepted_state || trim_change_log
Output:       setbacks (TeleportState); tile events of entities; cross_players + cross_entities; HaloBodies
```

- `predict` and `step_entities` are ordered, not concurrent: each saturates the compute pool, and
  `predict` reads entity boxes that `step_entities` writes.
- Inside `par_iter`: no `Commands`, no structural change. Effects go into `Local<Parallel<Vec<_>>>`
  and are sorted by `(TickListSeq, sequence)` before they become messages or cell mail; the cell
  mail keeps the cell order (CELLS §4), so the result does not depend on thread timing.
- The plugins join `MinecraftServerPlugins`; `tests/it/schedule_ambiguity.rs` covers them, and
  `tests/it/tick_determinism.rs` compares `Inline` against `Thread` placements.

## 9. Fit with cells

| Part | Status | Against |
|------|--------|---------|
| Partition unit | reuses the cell; no `EntityShard` type | CELLS §1, §3 |
| Ownership of entities | reuses "owner of the column", as for players | CELLS §3 |
| Entity ids | reuses the coordinator's ranges and `EntityId` | REPL §7.2, `entity_ids.rs` |
| World view of the kernel | reuses `DimensionView` over owned + halo sections; adds `SectionBricks` | CELLS §4 halo, `cell_volume.rs` |
| Brick projection input | reuses `BlockPlaced` (owned, foreign writes and halo changes) | `voxel_update.rs` |
| Halo width | reuses `HALO` = 2 columns; enough for every bounded displacement | CELLS §3, PHY-RCH-001/003/005; conflict C2 |
| Deferred block actions across a boundary | reuses `CellBlockWrite` | CELLS §4 |
| Entity colliders across a boundary | new `HaloBodies` mail kind | conflict C1 |
| Entity crossing | extends `cell_crossing` (`Crossing` gains `Prediction`; new `EntityCrossing`) | CELLS §4 row 1 |
| Tick, sync point, lockstep, idle skip | reuses | CELLS §4 |
| Exact-parity mode | reuses one cell + `Inline` | CELLS §10, ARCH Decision 20 |
| L0 | extends `bridge_inbound` after `PacketDispatcher` | CELLS §6 |
| L1 | new `MoveGuard` on the connection; hooks in `bridge_inbound` and `write_queue` | conflict C4 |
| L1 → L2 transport | reuses `DimInputs` and session placement | CELLS §6 "input to the owner cell" |
| Probes around column changes | extends `ChunkStream` changes with a `(column, tick)` tag | CELLS §6 |
| Kept changes (MVC-PRED-018) | new `ChangeLog` per cell, keyed by the owner's tick | — |
| Setbacks | reuses `TeleportState` and its `player_position` writer | `teleport.rs`, `movement.rs` |
| Outbound positions of entities | reuses tile deltas, delta bases, keyframes; extends the codec | REPL §7; conflict C3 |
| Player position today | replaces `process_movement` ("takes the position of the client and does no collision yet") | CELLS §4 table |
| Latency | keep-alive `Latency` unchanged; L1 uses play `ping` probes | `muxer/latency.rs` |

Conflicts:

- **C1. Colliders of another cell are one tick old.** PHY-CMP-008 allows boxes from the start of
  the tick. `HaloBodies` sent at T apply at T+1, so a reader near a boundary sees a neighbour's
  boat one tick older than its own boats: up to two ticks of collider displacement instead of one.
  The same lag applies to the entity triggers of MVC-MODE-003 for a player near a boundary.
  Needs request R1/R3.
- **C2. A read zone beyond the halo.** Only displacements above ~30 blocks per tick reach it
  (impulses, commands). One cell may hold those columns loaded where many cells do not. CELLS
  §4 has no row for entity motion; one is needed (R2).
- **C3. Tiers against client-simulated categories.** PHY-CMP-001: the client simulates items,
  falling blocks, TNT, arrows and thrown items from the last state, and sets the position from an
  update with no interpolation. REPL tiers 1–3 send paths of steps at 5, 2 and 1 updates per
  second, and C5 assumes the client interpolates. For those categories a far tier shows jumps at
  each update and the client's own simulation in between. REPL §4.7 allows a different rate, but
  REPL has no rule for categories that the client simulates. The codec also hard-codes
  `on_ground: true`, which PHY-CMP-002 uses for precise syncs. Needs request R4.
- **C4. Priority queues and shedding.** `write_queue` sends critical, then high, then normal, and
  `ready_to_write` sheds normal packets past `DEPTH_LIMIT`. MVC needs every predicted-state change
  to arrive, and each probe to stay between the same changes on the wire. So the L1 outbound hook
  runs in `write_queue` after the merge, and every predicted-state change (`ColumnChange` with
  `PacketPriority::Normal` today) must be high or critical, or exempt from shedding. Code change,
  no spec change.
- **C5. Serial host time.** `bridge_inbound` loops over connections on the host thread between
  ticks, and the host time adds to the lockstep tick (cells tick after the host's `Last`). L0+L1
  at ~100 ns per packet and ~50k packets per tick adds ~5 ms serial. `bridge_inbound` must become
  a `par_iter_mut` over connections (each connection's bucket and `MoveGuard` are independent),
  with routed output merged in connection order.

## 10. Performance budget

### 10.1 Parallelism model

- Cells of all worlds tick in lockstep after the host's `Last`, `Inline` or one `Thread` each
  (`worlds` field). The tick lasts as long as the slowest cell plus the host's serial work.
- Inside a cell, `par_iter` runs on the process-wide compute pool, which all cell threads share.
  Under `Placement::Inline` the server sizes that pool to 1 thread (`server_loop.rs`), which is
  the deterministic test mode.
- Static division is by region (32 × 32 columns, 512 blocks). Players cluster, so the load across
  cells is uneven; a dense crowd sits in one cell (REPL §3 notes the same of Folia). Cell threads
  give parallelism across spread-out load; `par_iter` inside the hot cell carries a crowd.

### 10.2 Load and budget

| Work | Where | Count per tick | Cost each | core-ms |
|------|-------|----------------|-----------|---------|
| L0+L1 | host, `bridge_inbound` / `write_queue` | ~50k packets | 100 ns | 5 (serial today, see C5) |
| `predict`, fast path | cell of each player | 10k | 2.0 µs | 20 |
| `step_entities`, awake | owner cell | 40k (60% at rest) | 1.0 µs | 40 |
| `step_entities`, resting | owner cell | 60k | 15 ns | 1 |
| `snapshot_colliders` + `HaloBodies` | each cell | ~2k colliders + boundary copies | 30 ns | 0.1 |
| `project_section_bricks` | each cell, owned + halo | ~10k changes, halo changes counted in each reader | 200 ns | 2–3 |
| entity tile events | owner cell | ~40k movers | 50 ns | 2 |
| **Total** | | | | **~70 core-ms** |

Target: physics and checks ≤ 5 ms wall of the 50 ms tick at 16 cores. Cases:

- Spread load (players and entities over ≥ 16 cells): each cell does ~4 core-ms on its own thread,
  with `par_iter` spare capacity: ~4–5 ms wall.
- One crowd in one cell (worst case): that cell carries most of the 65 core-ms of cell work; its
  `par_iter` systems spread it over the shared pool while the other cells idle: ~5–6 ms wall at 80%
  efficiency. The serial parts of that one cell (snapshot, effect sort, cell mail, crossing)
  must stay under 0.5 ms.
- Host serial: 5 ms today if L0/L1 stay in the serial loop. After C5, ~0.4 ms wall at 16 cores.
  This is the largest change against the first version of this note, which put L0/L1 on IO
  threads.
- New muxer load: 40k moving entities a tick become tile events. REPL budgets bytes per viewer;
  the encode cost of non-player subjects is not in REPL's measurements yet (open question 4).

### 10.3 Kernel targets (median, one core, warm cache)

| Benchmark | Target |
|-----------|--------|
| `resolve`, inert air | ≤ 60 ns |
| `resolve`, walk on flat ground | ≤ 300 ns |
| `resolve`, step-up onto a slab | ≤ 1.2 µs |
| `tick_entity`, walker on ground | ≤ 600 ns |
| `tick_entity`, item in air, scalar / f64 batch per entity | ≤ 80 ns / ≤ 30 ns |
| `tick_player`, one candidate, walking | ≤ 1.5 µs |
| `predict`, legit tick, fast path | ≤ 2.5 µs |
| L0 `check` / L1 per packet | ≤ 30 ns / ≤ 100 ns |
| brick recompute per block change / per new section | ≤ 300 ns / ≤ 10 µs |

### 10.4 Levers

| Lever | Expected gain | Spec dependency | Deviation entry |
|-------|---------------|-----------------|-----------------|
| Skip resting entities | 2–3× on entity cost | PHY-SKIP-001..005, PHY-RCH-005 | no |
| Per-entity `par_iter` inside the cell | ~12× wall in a crowded cell | PHY-RCH-004, PHY-CMP-008 | PHY-DEV-002 |
| Cells on threads | up to the cell count for spread load | CELLS §4 | no |
| Brick masks + shape classes | 3–5× fewer block reads; 0 in air | none | no |
| L2 fast path | ~3× on L2 | MVC-PRED-013..016 | no |
| Parallel `bridge_inbound` (C5) | ~5 ms → ~0.4 ms of serial host time | none | no |
| In-air batch, f64 / f32 local frame | 2–4× / further 2× on that subset | none / PHY-CMP-006 | no / PHY-DEV-001 |
| `set_if_neq` + rest skip on tile events | muxer work O(movers) | REPL §4.6 | no |
| Reduced tick rate for far entities | rejected: changes trajectories; idle cells and the reference ticking range already cover far space | — | — |

## 11. Spec changes

All seven requests of the first version are resolved:

| # | Request | Resolved by | Where this note uses it |
|---|---------|-------------|-------------------------|
| S1 | Reference order of entity ticks | PHY-ORD-010, PHY-ORD-011 | `TickListSeq`, `step_entities_in_order` (§6.1) |
| S2 | World view of a replay; retention | MVC-PRED-017, MVC-PRED-018 | `ChangeLog`, `ColumnAcks` (§5.4) |
| S3 | "Pushable" as data | PHY-ORD-009, PHY-ORD-012 | `MotionRow::pushable`, derived on read |
| S4 | Coarse blocks as data | PHY-ORD-013 | `COARSE` flag and `coarse` mask (§3.2, §3.3) |
| S5 | Extra wakes allowed | PHY-SKIP-005 | per-section-revision wake (§6.3) |
| S6 | Read zone at rest | PHY-RCH-005 | `ZoneStamp` (§6.3) |
| S7 | Attribute and effect ids at a proxy | MVC-LAY-009 | `ProtocolIds` (§5.1) |

New requests, from the fit with cells. The own player stays exact in every item.

| # | To | Rule | Change | Visible effect | Test |
|---|----|------|--------|----------------|------|
| R1 | PHY | PHY-CMP-008, PHY-DEV-002 | Allow the box of a collider owned by another cell to be the box at the start of the previous tick. | an entity next to a boat across a cell boundary stops up to one tick later than with a one-cell world | one cell against two cells: equal except entities with a moving collider of another cell in the read zone |
| R2 | CELLS §4 | new row "Entity motion" | State the boundary rule of physics: read the halo; a move whose read zone leaves the halo reads unloaded space; deferred block actions are `CellBlockWrite`s; colliders arrive as `HaloBodies` one tick late. State it as the ordering policy of the entity domain (ARCH §18). | as R1, plus a >30-block impulse near a boundary | one cell against many cells, `tick_determinism.rs` |
| R3 | MVC | MVC-MODE-003 rows 10–11 | Grow the entity part of the near zone by one tick of the entity's displacement for entities owned by another cell, or accept a one-tick-late trigger. | none for a legit client when grown | C-new: a boat across a cell boundary moving into the near zone; expect no setback |
| R4 | REPL / PHY | REPL §7.4, PHY-CMP-001/002 | State the update rule for the categories that the client simulates (items, falling blocks, TNT, projectiles, orbs): tier paths, or the precise-update rule of PHY-CMP-002. Add the client behaviour of PHY-CMP-001 to REPL §5 as a rule-backed line. | far items jump at each update with tier paths | client model of REPL §7.10 decodes item updates; rest position equal within 1/4096 |

No new intentional deviation is proposed. PHY-DEV-001 (optional) and PHY-DEV-002 stay the only
ones in use; R1 widens PHY-DEV-002.

## 12. Verification

### 12.1 Kernel tests without Bevy

- PHY-GRV-007: horizontal factor `0.546000063419342` on friction `0.6F`.
- PHY-SKIP-003: walker rest `v.y = -0.0784000015258789`; item cycle `0, -0.04, -0.08, -0.12`.
- PHY-SHP-005: bottom slab lines 0, 0.5, 1; a `y` 0..0.75 box stops a penetrating mover at 0.25.
- PHY-COL-005, PHY-STP-005, PHY-NUM-005 table entries.
- PHY-RCH-001/005: `read_zone` for a walker, an item, a resting walker.
- A `CollisionWorld` test double: a `HashMap<BlockPos, BlockStateId>` world.

### 12.2 Property tests

- PHY-INV-001..013 on `resolve` and `move_body`; brick masks give the same `Resolved` as reading
  every cell; f64 batch bit-identical to scalar, f32 within PHY-CMP-006.
- Rest: skipping equals never skipping under random wakes (PHY-SKIP-001, PHY-SKIP-005), with the
  item tick count advancing on skipped ticks.
- Replay sees the original block states (MVC-PRED-017); no kept change is dropped early
  (MVC-PRED-018).
- MVC-INV-001, 003, 005, 006, 007 over random packet streams through L0–L2; L0/L1 fuzzed bytes.

### 12.3 Thread count, cell layout, ambiguity

- `tests/it/tick_determinism.rs` gains entities and scripted players: `Inline` against `Thread`
  placements give equal world hashes, tile deltas included (REPL §7.10).
- One cell against many cells: equal for every entity and player away from a boundary, and within
  R1/R2 near one.
- Exact-parity mode (one cell, `Inline`) against vanilla fixtures, bit for bit with the batch off.
- `tests/it/schedule_ambiguity.rs` with the new plugins.

### 12.4 Vanilla parity

- Server entities: PHY E1–E8, E15–E20 from `tools/vanilla-oracle`, within 1e-6 and flags exact.
- Own player: PHY E9–E14, E21–E23 and MVC C1–C7 as recorded serverbound packets: `tick_player`
  bit for bit, L0–L2 with zero violations.

### 12.5 Benchmarks

- `crates/mcrs_minecraft_physics/benches/`: every row of §10.3, failing CI at 1.5× the target.
- The existing `tick_load` example and `bot_load` bench gain entities and legit movement: 10k
  players and 100k entities over 16 cells and in one cell, measuring the slowest cell and the host
  time between ticks. Target ≤ 5 ms for the physics and movement sets of the slowest cell, and
  ≤ 0.5 ms of L0/L1 host time.

## 13. Open questions

1. C5: parallelising `bridge_inbound` touches the muxer bridge; its owner decides the shape (par
   over connections, or moving the per-connection part into the network reader task).
2. U4/U6 in PHY: the corpus gives f32-rounded boxes, not grid lines. `OFF_GRID` shapes force
   coarse mode near them until a corpus component gives exact values.
3. The client's placeholder fly model (`local_player.rs`) could move to `tick_player`.
4. Muxer cost of non-player subjects: REPL measured players only. 40k moving entities per tick
   need a measurement of tile encode and keyframes before the entity codec lands.
5. Push between entities (PHY §12) will couple entities; a cluster path inside a cell, and the
   boundary rule for pushes across cells, wait for its spec.
6. AI order: the AI set must run before `step_entities` and write only `LivingInput`.
