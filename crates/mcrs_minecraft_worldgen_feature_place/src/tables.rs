use rustc_hash::FxHashMap as HashMap;

use mcrs_minecraft_chunk::VoxelId;

use crate::huge_mushroom::MushroomFaces;
use crate::mossy_carpet::MossyCarpetStates;
use crate::patch::Waterlogging;
use crate::simple_block::DoublePlant;

#[derive(Clone, Debug, Default)]
pub struct BlockTables {
    pub double_plants: HashMap<u16, DoublePlant>,
    pub mossy_carpet: Option<MossyCarpetStates>,
    pub snowy: HashMap<VoxelId, VoxelId>,
    pub waterlogging: Waterlogging,
    pub vine: Option<[VoxelId; 6]>,
    pub mushroom_faces: MushroomFaces,
}
