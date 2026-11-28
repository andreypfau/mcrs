use bevy_ecs::entity::Entity;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Single;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerClose;

use crate::clear_first_slot::clear_first_slot;
use crate::leave_container_menu::leave_container_menu;
use crate::local_inventory::shows_inventory_menu;

pub(crate) fn container_close(
    received: On<Received<ClientboundContainerClose>>,
    player: Single<(Entity, &PlayerHolders, &Menus)>,
    holders: Query<&Slots>,
    mut commands: Commands,
) {
    if received.packet().is_none() {
        return;
    }
    let (viewer, player_holders, menus) = *player;
    if menus.open().is_none() {
        return;
    }
    if shows_inventory_menu(menus) {
        clear_first_slot(player_holders.craft_result, &holders, commands.reborrow());
    } else {
        leave_container_menu(viewer, player_holders, &holders, commands.reborrow());
    }
}
