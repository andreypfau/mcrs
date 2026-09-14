use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mcrs_minecraft_anvil::fixture;
use mcrs_minecraft_anvil::{PaletteLookup, Properties, parse_chunk};
use mcrs_voxel_storage::VoxelId;

/// A stand-in registry, so the resolve pass does real work.
struct Names;

impl Names {
    fn id(name: &str, properties: Properties<'_>) -> u32 {
        let mut id = name.len() as u32;
        for (key, value) in properties.iter() {
            id = id
                .wrapping_mul(31)
                .wrapping_add((key.len() + value.len()) as u32);
        }
        id
    }
}

impl PaletteLookup<VoxelId> for Names {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<VoxelId> {
        Some(VoxelId(Self::id(name, properties) as u16))
    }
}

impl PaletteLookup<u8> for Names {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<u8> {
        Some(Self::id(name, properties) as u8)
    }
}

fn bench_decode(c: &mut Criterion) {
    let chunks = fixture::region_chunks();

    let mut group = c.benchmark_group("anvil_decode");
    group.throughput(Throughput::Elements(fixture::cell_count()));
    group.sample_size(20);

    group.bench_function("parse_region", |b| {
        b.iter(|| {
            for nbt in &chunks {
                black_box(parse_chunk(nbt, &Names, &Names).expect("fixture parses"));
            }
        });
    });

    group.finish();
}

criterion_group!(benches, bench_decode);
criterion_main!(benches);
