use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_inventory_menu::InventoryMenu;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::read_wire_with_length;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCreativeModeSlot;
use mcrs_minecraft_wire::Encode;

use crate::InventoryInput;
use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::PlayerInventoryCommandsExt;
use crate::creative_drops::CreativeDrops;
use crate::holder_slots::holder_entity;
use crate::stack_size::max_stack_size;
use crate::wire_names::WireNames;

/// Writes the [`InventoryInput::CreativeSet`] of each `set_creative_mode_slot` packet whose stack
/// decodes.
pub(crate) fn set_creative_mode_slot(
    received: On<Received<ServerboundSetCreativeModeSlot>>,
    wire_names: WireNames,
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
    let Ok(stack) = read_wire_with_length(&mut bytes.as_slice(), &wire_names.names()) else {
        return;
    };
    inputs.write(InventoryInput::CreativeSet {
        player: received.entity,
        slot: packet.slot,
        stack,
    });
}

/// Gives the player a creative set or a creative drop for a packet that passes the checks of
/// INV-CRE-003 and the counter of the connection (IEN-SPN-007). A set of an entry, and a set
/// that fails a check, is a claim of the client for its entry (INV-CRE-004, INV-CRE-006).
pub(crate) fn apply_creative_packet(
    InRef(input): InRef<InventoryInput>,
    mut players: Query<(
        &PlayerHolders,
        &mut InventoryMenuSync,
        Option<&PlayerGameMode>,
        &mut CreativeDrops,
    )>,
    table: Res<ItemTable>,
    clock: Res<BirthClock>,
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
    let Ok((holders, mut sync, game_mode, mut drops)) = players.get_mut(player) else {
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
        if drops.try_drop(clock.0)
            && let Some(stack) = stack
        {
            commands.entity(player).creative_drop(stack.clone());
        }
        return;
    }
    if creative
        && fits
        && let Some(entry) = entry
    {
        let target = SlotRef {
            holder: holder_entity(holders, player, entry.holder),
            index: entry.index,
        };
        commands.entity(player).creative_set(target, stack.cloned());
    }
    set_view(&mut sync, slot, stack);
}

/// Makes `stack` the client view of the entry `slot`, when the inventory menu has such an entry.
fn set_view(sync: &mut InventoryMenuSync, slot: i16, stack: Option<&ItemStack>) {
    let view = usize::try_from(slot)
        .ok()
        .and_then(|index| sync.view.entry_mut(index));
    if let Some(view) = view {
        *view = SlotView::Stack(stack.cloned());
    }
}
