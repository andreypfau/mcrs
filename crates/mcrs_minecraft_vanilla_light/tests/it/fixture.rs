use std::sync::Arc;

use mcrs_minecraft_chunk::{PalettedContainer, VoxelId, VoxelPalette};
use mcrs_minecraft_light_engine::prelude::*;
use mcrs_voxel::BlockStateId;

pub const AIR: VoxelId = VoxelId(0);
const STONE: VoxelId = VoxelId(1);
pub const TORCH: VoxelId = VoxelId(2);

pub fn state(block: VoxelId) -> BlockStateId {
    BlockStateId::checked(block.0, 3).expect("a block of the test table")
}

pub fn filled(block: VoxelId) -> SectionBlocks {
    VoxelPalette(PalettedContainer::Homogeneous(state(block)))
}

pub fn registry() -> Arc<LightRegistry> {
    Arc::new(LightRegistry::new(
        vec![
            LightProperties::AIR,
            LightProperties::SOLID,
            LightProperties::emitter(14),
        ],
        SpecialBlocks {
            unloaded: STONE,
            outside: AIR,
        },
    ))
}
