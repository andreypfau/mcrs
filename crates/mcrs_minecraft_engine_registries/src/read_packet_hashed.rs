use mcrs_minecraft_item::stack;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::WireError;
use mcrs_minecraft_item_stack::read_hashed;
use mcrs_minecraft_wire::Encode;

use crate::RegistryNames;

pub fn read_packet_hashed(
    hashed: Option<&stack::HashedStack>,
    names: &RegistryNames<'_>,
    bytes: &mut Vec<u8>,
) -> Result<Option<HashedStack>, WireError> {
    bytes.clear();
    hashed
        .encode(&mut *bytes)
        .map_err(|_| WireError::Malformed)?;
    read_hashed(&mut bytes.as_slice(), names)
}
