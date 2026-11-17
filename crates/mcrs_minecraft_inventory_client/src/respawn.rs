use bevy_ecs::observer::On;
use bevy_ecs::query::With;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Single;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol_game::clientbound::ClientboundRespawn;

use crate::RemoteMenu;

pub(crate) fn respawn(
    _: On<Received<ClientboundRespawn>>,
    menus: Single<&Menus, With<PlayerHolders>>,
    mut commands: Commands,
) {
    if let Some(menu) = menus.iter().next() {
        commands.entity(menu).remove::<RemoteMenu>();
    }
}
