//! The rows pass against a pass over plain Bevy components on the same scene, the same kernel,
//! the same blocks and the same task pool: the steady tick, the skip of resting items, two kinds
//! of block access, the list of the entities that moved, item spawns, and one tick at 1M.
//!
//! Both sides run the scene of `entity_pass` with no item lifetime, so that no item leaves the
//! scene while criterion samples.

use std::hint::black_box;
use std::time::Duration;
use std::time::Instant;

use bevy_ecs::batching::BatchingStrategy;
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryFilter;
use bevy_ecs::system::ParallelCommands;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use bevy_tasks::TaskPoolBuilder;
use bevy_tasks::available_parallelism;
use criterion::BatchSize;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use glam::DVec3;
use glam::IVec3;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_entity_key::TILE_WIDTH;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::BodyGrid;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_entity_rows::TileBodies;
use mcrs_minecraft_entity_rows::tiles_per_task;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BlockAction;
use mcrs_minecraft_physics::BodyView;
use mcrs_minecraft_physics::CollisionSection;
use mcrs_minecraft_physics::CollisionWorld;
use mcrs_minecraft_physics::EntityCollider;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::GridFlags;
use mcrs_minecraft_physics::GridRow;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_physics::MovePath;
use mcrs_minecraft_physics::PushRules;
use mcrs_minecraft_physics::PushTargets;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_physics::Scratch;
use mcrs_minecraft_physics::SectionLookup;
use mcrs_minecraft_physics::SectionMasks;
use mcrs_minecraft_physics::Side;
use mcrs_minecraft_physics::TeamSlot;
use mcrs_minecraft_physics::View;
use mcrs_minecraft_physics::WorldBorder;
use mcrs_minecraft_physics::block_at;
use mcrs_minecraft_physics::floor_block;
use mcrs_minecraft_physics::push_entity;
use mcrs_minecraft_physics::same_bits;
use mcrs_minecraft_physics::tick_entity;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::LocalPos;
use mcrs_voxel::SectionPos;
use rustc_hash::FxHashMap;

const SEED: u64 = 11;
const WARM_UP_TICKS: u32 = 100;
const REST_TICKS: u32 = 400;
const SPAWNS: u32 = 10_000;
const SERVER: Side = Side::Server { authority: true };
const BUCKET: i32 = 1;
const BUCKETS: usize = (TILE_WIDTH.get() / BUCKET.unsigned_abs()) as usize;
/// A batch cache that grows past this many sections starts again, so its size has a bound.
const CACHE_SECTIONS: usize = 512;

fn pool() -> &'static TaskPool {
    ComputeTaskPool::get_or_init(|| {
        TaskPoolBuilder::new()
            .num_threads(available_parallelism())
            .build()
    })
}

fn batching() -> BatchingStrategy {
    BatchingStrategy::new().batches_per_thread(4)
}

/// The scene of `entity_pass`, rebuilt with no item lifetime.
fn scene(half: i32, walkers: u32, items: u32) -> TestScene<()> {
    let mut scene = TestScene::new(half, walkers, items, SEED);
    let mut rows = RowStore::default();
    for (_, tile) in scene.rows.tiles() {
        for row in tile.rows() {
            let mut motion = *row.motion;
            motion.lifetime = None;
            rows.spawn(row.id, *row.state, motion);
        }
    }
    scene.rows = rows;
    scene
}

struct RowsSide {
    scene: TestScene<()>,
    writes: EntityWrites,
    tiles_per_task: usize,
}

impl RowsSide {
    fn new(scene: TestScene<()>) -> Self {
        let tiles_per_task = tiles_per_task(pool(), scene.rows.tile_count());
        Self {
            scene,
            writes: EntityWrites::default(),
            tiles_per_task,
        }
    }

    fn step(&mut self) {
        self.scene
            .step(pool(), self.tiles_per_task, &mut self.writes);
    }

    fn resting(&self) -> usize {
        let mut count = 0;
        for (_, tile) in self.scene.rows.tiles() {
            count += tile
                .states()
                .iter()
                .filter(|state| matches!(state.rest, Rest::Resting(_)))
                .count();
        }
        count
    }
}

#[derive(Component)]
struct Kinetic(EntityState);

