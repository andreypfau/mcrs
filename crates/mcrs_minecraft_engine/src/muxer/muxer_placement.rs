use bevy_ecs::resource::Resource;
use mcrs_engine_link::Placement;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MuxerPlacement(pub Placement);
