//! Bot-load benchmarks for the `bridge_outbound` routing path and the
//! `SessionRegistry` cross-dim + teardown path.
//!
//! The functional invariants each profile must hold — every injected packet
//! routed, the expected number of cross-dim transfers, no orphan entities, an
//! empty registry after teardown — are asserted on every measured iteration,
//! so a regression fails the bench rather than showing up as a timing wobble.

use criterion::{Criterion, criterion_group, criterion_main};
use std::time::{Duration, Instant};

#[path = "common/scale_bots.rs"]
mod scale_bots;

use scale_bots::{ScaleReport, run_profile_ticks};

/// Even, so exactly `TICKS / 2` injection ticks occur (one BlockUpdate per bot
/// on every even tick), and well past the halfway mark where the cross-dim
/// transfer fires.
const TICKS: u64 = 24;

fn check(report: &ScaleReport, bots: usize, cross_dim_rate: f32) {
    assert_eq!(
        report.tick_count, TICKS,
        "run should execute exactly the tick budget"
    );

    let expected_injected = bots as u64 * (TICKS / 2);
    assert_eq!(
        report.packets_injected, expected_injected,
        "expected {expected_injected} injected packets",
    );

    assert_eq!(
        report.total_queued, report.packets_injected,
        "every injected packet should route to a bot queue (queued={}, injected={})",
        report.total_queued, report.packets_injected,
    );

    let expected_transfers = (bots as f32 * cross_dim_rate) as u64;
    assert_eq!(
        report.cross_dim_transfers, expected_transfers,
        "expected {expected_transfers} cross-dim transfers",
    );

    assert!(
        report.entity_delta() <= 0,
        "entity_delta should be non-positive (no orphan leak), got {} (start={}, end={})",
        report.entity_delta(),
        report.entity_count_start,
        report.entity_count_end,
    );

    assert_eq!(
        report.sessions_remaining_after_teardown, 0,
        "teardown should leave no sessions in the registry",
    );
}

fn bench_profile(c: &mut Criterion, label: &str, dims: usize, bots: usize) {
    let cross_dim_rate = 0.1;
    let mut group = c.benchmark_group("tr07_scale");
    group.sample_size(10);
    group.bench_function(label, |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let start = Instant::now();
                let report = run_profile_ticks(label, dims, bots, cross_dim_rate, TICKS);
                total += start.elapsed();
                check(&report, bots, cross_dim_rate);
            }
            total
        });
    });
    group.finish();
}

fn vanilla(c: &mut Criterion) {
    bench_profile(c, "vanilla", 2, 100);
}

fn minigame(c: &mut Criterion) {
    bench_profile(c, "minigame", 20, 320);
}

criterion_group!(benches, vanilla, minigame);
criterion_main!(benches);
