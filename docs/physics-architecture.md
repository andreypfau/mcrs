# Physics and movement checks: architecture

Design note for collision, gravity, the one-tick step of every entity, and the movement checks of
the player. Behaviour comes from `docs/spec/entity-physics.md` (PHY) and
`docs/spec/movement-checks.md` (MVC). Structure follows
`docs/minecraft-bevy-sharding-architecture.md` (cited as ARCH §n).

Three users, each linking only what it needs:

| User | Needs | Links |
|------|-------|-------|
| L7 proxy | L0 + L1 checks (MVC-LAY-002, MVC-LAY-003, MVC-LAY-007) | `mcrs_minecraft_move_validate` |
| Anticheat on a foreign host | L0–L2 over the host's own block storage | `_move_validate`, `_move_predict`, `_physics` |
| mcrs server | everything, driven by Bevy | all five crates |
| mcrs client | own-player tick (PHY-ORD-001) for local prediction | `_physics` |
| Parity tools, `tools/vanilla-oracle` | the kernel alone | `_physics` |

## 1. Policies this design relies on

| Domain | Policy | Rule | Consequence here |
|--------|--------|------|------------------|
| Own player tick, re-application, `resolve` query, PHY-SRV-008 velocity | exact, bit for bit | PHY-CMP-005 | one scalar f64 path, no tolerance, no reordering |
| Server-simulated motion | semantic, 1e-6 per axis per tick | PHY-CMP-006, PHY-DEV-001 | optional local-frame f32 batch path |
| Discrete results of server entities (step-up candidates, on-ground, item rest cycle, type flags) | exact | PHY-CMP-007 | batch path never decides these |
| Tick order of server entities | semantic; colliders from start of tick | PHY-CMP-008, PHY-DEV-002 | every entity is an independent unit of work |
| Skip of a fixed-point tick | exact (observably identical) | PHY-SKIP-001..005 | rest/wake machinery |
| L0/L1/L2 | layered, lower never reads higher | MVC-LAY-001..008 | three crates, strict dependency direction |

## 2. Crate split

### 2.1 Crates

| Crate | Bevy | Content | Deps |
|-------|------|---------|------|
| `mcrs_minecraft_physics` | no, no tokio | f64 boxes; collision shapes with grid lines; per-state collision and motion tables; section brick masks (types only); world-view traits; collision query; clip and axis resolution; step-up; move procedure; supporting block; flags and restitution; one-tick step per category; sine table; rest detection; in-air batch path | `mcrs_voxel`, `glam` |
| `mcrs_minecraft_move_validate` | no, no tokio | L0: raw decode and MVC-CHK-023 for the dozen fixed-size packets of both directions. L1: per-connection state, client-tick assembly, probes and windows, tick-rate balance, packet check, L1 speed bound (MVC-CHK-024) | `mcrs_minecraft_wire`, `glam` |
| `mcrs_minecraft_move_predict` | no, no tokio | L2: candidate set, try order, fast path, replay, windows applied to candidates, client world overlay, modes, offset/on-ground/collision checks, offset buffer, violation levels, setback decision | `_physics`, `_move_validate` |
| `mcrs_minecraft_engine_physics` | yes | dimension plugin: tables as resources, section brick projection, world-view impl over `ChunkBlocks`, collider snapshot, server entity tick systems, rest/wake, effect messages | `_physics`, `mcrs_minecraft_engine_world`, `mcrs_minecraft_block`, bevy |
| `mcrs_minecraft_engine_movement` | yes | L0/L1 hook in the connection task, `ClientTick` bridge into the dimension, L2 systems, projection of the accepted state, setbacks. Replaces `crates/mcrs_minecraft_engine/src/movement.rs` | `_move_predict`, `_engine_physics`, `mcrs_minecraft_network`, `mcrs_minecraft_protocol_game`, `mcrs_minecraft_engine` |

All five get `[lints] workspace = true`, `[lib] doctest = false`, a `[workspace.dependencies]`
entry and flat `src/`. The three kernel crates go in `NO_BEVY_PACKAGES`. They need `std` only for
`f64::sqrt` (IEEE correctly rounded, which PHY-NUM-001 needs) — `// std: f64::sqrt`.

Why the kernel is one crate and not `_collision` + `_tick`: both halves have the same dependencies,
and rust-style forbids a topic-only split. The seam is kept clean (shapes/query/move procedure
never import the tick model) so that a split costs one move when a user needs collision without
the tick model.

Why L0 and L1 share a crate: same dependencies, and every user of L1 also runs L0. Why L2 is a
separate crate: it pulls in the physics kernel, which a proxy must not pay for (MVC-LAY-007).

Why the vanilla data is not in the kernel: rust-style forbids a default that names vanilla. The
kernel defines `MotionRow` and `BlockMotion`; the PHY-ORD-005 and PHY-GRV-017 rows are data the
app supplies — a corpus file under `assets/mcrs/` once U2/U3 close, a const table in
`mcrs_minecraft_server` until then.

### 2.2 Dependency graph

