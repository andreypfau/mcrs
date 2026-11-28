use mcrs_minecraft_model::model_id::ModelId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IconLayer {
    pub model: ModelId,
    pub tints: [u32; IconLayer::MAX_TINTS],
}

impl IconLayer {
    pub const MAX_TINTS: usize = 4;
    pub const UNTINTED: u32 = 0xFFFF_FFFF;
}
