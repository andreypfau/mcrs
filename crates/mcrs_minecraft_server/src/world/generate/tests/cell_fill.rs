use bevy_math::IVec3;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::router::FINAL_DENSITY;
use mcrs_minecraft_worldgen::volume::Volume;
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::VoxelId;

use crate::world::chunk::CancellationToken;
use crate::world::generate::{ColumnBlocks, NO_TOP, fill_column_dense_any, generate_column};

use super::build_settings_router as build_router;

/// Whole-cell elimination and the sea-level split settle most of a chunk from
/// the corner lattice alone, without ever evaluating `final_density` inside
/// those cells. This pins the result to what a naive step-1 fill of every
/// section would have produced.
#[test]
fn cell_elimination_matches_the_block_by_block_fill() {
    let router = build_router("overworld", 845);
    let y_sections: Vec<i32> = (-4..20).collect();
    let (section_x, section_z) = (3, -7);
    let results = generate_column(
        section_x,
        section_z,
        &y_sections,
        &router,
        None,
        None,
        &CancellationToken::new(),
    );

    let sea_level = router.sea_level();
    let stone = router.default_block_state();
    let water = router.default_fluid_state();
    let mut ws = Workspace::new();
    let mut checked = 0usize;

    for (index, &section_y) in y_sections.iter().enumerate() {
        let (blocks, _) = results[index].as_ref().expect("the section generates");
        let volume = Volume::dense(
            IVec3::splat(16),
            IVec3::new(section_x * 16, section_y * 16, section_z * 16),
        );
        let mut density = vec![0.0f32; volume.len()];
        router.fill(&mut ws, &volume, FINAL_DENSITY, &mut density);
        for z in 0..16 {
            for x in 0..16 {
                for y in 0..16 {
                    let world_y = volume.block_y(y);
                    let expected = if density[volume.index_unchecked(x, y, z)] > 0.0 {
                        stone
                    } else if world_y < sea_level {
                        water
                    } else {
                        VoxelId(0)
                    };
                    let got = blocks.get(BlockPos::new(x, y, z));
                    assert_eq!(
                        got,
                        expected,
                        "block at ({}, {world_y}, {}) differs from the block-by-block fill",
                        volume.block_x(x),
                        volume.block_z(z),
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 24 * 16 * 16 * 16);
}

/// The fill settles each strip's highest non-air block as it writes, per cell
/// class, so a second pass over the column is never owed. Whole-cell classes
/// answer for a 4x4 footprint at once, which is the part that can be wrong
/// without any block being wrong.
#[test]
fn the_fill_records_the_top_of_every_strip() {
    let router = build_router("overworld", 845);
    let y_sections: Vec<i32> = (-4..20).collect();
    let mut column = ColumnBlocks::new(&y_sections);
    let filled = fill_column_dense_any(
        &mut column,
        3,
        -7,
        &y_sections,
        &router,
        None,
        None,
        &CancellationToken::new(),
    )
    .expect("the column is not cancelled");

    let bottom = y_sections[0] * 16;
    let top = y_sections[y_sections.len() - 1] * 16 + 15;
    let mut settled = 0usize;
    for z in 0..16 {
        for x in 0..16 {
            let expected = (bottom..=top)
                .rev()
                .find(|&y| matches!(column.get(x, y, z), Some(id) if id != VoxelId(0)))
                .unwrap_or(NO_TOP);
            assert_eq!(
                filled.tops[(z * 16 + x) as usize],
                expected,
                "strip ({x}, {z})"
            );
            settled += (expected != NO_TOP) as usize;
        }
    }
    assert_eq!(
        settled, 256,
        "every strip of an overworld column holds blocks"
    );
}
