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
use mcrs_minecraft_engine_world::loading_rings::LOADING_RINGS;
use mcrs_minecraft_engine_world::world::dimension::{Dimension, DimensionTypeConfig};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_engine_world::world::storage::section_stage::SectionStage;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{ColumnPos, SectionPos};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
const OVERWORLD: &str = "minecraft:overworld";
const MAX_SIDE: i32 = 128;
const MARGIN_COLUMNS: i32 = 8;
const BORDER_COLUMNS: i64 = 30_000_000 / 16;
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
        assert!(
            side <= MAX_SIDE,
            "the side of the square is at most {MAX_SIDE} columns"
        );
        let reach = i64::from(offset).abs() + i64::from(side) + i64::from(MARGIN_COLUMNS);
        assert!(
            reach <= BORDER_COLUMNS,
            "the offset, the side and the margin of {MARGIN_COLUMNS} columns must fit the world border of {BORDER_COLUMNS} columns"
        );
        Self {
            side,
            seed: seed(&args, 1, 845),
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

fn seed(args: &[String], at: usize, default: u64) -> u64 {
    args.get(at).map_or(default, |text| {
        text.parse::<u64>()
            .or_else(|_| text.parse::<i64>().map(|signed| signed as u64))
            .unwrap_or_else(|_| panic!("argument {at} `{text}` is not a seed"))
    })
}

fn square(from: i32, side: i32) -> Vec<ColumnPos> {
    (0..side)
        .flat_map(|z| (0..side).map(move |x| ColumnPos::new(from + x, from + z)))
        .collect()
}

fn start(seed: u64) -> App {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .try_init();
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
    loop {
        assert!(
            app.should_exit().is_none(),
            "the app exited before Playing: {:?}",
            app.should_exit()
        );
        let playing = app
            .world()
            .get_resource::<State<AppState>>()
            .is_some_and(|state| *state.get() == AppState::Playing);
        if playing {
            break;
        }
        assert!(Instant::now() < deadline, "never reached Playing");
        app.update();
        assert!(
            app.should_exit().is_none(),
            "the app exited before Playing: {:?}",
            app.should_exit()
        );
    }
    runner::between_ticks(&mut app);
    app
}

#[cfg(unix)]
const PROCESS_CLOCK: &str = "cpu";

#[cfg(not(unix))]
const PROCESS_CLOCK: &str = "wall";

#[cfg(unix)]
fn process_cpu() -> Duration {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` is a valid, writable rusage and RUSAGE_SELF is a valid target.
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    assert_eq!(status, 0, "getrusage reads the usage of this process");
    // SAFETY: getrusage returned 0, so it filled the struct.
    let usage = unsafe { usage.assume_init() };
    let micros = |time: libc::timeval| time.tv_sec as u64 * 1_000_000 + time.tv_usec as u64;
    Duration::from_micros(micros(usage.ru_utime) + micros(usage.ru_stime))
}

#[cfg(not(unix))]
fn process_cpu() -> Duration {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
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

fn retain_pending(world: &mut World, pending: &mut Vec<ColumnPos>) {
    let dim = dimension_of(world);
    let Some(index) = world.get::<SectionIndex>(dim) else {
        return;
    };
    let Some(rows) = world.get::<DimensionTypeConfig>(dim) else {
        return;
    };
    let first = rows.min_y >> SectionPos::BITS;
    let rows = first..first + rows.section_count as i32;
    pending.retain(|column| {
        !rows.clone().all(|y| {
            index
                .get(SectionPos::new(column.x, y, column.z))
                .and_then(|section| world.get::<SectionStage>(section))
                == Some(&SectionStage::Loaded)
        })
    });
}

fn main() {
    let args = Args::read();
    let threads = std::thread::available_parallelism().map_or(0, |p| p.get());

    let booted = Instant::now();
    let mut app = start(args.seed);
    let label = label_of(&app);
    assert!(
        cell(&mut app, label).contains_resource::<FillContext>(),
        "the generation context is missing from the cell world"
    );
    let boot = booted.elapsed();

    let tickets = square(args.offset, args.side);
    let columns = square(args.offset - LOADING_RINGS, args.side + 2 * LOADING_RINGS);
    let world = cell(&mut app, label);
    let dim = dimension_of(world);
    let mut loading = world
        .get_mut::<ColumnLoading>(dim)
        .expect("the dimension loads columns");
    let cpu_started = process_cpu();
    let started = Instant::now();
    for &column in &tickets {
        loading.add_ticket(column);
    }

    let mut pending = columns.clone();
    let mut ticks = 0u64;
    loop {
        tick(&mut app);
        ticks += 1;
        assert!(
            app.should_exit().is_none(),
            "the app exited while loading columns: {:?}",
            app.should_exit()
        );
        retain_pending(cell(&mut app, label), &mut pending);
        if pending.is_empty() {
            break;
        }
        assert!(
            started.elapsed() < PATIENCE,
            "only {} of {} columns loaded",
            columns.len() - pending.len(),
            columns.len()
        );
    }
    let wall = started.elapsed();
    let cpu = process_cpu().saturating_sub(cpu_started);

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
        "wall={:.0} ms  ticks={ticks}  {:.1} columns/s  {:.2} ms/column  {:.2} {PROCESS_CLOCK}-ms/column",
        wall.as_secs_f64() * 1e3,
        count / wall.as_secs_f64(),
        wall.as_secs_f64() * 1e3 / count,
        cpu.as_secs_f64() * 1e3 / count,
    );
}
