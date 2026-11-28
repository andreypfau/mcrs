use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_voxel::BlockPos;

use crate::identifier::Identifier;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DimensionPos {
    dimension: Identifier,
    pub pos: BlockPos,
}

impl DimensionPos {
    #[must_use]
    pub const fn new(dimension: ResourceLocationBuf, pos: BlockPos) -> Self {
        Self {
            dimension: Identifier::Owned(dimension),
            pos,
        }
    }

    #[must_use]
    pub fn dimension(&self) -> ResourceLocation<'_> {
        self.dimension.as_location()
    }
}