```text
mcrs_voxel  glam                       mcrs_minecraft_wire
     \      /                                  |
  mcrs_minecraft_physics            mcrs_minecraft_move_validate   <- proxy stops here
          \                           /
           mcrs_minecraft_move_predict                             <- foreign-host anticheat stops here
          |                           |
mcrs_minecraft_engine_physics         |
  (+ engine_world, block, bevy)       |
           \                          |
            mcrs_minecraft_engine_movement
              (+ network, protocol_game, engine)                   <- server
```

`mcrs_minecraft_protocol_game` is deliberately absent below the Bevy layer: it pulls in block,
chunk, item and command crates. L0 decodes the ~12 packets it needs straight from body bytes; the
server runs that same decoder in the connection task, so there is one definition of the checked
layout, and "bytes after the last field" (MVC-CHK-023) is checked on the real bytes.

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

`mcrs_minecraft_engine_physics` implements `CollisionWorld` over `SectionIndex` + `ChunkBlocks` +
`SectionBricks`. A foreign host implements it over its own chunks (ARCH §13). L2 wraps any
implementation in `ClientView` (§5.4).

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
    pub flags: StateFlags,              // u8: LARGE, CONTEXTUAL, INSIDE_ACTION, STEP_ACTION, ...
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
  stops a penetrating mover at an interior line (PHY-COL-003); it must take the `Grid` path, so
  "single box" alone is not the criterion. The table builder decides this once per shape.
- `Full` is the full cube: grid lines 0 and 1 only, so face clipping is exact.
- `LARGE` (PHY-SHP-007 step 3) is precomputed per state.
- Shapes are built in f64 from the corpus boxes (PHY-SHP-002). `BlockDefinitions` stores f32
  `Aabb`; the builder converts each value back to the pixel grid and recomputes f64, which is exact
  only for dyadic values. U4/U6 shapes are flagged `OFF_GRID` and feed MVC-MODE-003 row 9.
- The existing `mcrs_minecraft_core::voxel_shape` is an occlusion shape (f32, clipped to the unit
  cube, 1/32 face masks). It is not reused for collision.

Size: ~30k states × 4 B = 120 KB plus a few hundred shapes: stays in L2 cache on every core.

### 3.3 Per-section brick masks

A section is 4×4×4 bricks of 4³ blocks. One `u64` per class, one bit per brick:

```rust
#[derive(Clone, Copy, Default, PartialEq)]
pub struct BrickMasks {
    pub solid: u64,       // brick has a non-empty or contextual collision shape
    pub full: u64,        // every block in the brick is a full cube
    pub large: u64,       // brick has a LARGE state (outer scan shell, PHY-SHP-007)
    pub action: u64,      // brick has an inside action or a step action (PHY-RCH-002, PHY-SKIP-004)
    pub coarse: u64,      // brick has a PHY-ORD-013 block, or an OFF_GRID shape (MVC-MODE-003 row 9)
    pub revision: u32,    // bumped on any change; wake test of PHY-SKIP-002
}
```

The `COARSE` state flag that feeds `coarse` comes from the block list of PHY-ORD-013, resolved to
state ids when the table is built (every state with a fluid, the pistons, shulker boxes, portals,
`big_dripleaf`, the offset blocks, `potent_sulfur`). Slime and honey blocks are coarse only while
a piston moves them, which is a `moving_piston` state, so they need no flag of their own.

48 bytes per section; 500k loaded sections cost 24 MB. A query reads the masks of the bricks
that its scan region touches:

- brick not `solid` → no block reads for that brick;
- brick `full` → every cell is `Full` without a block read;
- outer shell of the scan region (PHY-SHP-007 steps 3–5) → read only bricks with `large`.

