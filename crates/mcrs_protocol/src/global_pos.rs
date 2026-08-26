use std::borrow::Cow;

use crate::{Decode, Encode};
use mcrs_engine::world::block::BlockPos;
use mcrs_core::ResourceLocation;

#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub struct GlobalPos<'a> {
    pub dimension_name: ResourceLocation<Cow<'a, str>>,
    pub position: BlockPos,
}
