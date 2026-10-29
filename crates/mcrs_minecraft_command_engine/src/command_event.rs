use crate::source_state::SourceState;
use bevy_ecs::entity::Entity;
use bevy_ecs::event::Event;

/// Raised in the world of the source dimension before a command line runs. When an observer
/// sets `cancelled`, the command does not run and the source receives nothing.
#[derive(Event, Debug)]
pub struct CommandEvent {
    pub(crate) player: Entity,
    pub(crate) source: SourceState,
    pub(crate) line: String,
    pub cancelled: bool,
}

/// Read-only: the parse of the chain belongs to this line and this source.
impl CommandEvent {
    pub fn player(&self) -> Entity {
        self.player
    }

    pub fn source(&self) -> &SourceState {
        &self.source
    }

    pub fn line(&self) -> &str {
        &self.line
    }
}
