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
    /// Set by an observer to stop the command.
    pub cancelled: bool,
}

/// Read-only: the parse of the chain belongs to this line and this source.
impl CommandEvent {
    /// The host anchor of the player who sent the line.
    pub fn player(&self) -> Entity {
        self.player
    }

    /// The source the line runs with.
    pub fn source(&self) -> &SourceState {
        &self.source
    }

    /// The command line, without its leading slash.
    pub fn line(&self) -> &str {
        &self.line
    }
}
