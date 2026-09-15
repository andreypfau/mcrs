use bevy_app::{App, Plugin};
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{Commands, Component, On, Query};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundClientInformation as ConfigurationPacket;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundClientInformation as GamePacket;
use mcrs_minecraft_protocol::setting::ChatMode;

pub struct ClientInfoPlugin;

impl Plugin for ClientInfoPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(update_client_info);
    }
}

/// What the client last said about itself.
#[derive(Clone, Debug, PartialEq, Component)]
pub struct ClientInfo {
    pub locale: String,
    pub view_distance: u8,
    pub chat_mode: ChatMode,
}

pub fn update_client_info(
    on: On<ReceivedPacketEvent>,
    mut query: Query<(&ConnectionState, Option<&mut ClientInfo>)>,
    mut commands: Commands,
) {
    let Ok((state, held)) = query.get_mut(on.entity) else {
        return;
    };
    let info = match state {
        ConnectionState::Login => return,
        ConnectionState::Configuration => on.decode::<ConfigurationPacket>().map(|p| p.0),
        ConnectionState::Game => on.decode::<GamePacket>().map(|p| p.0),
    };
    let Some(info) = info else {
        return;
    };
    let info = ClientInfo {
        locale: info.locale.to_string(),
        view_distance: info.view_distance,
        chat_mode: info.chat_mode,
    };
    match held {
        Some(mut held) => {
            held.set_if_neq(info);
        }
        None => {
            commands.entity(on.entity).insert(info);
        }
    }
}
