//! Per-phase timing profiler for the spawn_warmup_vd12/one_edge_column
//! scenario. Runs the same workload the criterion bench measures and reports
//! wall-clock time spent in each lifecycle / lighting-set stage so the ~925
//! us tick can be broken down into the systems that actually contribute.
//!
//! Run with:
//! ```
//! cargo run --release --example profile_spawn_warmup \
//!     -p mcrs_minecraft_lighting --features bench-helpers
//! ```

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use mcrs_engine::world::column::ColumnLifecycleSet;
use mcrs_minecraft_lighting::components::{BlockBfsPending, SkyBfsPending};
use mcrs_minecraft_block::block_update::BlockUpdateSet;
use mcrs_minecraft_lighting::sets::LightingSet;
use mcrs_minecraft_lighting::telemetry::snapshot as lighting_snapshot;
use mcrs_minecraft_lighting::test_bench::bench_helpers;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const SAMPLES: usize = 30;

/// Per-set wall-clock accumulators, in nanoseconds. Touched only by the
/// inline timing systems registered by `PhaseTimingPlugin`, so single-thread
/// `Relaxed` is enough.
#[derive(Default)]
struct PhaseAccumulators {
    reconcile_index_start: AtomicU64,
    reconcile_index_total: AtomicU64,
    prime_heightmaps_start: AtomicU64,
    prime_heightmaps_total: AtomicU64,
    attach_state_start: AtomicU64,
    attach_state_total: AtomicU64,
    enqueue_start: AtomicU64,
    enqueue_total: AtomicU64,
    converge_start: AtomicU64,
    converge_total: AtomicU64,
    emit_dirty_start: AtomicU64,
    emit_dirty_total: AtomicU64,
    tick_start: AtomicU64,
    tick_total: AtomicU64,
}

