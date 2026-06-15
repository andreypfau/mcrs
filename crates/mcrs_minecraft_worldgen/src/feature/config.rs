use mcrs_protocol::BlockStateId;

pub struct TargetBlockState {
    pub target: BlockStateId,
    pub state: BlockStateId,
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
