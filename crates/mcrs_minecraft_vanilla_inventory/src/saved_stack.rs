use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::SlotStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::compound::NbtCompound;

/// The stack of a saved value, or `None` with an error in the log.
pub(crate) fn from_saved(saved: &NbtCompound, table: &ItemTable) -> Option<ItemStack> {
    match mcrs_minecraft_item_stack::read_saved(saved, table) {
        Ok(stack) => Some(stack),
        Err(error) => {
            tracing::error!(%error, "a stack of a player does not load");
            None
        }
    }
}

/// The saved value of `stack`, or `None` with an error in the log.
pub(crate) fn to_saved(stack: &ItemStack, table: &ItemTable) -> Option<NbtCompound> {
    match mcrs_minecraft_item_stack::write_saved(stack, table) {
        Ok(saved) => Some(saved),
        Err(error) => {
            tracing::error!(%error, "a stack of a player has no saved form; it is not written");
            None
        }
    }
}

pub(crate) fn from_saved_with_slot(saved: &NbtCompound, table: &ItemTable) -> Option<SlotStack> {
    match mcrs_minecraft_item_stack::read_saved_with_slot(saved, table) {
        Ok(slotted) => Some(slotted),
        Err(error) => {
            tracing::error!(%error, "a stack of a player does not load");
            None
        }
    }
}

pub(crate) fn to_saved_with_slot(
    slot: u8,
    stack: &ItemStack,
    table: &ItemTable,
) -> Option<NbtCompound> {
    let slotted = SlotStack {
        slot,
        stack: stack.clone(),
    };
    match mcrs_minecraft_item_stack::write_saved_with_slot(&slotted, table) {
        Ok(saved) => Some(saved),
        Err(error) => {
            tracing::error!(%error, "a stack of a player has no saved form; it is not written");
            None
        }
    }
}
