use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::WireError;
use mcrs_minecraft_item_stack::read_wire;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::Encode;

use crate::RegistryNames;

pub fn read_packet_stack(
    stack: &ProtoStack<Wire>,
    names: &RegistryNames<'_>,
    bytes: &mut Vec<u8>,
) -> Result<Option<ItemStack>, WireError> {
    bytes.clear();
    stack
        .encode(&mut *bytes)
        .map_err(|_| WireError::Malformed)?;
    read_wire(&mut bytes.as_slice(), names)
}
