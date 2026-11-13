use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClose;

use crate::PlayerHolders;
use crate::close_inventory_menu::Carried;
use crate::close_inventory_menu::CloseMove;
use crate::close_inventory_menu::Destination;
use crate::close_inventory_menu::close_inventory_menu;
use crate::player_stacks::PlayerStacks;

pub(crate) const CLOSE_REQUEST: RequestId = RequestId(3);

/// Closes the open menu of the player for each `container_close` packet, whatever its container
/// id: the moves of INV-CLS-001 become one request, and the comparison sends what they change
/// (INV-CLS-002). The open menu is always the inventory menu.
// chisle: a close with more than 128 moves rejects and leaves the cursor and the craft grid as
// they are; a request without a bound on its operations lifts it.
pub(crate) fn close_container(
    received: On<Received<ServerboundContainerClose>>,
    players: Query<(&Slots, &Selected, &PlayerHolders)>,
    holder_slots: Query<&Slots>,
    mut stacks: Stacks,
    table: Res<ItemTable>,
    mut commands: Commands,
) {
    if received.packet().is_none() {
        return;
    }
    let player = received.entity;
    let Ok((inventory, selected, holders)) = players.get(player) else {
        return;
    };
    let values = PlayerStacks::from_holders(
        inventory,
        selected.0,
        Some(holders),
        &holder_slots,
        &mut stacks,
    );
    if !values.has_carried() {
        return;
    }
    let mut request = Request::new(CLOSE_REQUEST);
    for CloseMove { from, to, count } in close_inventory_menu(&values, &table) {
        let from = match from {
            Carried::Cursor => SlotRef {
                holder: holders.cursor,
                index: 0,
            },
            Carried::CraftGrid(index) => SlotRef {
                holder: holders.craft_grid,
                index,
            },
        };
        request = match to {
            Destination::Inventory(index) => {
                let to = SlotRef {
                    holder: player,
                    index,
                };
                request.split(from, to, count)
            }
            Destination::Drop => request.drop(from, count),
        };
    }
    commands.entity(player).request(request);
}
