use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::read_wire_with_length;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::NbtBudget;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCreativeModeSlot;
use mcrs_minecraft_wire::Encode;

use crate::InventoryInput;
use crate::MenuSync;
use crate::creative_drops::CreativeDrops;
use crate::menu_slots::MenuSlots;
use crate::wire_names::WireNames;
use mcrs_minecraft_vanilla_inventory::PlayerInventoryCommandsExt;

pub(crate) fn set_creative_mode_slot(
    received: On<Received<ServerboundSetCreativeModeSlot>>,
    wire_names: WireNames,
    mut inputs: MessageWriter<InventoryInput>,
    mut bytes: Local<Vec<u8>>,
) {
    let unlimited = NbtBudget {
        bytes: u64::MAX,
        tags: u64::MAX,
    };
    let read = mcrs_minecraft_nbt::with_nbt_budget(unlimited, || {
        let packet = received.packet()?;
        bytes.clear();
        packet.item.encode(&mut *bytes).ok()?;
        let stack = read_wire_with_length(&mut bytes.as_slice(), &wire_names.names()).ok()?;
        Some((packet.slot, stack))
    });
    let Ok(Some((slot, stack))) = read else {
        return;
    };
    inputs.write(InventoryInput::CreativeSet {
        player: received.entity,
        slot,
        stack,
    });
}

pub(crate) fn apply_creative_packet(
    InRef(input): InRef<InventoryInput>,
    mut players: Query<(&Menus, Option<&PlayerGameMode>, &mut CreativeDrops)>,
    mut menus: Query<(&MenuLayout, &MenuParts, &mut MenuSync)>,
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
    let Ok((player_menus, game_mode, mut drops)) = players.get_mut(player) else {
        return;
    };
    let Some(first) = player_menus.iter().next() else {
        return;
    };
    let Ok((layout, parts, mut sync)) = menus.get_mut(first) else {
        return;
    };
    let menu = MenuSlots { layout, parts };
    let stack = stack.as_ref();
    let creative = game_mode.is_some_and(|game_mode| game_mode.0 == GameMode::Creative);
    let fits = stack.is_none_or(|stack| {
        stack.count.get() <= Count::capacity(table.effective::<MaxStackSize>(stack))
    });
    let entry = u16::try_from(slot)
        .ok()
        .filter(|index| (1..menu.len()).contains(index))
        .and_then(|index| menu.slot(index));
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
        && let Some(target) = entry
    {
        commands.entity(player).creative_set(target, stack.cloned());
    }
    set_view(&mut sync, slot, stack);
}

fn set_view(sync: &mut MenuSync, slot: i16, stack: Option<&ItemStack>) {
    let view = usize::try_from(slot)
        .ok()
        .and_then(|index| sync.view.entry_mut(index));
    if let Some(view) = view {
        *view = SlotView::Stack(stack.cloned());
    }
}
