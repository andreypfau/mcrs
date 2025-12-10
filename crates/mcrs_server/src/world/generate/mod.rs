use crate::world::block::minecraft::{tnt, STONE};
use crate::world::chunk::{BiomesChunk, ChunkBlockStates};
use mcrs_protocol::ChunkPos;

pub fn generate_chunk(
    pos: ChunkPos,
    block_states: &mut ChunkBlockStates,
    biomes: &mut BiomesChunk,
) {
    if pos.y == 0 {
        if pos.x == 0 && pos.z == 0 {
            block_states.fill(tnt::UNSTABLE_STATE);
        } else {
            block_states.fill(&STONE);
        }
    }
}