#[derive(Component)]
struct Motion(MotionRow);

/// A living entity: a body in the grid. It never changes after the spawn.
#[derive(Component)]
struct Mob;

#[derive(Component)]
#[component(storage = "SparseSet")]
struct Resting;

/// The blocks and the tables of the scene; its rows are empty.
#[derive(Resource)]
struct Blocks(TestScene<()>);

#[derive(Resource, Default)]
struct Bodies {
    grid: BodyGrid,
    reach: f32,
}

#[derive(Resource, Default)]
struct Clock(u64);

#[derive(Resource, Default)]
struct Movers(Vec<Entity>);

#[derive(Clone, Copy)]
enum Blocking {
    Direct,
    Cached,
}

#[derive(Resource)]
struct SkipBy(Skip);

#[derive(Clone, Copy)]
enum Skip {
    RestValue,
    RestMarker,
}

struct EcsSide {
    world: World,
    tick: Schedule,
}

impl EcsSide {
    fn new(mut scene: TestScene<()>, blocking: Blocking, skip: Skip) -> Self {
        let mut world = World::new();
        let mut mobs = Vec::new();
        let mut others = Vec::new();
        for (_, tile) in scene.rows.tiles() {
            for row in tile.rows() {
                let entity = (Kinetic(*row.state), Motion(*row.motion));
                if row.state.living.is_some() {
                    mobs.push((entity.0, entity.1, Mob));
                } else {
                    others.push(entity);
                }
            }
        }
        world.spawn_batch(mobs);
        world.spawn_batch(others);
        scene.rows = RowStore::default();
        world.insert_resource(Blocks(scene));
        world.init_resource::<Bodies>();
        world.init_resource::<Clock>();
        world.init_resource::<Movers>();
        world.insert_resource(SkipBy(skip));
        let mut tick = Schedule::default();
        let pass = match (blocking, skip) {
            (Blocking::Direct, Skip::RestValue) => pass_direct::<()>.into_configs(),
            (Blocking::Cached, _) => pass_cached.into_configs(),
            (Blocking::Direct, Skip::RestMarker) => pass_direct::<Without<Resting>>.into_configs(),
        };
        tick.add_systems((build_grid, pass, advance).chain());
        Self { world, tick }
    }

    fn step(&mut self) {
        self.tick.run(&mut self.world);
    }

    fn resting(&mut self) -> usize {
        let mut query = self.world.query::<&Kinetic>();
        query
            .iter(&self.world)
            .filter(|kinetic| matches!(kinetic.0.rest, Rest::Resting(_)))
            .count()
    }
}

fn advance(mut clock: ResMut<Clock>) {
    clock.0 = clock.0.wrapping_add(1);
}

#[derive(Default)]
struct GridBuild {
    index: FxHashMap<TileKey, usize>,
    tiles: Vec<TileBuild>,
}

struct TileBuild {
    tile: TileKey,
    bodies: Vec<GridRow>,
    back: TileBodies,
}

/// Bins the bodies of the mobs by tile, sorts each tile into its back buffer in parallel and swaps
/// it in. Every tile with a mob is written in every tick.
fn build_grid(
    mobs: Query<(Entity, &Kinetic, &Motion), With<Mob>>,
    mut bodies: ResMut<Bodies>,
    mut build: Local<GridBuild>,
) {
    let build = &mut *build;
    for tile in &mut build.tiles {
        tile.bodies.clear();
    }
    for (entity, kinetic, motion) in &mobs {
        let Some(row) = grid_row(&kinetic.0, &motion.0, entity_id(entity)) else {
            continue;
        };
        let tile = TileKey::at(row.position);
        let next = build.tiles.len();
        let at = *build.index.entry(tile).or_insert(next);
        if at == next {
            build.tiles.push(TileBuild {
                tile,
                bodies: Vec::new(),
                back: TileBodies::default(),
            });
        }
        if let Some(entry) = build.tiles.get_mut(at) {
            entry.bodies.push(row);
        }
    }
    let pool = pool();
    let per_task = build.tiles.len().div_ceil(pool.thread_num() * 4).max(1);
    let reaches: Vec<f32> = pool.scope(|scope| {
        for chunk in build.tiles.chunks_mut(per_task) {
            scope.spawn(async move {
                let mut reach = 0.0_f32;
                for tile in chunk {
                    sort_bodies(tile.tile, &tile.bodies, &mut tile.back);
                    for body in &tile.bodies {
                        reach = reach.max(body.half_width);
                    }
                }
                reach
            });
        }
    });
    let bodies = &mut *bodies;
    for tile in &mut build.tiles {
        bodies.grid.swap(tile.tile, &mut tile.back);
    }
    bodies.reach = reaches.into_iter().fold(bodies.reach, f32::max);
}

