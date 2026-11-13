use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::read_wire_with_length;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_slots::Holder;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCreativeModeSlot;
use mcrs_minecraft_wire::Encode;

use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::creative_drops::CreativeDrops;
use crate::insert_stack::max_stack_size;
use crate::launch_drops::launch_from_player;
use crate::menu_queue::MenuQueue;

const CREATIVE_REQUEST: RequestId = RequestId(4);

/// Applies each `set_creative_mode_slot` packet that passes the checks of INV-CRE-003: a set of
/// an entry of the inventory menu (INV-CRE-004), or a creative drop (IEN-SPN-007). A packet that
/// fails a check is a claim of the client for its entry (INV-CRE-006).
#[expect(
    clippy::type_complexity,
    reason = "one query reads each part of the player that the packet needs"
)]
pub(crate) fn set_creative_mode_slot(
    received: On<Received<ServerboundSetCreativeModeSlot>>,
    mut players: Query<(
        &PlayerHolders,
        &mut InventoryMenuSync,
        Option<&PlayerGameMode>,
        Option<&CreativeDrops>,
        Option<&Transform>,
        Option<&GameProfile>,
    )>,
    table: Res<ItemTable>,
    clock: Res<BirthClock>,
    mut queue: ResMut<MenuQueue>,
    mut commands: Commands,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    let player = received.entity;
    let Ok((holders, mut sync, game_mode, drops, transform, profile)) = players.get_mut(player)
    else {
        return;
    };
    let mut bytes = Vec::new();
    if packet.item.encode(&mut bytes).is_err() {
        return;
    }
    // STK-NET-009: a stack without a saved form is a decode error of the packet.
    let Ok(stack) = read_wire_with_length(&mut bytes.as_slice(), &*table) else {
        return;
    };
    let creative = game_mode.is_some_and(|game_mode| game_mode.0 == GameMode::Creative);
    let fits =
        (stack.as_ref()).is_none_or(|stack| stack.count.get() <= max_stack_size(stack, &table));
    let entry = u16::try_from(packet.slot)
        .ok()
        .filter(|index| (1..InventoryMenu::LEN).contains(index))
        .and_then(InventoryMenu::slot);
    if creative && fits && packet.slot < 0 {
        let mut drops = drops.copied().unwrap_or_default();
        let thrown = drops.try_drop(clock.0);
        commands.entity(player).insert(drops);
        if thrown && let Some(stack) = stack {
            let mut entity = commands.spawn_stack(stack);
            launch_from_player(&mut entity, transform, profile, Cause::Throw);
        }
        return;
    }
    if creative
        && fits
        && let Some(entry) = entry
    {
        let holder = match entry.holder {
            Holder::Inventory => player,
            Holder::Cursor => holders.cursor,
            Holder::CraftGrid => holders.craft_grid,
            Holder::CraftResult => holders.craft_result,
        };
        let slot = SlotRef {
            holder,
            index: entry.index,
        };
        let request = Request::new(CREATIVE_REQUEST);
        let request = match &stack {
            Some(stack) => {
                let count = stack.count.get();
                let stack = commands.spawn_stack(stack.clone()).id();
                request.set(slot, Some(stack), count)
            }
            None => request.set(slot, None, 0),
        };
        commands.entity(player).request(request);
        set_view(&mut sync, packet.slot, stack);
        return;
    }
    if set_view(&mut sync, packet.slot, stack) {
        queue.compare.push(player);
    }
}

/// Makes `stack` the client view of the entry `slot`. Gives false when the inventory menu has no
/// such entry.
fn set_view(sync: &mut InventoryMenuSync, slot: i16, stack: Option<ItemStack>) -> bool {
    let view = usize::try_from(slot)
        .ok()
        .and_then(|index| sync.view.entry_mut(index));
    let Some(view) = view else {
        return false;
    };
    *view = SlotView::Stack(stack);
    true
}
