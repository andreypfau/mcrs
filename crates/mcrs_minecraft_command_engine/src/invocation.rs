use crate::source_state::SourceState;
use bevy_ecs::entity::Entity;

/// One run of a stage for one source: the host anchor of the player who receives the output,
/// the source, and the stage whose nodes the handler reads.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub output: Entity,
    pub source: SourceState,
    pub(crate) stage: usize,
    pub(crate) branch: u16,
}
