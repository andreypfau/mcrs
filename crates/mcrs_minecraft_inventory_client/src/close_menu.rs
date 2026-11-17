use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClose;

use crate::RemoteMenu;
use crate::clear_craft_result::clear_craft_result;
use crate::local_inventory::shows_inventory_menu;
use crate::outbox::Outbox;
use crate::outgoing::Outgoing;

pub(crate) fn close_menu(
    In(viewer): In<Entity>,
    viewers: Query<(&PlayerHolders, &Menus)>,
    holders: Query<&Slots>,
    mut outbox: ResMut<Outbox>,
    mut commands: Commands,
) {
    let Ok((player_holders, menus)) = viewers.get(viewer) else {
        return;
    };
    if !shows_inventory_menu(menus) {
        return;
    }
    outbox.0.push(Outgoing::Close(ServerboundContainerClose {
        container_id: VarInt(RemoteMenu::INVENTORY),
    }));
    clear_craft_result(player_holders, &holders, &mut commands);
}
