use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use mcrs_minecraft_lighting::telemetry::{snapshot, TELEMETRY_TEST_LOCK};
use mcrs_minecraft_lighting::test_bench::bench_helpers;

fn bench_spawn_warmup(c: &mut Criterion) {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = snapshot();

    let mut group = c.benchmark_group("spawn_warmup_vd12");
    group.sample_size(20);

    let factory = bench_helpers::build_warmed_vd12_app_factory();

    group.bench_function("one_edge_column", |b| {
        b.iter_batched(
            || factory(),
            |mut app| {
                bench_helpers::spawn_edge_column(&mut app);
                bench_helpers::run_until_converged(&mut app);
            },
            BatchSize::SmallInput,
        );
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