#[derive(Resource)]
struct PhaseTimings(&'static PhaseAccumulators);

static PHASE_ACCUMULATORS: PhaseAccumulators = PhaseAccumulators {
    reconcile_index_start: AtomicU64::new(0),
    reconcile_index_total: AtomicU64::new(0),
    prime_heightmaps_start: AtomicU64::new(0),
    prime_heightmaps_total: AtomicU64::new(0),
    attach_state_start: AtomicU64::new(0),
    attach_state_total: AtomicU64::new(0),
    enqueue_start: AtomicU64::new(0),
    enqueue_total: AtomicU64::new(0),
    converge_start: AtomicU64::new(0),
    converge_total: AtomicU64::new(0),
    emit_dirty_start: AtomicU64::new(0),
    emit_dirty_total: AtomicU64::new(0),
    tick_start: AtomicU64::new(0),
    tick_total: AtomicU64::new(0),
};

/// Anchor used by all timing functions — every duration we record is a delta
/// from this fixed monotonic start. `Instant::elapsed` itself is monotonic
/// but is not `Copy`, so anchoring once per process gives us a u64 nanos
/// scalar we can `fetch_add` into the atomics.
static ANCHOR: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn anchor_nanos() -> u64 {
    let a = *ANCHOR.get_or_init(Instant::now);
    Instant::now().duration_since(a).as_nanos() as u64
}

macro_rules! start_phase {
    ($field:ident) => {
        |t: Res<PhaseTimings>| {
            t.0.$field.store(anchor_nanos(), Ordering::Relaxed);
        }
    };
}

macro_rules! end_phase {
    ($start_field:ident, $total_field:ident) => {
        |t: Res<PhaseTimings>| {
            let start = t.0.$start_field.load(Ordering::Relaxed);
            let now = anchor_nanos();
            t.0.$total_field
                .fetch_add(now.saturating_sub(start), Ordering::Relaxed);
        }
    };
}

static DIRTY_AT_CONVERGE_ENTRY: AtomicU64 = AtomicU64::new(0);
static DIRTY_FIRST_X: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(i32::MIN);
static DIRTY_FIRST_Y: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(i32::MIN);
static DIRTY_FIRST_Z: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(i32::MIN);

struct PhaseTimingPlugin;

impl Plugin for PhaseTimingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PhaseTimings(&PHASE_ACCUMULATORS));

        // Probe: count any chunk with pending work on either channel
        // immediately before `LightingSet::Converge` runs, and remember
        // the first one's chunk_pos so the harness can identify it.
        app.add_systems(
            FixedUpdate,
            (|q: Query<
                &mcrs_engine::world::chunk::ChunkPos,
                Or<(With<BlockBfsPending>, With<SkyBfsPending>)>,
            >| {
                let positions: Vec<_> = q.iter().copied().collect();
                DIRTY_AT_CONVERGE_ENTRY.store(positions.len() as u64, Ordering::Relaxed);
                if let Some(p) = positions.first() {
                    DIRTY_FIRST_X.store(p.x, Ordering::Relaxed);
                    DIRTY_FIRST_Y.store(p.y, Ordering::Relaxed);
                    DIRTY_FIRST_Z.store(p.z, Ordering::Relaxed);
                }
            })
            .after(LightingSet::Enqueue)
            .before(LightingSet::Converge),
        );

        // Each "start" timing system runs strictly AFTER the previous set
        // ends so its recorded anchor lands at the actual transition point,
        // not at the start of the tick. Without `.after(previous_set)` Bevy
        // would schedule the start systems early and the recorded
        // duration would include every prior stage's work.

        // Tick-wide bracket.
        app.add_systems(
            FixedUpdate,
            (
                (|t: Res<PhaseTimings>| {
                    t.0.tick_start.store(anchor_nanos(), Ordering::Relaxed);
                })
                .before(ColumnLifecycleSet::Reconcile),
                (|t: Res<PhaseTimings>| {
                    let start = t.0.tick_start.load(Ordering::Relaxed);
                    let now = anchor_nanos();
                    t.0.tick_total
                        .fetch_add(now.saturating_sub(start), Ordering::Relaxed);
                })
                .after(LightingSet::EmitDirty),
            ),
        );

        // Reconcile + ReconcileIndex stages.
        app.add_systems(
            FixedUpdate,
            (
                start_phase!(reconcile_index_start)
                    .before(ColumnLifecycleSet::Reconcile),
                end_phase!(reconcile_index_start, reconcile_index_total)
                    .after(ColumnLifecycleSet::ReconcileIndex)
                    .before(ColumnLifecycleSet::PrimeHeightmaps),
            ),
        );

        // PrimeHeightmaps stage.
        app.add_systems(
            FixedUpdate,
            (
                start_phase!(prime_heightmaps_start)
                    .after(ColumnLifecycleSet::ReconcileIndex)
                    .before(ColumnLifecycleSet::PrimeHeightmaps),
                end_phase!(prime_heightmaps_start, prime_heightmaps_total)
                    .after(ColumnLifecycleSet::PrimeHeightmaps)
                    .before(ColumnLifecycleSet::AttachState),
            ),
        );

        // AttachState stage.
        app.add_systems(
            FixedUpdate,
            (
                start_phase!(attach_state_start)
                    .after(ColumnLifecycleSet::PrimeHeightmaps)
                    .before(ColumnLifecycleSet::AttachState),
                end_phase!(attach_state_start, attach_state_total)
                    .after(ColumnLifecycleSet::AttachState)
                    .before(BlockUpdateSet::ApplyChanges),
            ),
        );

        // LightingSet::Enqueue. Brackets after BlockUpdateSet so the
        // BlockUpdate stage time is not folded in.
        app.add_systems(
            FixedUpdate,
            (
                start_phase!(enqueue_start)
                    .after(BlockUpdateSet::ApplyChanges)
                    .before(LightingSet::Enqueue),
                end_phase!(enqueue_start, enqueue_total)
                    .after(LightingSet::Enqueue)
                    .before(LightingSet::Converge),
            ),
        );

        // LightingSet::Converge.
        app.add_systems(
            FixedUpdate,
            (
                start_phase!(converge_start)
                    .after(LightingSet::Enqueue)
                    .before(LightingSet::Converge),
                end_phase!(converge_start, converge_total)
                    .after(LightingSet::Converge)
                    .before(LightingSet::EmitDirty),
            ),
        );

        // LightingSet::EmitDirty.
        app.add_systems(
            FixedUpdate,
            (
                start_phase!(emit_dirty_start)
                    .after(LightingSet::Converge)
                    .before(LightingSet::EmitDirty),
                end_phase!(emit_dirty_start, emit_dirty_total)
                    .after(LightingSet::EmitDirty),
            ),
        );
    }
}

