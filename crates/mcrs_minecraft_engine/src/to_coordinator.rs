use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_resource::ResourceKeyBuf;

use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};

use crate::bus::InboundPlayerPacket;
use crate::channel_types::ToDim;
use crate::column_request::ColumnRequest;
use crate::persisted_registry::PersistedSnapshot;

#[derive(Message)]
pub struct RoutedInput {
    pub dim: Entity,
    pub control: Vec<ToDim>,
    pub serverbound: Vec<InboundPlayerPacket>,
    pub column_requests: Vec<ColumnRequest>,
}

impl RoutedInput {
    #[must_use]
    pub fn new(dim: Entity) -> Self {
        Self {
            dim,
            control: Vec::new(),
            serverbound: Vec::new(),
            column_requests: Vec::new(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.control.is_empty() && self.serverbound.is_empty() && self.column_requests.is_empty()
    }

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

#[derive(Message, Clone, Debug)]
pub struct JoinRequested {
    pub session: PlayerSession,
    pub saved_dimension: Option<ResourceKeyBuf<DimensionKey>>,
    pub snapshot: PersistedSnapshot,
}
