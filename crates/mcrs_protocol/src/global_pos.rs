use std::borrow::Cow;

use valence_ident::Ident;

use crate::block_pos::BlockPos;
use crate::{Decode, Encode};

#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
#[derive(bevy_ecs::component::Component)]
pub struct GlobalPos<'a> {
    pub dimension_name: Ident<Cow<'a, str>>,
    pub position: BlockPos,
}
