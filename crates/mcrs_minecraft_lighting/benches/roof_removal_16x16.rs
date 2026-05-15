use bevy_ecs::prelude::*;
use criterion::{criterion_group, criterion_main, Criterion};
use mcrs_minecraft_block::palette::BlockPalette;
use mcrs_minecraft_lighting::components::LightDirty;
use mcrs_minecraft_lighting::telemetry::{snapshot, TELEMETRY_TEST_LOCK};
use mcrs_minecraft_lighting::test_bench::bench_helpers;
use mcrs_protocol::BlockStateId;
use std::time::{Duration, Instant};

fn bench_roof_removal(c: &mut Criterion) {
    let _lock = TELEMETRY_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = snapshot();

    let mut group = c.benchmark_group("roof_removal_16x16");
    // `iter_custom` keeps per-iteration `App` construction and drop outside
    // the timing window so the bench tracks sky-light decrease propagation,
    // not entity teardown.
    group.bench_function("sky_decrease_to_quiescence", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let mut app = bench_helpers::build_roof_removal_app();
                // The palette-mutation + LightDirty insertion is the
                // setup-equivalent for this scenario — it lives outside
                // the timing window like the App build itself.
                {
                    let mut q = app.world_mut().query_filtered::<Entity, With<BlockPalette>>();
                    let sections: Vec<Entity> = q.iter(app.world()).collect();
                    for entity in sections {
                        let mut palette = app.world_mut().get_mut::<BlockPalette>(entity).unwrap();
                        for x in 0i32..16 {
                            for z in 0i32..16 {
                                palette.set((x, 15i32, z), BlockStateId(0));
                            }
                        }
                        app.world_mut().entity_mut(entity).insert(LightDirty);
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
        "cross-dim guard fired during roof_removal — structural bug"
    );
    eprintln!(
        "roof_removal counter deltas: iters={} capped={} overflow={}",
        after.iterations - before.iterations,
        after.capped - before.capped,
        after.overflow - before.overflow,
    );
}

criterion_group!(benches, bench_roof_removal);
criterion_main!(benches);
