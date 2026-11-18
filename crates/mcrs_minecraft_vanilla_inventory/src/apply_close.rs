use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::RequestPurpose;
use crate::VanillaInventoryRequests;
use crate::carried_codec::has_carried;
use crate::close_inventory_menu::Carried;
use crate::close_inventory_menu::CloseMove;
use crate::close_inventory_menu::Destination;
use crate::close_inventory_menu::close_inventory_menu;
use crate::close_place::close_place;
use crate::close_scope::CloseScope;
use crate::from_holders::from_holders;

pub(crate) fn apply_close(
    In((player, scope)): In<(Entity, CloseScope)>,
    world: &World,
    players: Query<(&Slots, &Selected, &PlayerHolders)>,
    holder_slots: Query<&Slots>,
    mut stacks: Stacks,
    table: Res<ItemTable>,
    mut commands: Commands,
) {
    let Some(requests) = world.get_resource::<VanillaInventoryRequests>() else {
        return;
    };
    let Ok((inventory, selected, holders)) = players.get(player) else {
        return;
    };
    let mut values = from_holders(
        inventory,
        selected.0,
        Some(holders),
        &holder_slots,
        &mut stacks,
    );
    if scope == CloseScope::OtherMenu {
        values.craft_grid = Default::default();
    }
    if !has_carried(&values) {
        return;
    }
    let mut request = Request::close(RequestId {
        source: requests.source,
        serial: RequestPurpose::Close.serial(),
    });
    let place = |from, index| close_place(world, player, *holders, from, index);
    for CloseMove { from, to, count } in close_inventory_menu(&values, &table, place) {
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
