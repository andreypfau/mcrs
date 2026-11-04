use bevy_ecs::prelude::Resource;
use mcrs_engine_link::Placement;

/// Where each dimension world of the host ticks.
///
/// [`Placement::Inline`] ticks every world on the server thread, one after another, and each
/// schedule runs one system at a time. [`Placement::Thread`] ticks each world on a thread of its
/// own, and a schedule runs in parallel the systems whose access does not conflict. World
/// generation, lighting and IO keep their own pools under either placement. Without this
/// resource, the worlds tick inline.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldPlacement(pub Placement);
