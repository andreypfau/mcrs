use bevy_tasks::TaskPoolBuilder;
use bevy_tasks::available_parallelism;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::TestScene;

const WARM_UP_TICKS: u32 = 100;

fn entity_pass(criterion: &mut Criterion) {
    let threads = available_parallelism();
    let pool = TaskPoolBuilder::new().num_threads(threads).build();
    let mut scene = TestScene::<()>::new(16, 50_000, 50_000, 11);
    let tiles_per_task = (32 * 32_usize).div_ceil(threads * 4);
    let mut writes = EntityWrites::default();
    for _ in 0..WARM_UP_TICKS {
        scene.step(&pool, tiles_per_task, &mut writes);
    }
    criterion.bench_function(
        &format!("tick of 100k items and walkers, {threads} threads"),
        |bencher| bencher.iter(|| scene.step(&pool, tiles_per_task, &mut writes)),
    );
}

criterion_group!(benches, entity_pass);
criterion_main!(benches);
