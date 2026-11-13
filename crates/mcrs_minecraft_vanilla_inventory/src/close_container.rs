use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundContainerClose;

use crate::InventoryInput;
use crate::PlayerHolders;
use crate::close_inventory_menu::Carried;
use crate::close_inventory_menu::CloseMove;
use crate::close_inventory_menu::Destination;
use crate::close_inventory_menu::close_inventory_menu;
use crate::player_stacks::PlayerStacks;
use crate::request_ids::CLOSE_REQUEST;

/// Writes the [`InventoryInput::Close`] of each `container_close` packet.
pub(crate) fn close_container(
    received: On<Received<ServerboundContainerClose>>,
    mut inputs: MessageWriter<InventoryInput>,
) {
    if received.packet().is_none() {
        return;
    }
    inputs.write(InventoryInput::Close {
        player: received.entity,
    });
}

/// Closes the open menu of the player: the moves of INV-CLS-001 become one request, and the
/// comparison sends what they change (INV-CLS-002). The open menu is always the inventory menu.
// chisle: a close with more than 128 moves rejects and leaves the cursor and the craft grid as
// they are; a request without a bound on its operations lifts it.
pub(crate) fn apply_close(
    InRef(input): InRef<InventoryInput>,
    players: Query<(&Slots, &Selected, &PlayerHolders)>,
    holder_slots: Query<&Slots>,
    mut stacks: Stacks,
    table: Res<ItemTable>,
    mut commands: Commands,
) {
    let &InventoryInput::Close { player } = input else {
        return;
    };
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