Projection owner: one system, `project_section_bricks`, fed by `BlockPlaced` (a change list, not
`Changed<ChunkBlocks>`), recomputes the touched brick: 64 block reads, ~200 ns. A newly loaded
section computes all 64 bricks once, ~10 µs, through the palette (a palette entry maps to one
`StateCollision`, so a uniform or small-palette section is one or a few table lookups).

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
    pub rest: Rest,                     // §6.2
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
```

`Scratch` holds reused collider and candidate buffers; a step never allocates once warm. The box
is never stored: PHY-BOX-002 rebuilds it from the position after every change, and a stored box
would be a second representation of the position.

Collider order is PHY-SHP-011 exactly (entities, border, blocks in scan order). It changes `r` by
less than 1e-7 (PHY-INV-009), but the exact policy of the player forbids relying on that.

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
    pub stuck_ignore: StuckSet,          // bitset over the stuck blocks
    pub powder_snow: PowderSnowFlags,
    pub block_bounce_factor: f32,
    pub travel_gate: TravelGate,
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

`PhysicsEffect` is the deferred write of PHY-RCH-002 (step action, inside action, fall action,
bounce, game events). It is a temporal fact: the driver delivers it as a `Message`.

Dispatch is a `match` on `Order`, not a trait object. A new entity type adds a row (PHY-ORD-005).

## 4. Data model (server, Bevy)

### 4.1 Components of a server-simulated entity

| Component | Content | Class (bevy-ecs) | Writer |
|-----------|---------|------------------|--------|
| `Transform` (existing) | position f64, rotation | authoritative | physics step (`set_if_neq`) |
| `Velocity` (existing) | DVec3 | authoritative | physics step, impulse sources |
| `MotionState` | flags, support, fall distance, stuck, jump delay, rest | authoritative | physics step only |
| `MotionClass(u16)` | index into `MotionTable` | authoritative, immutable after spawn | spawn |
| `BodySize` | f32 half width, height, eye height | materialized from row + pose + `scale` + baby | `project_body_size` |
| `LivingInput` | input vector f32, jump, speed | authoritative per tick | AI |
| `MotionAttributes` | the ten attributes of PHY-SRV-005 | materialized from the attribute system | attribute projection |
| `MotionEffects` | amplifiers of `slow_falling`, `levitation`, `jump_boost`, `weaving` | materialized from effects | effect projection |
| `TickListSeq(u64)` | position in the dimension's tick list (PHY-ORD-010 step 4) | authoritative | the system that adds an entity to the tick list; re-entry takes a new, larger value |
| box | — | derived on read, never stored | — |
| `pushable` (PHY-ORD-012) | — | derived on read from the row flag and state (alive, spectator, climbable, chunk ticks entities) | — |

`pushable` is not stored: its default depends on state that changes every tick (PHY-ORD-012), and
its only reader is the near-entity test of L2, which evaluates it for a few entities.

`MotionClass` has `#[require(Velocity, MotionState, BodySize)]`. Rows live in a `MotionTable`
resource, not an asset: they never hot-reload and a `Handle` lookup per entity per tick is waste.

Layout: the hot query is `(&mut Transform, &mut Velocity, &mut MotionState, &BodySize,
&MotionClass)` plus `Option<&LivingInput>`. Bevy tables store each component as a column, so
the hot path is already SoA by component. Non-living entities lack `LivingInput`,
`MotionAttributes` and `MotionEffects`, so items and projectiles sit in narrow archetypes.

No markers for motion state. On-ground, rest, pose and policy flip often or are read only
after the entity already matched; they are values. Marker churn at 1% of 100k entities per tick
is ~1000 archetype moves, about 0.5 ms of serial command application; a value check is ~2 ns.

### 4.2 Player components

| Component | Content | Class | Writer |
|-----------|---------|-------|--------|
| `Prediction` | `Predictor` (§5.3): accepted candidates, replay record, buffer, levels | authoritative (L2) | `predict` |
| `Transform`, `Velocity` | first candidate's position and velocity | materialized (MVC-PRED-012) | `project_accepted_state` |
| `MovementMode` | predicted/coarse/resync/exempt of the last tick | materialized | `predict` |

The player has no `MotionState` of its own: the candidate holds it (PHY-ORD-007 `S`). The server
does not run the travel step of the player (PHY-SRV-001); the block effects of the player
(PHY-SRV-004) come from the first candidate.

### 4.3 Change detection

- Physics writes `Transform` and `Velocity` through `set_if_neq`. Resting and blocked entities do
  not mark them, so network sync (PHY-CMP-002) and the section index only see movers.
- Nothing may filter on `Changed<MotionState>`. It is written every awake tick.
- Crossing a section boundary is known inside the step. The step emits `SectionCrossed { entity,
  from, to }` and the section index consumes it. This retires `OldTransform` and the full scan of
  `update_chunk_entities` (bevy-ecs D1, D4).
- Wake tests use `Ref<Velocity>::is_changed()`, `Ref<Transform>::is_changed()` and the same on the
  attribute and effect projections. Physics' own `set_if_neq` writes happen before its last run,
  so they never wake the entity itself; an impulse, a teleport or a command does.

## 5. L0, L1, L2

### 5.1 L0 (`mcrs_minecraft_move_validate`)

```rust
pub enum Decoded {
    Move(MovePacket), Input(KeyBits), Command(PlayerCommand), FlyRequest(bool),
    TickEnd, Loaded, Pong(i32), Accept(AcceptTeleport),
    // clientbound, for L1: Ping, PlayerPosition, EntityMotion, Explode, Abilities, Effect, Attributes, ...
    Clientbound(ClientboundMovement), Other,
}

#[must_use]
pub fn check(direction: Direction, id: PacketKind, body: &[u8], ids: &ProtocolIds) -> L0Output;

pub struct L0Output { pub decoded: Decoded, pub reports: ReportSet, pub response: L0Response }
```

`ProtocolIds` maps packet ids and the numeric ids of `minecraft:attribute` and
`minecraft:mob_effect`. Both registries are static per protocol version and never arrive in
`registry_data`, so the host builds `ProtocolIds` from the registry report of that version,
`assets/mcrs/reports/registries.json` (MVC-LAY-009). The protocol version comes from the
`intention` packet. With no report for the version, `ProtocolIds` has no attribute or effect ids,
and L1 turns off MVC-CHK-024 and runs every other rule.