fn reset_accumulators() {
    PHASE_ACCUMULATORS.reconcile_index_total.store(0, Ordering::Relaxed);
    PHASE_ACCUMULATORS.prime_heightmaps_total.store(0, Ordering::Relaxed);
    PHASE_ACCUMULATORS.attach_state_total.store(0, Ordering::Relaxed);
    PHASE_ACCUMULATORS.enqueue_total.store(0, Ordering::Relaxed);
    PHASE_ACCUMULATORS.converge_total.store(0, Ordering::Relaxed);
    PHASE_ACCUMULATORS.emit_dirty_total.store(0, Ordering::Relaxed);
    PHASE_ACCUMULATORS.tick_total.store(0, Ordering::Relaxed);
}

fn read_accumulators() -> PhaseSnapshot {
    PhaseSnapshot {
        reconcile_index: PHASE_ACCUMULATORS.reconcile_index_total.load(Ordering::Relaxed),
        prime_heightmaps: PHASE_ACCUMULATORS.prime_heightmaps_total.load(Ordering::Relaxed),
        attach_state: PHASE_ACCUMULATORS.attach_state_total.load(Ordering::Relaxed),
        enqueue: PHASE_ACCUMULATORS.enqueue_total.load(Ordering::Relaxed),
        converge: PHASE_ACCUMULATORS.converge_total.load(Ordering::Relaxed),
        emit_dirty: PHASE_ACCUMULATORS.emit_dirty_total.load(Ordering::Relaxed),
        tick: PHASE_ACCUMULATORS.tick_total.load(Ordering::Relaxed),
    }
}

#[derive(Default, Clone, Copy)]
struct PhaseSnapshot {
    reconcile_index: u64,
    prime_heightmaps: u64,
    attach_state: u64,
    enqueue: u64,
    converge: u64,
    emit_dirty: u64,
    tick: u64,
}

fn build_instrumented_factory() -> Box<dyn Fn() -> App + Send + Sync> {
    use bevy_app::App as BApp;
    use bevy_state::app::{AppExtStates, StatesPlugin};
    use mcrs_core::AppState;
    use mcrs_engine::world::chunk::ChunkPos;
    use mcrs_engine::world::column::ColumnPlugin;
    Box::new(|| {
        let mut app = BApp::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<AppState>();
        app.add_plugins(ColumnPlugin);
        app.add_plugins(mcrs_minecraft_lighting::LightingPlugin);
        app.add_plugins(PhaseTimingPlugin);
        app.insert_resource(bench_helpers::make_stub_block_light_table());
        let dim = bench_helpers::spawn_test_dimension(&mut app, true);
        for chunk_x in -12i32..=12 {
            for chunk_z in -12i32..=12 {
                for chunk_y in 0..24i32 {
                    let palette = if chunk_y % 2 == 0 {
                        bench_helpers::stone_cap_then_air_palette()
                    } else {
                        bench_helpers::air_palette()
                    };
                    bench_helpers::spawn_test_chunk(
                        &mut app,
                        dim,
                        ChunkPos::new(chunk_x, chunk_y, chunk_z),
                        palette,
                    );
                }
            }
        }
        let _ = bench_helpers::run_until_converged(&mut app);
        // Reset accumulators after the factory's own converge — we only care
        // about the timed edge-column add, not the warmup.
        reset_accumulators();
        app
    })
}

