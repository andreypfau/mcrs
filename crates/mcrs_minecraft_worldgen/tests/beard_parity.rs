//! The beard term against the reference's `Beardifier` over synthetic piece sets:
//! the kernel entry by entry, then every point of each case's volume through both
//! the point sampler and the volume fill.

use bevy_math::IVec3;
use bytes::{Buf, Bytes};
use mcrs_minecraft_core::{BlockPos, BoundingBox};
use mcrs_minecraft_worldgen::beard::{Beard, JunctionPoint, KERNEL, KERNEL_LEN, Rigid};
use mcrs_minecraft_worldgen_noise::SampleGrid;
use mcrs_minecraft_worldgen_structure::TerrainAdaptation;
use mcrs_minecraft_worldgen_testing::{dump_string, open_dump};
use std::path::PathBuf;

const MAGIC: &[u8; 8] = b"MCBEARD0";

struct Case {
    name: String,
    beard: Beard,
    grid: SampleGrid,
    expected: Vec<f32>,
}

fn read_ivec3(r: &mut Bytes) -> IVec3 {
    IVec3::new(r.get_i32_le(), r.get_i32_le(), r.get_i32_le())
}

fn read_box(r: &mut Bytes) -> BoundingBox {
    let min = BlockPos::from(read_ivec3(r));
    let max = BlockPos::from(read_ivec3(r));
    BoundingBox { min, max }
}

fn adaptation(tag: u8) -> TerrainAdaptation {
    match tag {
        0 => TerrainAdaptation::None,
        1 => TerrainAdaptation::Bury,
        2 => TerrainAdaptation::BeardThin,
        3 => TerrainAdaptation::BeardBox,
        4 => TerrainAdaptation::Encapsulate,
        other => panic!("unknown terrain adjustment tag {other}"),
    }
}

fn read_dump() -> (Vec<f32>, Vec<Case>) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vanilla/beard.bin");
    let mut r = open_dump(&path, MAGIC);

    let kernel_len = r.get_u32_le() as usize;
    let dumped_kernel = (0..kernel_len).map(|_| r.get_f32_le()).collect();

    let cases = (0..r.get_u32_le())
        .map(|_| {
            let name = dump_string(&mut r);
            let rigids = (0..r.get_u32_le())
                .map(|_| {
                    let bounds = read_box(&mut r);
                    let adaptation = adaptation(r.get_u8());
                    Rigid {
                        bounds,
                        adaptation,
                        ground_level_delta: r.get_i32_le(),
                    }
                })
                .collect();
            let junctions = (0..r.get_u32_le())
                .map(|_| {
                    let (x, ground_y, z) = (r.get_i32_le(), r.get_i32_le(), r.get_i32_le());
                    let _delta_y = r.get_i32_le();
                    let projection = r.get_u8();
                    assert!(
                        projection <= 1,
                        "{name}: unknown projection tag {projection}"
                    );
                    JunctionPoint { x, ground_y, z }
                })
                .collect();
            let affected = match r.get_u8() {
                0 => None,
                1 => Some(read_box(&mut r)),
                other => panic!("{name}: bad affected-box flag {other}"),
            };
            let min = read_ivec3(&mut r);
            let size = read_ivec3(&mut r);
            let step = read_ivec3(&mut r);
            let grid = SampleGrid::new(size, min, step);
            let mut expected = vec![0.0f32; grid.len()];
            for z in 0..size.z {
                for x in 0..size.x {
                    for y in 0..size.y {
                        expected[grid.index_unchecked(x, y, z)] = r.get_f32_le();
                    }
                }
            }
            Case {
                name,
                beard: Beard {
                    rigids,
                    junctions,
                    affected,
                },
                grid,
                expected,
            }
        })
        .collect();
    assert!(!r.has_remaining(), "trailing bytes in {}", path.display());
    (dumped_kernel, cases)
}

#[test]
#[ignore = "reference parity check; run with --ignored"]
fn kernel_matches_the_reference_bit_for_bit() {
    let (dumped, _) = read_dump();
    assert_eq!(dumped.len(), KERNEL_LEN);
    let ours = &*KERNEL;
    let mismatches: Vec<_> = (0..KERNEL_LEN)
        .filter(|&i| ours[i].to_bits() != dumped[i].to_bits())
        .collect();
    if let Some(&first) = mismatches.first() {
        let (zi, xi, yi) = (first / 576, first / 24 % 24, first % 24);
        panic!(
            "{} of {KERNEL_LEN} kernel entries differ; first at zi={zi} xi={xi} yi={yi}: ours {:e} ({:#010x}), reference {:e} ({:#010x})",
            mismatches.len(),
            ours[first],
            ours[first].to_bits(),
            dumped[first],
            dumped[first].to_bits(),
        );
    }
}

#[test]
#[ignore = "reference parity check; run with --ignored"]
fn sample_and_fill_match_the_reference_bit_for_bit() {
    let (_, cases) = read_dump();
    assert!(!cases.is_empty(), "the dump holds no cases");
    let mut failures = Vec::new();
    for case in &cases {
        let grid = case.grid;
        let mut filled = vec![0.0f32; grid.len()];
        case.beard.fill(&grid, &mut filled);

        for (label, produce) in [
            (
                "sample",
                &(|x, y, z| case.beard.sample(x, y, z)) as &dyn Fn(i32, i32, i32) -> f32,
            ),
            ("fill", &|x, y, z| {
                let index = grid
                    .index_of_block(x, y, z)
                    .expect("a lattice point of the grid");
                filled[index]
            }),
        ] {
            let mut count = 0usize;
            let mut first = None;
            for z in 0..grid.size().z {
                for x in 0..grid.size().x {
                    for y in 0..grid.size().y {
                        let (bx, by, bz) = (grid.block_x(x), grid.block_y(y), grid.block_z(z));
                        let ours = produce(bx, by, bz);
                        let reference = case.expected[grid.index_unchecked(x, y, z)];
                        if ours.to_bits() != reference.to_bits() {
                            count += 1;
                            first.get_or_insert((bx, by, bz, ours, reference));
                        }
                    }
                }
            }
            if let Some((bx, by, bz, ours, reference)) = first {
                failures.push(format!(
                    "{} ({label}): {count} of {} points differ; first at ({bx}, {by}, {bz}): ours {ours:e} ({:#010x}), reference {reference:e} ({:#010x})",
                    case.name,
                    grid.len(),
                    ours.to_bits(),
                    reference.to_bits(),
                ));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
