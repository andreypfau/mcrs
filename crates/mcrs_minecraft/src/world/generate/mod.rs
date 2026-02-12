use crate::world::block::minecraft::{BEDROCK, DIRT, GRASS_BLOCK, STONE};
use crate::world::palette::{BiomePalette, BlockPalette};
use bevy_math::IVec3;
use mcrs_engine::world::block::BlockPos;
use mcrs_engine::world::chunk::ChunkPos;
use mcrs_minecraft_worldgen::density_function::NoiseRouter;

// i need layer:
// grass_block - top 1
// dirt - 3
// stone - 59
// bedrock - bottom 1
pub fn generate_chunk(pos: ChunkPos, block_states: &mut BlockPalette, _biomes: &mut BiomePalette) {
    let chunk_y_start = pos.y * 16;
    let chunk_y_end = chunk_y_start + 16;

    // Handle stone layer (y=1 to y=59)
    if chunk_y_start < 60 && chunk_y_end > 1 {
        block_states.fill(&STONE);
    }

    // Handle bedrock layer (y=0)
    if chunk_y_start <= 0 && chunk_y_end > 0 {
        for x in 0..16 {
            for z in 0..16 {
                block_states.set(BlockPos::new(x, 0, z), &BEDROCK);
            }
        }
    }

    // Handle dirt layer (y=60 to y=62)
    if chunk_y_start < 63 && chunk_y_end > 60 {
        let dirt_start = chunk_y_start.max(60);
        let dirt_end = chunk_y_end.min(63);
        for y in dirt_start..dirt_end {
            for x in 0..16 {
                for z in 0..16 {
                    block_states.set(BlockPos::new(x, y - chunk_y_start, z), &DIRT);
                }
            }
        }
    }

    // Handle grass_block layer (y=63)
    if chunk_y_start <= 63 && chunk_y_end > 63 {
        for x in 0..16 {
            for z in 0..16 {
                block_states.set(BlockPos::new(x, 63 - chunk_y_start, z), &GRASS_BLOCK);
            }
        }
    }
}

pub fn generate_noise(
    pos: ChunkPos,
    block_states: &mut BlockPalette,
    biomes: &mut BiomePalette,
    noise_router: &NoiseRouter,
) {
    let block_x = pos.x * 16;
    let block_z = pos.z * 16;
    let block_y = pos.y * 16;
    let mut cache = noise_router.new_cache();
    for x in 0..16 {
        let world_x = block_x + x;
        for z in 0..16 {
            let world_z = block_z + z;
            for y in 0..16 {
                let world_y = block_y + y;

                let density = noise_router.final_density(IVec3::new(world_x, world_y, world_z));

                if density > 0.0 {
                    block_states.set(BlockPos::new(x, y, z), &STONE);
                } else {
                    // Air block, do nothing or set to air if necessary
                }
            }
        }
    }
}
