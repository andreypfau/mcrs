use crate::world::block::minecraft::STONE;
use crate::world::palette::{BiomePalette, BlockPalette};
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::ChunkPos;
use mcrs_minecraft_worldgen::density_function::NoiseRouter;

pub fn generate_noise(
    pos: ChunkPos,
    block_states: &mut BlockPalette,
    _biomes: &mut BiomePalette,
    noise_router: &NoiseRouter,
) {
    let block_x = pos.x * 16;
    let block_z = pos.z * 16;
    let block_y = pos.y * 16;

    let mut cache = noise_router.new_cache();

    let mut interp = noise_router.new_section_interpolator();
    let h_cell_blocks = interp.h_cell_blocks();
    let v_cell_blocks = interp.v_cell_blocks();
    let h_cells = interp.h_cells();
    let v_cells = interp.v_cells();

    // Fill the initial X start plane
    interp.fill_plane(true, block_x, block_y, block_z, noise_router, &mut cache);

    for cell_x in 0..h_cells {
        // Fill end plane at x = block_x + (cell_x + 1) * h_cell_blocks
        let next_x = block_x + ((cell_x + 1) * h_cell_blocks) as i32;
        interp.fill_plane(false, next_x, block_y, block_z, noise_router, &mut cache);

        for cell_z in 0..h_cells {
            for cell_y in (0..v_cells).rev() {
                interp.on_sampled_cell_corners(cell_y, cell_z);

                for local_y in (0..v_cell_blocks).rev() {
                    let delta_y = local_y as f64 / v_cell_blocks as f64;
                    interp.interpolate_y(delta_y);

                    for local_x in 0..h_cell_blocks {
                        let delta_x = local_x as f64 / h_cell_blocks as f64;
                        interp.interpolate_x(delta_x);

                        for local_z in 0..h_cell_blocks {
                            let delta_z = local_z as f64 / h_cell_blocks as f64;
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
}