The clientbound packets L0 decodes are the MVC-LAY-009 list: `login`, `respawn`,
`update_attributes` (only `movement_speed` of the player), `update_mob_effect`,
`remove_mob_effect`, `player_abilities`, `set_entity_motion`, `explode` (the impulse; the frame
length bounds the rest), `player_position`, `ping`, `ticking_state`, `ticking_step`,
`set_passengers`, `set_camera` and `start_configuration`. Cost: one bounds check and ≤ 4 float
classifications per packet, ≤ 30 ns.

### 5.2 L1

```rust
pub struct Connection { /* MVC-LAY-003 state; bounded by max_ack_delay */ }

impl Connection {
    pub fn inbound(&mut self, at: Millis, packet: &L0Output, out: &mut L1Output);
    pub fn outbound(&mut self, at: Millis, packet: &L0Output, out: &mut L1Output);
    pub fn plan_probes(&mut self, group: ChangeGroup) -> ProbePair;   // MVC-LAT-001, MVC-LAY-007
}

pub struct ClientTick {
    pub index: u64,
    pub moves: MoveSlot,                 // None | One(MovePacket); a second is a report (MVC-TICK-003)
    pub keys: KeyBits,
    pub sprint: Option<SprintChange>,
    pub fly_request: Option<bool>,
    pub acknowledged: ProbeRange,        // probes acknowledged for this tick (MVC-TICK-002)
    pub silent_run: u8,                  // MVC-TICK-004
    pub reports: ReportSet,              // MVC-LAY-005
}
```

Time is an input (`Millis` of arrival). L1 never reads a clock, so its result depends only on the
packets and their arrival times (MVC-INV-003).

Placement: L0 and L1 run in the connection task, on the IO thread that already decodes the
frame. This is the same code a proxy runs. The ECS receives one `ClientTick` per
`client_tick_end` through the existing host→dimension bridge, instead of one observer event per
packet: about 1M observer dispatches per second leave the serial part of the schedule at 10k
players.

In the server, the ECS owns the outbound groups of predicted-state changes. The system that emits
a group asks `plan_probes` for the probe pair and stores the pair on each kept change of that group
(§5.4, MVC-PRED-018). A proxy calls the same function from its forwarding loop (MVC-LAY-007).

L1 computes `movement_speed` from base and modifiers as `(b + A) × (1 + B) × C`, clamped
(MVC-LAY-009). It is the only attribute that L1 evaluates.

### 5.3 L2 (`mcrs_minecraft_move_predict`)

```rust
pub struct Predictor {
    accepted: SmallVec<[Candidate; 2]>,
    record: ReplayRing,                  // replay_depth = 20 (MVC-PRED-015)
    windows: WindowQueue,                // non-block change contents keyed by probe (MVC-LAT-004/005)
    kept: KeptChanges,                   // block, chunk and border changes sent to this player (MVC-PRED-018)
    buffer: OffsetBuffer,                // MVC-RSP-004
    levels: ViolationLevels,
}

pub fn predict<W: CollisionWorld, E: EntityColliders>(
    predictor: &mut Predictor, tick: &ClientTick, view: &ClientView<'_, W>, near: &E,
    tables: &Tables, config: &CheckConfig, scratch: &mut PredictScratch,
) -> Verdict;

pub enum Verdict { Accepted(AcceptedTick), Refused(Setback), Coarse(AcceptedTick), Exempt }
```

Per tick:

1. Classify the mode (MVC-MODE-002/003). Block triggers read only the `coarse` brick masks of the
   near zone (PHY-ORD-013 blocks); the potent-sulfur column (25 blocks) reads two sections'
   masks. Entity triggers (rows 10–11) query the section entity index for the near zone, a few
   sections, and evaluate `pushable` (PHY-ORD-012) on the entities found.
2. Fast path (MVC-PRED-013/014): run `tick_player` for the primary candidate only; stop at the
   first match. Expected ~1.05 steps per tick for a legit client.
3. On a miss, the full set, then replay (MVC-PRED-015), capped by `max_steps_per_tick`
   (MVC-PRED-016). A replayed tick uses the world view of that tick (MVC-PRED-017, §5.4).

A `Candidate` is a `PlayerState` (~250 B). The record costs ~7 KB per player, 70 MB at 10k
players.

### 5.4 Client world view

```rust
pub struct ClientView<'a, W> {
    base: &'a W,
    overrides: &'a [BlockOverride],      // blocks whose change is not applied or in its window, in reach
    unloaded: &'a ColumnSet,             // columns not loaded on the client, or in a window
}
```

Kept changes follow MVC-PRED-018. The block states are shared and the probes are per player:

```rust
#[derive(Resource)]
pub struct ChangeLog {                   // per dimension; one entry per sent change
    entries: VecDeque<LoggedChange>,     // ChangeSeq, BlockPos, old, new; or a column load/unload; or a border change
    first: ChangeSeq,
}

pub struct KeptChanges {                 // per player, in Predictor
    entries: VecDeque<KeptChange>,       // ChangeSeq, probe before, probe after, first applied client tick
}
```