fn entity_id(entity: Entity) -> EntityId {
    EntityId(entity.to_bits())
}

fn pass_direct<F: QueryFilter>(
    mut entities: Query<(Entity, &mut Kinetic, &Motion), F>,
    blocks: Res<Blocks>,
    bodies: Res<Bodies>,
    clock: Res<Clock>,
    skip: Res<SkipBy>,
    commands: ParallelCommands,
) {
    let skip = skip.0;
    let world = blocks.0.block_world();
    let grid = GridView {
        grid: &bodies.grid,
        reach: bodies.reach,
    };
    let view = View::new(&world, &grid, &blocks.0.tables);
    let push = blocks.0.push;
    let tick = clock.0;
    entities
        .par_iter_mut()
        .batching_strategy(batching())
        .for_each_init(
            || (Scratch::new(), Vec::<BlockAction>::new()),
            |(scratch, actions), (entity, mut kinetic, motion)| {
                let state = &mut kinetic.bypass_change_detection().0;
                let stepped = step(view, push, tick, entity, &motion.0, state, scratch, actions);
                finish(entity, &mut kinetic, stepped, skip, &commands);
            },
        );
}

fn pass_cached(
    mut entities: Query<(Entity, &mut Kinetic, &Motion)>,
    blocks: Res<Blocks>,
    bodies: Res<Bodies>,
    clock: Res<Clock>,
    commands: ParallelCommands,
) {
    let world = blocks.0.block_world();
    let world = &world;
    let grid = GridView {
        grid: &bodies.grid,
        reach: bodies.reach,
    };
    let tables = &blocks.0.tables;
    let push = blocks.0.push;
    let tick = clock.0;
    entities
        .par_iter_mut()
        .batching_strategy(batching())
        .for_each_init(
            || {
                (
                    Scratch::new(),
                    Vec::<BlockAction>::new(),
                    SectionCache::new(world),
                )
            },
            |(scratch, actions, cache), (entity, mut kinetic, motion)| {
                let state = &mut kinetic.bypass_change_detection().0;
                cache.cover(state.body.position);
                let view = View::new(&*cache, &grid, tables);
                let stepped = step(view, push, tick, entity, &motion.0, state, scratch, actions);
                finish(entity, &mut kinetic, stepped, Skip::RestValue, &commands);
            },
        );
}

/// Marks the visible change, despawns an expired entity and, with [`Skip::RestMarker`], gives an
/// item that entered rest the marker.
fn finish(
    entity: Entity,
    kinetic: &mut Mut<Kinetic>,
    stepped: Stepped,
    skip: Skip,
    commands: &ParallelCommands,
) {
    if stepped.changed {
        kinetic.set_changed();
    }
    if stepped.expired {
        commands.command_scope(|mut commands| commands.entity(entity).despawn());
    } else if stepped.rests && matches!(skip, Skip::RestMarker) {
        commands.command_scope(|mut commands| {
            commands.entity(entity).insert(Resting);
        });
    }
}

#[derive(Clone, Copy, Default)]
struct Stepped {
    /// The body or the rotations changed: what a viewer sees.
    changed: bool,
    expired: bool,
    /// The entity is an item that entered rest in this tick.
    rests: bool,
}

