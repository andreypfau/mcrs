use bevy_app::{App, Plugin};
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::{Commands, Component, Entity, On, Query};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::packets::common::serverbound::ClientInformation;
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundClientInformation as ConfigurationPacket;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundClientInformation as GamePacket;
use mcrs_minecraft_protocol::setting::ChatMode;

pub struct ClientInfoPlugin;

impl Plugin for ClientInfoPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_configuration_information);
        app.add_observer(on_game_information);
    }
}

/// What the client last said about itself.
#[derive(Clone, Debug, PartialEq, Component)]
pub struct ClientInfo {
    pub locale: String,
    pub view_distance: u8,
    pub chat_mode: ChatMode,
}

fn on_configuration_information(
    on: On<Received<ConfigurationPacket<'static>>>,
    query: Query<Option<&mut ClientInfo>>,
    commands: Commands,
) {
    if let Some(packet) = on.packet() {
        update_client_info(on.entity, &packet.0, query, commands);
    }
}

fn on_game_information(
    on: On<Received<GamePacket<'static>>>,
    query: Query<Option<&mut ClientInfo>>,
    commands: Commands,
) {
    if let Some(packet) = on.packet() {
        update_client_info(on.entity, &packet.0, query, commands);
    }
}

fn update_client_info(
    entity: Entity,
    info: &ClientInformation,
    mut query: Query<Option<&mut ClientInfo>>,
    mut commands: Commands,
) {
    let Ok(held) = query.get_mut(entity) else {
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
            commands.entity(entity).insert(info);
        }
    }
}