- The emitter appends to `ChangeLog` and pushes a `KeptChange` for each player it sends the change
  to. The states are stored once, so there is no clone per recipient (bevy-ecs D5) and no shared
  pointer.
- A player drops a kept change `replay_depth` client ticks after the first client tick in which
  it is applied. It can drop it earlier when no recorded box, grown by the near zone of
  MVC-MODE-003, can touch the position (MVC-PRED-018).
- `ChangeLog` trims its front up to the smallest sequence any player still keeps. One writer,
  `trim_change_log`, in `TickSet::Project`.
- Chunk loads and unloads and border changes are kept changes too (MVC-PRED-017).

The overrides of a tick come from the player's kept changes in the read zone, classified by
MVC-LAT-004 for that tick: not applied gives the old state, window gives both runs (MVC-LAT-006),
applied gives none. For a replayed tick (MVC-PRED-017), the same classification runs for the
replayed tick's index, which undoes the kept changes from the newest to the oldest. Every block
change still in a record is kept, so a replay never needs the current world beyond what the kept
changes undo. `ClientView` reports a brick that holds an override as `solid` and not `full`, so
the brick fast path never hides an override.

Memory: a kept change is ~24 B per player, plus ~24 B once in `ChangeLog`. Retention is about
`replay_depth` client ticks plus the latency of the player.

## 6. Server entity tick

### 6.1 Shards

PHY-RCH-004: only entity colliders couple two physics ticks. PHY-CMP-008 lets every entity read
collider boxes from the start of the tick. Together they make each entity its own `EntityShard`
for the physics part of the tick: reads are the frozen world plus a frozen snapshot, writes are the
entity itself plus deferred effects (PHY-RCH-002).

```rust
#[derive(Resource)]
pub struct ColliderSnapshot {            // PHY-SHP-009 entities only: boats, shulkers, happy ghasts
    cells: FxHashMap<SectionPos, SmallVec<[SnapshotCollider; 2]>>,
    border: Option<WorldBorder>,
}
```

Built once per tick by `snapshot_colliders`. Collidable entities are rare, so a full rebuild
(~2k inserts, ~60 µs) is cheaper than incremental maintenance. It implements `EntityColliders`.

Exact-parity mode (ARCH Decision 20): one shard per dimension, `LiveColliders` in place of the
snapshot, and the order of PHY-ORD-010/011. Same kernel, same systems; the driver swaps the
`EntityColliders` implementation and the iteration:

- Client ticks are applied before any entity ticks (PHY-ORD-010 step 1). `predict` already runs
  before `EntityMotion`.
- One pass over the tick list in `TickListSeq` order (step 4). Players are in the list; their
  entry runs the server part of the player, not a travel step (PHY-SRV-001).
- An entity added during the pass waits for the next tick; a removed entity stops ticking at
  removal; only entities in entity-ticking chunks and players tick; frozen entities do not
  (PHY-ORD-011).
- A passenger ticks right after its vehicle, depth-first in passenger-list order, not in its own
  list slot. A passenger whose ride ends ticks in its own slot (PHY-ORD-011).
- The server part of each player (PHY-SRV-004) runs after the dimension pass, in connection order
  (step 6).
- Dimensions run in creation order, overworld first (step 2). This holds already, because the host
  updates dimension worlds one after another.

The sharded mode sorts its effect messages by `TickListSeq` too (§7.1), so both modes share one
stable key. A differential test between the two modes skips entities with a moving collider in
their read zone (PHY-DEV-002).

When the push between entities lands (PHY §12), push edges couple entities; the driver then forms
clusters by union-find over read zones (PHY-RCH-001), each cluster stepped sequentially in stable
order, clusters in parallel. The systems below already take the `EntityColliders` seam that this
needs.

### 6.2 Rest and wake

```rust
pub enum Rest {
    Awake,
    Resting { since: GameTick, zone: ZoneStamp },   // ZoneStamp: max brick revision over the read zone
}
```

- Enter: after a full tick with `S' == S` bit for bit (PHY-SKIP-001), no `action` brick in the
  read zone (PHY-SKIP-004), zero stuck multiplier, zero input. Items enter on the 4-tick cycle of
  PHY-SKIP-003 and, while skipped, take `v.y` from the phase `(t + n) mod 4`, not from the last
  value (PHY-CMP-007 keeps that cycle exact).
- Read zone at rest (PHY-RCH-005): the box grown by 1 on `x` and `z`, by `1 + h` up and by
  `1 + h + |d.y|` down, with `d.y` from the row: the walker rest velocity, `-0.16` for an item,
  `-0.04` for primed TNT, `-0.03` for an experience orb. `ZoneStamp` covers the sections this zone
  touches, computed once on entering rest.
- Wake (PHY-SKIP-002): any brick revision in those sections above `zone`, a loaded/unloaded
  section there (lookup changes), a snapshot collider in the zone, a border change (one global
  stamp), `Ref<Velocity>`/`Ref<Transform>`/attribute/effect projection changed, non-zero
  `LivingInput`.
