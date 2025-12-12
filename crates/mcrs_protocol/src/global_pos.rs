use std::borrow::Cow;

use crate::{Decode, Encode};
use mcrs_engine::world::block::BlockPos;
use valence_ident::Ident;

#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode, bevy_ecs::component::Component)]
pub struct GlobalPos<'a> {
    pub dimension_name: Ident<Cow<'a, str>>,
    pub position: BlockPos,
}
