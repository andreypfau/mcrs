use bevy_math::IVec3;
use mcrs_minecraft_worldgen::density_function::{FillScratch, Volume};
use mcrs_voxel_math::BlockPos;
use mcrs_voxel_storage::VoxelId;

use crate::world::chunk::CancellationToken;
use crate::world::generate::generate_column;

use super::bench_columns::build_router;

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
        super::corpus(),
        &CancellationToken::new(),
    );

    let sea_level = router.sea_level();
    let stone = router.default_block_state();
    let water = router.default_fluid_state();
    let mut scratch = FillScratch::new();
    let mut checked = 0usize;

    for (index, &section_y) in y_sections.iter().enumerate() {
        let (blocks, _) = results[index].as_ref().expect("the section generates");
        let volume = Volume::dense(
            IVec3::splat(16),
            IVec3::new(section_x * 16, section_y * 16, section_z * 16),
        );
        let mut density = vec![0.0f32; volume.len()];
        router.sample_volume(
            router.final_density_index(),
            &volume,
            &mut density,
            &mut scratch,
        );
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