- Extra wakes are allowed (PHY-SKIP-005), so the test runs per section revision, not per block,
  and a revision bump anywhere in a zone section wakes the entity. A wake is a full tick from the
  correct rest state, so an item's `v.y` must be the phase value before the wake tick runs.
- The tick count advances every tick, skipped or not (PHY-SKIP-005). It lives outside
  `MotionState` (the entity age), and the skip path reads it and never writes it, so the item phase
  stays correct.
- Cost of a resting entity: 1–4 section revision reads and one snapshot hash miss, ~15 ns.

### 6.3 In-air batch

An awake entity whose swept read zone has no `solid`, `action` or `coarse` brick and no snapshot
collider moves with `r = d` and false flags. Its tick is integration only. These entities go
into a per-task batch:

- `Policy::Exact` rows and the default for every row: f64 lanes (4 on AVX2, 2 on NEON). IEEE
  add and multiply are bit-identical to the scalar path, and the f32 constants of PHY-NUM-003 are
  exact in f32 lanes, so this needs no deviation.
- Optional, behind PHY-DEV-001: f32 lanes with positions relative to the section origin
  (PHY-CMP-004 allows a local origin within 16 blocks). This doubles the width for items and
  projectiles. Turn it on only if a benchmark shows the f64 batch is the bottleneck.

The batch never decides a flag, a step-up or a rest phase (PHY-CMP-007).

## 7. Schedule

All systems live in the dimension sub-app (`add_dimension_plugin`). Existing `TickSet` phases in
`FixedUpdate`:

```rust
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicsSet {
    Snapshot,       // TickSet::Simulate
    Predict,        // TickSet::Simulate, after Snapshot
    EntityMotion,   // TickSet::Simulate, after Predict and after the AI set
    Effects,        // TickSet::Simulate, after EntityMotion: sort and write PhysicsEffect messages
    ProjectBricks,  // TickSet::Project
    ProjectPlayers, // TickSet::Project
}
```

```text
connection task:  frame -> L0 -> L1 -> ClientTick ----bridge----+
                                                                v
Input:        drain_client_ticks (per player queue)
Simulate:     snapshot_colliders
              -> predict            (par_iter over players)
              -> [entity AI]        (outside this note; writes LivingInput)
              -> step_entities      (par_iter over awake entities; batch in-air)
              -> write_effects      (merge per-thread buffers, sort, MessageWriter)
WorldWrite:   block actions from PhysicsEffect land in the sections
Project:      project_section_bricks (BlockPlaced)  ||  project_accepted_state  ||  section index (SectionCrossed)
              then trim_change_log (after every player dropped its expired kept changes)
Output:       setbacks (player_position, MVC-RSP-001), entity sync (PHY-CMP-002)
```

### 7.1 Parallelism

- `predict`: `Query<(&mut Prediction, &mut ClientTickQueue), With<Player>>` with `par_iter_mut`,
  `BatchingStrategy::new().min_batch_size(32)` (~2 µs per player → ~60 µs per batch). MVC-LAY-008:
  each player reads only the world and its own state.
- `step_entities`: `par_iter_mut` over `Without<Player>`, `min_batch_size(256)` (~1 µs awake,
  ~15 ns resting).
- `predict` and `step_entities` are ordered, not concurrent. Each already saturates the compute
  pool, so overlapping them gains only tail balance, while the near-entity test of `predict`
  reads the `Transform` that `step_entities` writes. Ordering removes that conflict without a
  second snapshot.
- Concurrent with physics: other domains whose sets do not touch these components (light,
  block-tick kernels on other data), as Bevy decides.
- Inside `par_iter`: no `Commands`, no structural change. Effects and `SectionCrossed` go into
  `Local<Parallel<Vec<_>>>`, drained and sorted by `(TickListSeq, sequence)` in `write_effects`.
  `TickListSeq` is the tick-list order of PHY-ORD-010, so the sharded mode delivers effects in the
  order the exact-parity mode produces them, and never in thread-finish order.
- Exact-parity mode replaces `step_entities` with `step_entities_in_order`: one sequential pass in
  `TickListSeq` order with passengers after their vehicle (PHY-ORD-011), then the server part of
  the players (PHY-ORD-010 step 6). It is selected by a run condition on the mode resource, so the
  two systems never both run.

### 7.2 Ambiguity and determinism

- `PhysicsPlugin` and `MovementPlugin` join `MinecraftServerPlugins`, so the existing
  `crates/mcrs_minecraft_server/tests/it/schedule_ambiguity.rs` covers them with ambiguity
  detection at error.
- Each unit of work reads frozen inputs and writes only itself, so the result does not depend on
  the thread count. Effects are sorted by a stable key. Floats: no `mul_add`, no libm
  trigonometry (the sine table of PHY-NUM-005), no fast-math, the same scalar or lane code on
  every thread count.

## 8. Performance budget

### 8.1 Load

| Work | Count per tick | Per second |
|------|----------------|------------|
| L0+L1 packets | ~50k (10k players × ~5) | ~1M, on IO threads |
| L2 client ticks | 10k | 200k |
| Server entity ticks | 100k | 2M |

