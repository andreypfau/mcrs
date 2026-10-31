use crate::HeightContext;
use mcrs_minecraft_core::codec::Validate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(remote = "Self", rename_all = "snake_case", deny_unknown_fields)]
pub enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
    RelativeToSeaLevel(i32),
}

mcrs_minecraft_core::validated!(VerticalAnchor);

impl Validate for VerticalAnchor {
    fn validate(&self) -> Result<(), String> {
        let (Self::Absolute(offset)
        | Self::AboveBottom(offset)
        | Self::BelowTop(offset)
        | Self::RelativeToSeaLevel(offset)) = *self;
        if !(Self::MIN_OFFSET..=Self::MAX_OFFSET).contains(&offset) {
            return Err(format!(
                "vertical anchor offset {offset} is outside [{};{}]",
                Self::MIN_OFFSET,
                Self::MAX_OFFSET
            ));
        }
        Ok(())
    }
}

impl VerticalAnchor {
    const PACKED_Y_BITS: u32 = 12;
    const Y_SIZE: i32 = (1 << Self::PACKED_Y_BITS) - 32;
    pub const MAX_OFFSET: i32 = (Self::Y_SIZE >> 1) - 1;
    pub const MIN_OFFSET: i32 = Self::MAX_OFFSET - Self::Y_SIZE + 1;

    pub fn resolve_y(self, context: HeightContext) -> i32 {
        match self {
            VerticalAnchor::Absolute(y) => y,
            VerticalAnchor::AboveBottom(offset) => context.min_y + offset,
            VerticalAnchor::BelowTop(offset) => context.min_y + context.depth - 1 - offset,
            VerticalAnchor::RelativeToSeaLevel(offset) => context.sea_level + offset,
        }
    }
}
