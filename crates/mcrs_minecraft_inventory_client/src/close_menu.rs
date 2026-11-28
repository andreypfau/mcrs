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
use crate::clear_first_slot::clear_first_slot;
use crate::leave_container_menu::leave_container_menu;
use crate::local_inventory::shows_inventory_menu;
use crate::outbox::Outbox;
use crate::outgoing::Outgoing;

pub(crate) fn close_menu(
    In(viewer): In<Entity>,
    viewers: Query<(&PlayerHolders, &Menus)>,
    remotes: Query<&RemoteMenu>,
    holders: Query<&Slots>,
    mut outbox: ResMut<Outbox>,
    mut commands: Commands,
) {
    let Ok((player_holders, menus)) = viewers.get(viewer) else {
        return;
    };
    let Some(open) = menus.open() else {
        return;
    };
    if shows_inventory_menu(menus) {
        outbox.0.push(Outgoing::Close(ServerboundContainerClose {
            container_id: VarInt(RemoteMenu::INVENTORY),
        }));
        clear_first_slot(player_holders.craft_result, &holders, commands.reborrow());
        return;
    }
    if let Ok(remote) = remotes.get(open) {
        outbox.0.push(Outgoing::Close(ServerboundContainerClose {
            container_id: VarInt(remote.container_id()),
        }));
    }
    leave_container_menu(viewer, player_holders, &holders, commands.reborrow());
}
