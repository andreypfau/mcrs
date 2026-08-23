use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use mcrs_anvil::fixture;
use mcrs_anvil::{BlockStateLookup, BlockStates, Properties, parse_chunk};

/// A stand-in registry, so the resolve pass does real work.
struct Names;

impl BlockStateLookup for Names {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<u32> {
        let mut id = name.len() as u32;
        for (key, value) in properties.iter() {
            id = id.wrapping_mul(31).wrapping_add((key.len() + value.len()) as u32);
        }
        Some(id)
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
                black_box(parse_chunk(nbt).expect("fixture parses"));
            }
        });
    });

    // Catches work relocated out of `parse_chunk` rather than removed.
    group.bench_function("parse_and_resolve_region", |b| {
        b.iter(|| {
            let mut cells = vec![0u32; BlockStates::ENTRY_COUNT];
            for nbt in &chunks {
                let chunk = parse_chunk(nbt).expect("fixture parses");
                for section in &chunk.sections {
                    let Some(blocks) = &section.block_states else {
                        continue;
                    };
                    let ids = blocks.resolve_palette(&Names).expect("every name resolves");
                    blocks.remap_into(&ids, &mut cells);
                    black_box(&cells);
                }
            }
        });
    });

    group.finish();
}

criterion_group!(benches, bench_decode);
criterion_main!(benches);
