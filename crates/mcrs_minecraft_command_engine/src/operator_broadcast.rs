use bevy_ecs::entity::Entity;
use bevy_ecs::event::Event;
use mcrs_minecraft_protocol::text::Text;

/// A command reported success with a message for the operators (CMDP-EXEC-004). The host,
/// which knows every online player and the operators, delivers it to each operator but `source`.
#[derive(Event, Clone, Debug)]
pub struct OperatorBroadcast {
    /// The host anchor of the player that ran the command.
    pub source: Entity,
    pub message: Text,
}
