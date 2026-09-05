//! Measures what a bulk world load costs the light engine, batch by batch.

use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy_app::{App, TaskPoolPlugin};
use bevy_ecs::prelude::Entity;
use mcrs_minecraft_light::prelude::*;
use mcrs_voxel_math::{ChunkPos, ColumnPos};
use mcrs_voxel_storage::{PalettedContainer, VoxelId, VoxelPalette};

fn filled(block: VoxelId) -> SectionBlocks {
    VoxelPalette(PalettedContainer::Homogeneous(block))
}
use rayon::prelude::*;

const AIR: VoxelId = VoxelId(0);
const STONE: VoxelId = VoxelId(1);

fn registry() -> Arc<LightRegistry> {
    Arc::new(LightRegistry::new(
        vec![LightProperties::AIR, LightProperties::SOLID],
        SpecialBlocks {
            unloaded: STONE,
            outside: AIR,
        },
    ))
}

fn arg<T: std::str::FromStr>(n: usize) -> Option<T> {
    std::env::args().nth(n).and_then(|a| a.parse().ok())
}

fn main() {
    let radius: i32 = arg(1).unwrap_or(16);
    let defaults = LightBudget::default();
    let budget: u64 = arg(2).unwrap_or(defaults.cells_per_epoch);
    let in_flight: usize = arg(3).unwrap_or(defaults.epochs_in_flight);

    let bounds = LightBounds::new(-4, 19);
    let sections_per_column = (bounds.max_section_y - bounds.min_section_y + 1) as usize;
    let mut world = LightWorld::new(registry(), bounds);
    let mut queue = LightQueue::default();

    let air = Arc::new(filled(AIR));
    let stone = Arc::new(filled(STONE));

    let mut edits = Vec::new();
    let mut columns = 0usize;
    for x in -radius..=radius {
        for z in -radius..=radius {
            columns += 1;
            for y in bounds.min_section_y..=bounds.max_section_y {
                let blocks = if y == 0 {
                    Arc::clone(&stone)
                } else {
                    Arc::clone(&air)
                };
                edits.push(Edit::LoadSection {
                    entity: Entity::PLACEHOLDER,
                    pos: ChunkPos::new(x, y, z),
                    blocks,
                });
            }
        }
    }

    let side = radius * 2 + 1;
    println!(
        "loading {columns} columns ({side}x{side}), {} sections, budget {budget} cells per epoch, \
         {in_flight} epochs in flight",
        edits.len()
    );

    if std::env::var("LIGHT_APP").is_ok() {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default())
            .add_plugins(LightPlugin {
                registry: registry(),
                bounds,
                sky: true,
            });
        let limit = app.world().resource::<IntakeBudget>().columns_per_tick;
        {
            let mut pending = app.world_mut().resource_mut::<PendingEdits>();
            for edit in edits {
                pending.push(edit);
            }
        }
        let mut tick = 0usize;
        let mut worst = Duration::ZERO;
        let mut worst_at = 0usize;
        let mut intake_ticks = 0usize;
        let mut worst_waiting = 0usize;
        let mut first = Duration::ZERO;
        let started = Instant::now();
        loop {
            let ran = Instant::now();
            app.update();
            let took = ran.elapsed();
            tick += 1;
            if tick == 1 {
                first = took;
            }
            if took > worst {
                worst = took;
                worst_at = tick;
            }
            let world = app.world();
            if took == worst {
                worst_waiting = world.resource::<LightWorkQueue>().0.len();
            }
            let drained = world.resource::<PendingEdits>().is_empty();
            if drained && intake_ticks == 0 {
                intake_ticks = tick;
            }
            if drained
                && !world.resource::<LightEpoch>().is_running()
                && world.resource::<LightWorkQueue>().0.is_empty()
            {
                break;
            }
        }
        println!(
            "app.update() at {limit} columns per tick: intake drained after {intake_ticks} ticks, \
             settled after {tick} ticks, {:?} wall, first tick {first:?}, worst tick {worst:?} \
             (tick {worst_at}, {worst_waiting} columns waiting in the queue)",
            started.elapsed()
        );
        return;
    }

    let per_tick: usize = std::env::var("LIGHT_INTAKE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut intake_ticks = 0usize;
    // A column's lighting work cannot reach the queue before the tick that
    // admits it, so both projections below have to carry the intake ticks.
    let mut origin_intake_tick = 0usize;
    let work = if per_tick == 0 {
        let applied = Instant::now();
        let work = world.apply_edits(edits);
        println!(
            "apply_edits (block writes + sky floor scans): {:?} for {} influences\n",
            applied.elapsed(),
            work.len()
        );
        work
    } else {
        let per_intake = per_tick * sections_per_column;
        origin_intake_tick = edits
            .iter()
            .position(|edit| edit.column() == ColumnPos::new(0, 0))
            .map_or(0, |index| index / per_intake + 1);
        let mut rest = edits;
        let mut work = Vec::new();
        let mut intake_wall = Duration::ZERO;
        let mut worst_intake = Duration::ZERO;
        while !rest.is_empty() {
            let tail = rest.split_off(per_intake.min(rest.len()));
            let applied = Instant::now();
            work.extend(world.apply_edits(rest));
            intake_wall += applied.elapsed();
            worst_intake = worst_intake.max(applied.elapsed());
            intake_ticks += 1;
            rest = tail;
        }
        println!(
            "apply_edits at {per_tick} columns per tick: {intake_ticks} ticks, {intake_wall:?} \
             total, {:?} mean, {worst_intake:?} worst single call, for {} influences\n",
            intake_wall / intake_ticks as u32,
            work.len()
        );
        work
    };

    // `by_distance` mirrors what the send queue wants: nearest first. Without it
    // every column carries DEFAULT_PRIORITY and the BTreeSet degenerates to
    // ColumnPos order, i.e. x-major raster across the whole world.
    let by_distance = std::env::var("LIGHT_PRIORITY").as_deref() == Ok("distance");
    for (column, influence) in work {
        if by_distance {
            let d = column.distance_squared(ColumnPos::new(0, 0));
            queue.push_with_priority(
                column,
                influence,
                d.clamp(0, Priority::MAX as i32) as Priority,
            );
        } else {
            queue.push(column, influence);
        }
    }
    println!(
        "queue order: {}\n",
        if by_distance {
            "by distance to the player"
        } else {
            "DEFAULT_PRIORITY for everything (the pre-fix ordering)"
        }
    );

    println!(
        "{:>4}  {:>5}  {:>9}  {:>12}  {:>7}  {:>8}  {:>8}  {:>8}  {:>9}  {:>10}",
        "tick",
        "batch",
        "influx",
        "area_cells",
        "cols",
        "fill",
        "seed",
        "relax",
        "read_back",
        "job"
    );

    let started = Instant::now();
    let mut tick = 0usize;
    let mut batch = 0usize;
    let mut origin_batch: Option<usize> = None;
    let mut worst = Duration::ZERO;
    let mut worst_tick = Duration::ZERO;
    let mut worst_cells = 0usize;
    let mut drain_wall = Duration::ZERO;
    let mut worst_drain = Duration::ZERO;
    let mut prepare_wall = Duration::ZERO;
    let mut worst_prepare = Duration::ZERO;
    let mut publish_wall = Duration::ZERO;
    let mut worst_publish = Duration::ZERO;
    while !queue.is_empty() {
        tick += 1;

        // Exactly what `dispatch_epoch` does: fill the concurrency window with
        // batches that avoid each other's fields, then let them run together.
        let drained = Instant::now();
        let taken = queue.drain_batches(budget, &[], in_flight);
        drain_wall += drained.elapsed();
        worst_drain = worst_drain.max(drained.elapsed());
        let mut jobs = Vec::new();
        for taken in taken {
            batch += 1;
            let origin_here = taken
                .iter()
                .any(|i| ColumnPos::from(i.core.min).x == 0 && ColumnPos::from(i.core.min).z == 0);
            if origin_here && origin_batch.is_none() {
                origin_batch = Some(batch);
            }
            let influences = taken.len();
            let prepared = Instant::now();
            let job = world.prepare_batch(taken);
            prepare_wall += prepared.elapsed();
            worst_prepare = worst_prepare.max(prepared.elapsed());
            let Some(job) = job else {
                continue;
            };
            jobs.push((batch, influences, job));
        }
        if jobs.is_empty() {
            break;
        }

        let dispatched = Instant::now();
        let updates: Vec<(usize, usize, Duration, LightUpdate)> = jobs
            .into_par_iter()
            .map(|(index, influences, job)| {
                let ran = Instant::now();
                let update = job.run();
                (index, influences, ran.elapsed(), update)
            })
            .collect();
        let tick_wall = dispatched.elapsed();
        worst_tick = worst_tick.max(tick_wall);

        let mut publish_tick = Duration::ZERO;
        for (index, influences, job_wall, update) in updates {
            let stats = update.stats;
            worst = worst.max(job_wall);
            worst_cells = worst_cells.max(stats.area_cells);
            if index <= 12 || index % 25 == 0 {
                println!(
                    "{tick:>4}  {index:>5}  {influences:>9}  {:>12}  {:>7}  {:>8?}  {:>8?}  \
                     {:>8?}  {:>9?}  {:>10?}",
                    stats.area_cells,
                    influences / sections_per_column,
                    stats.timings.fill,
                    stats.timings.seed,
                    stats.timings.relax,
                    stats.timings.read_back,
                    job_wall
                );
            }
            let published = Instant::now();
            world.apply(update);
            publish_tick += published.elapsed();
        }
        publish_wall += publish_tick;
        worst_publish = worst_publish.max(publish_tick);
    }

    let total = started.elapsed();
    println!(
        "\n{batch} batches over {tick} ticks, {total:?} total, worst job {worst:?}, \
         worst tick {worst_tick:?}, largest field {worst_cells} cells"
    );
    println!(
        "queue drain on the main thread: {drain_wall:?} total, {:?} mean per tick, {worst_drain:?} worst tick",
        drain_wall / tick as u32
    );
    println!(
        "prepare_batch on the main thread: {prepare_wall:?} total over {batch} batches, {:?} mean, {worst_prepare:?} worst",
        prepare_wall / batch as u32
    );
    println!(
        "publish on the main thread: {publish_wall:?} total, {:?} mean per tick, {worst_publish:?} worst tick",
        publish_wall / tick as u32
    );
    println!(
        "a 20 tps server publishes at most {in_flight} epochs per tick: {:.1}s of wall clock at \
         best ({intake_ticks} intake ticks, {tick} drain ticks)",
        (intake_ticks + tick) as f64 * 0.05
    );
    match origin_batch {
        Some(n) => println!(
            "the player's own column (0,0) was admitted by intake tick {origin_intake_tick} and \
             lit in batch {n}/{batch} => it waits {:.1}s at 20 tps",
            (origin_intake_tick + n.div_ceil(in_flight)) as f64 * 0.05
        ),
        None => println!("the player's own column (0,0) was never lit"),
    }
}