fn main() {
    println!("# spawn_warmup_vd12 / one_edge_column — phase timings (release)\n");

    let _ = ANCHOR.get_or_init(Instant::now);

    let factory = build_instrumented_factory();
    let _ = factory(); // JIT / allocator warm-up.

    let mut spawn_times = Vec::with_capacity(SAMPLES);
    let mut tick1_times = Vec::with_capacity(SAMPLES);
    let mut converge_extra_times = Vec::with_capacity(SAMPLES);
    let mut total_routine_times = Vec::with_capacity(SAMPLES);
    let mut tick_counts = Vec::with_capacity(SAMPLES);

    let mut phase_snaps: Vec<PhaseSnapshot> = Vec::with_capacity(SAMPLES);

    let mut iters_deltas = Vec::with_capacity(SAMPLES);
    let mut dirty_at_converge_entry: Vec<usize> = Vec::with_capacity(SAMPLES);

    // Probe one sample to see how many chunks are dirty AT THE MOMENT
    // light_converge_driver starts (after Enqueue, before Converge).
    {
        use mcrs_engine::world::chunk::ChunkPos as CPos;

        let mut probe_app = factory();
        // Check if factory left anything dirty.
        let mut q0 = probe_app
            .world_mut()
            .query_filtered::<&CPos, Or<(With<BlockBfsPending>, With<SkyBfsPending>)>>();
        let factory_residue: Vec<CPos> = q0.iter(probe_app.world()).copied().collect();
        println!(
            "## PROBE: dirty after factory (untimed) = {} chunks {:?}",
            factory_residue.len(),
            &factory_residue[..factory_residue.len().min(8)]
        );

        let _ = bench_helpers::spawn_edge_column(&mut probe_app);
        let iters_before = lighting_snapshot().iterations;
        probe_app.world_mut().run_schedule(FixedUpdate);
        let iters_after = lighting_snapshot().iterations;
        let dirty_at_entry = DIRTY_AT_CONVERGE_ENTRY.load(Ordering::Relaxed);
        let dx = DIRTY_FIRST_X.load(Ordering::Relaxed);
        let dy = DIRTY_FIRST_Y.load(Ordering::Relaxed);
        let dz = DIRTY_FIRST_Z.load(Ordering::Relaxed);
        println!(
            "## PROBE: dirty at Converge entry = {} (first at chunk ({},{},{})), iters consumed = {}",
            dirty_at_entry,
            dx, dy, dz,
            iters_after - iters_before
        );
        // Walk a second tick to see what's left dirty
        let mut q = probe_app
            .world_mut()
            .query_filtered::<&CPos, Or<(With<BlockBfsPending>, With<SkyBfsPending>)>>();
        let still: Vec<CPos> = q.iter(probe_app.world()).copied().collect();
        println!(
            "## PROBE: dirty after tick 1 = {} {:?}",
            still.len(),
            &still[..still.len().min(8)]
        );
        drop(probe_app);
    }

    for _ in 0..SAMPLES {
        let mut app = factory();
        // factory() already reset; clear once more to flush any plugin-build
        // side effects.
        reset_accumulators();

        let routine_start = Instant::now();

        let spawn_start = Instant::now();
        let _ = bench_helpers::spawn_edge_column(&mut app);
        spawn_times.push(spawn_start.elapsed());

        let iters_before = lighting_snapshot().iterations;

        let tick1_start = Instant::now();
        app.world_mut().run_schedule(FixedUpdate);
        tick1_times.push(tick1_start.elapsed());

        let iters_after = lighting_snapshot().iterations;
        iters_deltas.push(iters_after - iters_before);

        // Count dirty chunks IMMEDIATELY after the tick — gives us the
        // residual count that the in-tick converge driver did NOT clear.
        // Not the same as what the driver saw on entry (post-Enqueue), but
        // a useful signal: if 0, the driver started with no dirty AND
        // emit_dirty didn't introduce any.
        let mut q = app
            .world_mut()
            .query_filtered::<(), Or<(With<BlockBfsPending>, With<SkyBfsPending>)>>();
        dirty_at_converge_entry.push(q.iter(app.world()).count());

        let mut ticks = 1;
        let converge_start = Instant::now();
        loop {
            if !has_any_light_dirty(&mut app) {
                break;
            }
            app.world_mut().run_schedule(FixedUpdate);
            ticks += 1;
            if ticks > 256 {
                panic!("profile harness did not converge in 256 ticks");
            }
        }
        converge_extra_times.push(converge_start.elapsed());
        tick_counts.push(ticks);

        total_routine_times.push(routine_start.elapsed());

        phase_snaps.push(read_accumulators());

        drop(app);
    }

    let avg_iters: f64 =
        iters_deltas.iter().copied().map(|n| n as f64).sum::<f64>() / SAMPLES as f64;
    let avg_dirty: f64 =
        dirty_at_converge_entry.iter().copied().map(|n| n as f64).sum::<f64>() / SAMPLES as f64;
    println!(
        "## converge counters per routine tick: iters_delta avg={:.2} min={} max={}, residual dirty after tick avg={:.2}",
        avg_iters,
        iters_deltas.iter().min().unwrap(),
        iters_deltas.iter().max().unwrap(),
        avg_dirty,
    );

    print_phase("spawn_edge_column (24 entity spawns)", &spawn_times);
    print_phase("FixedUpdate tick 1 (everything)", &tick1_times);
    print_phase("extra converge ticks past tick 1", &converge_extra_times);
    print_phase("total routine (criterion-measured)", &total_routine_times);

    println!();
    println!("## FixedUpdate tick 1 — per-set breakdown");

    print_phase_atomic("reconcile + reconcile_index", phase_snaps.iter().map(|s| s.reconcile_index));
    print_phase_atomic("prime_heightmaps", phase_snaps.iter().map(|s| s.prime_heightmaps));
    print_phase_atomic("attach_lighting_state", phase_snaps.iter().map(|s| s.attach_state));
    print_phase_atomic(
        "LightingSet::Enqueue (seed/pull/enqueue)",
        phase_snaps.iter().map(|s| s.enqueue),
    );
    print_phase_atomic(
        "LightingSet::Converge (light_converge_driver)",
        phase_snaps.iter().map(|s| s.converge),
    );
    print_phase_atomic(
        "LightingSet::EmitDirty",
        phase_snaps.iter().map(|s| s.emit_dirty),
    );
    println!();
    print_phase_atomic("(sum of set timings, sanity check)", phase_snaps.iter().map(|s| {
        s.reconcile_index + s.prime_heightmaps + s.attach_state + s.enqueue + s.converge + s.emit_dirty
    }));
    print_phase_atomic("tick (across all ticks)", phase_snaps.iter().map(|s| s.tick));

    let avg_ticks: f64 =
        tick_counts.iter().copied().map(|n| n as f64).sum::<f64>() / SAMPLES as f64;
    println!(
        "\ntick count (run_until_converged): min={} max={} avg={:.2}",
        tick_counts.iter().min().unwrap(),
        tick_counts.iter().max().unwrap(),
        avg_ticks,
    );
}

