use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetContent;
use mcrs_minecraft_wire::VarInt;

use crate::MenuSync;
use crate::menu_slots::MenuSlots;
use crate::sent_stack::SentStack;
use crate::slot_stack::slot_stack;

pub(crate) fn menu_content(
    sync: &mut MenuSync,
    menu: MenuSlots,
    cursor: Option<SlotRef>,
    holders: &Query<&Slots>,
    stacks: &mut Stacks,
    names: &RegistryNames<'_>,
    bytes: &mut Vec<u8>,
) -> ClientboundContainerSetContent {
    sync.state_id = sync.state_id.next();
    let mut slot_data = Vec::with_capacity(usize::from(menu.len()));
    for index in 0..menu.len() {
        let stack = menu
            .slot(index)
            .and_then(|slot| slot_stack(holders, slot))
            .and_then(|stack| stacks.to_value(stack));
        let sent = SentStack::of(stack, names, bytes);
        slot_data.push(sent.wire);
        if let Some(view) = sync.view.entry_mut(usize::from(index)) {
            *view = SlotView::Stack(sent.stack);
        }
    }
    let cursor = cursor
        .and_then(|slot| slot_stack(holders, slot))
        .and_then(|stack| stacks.to_value(stack));
    let carried = SentStack::of(cursor, names, bytes);
    *sync.view.cursor_mut() = SlotView::Stack(carried.stack);
    ClientboundContainerSetContent {
        container_id: VarInt(i32::from(sync.container_id)),
        state_seqno: VarInt(i32::from(sync.state_id.get())),
        slot_data,
        carried_item: carried.wire,
    }
}
