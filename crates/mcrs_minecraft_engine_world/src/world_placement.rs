use bevy_ecs::prelude::Resource;
use mcrs_engine_link::Placement;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldPlacement(pub Placement);
