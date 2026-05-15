use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use mcrs_minecraft_lighting::telemetry::{snapshot, TELEMETRY_TEST_LOCK};
use mcrs_minecraft_lighting::test_bench::bench_helpers;

fn bench_pit_dig(c: &mut Criterion) {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = snapshot();

    let mut group = c.benchmark_group("pit_dig_16x16x64");
    group.bench_function("propagate_to_quiescence", |b| {
        b.iter_batched(
            || bench_helpers::build_pit_dig_app(),
            |mut app| bench_helpers::run_until_converged(&mut app),
            BatchSize::SmallInput,
        );
    });
    group.finish();

    let after = snapshot();
    assert_eq!(
        after.cross_dim, before.cross_dim,
        "cross-dim guard fired during pit_dig — structural bug"
    );
    eprintln!(
        "pit_dig counter deltas: iters={} capped={} overflow={}",
        after.iterations - before.iterations,
        after.capped - before.capped,
        after.overflow - before.overflow,
    );
}

criterion_group!(benches, bench_pit_dig);
criterion_main!(benches);
