use mcrs_minecraft_engine::persisted_registry::PersistedError;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::SlotStack;
use mcrs_minecraft_nbt::compound::NbtCompound;

use crate::player_file_codec::decode_error;
use crate::player_file_codec::encode_error;

pub(crate) fn from_saved_with_slot(
    key: &'static str,
    saved: &NbtCompound,
    names: &RegistryNames<'_>,
) -> Result<SlotStack, PersistedError> {
    mcrs_minecraft_item_stack::read_saved_with_slot(saved, names)
        .map_err(|error| decode_error(key, error))
}

pub(crate) fn to_saved_with_slot(
    key: &'static str,
    slot: u8,
    stack: &ItemStack,
    names: &RegistryNames<'_>,
) -> Result<NbtCompound, PersistedError> {
    let slotted = SlotStack {
        slot,
        stack: stack.clone(),
    };
    mcrs_minecraft_item_stack::write_saved_with_slot(&slotted, names)
        .map_err(|error| encode_error(key, error))
}
