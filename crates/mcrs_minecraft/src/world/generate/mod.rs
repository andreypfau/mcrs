use crate::world::block::minecraft::STONE;
use crate::world::palette::{BiomePalette, BlockPalette};
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_worldgen::density_function::{
    ChunkColumnCache, NoiseRouter, SectionInterpolator,
};

/// Generate a single section using a pre-populated column cache and interpolator.
/// The column cache and interpolator are passed in so they can be reused across
/// multiple Y sections in the same column.
///
/// Uses `fill_plane_cached_reuse` for Y-boundary sharing: the top-Y row of the
/// previous section is reused as the bottom-Y row of this section, eliminating
/// ~33% of density evaluations for all sections after the first.
fn generate_section(
    block_x: i32,
    block_y: i32,
    block_z: i32,
    block_states: &mut BlockPalette,
    noise_router: &NoiseRouter,
    column_cache: &mut ChunkColumnCache,
    interp: &mut SectionInterpolator,
) {
    let h_cell_blocks = interp.h_cell_blocks();
    let v_cell_blocks = interp.v_cell_blocks();
    let h_cells = interp.h_cells();
    let v_cells = interp.v_cells();

    // Fill the initial X start plane using column cache (with Y-boundary reuse)
    interp.fill_plane_cached_reuse(
        0,
        true,
        block_x,
        block_y,
        block_z,
        noise_router,
        column_cache,
    );

    for cell_x in 0..h_cells {
        // Fill end plane at x = block_x + (cell_x + 1) * h_cell_blocks
        let next_x = block_x + ((cell_x + 1) * h_cell_blocks) as i32;
        interp.fill_plane_cached_reuse(
            cell_x + 1,
            false,
            next_x,
            block_y,
            block_z,
            noise_router,
            column_cache,
        );

        for cell_z in 0..h_cells {
            for cell_y in (0..v_cells).rev() {
                interp.on_sampled_cell_corners(cell_y, cell_z);

                // Fast path: if all 8 corners agree on sign, skip interpolation
                match interp.corners_uniform_sign() {
                    Some(false) => {
                        // All air — BlockPalette defaults to air, nothing to do
                        continue;
                    }
                    Some(true) => {
                        // All solid — fill the entire cell with stone
                        let bx_base = (cell_x * h_cell_blocks) as i32;
                        let by_base = (cell_y * v_cell_blocks) as i32;
                        let bz_base = (cell_z * h_cell_blocks) as i32;
                        for ly in 0..v_cell_blocks as i32 {
                            for lx in 0..h_cell_blocks as i32 {
                                for lz in 0..h_cell_blocks as i32 {
                                    block_states.set(
                                        BlockPos::new(bx_base + lx, by_base + ly, bz_base + lz),
                                        &STONE,
                                    );
                                }
                            }
                        }
                        continue;
                    }
                    None => {}
                }

                for local_y in (0..v_cell_blocks).rev() {
                    let delta_y = local_y as f32 / v_cell_blocks as f32;
                    interp.interpolate_y(delta_y);

                    for local_x in 0..h_cell_blocks {
                        let delta_x = local_x as f32 / h_cell_blocks as f32;
                        interp.interpolate_x(delta_x);

                        for local_z in 0..h_cell_blocks {
                            let delta_z = local_z as f32 / h_cell_blocks as f32;
                            interp.interpolate_z(delta_z);

                            let density = interp.result();

                            if density > 0.0 {
                                let bx = (cell_x * h_cell_blocks + local_x) as i32;
                                let by = (cell_y * v_cell_blocks + local_y) as i32;
                                let bz = (cell_z * h_cell_blocks + local_z) as i32;
                                block_states.set(BlockPos::new(bx, by, bz), &STONE);
                            }
                        }
                    }
                }
            }
        }

        interp.swap_buffers();
    }

    // Mark section complete so the next section can reuse our top-Y row
    interp.end_section();
}

/// Generate all sections in a column using a pre-populated ChunkColumnCache.
/// Zone A (column-only density functions) is computed once for all 17x17 XZ positions
/// and reused across all Y sections, eliminating per-block column-change branches.
///
/// Adjacent Y sections share cell corners at their boundary via Y-boundary reuse,
/// eliminating ~33% of density evaluations for all sections after the first.
pub fn generate_column(
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    noise_router: &NoiseRouter,
) -> Vec<(BlockPalette, BiomePalette)> {
    let mut interp = noise_router.new_section_interpolator();
    let block_x = section_x * 16;
    let block_z = section_z * 16;

    // Pre-populate Zone A values for all 17x17 XZ positions in one pass
    let mut column_cache = noise_router.new_column_cache(block_x, block_z);
    noise_router.populate_columns(&mut column_cache);

    y_sections
        .iter()
        .map(|&sy| {
            let mut blocks = BlockPalette::default();
            let biomes = BiomePalette::default();
            generate_section(
                block_x,
                sy * 16,
                block_z,
                &mut blocks,
                noise_router,
                &mut column_cache,
                &mut interp,
            );
            (blocks, biomes)
        })
        .collect()
}
