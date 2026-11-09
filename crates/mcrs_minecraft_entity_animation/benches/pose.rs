#![expect(clippy::unwrap_used, reason = "a benchmark with a broken setup stops")]
#![expect(
    clippy::indexing_slicing,
    reason = "a benchmark with a broken setup stops"
)]

use std::hint::black_box;
use std::path::Path;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mcrs_minecraft_entity_animation::{
    ClientEntityDefinition, ClipLibrary, EntityAnimation, EntityQuery, Pose, PoseBinding,
    QuerySource, Rig,
};
use mcrs_minecraft_entity_model::GeometryFile;
use mcrs_molang::{ScalarVm, Value};

const PIGS: usize = 10_000;

#[derive(Clone, Copy)]
struct PigInputs {
    walk_position: f32,
    walk_speed: f32,
    yaw: f32,
    pitch: f32,
}

impl QuerySource for PigInputs {
    fn query(&self, query: EntityQuery) -> Option<Value> {
        let value = match query {
            EntityQuery::ModifiedDistanceMoved => self.walk_position,
            EntityQuery::ModifiedMoveSpeed => self.walk_speed,
            EntityQuery::TargetXRotation => self.pitch,
            EntityQuery::TargetYRotation => self.yaw,
            _ => return None,
        };
        Some(Value::Number(value))
    }
}

fn read(path: &str) -> String {
    let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mcrs/entity");
    std::fs::read_to_string(pack.join(path)).unwrap()
}

fn bench_pig(c: &mut Criterion) {
    let definition = ClientEntityDefinition::parse(&read("entity/pig.entity.json")).unwrap();
    let mut library = ClipLibrary::new();
    library
        .add_file(&read("animations/pig.animation.json"))
        .unwrap();
    let rig = Rig::compile(&definition, &library, &mut Vec::new());
    let file: GeometryFile = read("geometry/pig.json").parse().unwrap();
    let geometry = file.geometries[0].bake(&mut Vec::new()).unwrap();
    let binding = PoseBinding::new(&rig, &geometry);

    let sources: Vec<PigInputs> = (0..PIGS)
        .map(|index| PigInputs {
            walk_position: index as f32 * 0.37,
            walk_speed: (index % 7) as f32 / 7.0,
            yaw: (index % 90) as f32 - 45.0,
            pitch: (index % 60) as f32 - 30.0,
        })
        .collect();
    let mut entities = vec![EntityAnimation::new(&rig); PIGS];
    let mut poses = vec![Pose::new(&rig); PIGS];
    let mut clock = 0.0f32;

    let mut group = c.benchmark_group("pig_pose");
    group.throughput(Throughput::Elements(PIGS as u64));

    let mut vm = ScalarVm::new();
    group.bench_function("scalar_10k", |b| {
        b.iter(|| {
            clock += 1.0;
            for ((entity, source), pose) in entities.iter_mut().zip(&sources).zip(&mut poses) {
                entity.update(&rig, &mut vm, clock, source);
                entity.pose(&rig, &mut vm, &binding, pose);
            }
            black_box(&poses);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_pig);
criterion_main!(benches);
