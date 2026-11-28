use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Single;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol_game::clientbound::ClientboundRespawn;

use crate::RemoteMenu;
use crate::leave_container_menu::leave_container_menu;
use crate::local_inventory::shows_inventory_menu;

pub(crate) fn respawn(
    _: On<Received<ClientboundRespawn>>,
    player: Single<(Entity, &PlayerHolders, &Menus)>,
    holders: Query<&Slots>,
    mut commands: Commands,
) {
    let (viewer, player_holders, menus) = *player;
    if menus.open().is_some() && !shows_inventory_menu(menus) {
        leave_container_menu(viewer, player_holders, &holders, commands.reborrow());
    }
    if let Some(menu) = menus.iter().next() {
        commands.entity(menu).remove::<RemoteMenu>();
    }
}
