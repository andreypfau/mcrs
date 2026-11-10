#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a benchmark with a broken setup stops"
)]

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mcrs_minecraft_entity_animation::EntityQuery;
use mcrs_minecraft_entity_render::{EntityLibrary, EntityPack};
use mcrs_minecraft_render_controller::RenderQuery;
use mcrs_molang::{ScalarVm, Value, string_hash};

const FROGS: usize = 10_000;

fn bench_frog_layers(c: &mut Criterion) {
    let pack = EntityPack::repository();
    let library = EntityLibrary::build(&pack.sources(), &[], 2048);
    let (definition, frog) = pack.entity("minecraft:frog").unwrap();
    let tables = &library.definitions[definition];
    let croaking = string_hash("croaking");
    let standing = string_hash("standing");
    let mut vm = ScalarVm::new();

    let mut group = c.benchmark_group("frog_layers");
    group.throughput(Throughput::Elements(FROGS as u64));
    group.bench_function("evaluate_and_hide_10k", |b| {
        b.iter(|| {
            let mut hidden = 0u16;
            for index in 0..FROGS {
                let pose = if index % 3 == 0 { croaking } else { standing };
                let choices = frog.controllers.evaluate(&mut vm, |query| match query {
                    RenderQuery::Entity(EntityQuery::Pose) => Some(Value::String(pose)),
                    _ => None,
                });
                for choice in &choices {
                    hidden ^= tables.hidden_mask(choice);
                }
            }
            black_box(hidden);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_frog_layers);
criterion_main!(benches);
