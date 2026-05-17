use bevy_ecs::prelude::*;
use criterion::{criterion_group, criterion_main, Criterion};
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_lighting::components::BlockBfsPending;
use mcrs_minecraft_lighting::telemetry::{snapshot, TELEMETRY_TEST_LOCK};
use mcrs_minecraft_lighting::test_bench::bench_helpers;
use mcrs_protocol::BlockStateId;
use std::time::{Duration, Instant};

fn bench_pit_dig(c: &mut Criterion) {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = snapshot();

    let mut group = c.benchmark_group("pit_dig_16x16x64");
    // `iter_custom` keeps per-iteration `App` construction and drop outside
    // the timing window so the bench tracks the vertical-drop convergence,
    // not entity teardown.
    group.bench_function("vertical_drop_to_quiescence", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let mut app = bench_helpers::build_pit_dig_app();
                // The palette-mutation + BlockBfsPending insertion is the
                // setup-equivalent for this scenario — outside timing.
                {
                    let mut q = app.world_mut().query_filtered::<Entity, With<BlockPalette>>();
                    let chunks: Vec<Entity> = q.iter(app.world()).collect();
                    for entity in chunks {
                        let mut palette = app.world_mut().get_mut::<BlockPalette>(entity).unwrap();
                        for x in 0i32..16 {
                            for y in 0i32..16 {
                                for z in 0i32..16 {
                                    palette.set((x, y, z), BlockStateId(0));
                                }
                            }
                        }
                        app.world_mut().entity_mut(entity).insert(BlockBfsPending);
                    }
                }
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
