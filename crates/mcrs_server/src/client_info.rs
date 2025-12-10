use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Commands, Component, On, Query};
use derive_more::Deref;
use mcrs_network::ConnectionState;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::packets::configuration::serverbound::ServerboundClientInformation as ConfigurationPacket;
use mcrs_protocol::packets::game::serverbound::ServerboundClientInformation as GamePacket;

pub struct ClientInfoPlugin;

impl Plugin for ClientInfoPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(update_client_info);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Component, Deref)]
pub struct ClientLocale(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Component, Deref)]
pub struct ClientViewDistance(u8);

pub fn update_client_info(
    on: On<ReceivedPacketEvent>,
    query: Query<&ConnectionState>,
    mut commands: Commands
) {
    let Ok(state) = query.get(on.entity) else {
        return;
    };
    let info = match state {
        ConnectionState::Login => return,
        ConnectionState::Configuration => {
            on.decode::<ConfigurationPacket>().map(|p| p.0)
        }
        ConnectionState::Game => {
            on.decode::<GamePacket>().map(|p| p.0)
        }
    };
    let Some(info) = info else { return; };
    let mut entity = commands.entity(on.entity);
    entity.insert(ClientLocale(info.locale.to_string()));
    entity.insert(ClientViewDistance(info.view_distance));
}