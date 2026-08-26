use mcrs_palette::VoxelId;

pub struct TargetBlockState {
    pub target: VoxelId,
    pub state: VoxelId,
}

pub enum OreYOffset {
    BetaPlus2,
    ModernMinus2,
}

pub struct OreConfig {
    pub targets: Vec<TargetBlockState>,
    pub size: i32,
    pub y_offset: OreYOffset,
}