fn has_any_light_dirty(app: &mut App) -> bool {
    let mut q = app
        .world_mut()
        .query_filtered::<(), Or<(With<BlockBfsPending>, With<SkyBfsPending>)>>();
    q.iter(app.world()).next().is_some()
}

fn print_phase(label: &str, samples: &[Duration]) {
    let mut sorted: Vec<Duration> = samples.to_vec();
    sorted.sort();
    let median = sorted[sorted.len() / 2];
    let p10 = sorted[sorted.len() / 10];
    let p90 = sorted[(sorted.len() * 9) / 10];
    let min = sorted[0];
    let max = *sorted.last().unwrap();
    println!(
        "{:<48}  min={:>8}  p10={:>8}  median={:>8}  p90={:>8}  max={:>8}",
        label,
        fmt_dur(min),
        fmt_dur(p10),
        fmt_dur(median),
        fmt_dur(p90),
        fmt_dur(max),
    );
}

fn print_phase_atomic(label: &str, samples: impl Iterator<Item = u64>) {
    let durations: Vec<Duration> = samples.map(Duration::from_nanos).collect();
    print_phase(label, &durations);
}

fn fmt_dur(d: Duration) -> String {
    if d.as_millis() >= 1 {
        format!("{:>6.3}ms", d.as_secs_f64() * 1000.0)
    } else if d.as_micros() >= 1 {
        format!("{:>6.1}us", d.as_secs_f64() * 1_000_000.0)
    } else {
        format!("{:>6}ns", d.as_nanos())
    }
}
