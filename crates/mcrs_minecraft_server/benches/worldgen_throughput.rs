#![recursion_limit = "256"]
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a benchmark has no caller to return an error to"
)]

use std::num::NonZeroU32;
use std::str::FromStr;
use std::time::{Duration, Instant};

use bevy_app::{App, PluginGroup};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_state::prelude::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_dimension::Dimension as DimensionKind;
use mcrs_minecraft_engine::coordinator::dim::DimSubAppHandle;
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine::muxer::muxer_link::{self, Answers};
use mcrs_minecraft_engine_server::{Placement, WorldCells, runner};
use mcrs_minecraft_engine_world::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::{ColumnLoading, LOADING_RINGS};
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{ColumnPos, SectionPos};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
const OVERWORLD: &str = "minecraft:overworld";
const PATIENCE: Duration = Duration::from_secs(600);

struct Args {
    side: i32,
    seed: u64,
    offset: i32,
}

impl Args {
    fn read() -> Self {
        let args: Vec<String> = std::env::args()
            .skip(1)
            .filter(|a| a != "--bench")
            .collect();
        let side: i32 = argument(&args, 0, 16);
        let offset: i32 = argument(&args, 2, 0);
        assert!(side >= 1, "the side of the square is at least one column");
        let fits = offset
            .checked_sub(LOADING_RINGS)
            .and(side.checked_add(2 * LOADING_RINGS))
            .and(offset.checked_add(side))
            .and_then(|end| end.checked_add(LOADING_RINGS));
        assert!(
            fits.is_some(),
            "the square and its loading rings fit in the column range"
        );
        Self {
            side,
            seed: argument(&args, 1, 845),
            offset,
        }
    }
}

fn argument<T: FromStr>(args: &[String], at: usize, default: T) -> T {
    args.get(at).map_or(default, |text| {
        text.parse()
            .unwrap_or_else(|_| panic!("argument {at} `{text}` is not a number"))
    })
}

fn square(from: i32, side: i32) -> Vec<ColumnPos> {
    (0..side)
        .flat_map(|z| (0..side).map(move |x| ColumnPos::new(from + x, from + z)))
        .collect()
}

fn start(seed: u64) -> App {
    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.core.worlds = Placement::Inline;
    plugins.core.muxer = Placement::Inline;
    plugins.core.cells = WorldCells {
        cells: NonZeroU32::new(1).expect("one is not zero"),
    };
    plugins.network.announce_on_lan = false;
    plugins.worlds.seed = seed;
    let mut app = App::new();
    app.add_plugins(plugins.build());
    app.finish();
    app.cleanup();
    let deadline = Instant::now() + PATIENCE;
    while *app.world().resource::<State<AppState>>().get() != AppState::Playing {
        assert!(Instant::now() < deadline, "never reached Playing");
        app.update();
    }
    runner::between_ticks(&mut app);
    app
}

fn tick(app: &mut App) {
    app.update();
    runner::between_ticks(app);
    muxer_link::deliver(app.world_mut(), Answers::All);
}

fn label_of(app: &App) -> Entity {
    app.world()
        .try_query_filtered::<(Entity, &ResourceKeyBuf<DimensionKind>), With<DimSubAppHandle>>()
        .expect("a dimension was spawned")
        .iter(app.world())
        .find(|(_, key)| key.as_str() == OVERWORLD)
        .map(|(entity, _)| entity)
        .unwrap_or_else(|| panic!("{OVERWORLD} was spawned"))
}

fn cell(app: &mut App, label: Entity) -> &mut World {
    app.world_mut()
        .resource_mut::<DimCells>()
        .into_inner()
        .cell_sub_apps_mut(label)
        .next()
        .expect("the dimension is running")
        .world_mut()
}

fn dimension_of(world: &mut World) -> Entity {
    world
        .query_filtered::<Entity, With<Dimension>>()
        .single(world)
        .expect("the dimension entity")
}

fn loaded(world: &mut World, columns: &[ColumnPos]) -> usize {
    let dim = dimension_of(world);
    let Some(index) = world.get::<SectionIndex>(dim) else {
        return 0;
    };
    let Some(rows) = world.get::<DimensionTypeConfig>(dim) else {
        return 0;
    };
    let first = rows.min_y >> SectionPos::BITS;
    let rows = first..first + rows.section_count as i32;
    columns
        .iter()
        .filter(|column| {
            rows.clone().all(|y| {
                index
                    .get(SectionPos::new(column.x, y, column.z))
                    .and_then(|section| world.get::<SectionStage>(section))
                    == Some(&SectionStage::Loaded)
            })
        })
        .count()
}

fn main() {
    let args = Args::read();
    let threads = std::thread::available_parallelism().map_or(0, |p| p.get());

    let booted = Instant::now();
    let mut app = start(args.seed);
    let label = label_of(&app);
    let deadline = Instant::now() + PATIENCE;
    while !cell(&mut app, label).contains_resource::<FillContext>() {
        assert!(
            Instant::now() < deadline,
            "the generation context never loaded"
        );
        tick(&mut app);
    }
    let boot = booted.elapsed();

    let tickets = square(args.offset, args.side);
    let columns = square(args.offset - LOADING_RINGS, args.side + 2 * LOADING_RINGS);
    let world = cell(&mut app, label);
    let dim = dimension_of(world);
    let mut loading = world
        .get_mut::<ColumnLoading>(dim)
        .expect("the dimension loads columns");
    for &column in &tickets {
        loading.add_ticket(column);
    }

    let started = Instant::now();
    let mut ticks = 0u64;
    loop {
        tick(&mut app);
        ticks += 1;
        let done = loaded(cell(&mut app, label), &columns);
        if done == columns.len() {
            break;
        }
        assert!(
            started.elapsed() < PATIENCE,
            "only {done} of {} columns loaded",
            columns.len()
        );
    }
    let wall = started.elapsed();

    let count = columns.len() as f64;
    println!(
        "seed={} tickets={} ({}x{} at {}) columns={} threads={threads} boot={:.1} s",
        args.seed,
        tickets.len(),
        args.side,
        args.side,
        args.offset,
        columns.len(),
        boot.as_secs_f64()
    );
    println!(
        "wall={:.0} ms  ticks={ticks}  {:.1} columns/s  {:.2} ms/column  {:.2} thread-ms/column",
        wall.as_secs_f64() * 1e3,
        count / wall.as_secs_f64(),
        wall.as_secs_f64() * 1e3 / count,
        wall.as_secs_f64() * 1e3 * threads as f64 / count,
    );
}
