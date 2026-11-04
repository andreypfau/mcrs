use bevy_ecs::resource::Resource;
use mcrs_engine_link::Placement;

/// Where the muxer runs. Without this resource, it runs inline on the host thread.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MuxerPlacement(pub Placement);
