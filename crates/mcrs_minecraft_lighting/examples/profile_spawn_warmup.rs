//! Per-phase timing profiler for the spawn_warmup_vd12/one_edge_column
//! scenario. Runs the same workload the criterion bench measures but reports
//! wall-clock time spent in each FixedUpdate stage so we can see where the
//! ~4.3 ms is actually going.
//!
//! Run with:
//! ```
//! cargo run --release --example profile_spawn_warmup \
//!     -p mcrs_minecraft_lighting --features bench-helpers
//! ```

use bevy_app::{App, FixedUpdate};
use bevy_ecs::prelude::*;
use mcrs_minecraft_lighting::components::LightDirty;
use mcrs_minecraft_lighting::test_bench::bench_helpers;
use std::time::{Duration, Instant};

const SAMPLES: usize = 30;

fn main() {
    println!("# spawn_warmup_vd12 / one_edge_column — phase timings (release)\n");

    // Warm up the JIT / allocator / OS caches with one untimed run.
    let factory = bench_helpers::build_warmed_vd12_app_factory();
    let _ = factory();

    // Time the warmup factory itself (untimed by criterion, but it matters
    // for understanding total work).
    let mut warmup_times = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        let _app = factory();
        warmup_times.push(start.elapsed());
    }
    print_phase("factory (warmup build + converge)", &warmup_times);

    // Per-phase timing inside the bench routine:
    //   (a) bench_helpers::spawn_edge_column     — entity spawn
    //   (b) run_until_converged                   — ticks until quiescent
    let mut spawn_times = Vec::with_capacity(SAMPLES);
    let mut tick1_times = Vec::with_capacity(SAMPLES);
    let mut converge_extra_times = Vec::with_capacity(SAMPLES);
    let mut drop_times = Vec::with_capacity(SAMPLES);
    let mut total_routine_times = Vec::with_capacity(SAMPLES);
    let mut total_with_drop_times = Vec::with_capacity(SAMPLES);
    let mut tick_counts = Vec::with_capacity(SAMPLES);

    for _ in 0..SAMPLES {
        let app = factory();
        let mut app = app;

        let routine_start = Instant::now();

        let spawn_start = Instant::now();
        let _ = bench_helpers::spawn_edge_column(&mut app);
        spawn_times.push(spawn_start.elapsed());

        // Tick 1: where the heavy lifting happens (reconcile, prime, attach,
        // seed, pull, converge).
        let tick1_start = Instant::now();
        app.world_mut().run_schedule(FixedUpdate);
        tick1_times.push(tick1_start.elapsed());

        // Any additional ticks until LightDirty is fully drained.
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

        let routine_only_elapsed = routine_start.elapsed();
        total_routine_times.push(routine_only_elapsed);

        // BatchSize::SmallInput drops the input after each routine
        // invocation — that drop time lands INSIDE criterion's
        // measurement window. Time it explicitly so the breakdown
        // matches the bench number.
        let drop_start = Instant::now();
        drop(app);
        drop_times.push(drop_start.elapsed());

        total_with_drop_times.push(routine_only_elapsed + drop_times.last().copied().unwrap());
    }

    print_phase("spawn_edge_column (24 entity spawns)", &spawn_times);
    print_phase("FixedUpdate tick 1 (everything)", &tick1_times);
    print_phase(
        "extra converge ticks past tick 1",
        &converge_extra_times,
    );
    print_phase("drop(app) — 15k+ entity teardown", &drop_times);
    print_phase("routine alone (no drop)", &total_routine_times);
    print_phase(
        "routine + drop (criterion-measured)",
        &total_with_drop_times,
    );

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
    let mut q = app.world_mut().query_filtered::<(), With<LightDirty>>();
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
        "{:<42}  min={:>8}  p10={:>8}  median={:>8}  p90={:>8}  max={:>8}",
        label,
        fmt_dur(min),
        fmt_dur(p10),
        fmt_dur(median),
        fmt_dur(p90),
        fmt_dur(max),
    );
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