/// The step of one row of the rows pass with no intents, no crowd damage and no tile moves.
#[expect(
    clippy::too_many_arguments,
    reason = "the inputs of one row step of the rows pass"
)]
fn step<W: CollisionWorld, B: BodyView>(
    view: View<'_, W, B>,
    push: PushRules,
    tick: u64,
    entity: Entity,
    motion: &MotionRow,
    state: &mut EntityState,
    scratch: &mut Scratch,
    actions: &mut Vec<BlockAction>,
) -> Stepped {
    let looked = state.living.map(rotations);
    let start = grid_row(state, motion, entity_id(entity));
    let from = state.body.position;
    let rested = matches!(state.rest, Rest::Resting(_));
    let mut stepped = Stepped::default();
    if loaded(view.world, from) {
        let before = state.body;
        if let Some(own) = start {
            push_row(view.bodies, push, tick, &own, state, scratch);
        }
        tick_entity(view, SERVER, motion, state, false, actions, scratch);
        actions.clear();
        stepped.changed = !same_bits(&before, &state.body) || state.living.map(rotations) != looked;
        state.age = state.age.wrapping_add(1);
        let expires = motion
            .lifetime
            .is_some_and(|lifetime| state.age >= lifetime);
        let dead = state.living.as_mut().is_some_and(count_down);
        if state.living.is_some_and(|living| !living.is_alive()) {
            state.rest = Rest::Awake;
        }
        stepped.expired = dead || expires;
    }
    if start.is_some() {
        state.last_path = MovePath {
            start: from,
            end: state.body.position,
        };
    }
    stepped.rests = start.is_none() && !rested && matches!(state.rest, Rest::Resting(_));
    stepped
}

fn push_row<B: BodyView>(
    bodies: &B,
    rules: PushRules,
    tick: u64,
    own: &GridRow,
    state: &mut EntityState,
    scratch: &mut Scratch,
) {
    let stream = KeyedStream::new(&state.key, tick);
    let pushed = push_entity(
        bodies,
        own,
        &rules,
        &stream,
        &mut state.body.velocity,
        scratch,
    );
    let crowded = rules.max_entity_cramming > 0
        && i64::from(pushed.candidates) > i64::from(rules.max_entity_cramming) - 1;
    if pushed.pushed || crowded {
        state.rest = Rest::Awake;
    }
}

fn count_down(living: &mut Living) -> bool {
    living.hurt_time = living.hurt_time.saturating_sub(1);
    living.damage_cooldown = living.damage_cooldown.saturating_sub(1);
    let Some(death_time) = living.death_time.as_mut() else {
        return false;
    };
    *death_time = death_time.saturating_add(1);
    *death_time >= 20
}

fn rotations(living: Living) -> [u32; 3] {
    [
        living.orientation.yaw,
        living.orientation.pitch,
        living.orientation.head_yaw,
    ]
    .map(f32::to_bits)
}

fn loaded<W: CollisionWorld>(world: &W, position: DVec3) -> bool {
    !matches!(
        world.section(SectionPos::from(block_at(position))),
        SectionLookup::Unloaded
    )
}

fn grid_row(state: &EntityState, motion: &MotionRow, id: EntityId) -> Option<GridRow> {
    let living = state.living?;
    let dimensions = motion.dimensions.scaled(living.attributes.scale as f32);
    let position = state.body.position;
    let previous = if state.last_path.end == position {
        state.last_path.start
    } else {
        position
    };
    Some(GridRow {
        position,
        previous,
        half_width: dimensions.width / 2.0,
        height: dimensions.height,
        key: state.key,
        id,
        root_vehicle: id,
        team: TeamSlot::default(),
        flags: if living.is_alive() {
            GridFlags::PUSHABLE
        } else {
            GridFlags::empty()
        },
        push_targets: PushTargets::Each,
    })
}

fn sort_bodies(tile: TileKey, bodies: &[GridRow], out: &mut TileBodies) {
    let mut order: Vec<(usize, EntityKey, usize)> = bodies
        .iter()
        .enumerate()
        .map(|(index, row)| (bucket(tile, row.position), row.key, index))
        .collect();
    order.sort_unstable();
    out.rows.clear();
    out.rows
        .extend(order.iter().filter_map(|&(.., index)| bodies.get(index)));
    out.starts.clear();
    let mut at = 0;
    for bucket in 0..=BUCKETS * BUCKETS {
        out.starts.push(at);
        while order.get(at).is_some_and(|&(of, ..)| of == bucket) {
            at += 1;
        }
    }
}

fn bucket(tile: TileKey, position: DVec3) -> usize {
    let [x, z] = bucket_xz(tile, position);
    z * BUCKETS + x
}

