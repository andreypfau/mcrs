use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_player_slots::Holder;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetContent;
use mcrs_minecraft_wire::VarInt;

use crate::InventoryMenuSync;
use crate::holder_slots::HolderSlots;
use crate::sent_stack::SentStack;

/// The `container_set_content` packet of the inventory menu under the next state id. Each sent
/// stack becomes the client view of its entry or of the cursor (INV-SYN-001, INV-SYN-005).
pub(crate) fn menu_content(
    sync: &mut InventoryMenuSync,
    slots: &HolderSlots,
    stacks: &mut Stacks,
    table: &ItemTable,
    bytes: &mut Vec<u8>,
) -> ClientboundContainerSetContent {
    sync.state_id = sync.state_id.next();
    let mut slot_data = Vec::with_capacity(usize::from(InventoryMenu::LEN));
    for index in 0..InventoryMenu::LEN {
        let stack = InventoryMenu::slot(index)
            .and_then(|slot| slots.stack(slot.holder, slot.index))
            .and_then(|stack| stacks.to_value(stack));
        let sent = SentStack::of(stack, table, bytes);
        slot_data.push(sent.wire);
        if let Some(view) = sync.view.entry_mut(usize::from(index)) {
            *view = SlotView::Stack(sent.stack);
        }
    }
    let cursor = slots
        .stack(Holder::Cursor, 0)
        .and_then(|stack| stacks.to_value(stack));
    let carried = SentStack::of(cursor, table, bytes);
    *sync.view.cursor_mut() = SlotView::Stack(carried.stack);
    ClientboundContainerSetContent {
        container_id: VarInt(i32::from(InventoryMenu::CONTAINER_ID)),
        state_seqno: VarInt(i32::from(sync.state_id.get())),
        slot_data,
        carried_item: carried.wire,
    }
}
