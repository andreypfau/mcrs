use criterion::{criterion_group, criterion_main, Criterion};
use mcrs_minecraft_lighting::telemetry::{snapshot, TELEMETRY_TEST_LOCK};
use mcrs_minecraft_lighting::test_bench::bench_helpers;
use std::time::{Duration, Instant};

fn bench_tnt_chain(c: &mut Criterion) {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = snapshot();

    let mut group = c.benchmark_group("tnt_chain");
    // `iter_custom` keeps per-iteration `App` construction and drop outside
    // the timing window so the bench number tracks the cascade itself, not
    // entity teardown.
    group.bench_function("cascade_to_quiescence", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let mut app = bench_helpers::build_tnt_chain_app();
                let start = Instant::now();
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
        "cross-dim guard fired during tnt_chain — structural bug"
    );
    eprintln!(
        "tnt_chain counter deltas: iters={} capped={} overflow={}",
        after.iterations - before.iterations,
        after.capped - before.capped,
        after.overflow - before.overflow,
    );
}

criterion_group!(benches, bench_tnt_chain);
criterion_main!(benches);
