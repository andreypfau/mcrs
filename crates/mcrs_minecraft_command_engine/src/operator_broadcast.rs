use bevy_ecs::entity::Entity;
use bevy_ecs::event::Event;
use mcrs_minecraft_text::Text;

/// A command reported success with a message for the operators, triggered on
/// the host world. The embedder must observe it there and deliver `message` to each operator but
/// `source`; the engine knows no operator list, so without that observer no other operator sees
/// the message. The source still receives its own success message.
#[derive(Event, Clone, Debug)]
pub struct OperatorBroadcast {
    /// The host anchor of the player that ran the command.
    pub source: Entity,
    /// The message, already in the `chat.type.admin` form.
    pub message: Text,
}
