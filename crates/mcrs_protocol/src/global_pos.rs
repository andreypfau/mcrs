use std::borrow::Cow;

use crate::{Decode, Encode};
use mcrs_core::ResourceLocation;
use mcrs_voxel_math::BlockPos;

#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub struct GlobalPos<'a> {
    pub dimension_name: ResourceLocation<Cow<'a, str>>,
    pub position: BlockPos,
}