### 8.2 Budget at 16 cores

Target: physics and checks ≤ 5 ms wall of the 50 ms tick, at 16 physical cores (80 core-ms).

| Item | Cost each | Count | core-ms |
|------|-----------|-------|---------|
| `predict`, fast path | 2.0 µs | 10k | 20 |
| `step_entities`, awake | 1.0 µs | 40k (assumes 60% at rest) | 40 |
| `step_entities`, resting | 15 ns | 60k | 1 |
| `snapshot_colliders` | 30 ns | 2k | 0.06 |
| `project_section_bricks` | 200 ns | 10k block changes | 2 |
| `write_effects` sort | — | ~5k effects | 0.5 serial |
| **Total** | | | **~64 core-ms ≈ 4 ms wall at 80% efficiency** |

Serial part (snapshot, effect sort, bridge drain) must stay under 0.5 ms (Amdahl).

### 8.3 Kernel targets (median, one core, warm cache)

| Benchmark | Target |
|-----------|--------|
| `resolve`, inert air | ≤ 60 ns |
| `resolve`, walk on flat ground | ≤ 300 ns |
| `resolve`, step-up onto a slab | ≤ 1.2 µs |
| `tick_entity`, walker on ground | ≤ 600 ns |
| `tick_entity`, item in air, scalar / f64 batch per entity | ≤ 80 ns / ≤ 30 ns |
| `tick_player`, one candidate, walking | ≤ 1.5 µs |
| `predict`, legit tick, fast path | ≤ 2.5 µs |
| L0 `check` per packet / L1 per packet | ≤ 30 ns / ≤ 100 ns |
| brick recompute per block change / per new section | ≤ 300 ns / ≤ 10 µs |

### 8.4 Levers

| Lever | Expected gain | Spec dependency | Deviation entry needed |
|-------|---------------|-----------------|------------------------|
| Skip resting entities | 2–3× on total entity cost (60–70% at rest) | PHY-SKIP-001..005, PHY-RCH-005 | no: identical by PHY-SKIP-001 |
| Per-entity parallelism | ~12× wall at 16 cores | PHY-RCH-004, PHY-CMP-008 | existing PHY-DEV-002 |
| Brick masks + shape classes | 3–5× fewer block reads per query; 0 reads in air | none (implementation) | no |
| L2 fast path | ~3× on L2 (from ~3–4 steps to ~1.05) | MVC-PRED-013..016 | no |
| L0/L1 in the connection task | ~1M ECS events/s off the serial schedule; ~1 core of serial time | MVC-LAY-002/003 | no |
| In-air batch, f64 lanes | 2–4× on in-air integration | none | no |
| In-air batch, f32 local frame | further 2× on that subset | PHY-CMP-006 | existing PHY-DEV-001 |
| No archetype churn (values, not markers) | removes ms-scale spikes | none | no |
| Section-sorted traversal of awake entities | unknown; benchmark-gated | none | no |
| Reduced tick rate for far entities | rejected | would change trajectories (PHY-ORD-005 is per tick) | would need one; not proposed. The reference ticking range already removes far entities |

Collision dominates the awake cost, so the f32 path is the last lever to pull, not the first.

## 9. Spec changes

The own player stays exact in every item. All seven requests are resolved; the table keeps the
test this design attaches to each.

| # | Request | Resolved by | Where this note uses it | Test |
|---|---------|-------------|-------------------------|------|
| S1 | Reference order of entity ticks, for exact-parity mode | PHY-ORD-010, PHY-ORD-011 | `TickListSeq` (§4.1); exact-parity pass (§6.1, §7.1); effect sort key | armor stand next to a moving boat, 5 ticks, bit for bit in exact-parity mode; passenger ticks right after its vehicle |
| S2 | World view of a replayed tick; retention of block changes | MVC-PRED-017, MVC-PRED-018 | `ChangeLog` + per-player `KeptChanges` (§5.4); replay undo (§5.3) | break the block under a walking player at 500 ms delay, withhold one probe until a replay runs; expect no setback |
| S3 | "Pushable" as data | PHY-ORD-009 `pushable` row, PHY-ORD-012 default | derived on read in the near-entity test (§4.1, §5.3) | unit test: the flag for each exception type and the state conditions of PHY-ORD-012 |
| S4 | Coarse blocks as data | PHY-ORD-013 | `COARSE` state flag and `coarse` brick mask (§3.3) | every state of the listed blocks, and every state with a fluid, has `COARSE`; no other state has it except `OFF_GRID` shapes |
| S5 | Extra wakes allowed | PHY-SKIP-005 | per-section-revision wake test (§6.2) | property: a random wake schedule gives the same states as never skipping, including the item phase |
| S6 | Read zone at rest | PHY-RCH-005 | `ZoneStamp` sections (§6.2) | unit test: zone of a resting walker, item, TNT and orb |
| S7 | Attribute and effect ids at a proxy | MVC-LAY-009 | `ProtocolIds` from `registries.json`; CHK-024 off without a report (§5.1) | proxy test: L1 with the version's report decodes E20 captures; without the report, CHK-024 does not run |

