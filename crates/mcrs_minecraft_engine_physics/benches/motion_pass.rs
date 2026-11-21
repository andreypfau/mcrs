use std::ops::ControlFlow;
use std::time::Duration;
use std::time::Instant;

use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::world::World;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use bevy_tasks::TaskPoolBuilder;
use bevy_tasks::available_parallelism;
use criterion::BatchSize;
use criterion::BenchmarkGroup;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::measurement::WallTime;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_engine_physics::EntityMotionPlugin;
use mcrs_minecraft_engine_physics::PhysicsTables;
use mcrs_minecraft_engine_physics::RowScene;
use mcrs_minecraft_engine_physics::RowWrites;
use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_minecraft_engine_world::palette::SectionStates;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::Dimension;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_entity_rows::tiles_per_task;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Rest;
use mcrs_minecraft_volume::Missing;
use mcrs_minecraft_volume::PalettedSection;
use mcrs_minecraft_volume::SectionBox;
use mcrs_minecraft_volume::SectionVolume;
use mcrs_minecraft_volume::VisitOptions;
use mcrs_minecraft_volume::VisitOrder;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

const SEED: u64 = 11;
const HALF: i32 = 16;
const ITEMS: u32 = 100_000;
const MILLION_HALF: i32 = 50;
const MILLION: u32 = 1_000_000;
const WARM_UP_TICKS: u32 = 400;
const SPAWNS: u32 = 10_000;

fn pool() -> &'static TaskPool {
    ComputeTaskPool::get_or_init(|| {
        TaskPoolBuilder::new()
            .num_threads(available_parallelism())
            .build()
    })
}

#[derive(Clone, Copy)]
enum Launch {
    All,
    Quarter,
}

impl Launch {
    fn includes(self, key: EntityUuid) -> bool {
        match self {
            Launch::All => true,
            Launch::Quarter => (key.as_u128() >> 2).is_multiple_of(4),
        }
    }
}

trait PassSide {
    fn step(&mut self);
    fn resting(&mut self) -> usize;
    fn launch(&mut self, launch: Launch);
}

fn start_bodies(rows: &RowStore<()>) -> Vec<Body> {
    let mut bodies: Vec<Body> =
        vec![Body::default(); rows.tiles().map(|(_, tile)| tile.ids().len()).sum()];
    for (_, tile) in rows.tiles() {
        for state in tile.states() {
            if let Some(body) = start_of(&mut bodies, state.key) {
                *body = state.body;
            }
        }
    }
    bodies
}

fn start_of(bodies: &mut [Body], key: EntityUuid) -> Option<&mut Body> {
    bodies.get_mut(usize::try_from(key.as_u128()).ok()?)
}

fn relaunch(state: &mut EntityState, starts: &[Body]) {
    let Some(start) = usize::try_from(state.key.as_u128())
        .ok()
        .and_then(|at| starts.get(at))
    else {
        return;
    };
    state.body = *start;
    state.rest = Rest::Awake;
}

struct RowsSide {
    scene: RowScene<()>,
    writes: RowWrites,
    tiles_per_task: usize,
    starts: Vec<Body>,
    placed: Vec<Placed>,
}

impl RowsSide {
    fn new(half: i32, items: u32) -> Self {
        let mut scene = RowScene::new(half, 0, items, SEED);
        let mut rows = RowStore::default();
        for (_, tile) in scene.rows.tiles() {
            for row in tile.rows() {
                let mut motion = *row.motion;
                motion.lifetime = None;
                rows.spawn(row.id, *row.state, motion);
            }
        }
        scene.rows = rows;
        Self {
            tiles_per_task: tiles_per_task(pool(), scene.rows.tile_count()),
            starts: start_bodies(&scene.rows),
            scene,
            writes: RowWrites::default(),
            placed: Vec::new(),
        }
    }
}

impl PassSide for RowsSide {
    fn step(&mut self) {
        self.scene
            .step(pool(), self.tiles_per_task, &mut self.writes);
    }

    fn resting(&mut self) -> usize {
        let rows = &self.scene.rows;
        let states = rows.tiles().flat_map(|(_, tile)| tile.states());
        states
            .filter(|state| matches!(state.rest, Rest::Resting(_)))
            .count()
    }

    fn launch(&mut self, launch: Launch) {
        self.placed.clear();
        for (tile, rows) in self.scene.rows.tiles() {
            let ids = rows.ids().iter();
            self.placed.extend(ids.map(|&id| Placed { tile, id }));
        }
        for &at in &self.placed {
            let included =
                (self.scene.rows.state_of(at)).is_some_and(|state| launch.includes(state.key));
            if included && let Some(state) = self.scene.rows.state_mut(at) {
                relaunch(state, &self.starts);
            }
        }
    }
}

struct ComponentsSide {
    app: App,
    starts: Vec<Body>,
}