fn bucket_xz(tile: TileKey, position: DVec3) -> [usize; 2] {
    let width = TILE_WIDTH.get().cast_signed();
    let local = |value: f64, origin: i32| {
        let block = floor_block(value).saturating_sub(origin.saturating_mul(width));
        usize::try_from(block / BUCKET).map_or(0, |bucket| bucket.min(BUCKETS - 1))
    };
    [local(position.x, tile.x), local(position.z, tile.z)]
}

/// The neighbour view of the rows pass over the grid that [`build_grid`] writes.
#[derive(Clone, Copy)]
struct GridView<'a> {
    grid: &'a BodyGrid,
    reach: f32,
}

impl BodyView for GridView<'_> {
    fn colliders(&self, _query: Aabb, _mover: EntityKey, _out: &mut Vec<EntityCollider>) {}

    fn neighbours(&self, of: &GridRow, out: &mut Vec<GridRow>) {
        let reach = f64::from(of.half_width) + f64::from(self.reach);
        let low = of.position - DVec3::new(reach, 0.0, reach);
        let high = of.position + DVec3::new(reach, 0.0, reach);
        let bounds = of.bounds();
        let (first, last) = (TileKey::at(low), TileKey::at(high));
        for tile_z in first.z..=last.z {
            for tile_x in first.x..=last.x {
                let tile = TileKey::new(tile_x, 0, tile_z);
                let bodies = self.grid.front(tile);
                let [x0, z0] = bucket_xz(tile, low);
                let [x1, z1] = bucket_xz(tile, high);
                for z in z0..=z1 {
                    out.extend(
                        bodies
                            .cells(z * BUCKETS + x0, z * BUCKETS + x1)
                            .iter()
                            .filter(|row| row.bounds().intersects(&bounds)),
                    );
                }
            }
        }
    }
}

/// The sections that the entities of one `par_iter` batch read, looked up once per batch. Before
/// each entity the batch adds the sections around it; the kernel then reads the cache, and a
/// section out of it from the base world.
struct SectionCache<'a, W: CollisionWorld + 'a> {
    base: &'a W,
    sections: FxHashMap<IVec3, SectionLookup<W::Section<'a>>>,
}

enum CachedSection<'s, S> {
    Cache(&'s S),
    Base(S),
}

impl<'a, W: CollisionWorld> SectionCache<'a, W> {
    fn new(base: &'a W) -> Self {
        Self {
            base,
            sections: FxHashMap::default(),
        }
    }

    /// Adds the sections from 2 blocks around `position` on `x` and `z` and 4 blocks on `y`, as
    /// the tile window of the rows pass does for the rows of a tile.
    fn cover(&mut self, position: DVec3) {
        if self.sections.len() > CACHE_SECTIONS {
            self.sections.clear();
        }
        let low = SectionPos::from(position - DVec3::new(2.0, 4.0, 2.0)).0;
        let high = SectionPos::from(position + DVec3::new(2.0, 4.0, 2.0)).0;
        let base = self.base;
        for y in low.y..=high.y {
            for z in low.z..=high.z {
                for x in low.x..=high.x {
                    let at = IVec3::new(x, y, z);
                    self.sections
                        .entry(at)
                        .or_insert_with(|| base.section(SectionPos(at)));
                }
            }
        }
    }
}

impl<'a, W: CollisionWorld> CollisionWorld for SectionCache<'a, W> {
    type Section<'s>
        = CachedSection<'s, W::Section<'a>>
    where
        Self: 's;

    fn section(&self, at: SectionPos) -> SectionLookup<Self::Section<'_>> {
        match self.sections.get(&at.0) {
            Some(SectionLookup::Loaded(section)) => {
                SectionLookup::Loaded(CachedSection::Cache(section))
            }
            Some(SectionLookup::Outside) => SectionLookup::Outside,
            Some(SectionLookup::Unloaded) => SectionLookup::Unloaded,
            None => match self.base.section(at) {
                SectionLookup::Loaded(section) => {
                    SectionLookup::Loaded(CachedSection::Base(section))
                }
                SectionLookup::Outside => SectionLookup::Outside,
                SectionLookup::Unloaded => SectionLookup::Unloaded,
            },
        }
    }

    fn border(&self) -> Option<&WorldBorder> {
        self.base.border()
    }
}

