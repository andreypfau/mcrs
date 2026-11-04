//! The messages that the muxer sends the coordinator.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_resource::ResourceKeyBuf;

use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};

use crate::bus::{InboundPlayerPacket, PlayerTransferSnapshot};
use crate::channel_types::ToDim;

/// The input that the muxer routed to the dimension `dim` during the update, for its next tick.
/// Each class keeps the order it was sent in.
#[derive(Message)]
pub struct RoutedInput {
    pub dim: Entity,
    pub control: Vec<ToDim>,
    pub serverbound: Vec<InboundPlayerPacket>,
}

impl RoutedInput {
    /// Whether the dimension runs commands in its next tick: a command line or a suggestion
    /// request from a player, or a command run that goes on there. A tick that runs commands
    /// reads the players of every dimension.
    #[must_use]
    pub fn carries_commands(&self) -> bool {
        self.control
            .iter()
            .any(|message| matches!(message, ToDim::Command(_)))
            || self.serverbound.iter().any(|packet| {
                packet.id == ServerboundChatCommand::ID
                    || packet.id == ServerboundCommandSuggestion::ID
            })
    }
}

/// A session in the game state that has no dimension yet asks where its player spawns.
#[derive(Message, Clone, Debug)]
pub struct JoinRequested {
    pub session: PlayerSession,
    /// The dimension that the save of the player names.
    pub saved_dimension: Option<ResourceKeyBuf<DimensionKey>>,
    pub snapshot: PlayerTransferSnapshot,
}