impl ComponentsSide {
    fn new(half: i32, items: u32) -> Self {
        let scene = RowScene::<()>::new(half, 0, items, SEED);
        let starts = start_bodies(&scene.rows);
        let mut app = App::new();
        app.configure_sets(
            FixedUpdate,
            (
                TickSet::DomainInput,
                TickSet::ClientInput,
                TickSet::Simulate,
                TickSet::WorldWrite,
                TickSet::Project,
                TickSet::Output,
            )
                .chain(),
        )
        .insert_resource(PhysicsTables::new(scene.tables))
        .add_plugins(EntityMotionPlugin);
        let world = app.world_mut();
        let mut index = SectionIndex::new();
        let edge = half.saturating_mul(2);
        let area = SectionBox {
            min: SectionPos::new(-edge, -4, -edge),
            max: SectionPos::new(edge, 3, edge),
        };
        let options = VisitOptions {
            order: VisitOrder::SectionMajor,
            missing: Missing::Skip,
        };
        let _: ControlFlow<()> = scene.blocks.visit_sections(area, options, |at, section| {
            if let Ok(PalettedSection(container)) = section {
                let mut states = SectionStates::homogeneous(BlockStateId::AIR);
                states.0 = container.clone();
                let entity = world.spawn_empty().id();
                index.insert(at, entity);
                index.land(at, entity, ChunkBlocks::new(states), None, false);
            }
            ControlFlow::Continue(())
        });
        world.spawn((Dimension, DimensionTypeConfig::new(-64, 384), index));
        let tiles = scene.rows.tiles();
        let rows = tiles.flat_map(|(_, tile)| tile.rows());
        world.spawn_batch(rows.map(|row| {
            let mut motion = *row.motion;
            motion.lifetime = None;
            (MotionState(*row.state), MotionKind(motion))
        }));
        Self { app, starts }
    }
}

impl PassSide for ComponentsSide {
    fn step(&mut self) {
        self.app.world_mut().run_schedule(FixedUpdate);
    }

    fn resting(&mut self) -> usize {
        let world = self.app.world_mut();
        let mut states = world.query::<&MotionState>();
        states
            .iter(world)
            .filter(|state| matches!(state.0.rest, Rest::Resting(_)))
            .count()
    }

    fn launch(&mut self, launch: Launch) {
        let world = self.app.world_mut();
        let mut states = world.query::<&mut MotionState>();
        for mut state in states.iter_mut(world) {
            if launch.includes(state.0.key) {
                relaunch(&mut state.0, &self.starts);
            }
        }
    }
}

fn warm_up(side: &mut impl PassSide) {
    for _ in 0..WARM_UP_TICKS {
        side.step();
    }
}

fn at_rest(criterion: &mut Criterion) {
    let threads = pool().thread_num();
    let mut group = criterion.benchmark_group(format!("100k items at rest, {threads} threads"));
    rest_case(&mut group, "rows", RowsSide::new(HALF, ITEMS));
    rest_case(&mut group, "components", ComponentsSide::new(HALF, ITEMS));
    group.finish();
}

fn rest_case(group: &mut BenchmarkGroup<'_, WallTime>, name: &str, mut side: impl PassSide) {
    warm_up(&mut side);
    println!("{name}: {} of {ITEMS} rest", side.resting());
    group.bench_function(name, |bencher| bencher.iter(|| side.step()));
}

fn in_flight(criterion: &mut Criterion) {
    let threads = pool().thread_num();
    let mut group = criterion.benchmark_group(format!("100k items in flight, {threads} threads"));
    flight_case(&mut group, "rows", RowsSide::new(HALF, ITEMS));
    flight_case(&mut group, "components", ComponentsSide::new(HALF, ITEMS));
    group.finish();
}

fn flight_case(group: &mut BenchmarkGroup<'_, WallTime>, name: &str, mut side: impl PassSide) {
    warm_up(&mut side);
    group.bench_function(name, |bencher| {
        bencher.iter_custom(|iterations| {
            let mut total = Duration::ZERO;
            for _ in 0..iterations {
                side.launch(Launch::All);
                let start = Instant::now();
                side.step();
                total += start.elapsed();
            }
            total
        })
    });
}

fn million(criterion: &mut Criterion) {
    let threads = pool().thread_num();
    let mut group = criterion.benchmark_group(format!("1M items, 25% awake, {threads} threads"));
    group.sample_size(10);
    million_case(&mut group, "rows", || RowsSide::new(MILLION_HALF, MILLION));
    million_case(&mut group, "components", || {
        ComponentsSide::new(MILLION_HALF, MILLION)
    });
    group.finish();
}

fn million_case<S: PassSide>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    new: impl Fn() -> S,
) {
    let mut scene: Option<S> = None;
    group.bench_function(name, |bencher| {
        let side = scene.get_or_insert_with(|| {
            let mut side = new();
            warm_up(&mut side);
            side.launch(Launch::Quarter);
            println!("1M items, {name}: {} of {MILLION} rest", side.resting());
            side
        });
        bencher.iter_custom(|iterations| {
            let mut total = Duration::ZERO;
            for _ in 0..iterations {
                side.launch(Launch::Quarter);
                let start = Instant::now();
                side.step();
                total += start.elapsed();
            }
            total
        })
    });
}

fn spawns(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("spawn 10k items");
    let source = RowScene::<()>::new(HALF, 0, SPAWNS, SEED);
    let mut items = Vec::new();
    for (_, tile) in source.rows.tiles() {
        items.extend(tile.rows().map(|row| (row.id, *row.state, *row.motion)));
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
        world.register_component::<MotionState>();
        world.register_component::<MotionKind>();
        world.register_component::<StepOutcome>();
        world
    };
    group.bench_function("components World::spawn_batch", |bencher| {
        bencher.iter_batched(
            fresh,
            |mut world| {
                let bundles = items
                    .iter()
                    .map(|&(_, state, motion)| (MotionState(state), MotionKind(motion)));
                world.spawn_batch(bundles);
                world
            },
            BatchSize::LargeInput,
        )
    });
    group.finish();
}

criterion_group!(benches, at_rest, in_flight, million, spawns);
criterion_main!(benches);
