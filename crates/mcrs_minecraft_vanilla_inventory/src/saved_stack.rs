use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::SlotStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_nbt::compound::NbtCompound;

use crate::player_file_codec::decode_error;
use crate::player_file_codec::encode_error;

/// The stack of a saved value under the key `key` of the player file or of a carry.
pub(crate) fn from_saved(
    key: &'static str,
    saved: &NbtCompound,
    table: &ItemTable,
) -> Result<ItemStack, PersistedError> {
    mcrs_minecraft_item_stack::read_saved(saved, table).map_err(|error| decode_error(key, error))
}

/// The saved value of `stack` under the key `key` of the player file or of a carry.
pub(crate) fn to_saved(
    key: &'static str,
    stack: &ItemStack,
    table: &ItemTable,
) -> Result<NbtCompound, PersistedError> {
    mcrs_minecraft_item_stack::write_saved(stack, table).map_err(|error| encode_error(key, error))
}

pub(crate) fn from_saved_with_slot(
    key: &'static str,
    saved: &NbtCompound,
    table: &ItemTable,
) -> Result<SlotStack, PersistedError> {
    mcrs_minecraft_item_stack::read_saved_with_slot(saved, table)
        .map_err(|error| decode_error(key, error))
}

pub(crate) fn to_saved_with_slot(
    key: &'static str,
    slot: u8,
    stack: &ItemStack,
    table: &ItemTable,
) -> Result<NbtCompound, PersistedError> {
    let slotted = SlotStack {
        slot,
        stack: stack.clone(),
    };
    mcrs_minecraft_item_stack::write_saved_with_slot(&slotted, table)
        .map_err(|error| encode_error(key, error))
}