impl<S: CollisionSection> CollisionSection for CachedSection<'_, S> {
    fn masks(&self) -> Option<&SectionMasks> {
        match self {
            Self::Cache(section) => section.masks(),
            Self::Base(section) => section.masks(),
        }
    }

    fn state(&self, at: LocalPos) -> BlockStateId {
        match self {
            Self::Cache(section) => section.state(at),
            Self::Base(section) => section.state(at),
        }
    }
}

fn collect_movers(moved: Query<Entity, Changed<Kinetic>>, mut movers: ResMut<Movers>) {
    movers.0.clear();
    movers.0.extend(&moved);
}

fn warm_up(mut step: impl FnMut(), ticks: u32) {
    for _ in 0..ticks {
        step();
    }
}

fn steady_tick(criterion: &mut Criterion) {
    let threads = pool().thread_num();
    let mut group =
        criterion.benchmark_group(format!("steady 50k walkers + 50k items, {threads} threads"));
    let mut rows = RowsSide::new(scene(16, 50_000, 50_000));
    warm_up(|| rows.step(), WARM_UP_TICKS);
    group.bench_function("rows", |bencher| bencher.iter(|| rows.step()));
    drop(rows);
    for (name, blocking) in [
        ("ecs direct blocks", Blocking::Direct),
        ("ecs batch cache", Blocking::Cached),
    ] {
        let mut ecs = EcsSide::new(scene(16, 50_000, 50_000), blocking, Skip::RestValue);
        warm_up(|| ecs.step(), WARM_UP_TICKS);
        group.bench_function(name, |bencher| bencher.iter(|| ecs.step()));
    }
    group.finish();
}

fn rest_skip(criterion: &mut Criterion) {
    let threads = pool().thread_num();
    let mut group = criterion.benchmark_group(format!("rest 100k items, {threads} threads"));
    let mut rows = RowsSide::new(scene(16, 0, 100_000));
    warm_up(|| rows.step(), REST_TICKS);
    println!("rows: {} of 100000 rest", rows.resting());
    group.bench_function("rows", |bencher| bencher.iter(|| rows.step()));
    drop(rows);
    let sides = [
        ("ecs direct blocks", Blocking::Direct, Skip::RestValue),
        ("ecs batch cache", Blocking::Cached, Skip::RestValue),
        ("ecs rest marker", Blocking::Direct, Skip::RestMarker),
    ];
    for (name, blocking, skip) in sides {
        let mut ecs = EcsSide::new(scene(16, 0, 100_000), blocking, skip);
        warm_up(|| ecs.step(), REST_TICKS);
        println!("{name}: {} of 100000 rest", ecs.resting());
        group.bench_function(name, |bencher| bencher.iter(|| ecs.step()));
        if let Skip::RestMarker = skip {
            wake(&mut group, &mut ecs.world);
        }
    }
    group.finish();
}

/// Removes the rest marker of the first 10k resting items, as a write that wakes them does, and
/// gives it back outside the measure.
fn wake(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    world: &mut World,
) {
    let mut query = world.query_filtered::<Entity, With<Resting>>();
    let resting: Vec<Entity> = query.iter(world).take(10_000).collect();
    group.bench_function("ecs wake 10k (remove marker)", |bencher| {
        bencher.iter_custom(|iterations| {
            let mut total = Duration::ZERO;
            for _ in 0..iterations {
                let start = Instant::now();
                let mut commands = world.commands();
                for &entity in &resting {
                    commands.entity(entity).remove::<Resting>();
                }
                world.flush();
                total += start.elapsed();
                let mut commands = world.commands();
                for &entity in &resting {
                    commands.entity(entity).insert(Resting);
                }
                world.flush();
            }
            total
        });
    });
}