No new intentional deviation is proposed. The two the design uses already exist: PHY-DEV-001
(optional f32 batch) and PHY-DEV-002 (collider snapshot).

## 10. Verification

### 10.1 Kernel tests without Bevy (`mcrs_minecraft_physics`)

Table-driven, one test per behaviour with a value the spec states:

- PHY-GRV-007: horizontal factor `0.546000063419342` on friction `0.6F`.
- PHY-SKIP-003: walker rest `v.y = -0.0784000015258789`; item cycle `0, -0.04, -0.08, -0.12`.
- PHY-SHP-005: bottom slab has `y` lines 0, 0.5, 1 and an open upper cell; a `y` 0..0.75 box stops a
  penetrating mover at 0.25 (the `Cell` fast path must refuse it).
- PHY-COL-005: a result within 1e-7 of the wrong sign is kept.
- PHY-STP-005: the lowest candidate that increases horizontal distance wins.
- PHY-NUM-005: sine table entries 0, 16384, 32768, 49152.
- A `CollisionWorld` test double in the crate: a `HashMap<BlockPos, BlockStateId>` world.

### 10.2 Property tests

Over random worlds built from corpus shapes and random displacements (proptest):

- PHY-INV-001..013 on `resolve` and `move_body`.
- Brick masks: a query with masks gives the same `Resolved` as a query that reads every cell.
- In-air batch, f64 lanes: bit-identical to scalar. f32 local frame: within PHY-CMP-006.
- Rest: a run with skipping equals a run without, bit for bit, over random wake events
  (PHY-SKIP-001, PHY-SKIP-005); the item tick count advances on skipped ticks.
- Replay: a replayed tick sees the same block states as the original run did, for random
  sequences of block changes and acknowledgements (MVC-PRED-017); no kept change is dropped
  while a record can still need it (MVC-PRED-018).
- MVC-INV-003, MVC-INV-005, MVC-INV-006, MVC-INV-007 over random packet streams through L0–L2.
- MVC-INV-001: a simulated legit client (the kernel's `tick_player` as the client) behind random
  latency and jitter never gets a setback.
- L0/L1: never panics, memory per connection bounded, on fuzzed byte streams.

### 10.3 Thread count and shard layout

- An `App` with 1, 2, 4, 8 and N compute threads runs 400 ticks of a seeded world with 5k
  entities and 200 scripted players. A hash of every `Transform`, `Velocity`, `MotionState` and
  `Prediction` is equal across thread counts (ARCH §25).
- Exact-parity mode vs sharded mode: equal for every entity without a moving collider in its read
  zone (PHY-DEV-002 test).
- Ambiguity: the existing schedule test, with both plugins added.

### 10.4 Vanilla parity

- Server entities: PHY E1–E8 and E15–E20 recorded with `tools/vanilla-oracle`; compare within
  1e-6 and flags exactly (PHY-DEV-001 test), and bit for bit in exact-parity mode with the batch
  path off.
- Own player: PHY E9–E14, E21–E23 and MVC C1–C7 as fixtures of serverbound packets from a vanilla
  client (needs the recorder of PHY U10). Two assertions: `tick_player` reproduces every reported
  position bit for bit, and L0–L2 report zero violations.

### 10.5 Benchmarks (criterion)

- `crates/mcrs_minecraft_physics/benches/`: every row of §8.3, with the target in the bench name
  and a CI check that fails at 1.5× the target.
- `crates/mcrs_minecraft_engine_physics/benches/tick_100k.rs`: one dimension, 100k entities (60%
  resting), 10k synthetic players sending legit client ticks, 16 threads. Target ≤ 5 ms per
  `FixedUpdate` run for the physics and movement sets.

## 11. Open questions

1. Where L1 runs in the server: the plan puts it in the connection task. It needs the network
   crate to expose body bytes and a per-connection hook on both directions. If that is not wanted,
   L1 runs in a system over a per-player packet queue, at the cost of the observer traffic.
2. Dimension sub-apps update one after another in the host tick, which is what exact-parity mode
   needs (PHY-ORD-010 step 2). The sharded mode could run dimension worlds in parallel, but whether
   that is allowed is outside this note.
3. U4/U6 in PHY: the corpus gives f32-rounded boxes, not grid lines. Until a corpus component
   gives exact f64 values and grid lines, `OFF_GRID` shapes force coarse mode near them and the
   kernel cannot be bit-exact for them.
4. The client (`mcrs_minecraft_client_world/src/local_player.rs`) has a placeholder fly model.
   Moving it to `tick_player` gives client and server one implementation of PHY-ORD-001; the
   client team decides when.
5. Push between entities (PHY §12) will couple entities in the physics tick. The cluster path of
   §6.1 is designed but not needed until then.
6. AI order: the AI set must run before `EntityMotion` and write only `LivingInput`. Its own reach
   is outside PHY; its spec decides whether AI can share the per-entity parallelism.
