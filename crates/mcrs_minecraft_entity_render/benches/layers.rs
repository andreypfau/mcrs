#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a benchmark with a broken setup stops"
)]

use std::hint::black_box;
use std::path::Path;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mcrs_minecraft_entity_animation::EntityQuery;
use mcrs_minecraft_entity_render::{EntityLibrary, EntityPack};
use mcrs_minecraft_render_controller::RenderQuery;
use mcrs_molang::{ScalarVm, Value, string_hash};

const FROGS: usize = 10_000;

fn pack_files() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mcrs/entity");
    let mut files = Vec::new();
    for folder in ["entity", "animations", "geometry", "render_controllers"] {
        for entry in std::fs::read_dir(root.join(folder)).unwrap().flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                let name = format!("{folder}/{}", path.file_name().unwrap().to_string_lossy());
                files.push((name, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    files
}

fn bench_frog_layers(c: &mut Criterion) {
    let files = pack_files();
    let pack = EntityPack::load(
        files
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    );
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
                    hidden ^= tables.hidden_mask(&choice.part_visibility);
                }
            }
            black_box(hidden);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_frog_layers);
criterion_main!(benches);
