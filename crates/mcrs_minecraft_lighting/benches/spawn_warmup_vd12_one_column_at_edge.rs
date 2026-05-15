use criterion::{criterion_group, criterion_main, Criterion};
use mcrs_minecraft_lighting::telemetry::{snapshot, TELEMETRY_TEST_LOCK};
use mcrs_minecraft_lighting::test_bench::bench_helpers;
use std::time::{Duration, Instant};

fn bench_spawn_warmup(c: &mut Criterion) {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = snapshot();

    let mut group = c.benchmark_group("spawn_warmup_vd12");
    group.sample_size(20);

    let factory = bench_helpers::build_warmed_vd12_app_factory();

    // `iter_custom` gives us explicit control over what lands inside the
    // measurement window. `iter_batched(SmallInput)` previously folded the
    // teardown of the 15 000+ section warmup grid (Box<NibbleArray> drops,
    // SmallVec drops, archetype despawn cascade) into criterion's timing
    // window — that 3.5 ms of pure destructor work swamped the ~0.9 ms of
    // actual lighting work and made the bench number track teardown
    // performance instead of the cascade we care about. With `iter_custom`,
    // both the per-iteration `factory()` setup and the post-iteration
    // `drop(app)` stay outside `start..elapsed`.
    group.bench_function("one_edge_column", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let mut app = factory();
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
        "cross-dim guard fired during spawn_warmup — structural bug"
    );
    eprintln!(
        "spawn_warmup counter deltas: iters={} capped={} overflow={}",
        after.iterations - before.iterations,
        after.capped - before.capped,
        after.overflow - before.overflow,
    );
}

criterion_group!(benches, bench_spawn_warmup);
criterion_main!(benches);
