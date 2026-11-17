use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Single;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerClose;

use crate::clear_craft_result::clear_craft_result;
use crate::local_inventory::shows_inventory_menu;

pub(crate) fn container_close(
    received: On<Received<ClientboundContainerClose>>,
    player: Single<(&PlayerHolders, &Menus)>,
    holders: Query<&Slots>,
    mut commands: Commands,
) {
    if received.packet().is_none() {
        return;
    }
    let (player_holders, menus) = *player;
    if !shows_inventory_menu(menus) {
        return;
    }
    clear_craft_result(player_holders, &holders, &mut commands);
}
