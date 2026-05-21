use bevy_app::{App, TaskPoolOptions, TaskPoolPlugin};
use criterion::{criterion_group, criterion_main, Criterion};
use mcrs_minecraft_lighting::metrics::{snapshot, TELEMETRY_TEST_LOCK};
use mcrs_minecraft_lighting::test_bench::bench_helpers;
use std::time::{Duration, Instant};

fn read_threads_env() -> usize {
    std::env::var("MCRS_BENCH_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

fn bench_parallel_scaling(c: &mut Criterion) {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = snapshot();

    let threads = read_threads_env();
    eprintln!("MCRS_BENCH_THREADS={threads}");
    let group_id = format!("parallel_scaling/threads_{threads}");
    let mut group = c.benchmark_group(group_id);
    group.sample_size(20);

    // `iter_custom` keeps per-iteration `App` construction (which warms up
    // the 25×25 VD12 grid — a 220 ms operation) AND its drop (which tears
    // down 15 000+ entities — 3-4 ms) both outside the timing window. The
    // bench number tracks the edge-column spawn + cascade only.
    group.bench_function("spawn_warmup_edge_column", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let mut app = App::new();
                app.add_plugins(TaskPoolPlugin {
                    task_pool_options: TaskPoolOptions::with_num_threads(threads),
                });
                bench_helpers::install_lighting_plugins(&mut app);
                bench_helpers::build_warmed_vd12_app_in_place(&mut app);
                let start = Instant::now();
                bench_helpers::spawn_edge_column(&mut app);
                bench_helpers::run_until_converged(&mut app);
                total += start.elapsed();
                drop(app);
            }
            total
        });
    });
    group.finish();

    let after = snapshot();
    assert_eq!(
        after.cross_dim, before.cross_dim,
        "cross-dim guard fired during parallel_scaling — structural bug"
    );
    eprintln!(
        "parallel_scaling (threads={}) counter deltas: iters={} capped={} overflow={}",
        threads,
        after.iterations - before.iterations,
        after.capped - before.capped,
        after.overflow - before.overflow,
    );
}

criterion_group!(benches, bench_parallel_scaling);
criterion_main!(benches);
