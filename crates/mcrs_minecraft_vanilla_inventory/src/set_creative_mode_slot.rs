use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Local;
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

use crate::InventoryInput;
use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::creative_drops::CreativeDrops;
use crate::insert_stack::max_stack_size;
use crate::launch_drops::launch_from_player;
use crate::menu_queue::MenuQueue;

const CREATIVE_REQUEST: RequestId = RequestId(4);

/// Writes the [`InventoryInput::CreativeSet`] of each `set_creative_mode_slot` packet whose stack
/// decodes.
pub(crate) fn set_creative_mode_slot(
    received: On<Received<ServerboundSetCreativeModeSlot>>,
    table: Res<ItemTable>,
    mut inputs: MessageWriter<InventoryInput>,
    mut bytes: Local<Vec<u8>>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    bytes.clear();
    if packet.item.encode(&mut *bytes).is_err() {
        return;
    }
    // STK-NET-009: a stack without a saved form is a decode error of the packet.
    let Ok(stack) = read_wire_with_length(&mut bytes.as_slice(), &*table) else {
        return;
    };
    inputs.write(InventoryInput::CreativeSet {
        player: received.entity,
        slot: packet.slot,
        stack,
    });
}

/// Applies a creative set that passes the checks of INV-CRE-003: a set of an entry of the
/// inventory menu (INV-CRE-004), or a creative drop (IEN-SPN-007). A set that fails a check is a
/// claim of the client for its entry (INV-CRE-006).
#[expect(
    clippy::type_complexity,
    reason = "one query reads each part of the player that the packet needs"
)]
pub(crate) fn apply_creative_set(
    InRef(input): InRef<InventoryInput>,
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
    let &InventoryInput::CreativeSet {
        player,
        slot,
        ref stack,
    } = input
    else {
        return;
    };
    let Ok((holders, mut sync, game_mode, drops, transform, profile)) = players.get_mut(player)
    else {
        return;
    };
    let stack = stack.as_ref();
    let creative = game_mode.is_some_and(|game_mode| game_mode.0 == GameMode::Creative);
    let fits = stack.is_none_or(|stack| stack.count.get() <= max_stack_size(stack, &table));
    let entry = u16::try_from(slot)
        .ok()
        .filter(|index| (1..InventoryMenu::LEN).contains(index))
        .and_then(InventoryMenu::slot);
    if creative && fits && slot < 0 {
        let mut drops = drops.copied().unwrap_or_default();
        let thrown = drops.try_drop(clock.0);
        commands.entity(player).insert(drops);
        if thrown && let Some(stack) = stack {
            let mut entity = commands.spawn_stack(stack.clone());
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
        let target = SlotRef {
            holder,
            index: entry.index,
        };
        let request = Request::new(CREATIVE_REQUEST);
        let request = match stack {
            Some(stack) => {
                let count = stack.count.get();
                let stack = commands.spawn_stack(stack.clone()).id();
                request.set(target, Some(stack), count)
            }
            None => request.set(target, None, 0),
        };
        commands.entity(player).request(request);
        set_view(&mut sync, slot, stack);
        return;
    }
    if set_view(&mut sync, slot, stack) {
        queue.compare.push(player);
    }
}

/// Makes `stack` the client view of the entry `slot`. Gives false when the inventory menu has no
/// such entry.
fn set_view(sync: &mut InventoryMenuSync, slot: i16, stack: Option<&ItemStack>) -> bool {
    let view = usize::try_from(slot)
        .ok()
        .and_then(|index| sync.view.entry_mut(index));
    let Some(view) = view else {
        return false;
    };
    *view = SlotView::Stack(stack.cloned());
    true
}
