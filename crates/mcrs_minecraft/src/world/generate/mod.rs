use crate::world::block::minecraft::{STONE, tnt};
use crate::world::palette::{BiomePalette, BlockPalette};
use mcrs_engine::world::chunk::ChunkPos;

pub fn generate_chunk(pos: ChunkPos, block_states: &mut BlockPalette, biomes: &mut BiomePalette) {
    if pos.y == 0 {
        if pos.x == 0 && pos.z == 0 {
            block_states.fill(tnt::UNSTABLE_STATE);
        } else {
            block_states.fill(&STONE);
        }
    }
}
