use mcrs_minecraft_engine_world::palette::ChunkBlocks;
use mcrs_voxel::{BlockPos, BlockStateId, SectionPos};

#[derive(Clone, Debug)]
pub enum ColdLightEdit {
    Load {
        pos: SectionPos,
        blocks: ChunkBlocks,
    },
    Unload {
        pos: SectionPos,
    },
    SetBlock {
        pos: BlockPos,
        state: BlockStateId,
    },
}
