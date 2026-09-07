use criterion::{Criterion, criterion_group, criterion_main};
use mcrs_minecraft_server::world::chunk::CancellationToken;
use mcrs_minecraft_server::world::generate::generate_column;
use mcrs_voxel_math::BlockPos;
use std::hint::black_box;

#[path = "../src/world/generate/tests/support.rs"]
mod support;

use support::build_settings_router;

/// Full overworld dimension: y -64..320.
fn y_sections() -> Vec<i32> {
    (-4..20).collect()
}

/// Content fingerprint over a few columns, read back per position rather than
/// over the packed palette, which also encodes the order blocks were written
/// in. Printed so before/after runs can be compared for identical output.
fn checksum(label: &str, router: &mcrs_minecraft_worldgen::router::NoiseRouter) {
    let cancel = CancellationToken::new();
    let y = y_sections();
    let mut checksum = 0u64;
    for i in 0..4 {
        let results = generate_column(i, -i, &y, router, None, &cancel);
        for (blocks, _) in results.iter().flatten() {
            for y in 0..16 {
                for z in 0..16 {
                    for x in 0..16 {
                        let id = blocks.get(BlockPos::new(x, y, z));
                        checksum = checksum
                            .wrapping_mul(1099511628211)
                            .wrapping_add(id.0 as u64);
                    }
                }
            }
        }
    }
    println!("[{label}] checksum={checksum:#x}");
}

fn bench_settings(c: &mut Criterion, label: &str) {
    let router = build_settings_router(label, 845);
    checksum(label, &router);

    let cancel = CancellationToken::new();
    let y = y_sections();
    let mut group = c.benchmark_group("generate_column");
    group.sample_size(10);
    group.bench_function(label, |b| {
        let mut i = 0i32;
        b.iter(|| {
            let (cx, cz) = (i % 8, i / 8);
            i = (i + 1) % 64;
            black_box(generate_column(cx, cz, &y, &router, None, &cancel))
        });
    });
    group.finish();
}

fn bench_beta(c: &mut Criterion) {
    bench_settings(c, "beta");
}

fn bench_overworld(c: &mut Criterion) {
    bench_settings(c, "overworld");
}

criterion_group!(benches, bench_beta, bench_overworld);
criterion_main!(benches);