fn movers(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("movers of the tick");
    let mut rows = RowsSide::new(scene(16, 0, 100_000));
    warm_up(|| rows.step(), REST_TICKS);
    group.bench_function("rows read, 100k resting items", |bencher| {
        bencher.iter(|| black_box(rows.scene.rows.moved().len()))
    });
    drop(rows);
    let mut collect = Schedule::default();
    collect.add_systems(collect_movers);
    let mut ecs = EcsSide::new(scene(16, 0, 100_000), Blocking::Direct, Skip::RestValue);
    warm_up(|| ecs.step(), REST_TICKS);
    group.bench_function("ecs Changed scan, 100k resting items", |bencher| {
        bencher.iter_custom(|iterations| timed_after_tick(&mut ecs, &mut collect, iterations))
    });
    let mut collect = Schedule::default();
    collect.add_systems(collect_movers);
    let mut ecs = EcsSide::new(scene(16, 50_000, 50_000), Blocking::Direct, Skip::RestValue);
    warm_up(|| ecs.step(), WARM_UP_TICKS);
    group.bench_function("ecs Changed scan, steady 100k", |bencher| {
        bencher.iter_custom(|iterations| timed_after_tick(&mut ecs, &mut collect, iterations))
    });
    group.finish();
}

fn timed_after_tick(ecs: &mut EcsSide, collect: &mut Schedule, iterations: u64) -> Duration {
    let mut total = Duration::ZERO;
    for _ in 0..iterations {
        ecs.step();
        let start = Instant::now();
        collect.run(&mut ecs.world);
        total += start.elapsed();
    }
    total
}

fn spawns(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("spawn 10k items");
    let source = scene(16, 0, SPAWNS);
    let mut items = Vec::new();
    for (_, tile) in source.rows.tiles() {
        for row in tile.rows() {
            items.push((row.id, *row.state, *row.motion));
        }
    }
    group.bench_function("rows RowStore::spawn", |bencher| {
        bencher.iter_batched(
            RowStore::<()>::default,
            |mut store| {
                for &(id, state, motion) in &items {
                    store.spawn(id, state, motion);
                }
                store
            },
            BatchSize::LargeInput,
        )
    });
    let fresh = || {
        let mut world = World::new();
        world.register_component::<Kinetic>();
        world.register_component::<Motion>();
        world
    };
    group.bench_function("ecs World::spawn_batch", |bencher| {
        bencher.iter_batched(
            fresh,
            |mut world| {
                world.spawn_batch(
                    items
                        .iter()
                        .map(|&(_, state, motion)| (Kinetic(state), Motion(motion))),
                );
                world
            },
            BatchSize::LargeInput,
        )
    });
    group.bench_function("ecs Commands::spawn", |bencher| {
        bencher.iter_batched(
            fresh,
            |mut world| {
                let mut commands = world.commands();
                for &(_, state, motion) in &items {
                    commands.spawn((Kinetic(state), Motion(motion)));
                }
                world.flush();
                world
            },
            BatchSize::LargeInput,
        )
    });
    group.finish();
}

/// One measure without criterion sampling: the median of 20 ticks at 1M entities.
fn million(_criterion: &mut Criterion) {
    const TICKS: usize = 20;
    let median = |mut times: Vec<Duration>| {
        times.sort_unstable();
        times.get(times.len() / 2).copied().unwrap_or_default()
    };
    let mut rows = RowsSide::new(scene(50, 500_000, 500_000));
    warm_up(|| rows.step(), WARM_UP_TICKS);
    let resting = rows.resting();
    let times: Vec<Duration> = (0..TICKS)
        .map(|_| {
            let start = Instant::now();
            rows.step();
            start.elapsed()
        })
        .collect();
    println!(
        "1M rows: median tick {:?}, {resting} resting",
        median(times)
    );
    drop(rows);
    for (name, blocking) in [
        ("ecs direct blocks", Blocking::Direct),
        ("ecs batch cache", Blocking::Cached),
    ] {
        let mut ecs = EcsSide::new(scene(50, 500_000, 500_000), blocking, Skip::RestValue);
        warm_up(|| ecs.step(), WARM_UP_TICKS);
        let resting = ecs.resting();
        let times: Vec<Duration> = (0..TICKS)
            .map(|_| {
                let start = Instant::now();
                ecs.step();
                start.elapsed()
            })
            .collect();
        println!(
            "1M {name}: median tick {:?}, {resting} resting",
            median(times)
        );
    }
}

criterion_group!(benches, steady_tick, rest_skip, movers, spawns, million);
criterion_main!(benches);
