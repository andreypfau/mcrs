//! Wall-clock benchmarks for column generation. Run with:
//!   cargo test -p mcrs_minecraft_server --release bench_ -- --ignored --nocapture
use mcrs_voxel_math::BlockPos;
use std::time::Instant;

use mcrs_minecraft_worldgen::interval::Interval;
use mcrs_minecraft_worldgen::router::NoiseRouter;

use crate::world::chunk::CancellationToken;
use crate::world::generate::{CellFill, CellLattice, FillBuffers, generate_column};

use super::build_settings_router as build_router;

fn summarize(label: &str, rep: usize, mut ms: Vec<f64>) {
    let n = ms.len();
    let total: f64 = ms.iter().sum();
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pick = |q: f64| ms[(((n - 1) as f64) * q).round() as usize];
    println!(
        "[{label}] rep {rep}: n={n} total={total:.1}ms mean={:.3} median={:.3} p95={:.3} min={:.3} max={:.3} ms/col  ({:.1} col/s)",
        total / n as f64,
        pick(0.5),
        pick(0.95),
        ms[0],
        ms[n - 1],
        n as f64 * 1000.0 / total,
    );
}

fn bench_columns(label: &str, router: &NoiseRouter, columns: i32, reps: usize) {
    // Full overworld dimension: y -64..320 (24 sections)
    let y_sections: Vec<i32> = (-4..20).collect();
    let cancel = CancellationToken::new();

    for i in 0..8 {
        std::hint::black_box(generate_column(
            1000 + i,
            1000,
            &y_sections,
            router,
            None,
            super::corpus(),
            &cancel,
        ));
    }

    for rep in 0..reps {
        let mut samples = Vec::with_capacity(columns as usize);
        for i in 0..columns {
            let (cx, cz) = (i % 8, i / 8);
            let t = Instant::now();
            let results =
                generate_column(cx, cz, &y_sections, router, None, super::corpus(), &cancel);
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(&results);
        }
        summarize(label, rep, samples);
    }

    // Content checksum over a few columns (outside the timed loop) so that
    // before/after runs can be compared for bit-identical block output. Read
    // back per position rather than over the packed palette, which encodes the
    // order the blocks were written in as well as what they are.
    let mut checksum = 0u64;
    for i in 0..4 {
        let results = generate_column(i, -i, &y_sections, router, None, super::corpus(), &cancel);
        for r in results.iter().flatten() {
            let (blocks, _) = r;
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

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[test]
#[ignore]
fn bench_beta_columns() {
    let router = build_router("beta", 845);
    bench_columns(
        "beta",
        &router,
        env_usize("MCRS_BENCH_COLS", 64) as i32,
        env_usize("MCRS_BENCH_REPS", 5),
    );
}

#[test]
#[ignore]
fn bench_overworld_columns() {
    let router = build_router("overworld", 845);
    bench_columns(
        "overworld",
        &router,
        env_usize("MCRS_BENCH_COLS", 64) as i32,
        env_usize("MCRS_BENCH_REPS", 5),
    );
}

/// How many of a chunk's cells the corner bounds settle without any per-block
/// density evaluation, and what that leaves for the block-by-block fill.
fn census(label: &str, router: &NoiseRouter, columns: i32) {
    let mut counts = [0usize; 5];
    let mut fill = FillBuffers::default();
    let sea_level = router.sea_level();
    for i in 0..columns {
        let (cx, cz) = (i % 8, i / 8);
        let lattice = CellLattice::fill(router, cx * 16, cz * 16, &mut fill.ws)
            .expect("the router has a cell lattice");
        fill.corners.resize(lattice.width, Interval::exact(0.0));
        for z in 0..lattice.volume.size().z - 1 {
            for x in 0..lattice.volume.size().x - 1 {
                for y in 0..lattice.volume.size().y - 1 {
                    let at = bevy_math::IVec3::new(x, y, z);
                    let slot = match lattice.classify(router, at, sea_level, &mut fill) {
                        CellFill::Solid => 0,
                        CellFill::Fluid => 1,
                        CellFill::Air => 2,
                        CellFill::Sea => 3,
                        CellFill::Mixed => 4,
                    };
                    counts[slot] += 1;
                }
            }
        }
    }
    let lattice = CellLattice::fill(router, 0, 0, &mut fill.ws).unwrap();
    println!(
        "[{label}] lattice: {} wrappers x {} corners = {} evaluations/chunk",
        lattice.width,
        lattice.volume.len(),
        lattice.width * lattice.volume.len(),
    );
    let total = counts.iter().sum::<usize>();
    let per_chunk = total as f64 / columns as f64;
    let settled = counts[0] + counts[1] + counts[2] + counts[3];
    let cell = router.cell_size().expect("the router has a cell lattice");
    let blocks_per_cell = (cell.x * cell.y * cell.z) as usize;
    println!(
        "[{label}] {columns} chunks: {per_chunk} cells/chunk, settled {:.1} ({:.2}%) solid={} fluid={} air={} sea={} mixed={}",
        settled as f64 / columns as f64,
        100.0 * settled as f64 / total as f64,
        counts[0],
        counts[1],
        counts[2],
        counts[3],
        counts[4],
    );
    println!(
        "[{label}] per-block density evaluations: {} of {} ({:.2}% avoided)",
        counts[4] * blocks_per_cell,
        total * blocks_per_cell,
        100.0 * settled as f64 / total as f64,
    );
}

#[test]
#[ignore]
fn cell_elimination_census() {
    census("overworld", &build_router("overworld", 845), 16);
}
