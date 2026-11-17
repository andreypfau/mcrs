use mcrs_minecraft_item::stack;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::WireError;
use mcrs_minecraft_item_stack::hash_stack;
use mcrs_minecraft_item_stack::write_hashed;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_wire::Decode;

use crate::RegistryNames;

const AIR: &str = "minecraft:air";

pub fn write_packet_hashed(
    item_stack: Option<&ItemStack>,
    names: &RegistryNames<'_>,
    scratch: &mut HashScratch,
    bytes: &mut Vec<u8>,
) -> Option<stack::HashedStack> {
    if let Ok(hashed) = hash_stack(item_stack, names, scratch)
        && let Ok(packet) = to_packet(hashed.as_ref(), bytes)
    {
        return packet;
    }
    let unhashable = HashedStack {
        kind: Kind(air_id(names)?),
        count: 1,
        values: Vec::new(),
        removals: Vec::new(),
    };
    to_packet(Some(&unhashable), bytes).ok().flatten()
}

fn to_packet(
    hashed: Option<&HashedStack>,
    bytes: &mut Vec<u8>,
) -> Result<Option<stack::HashedStack>, WireError> {
    bytes.clear();
    write_hashed(hashed, bytes)?;
    Option::<stack::HashedStack>::decode(&mut bytes.as_slice()).map_err(|_| WireError::Malformed)
}

fn air_id(names: &RegistryNames<'_>) -> Option<u16> {
    names.id(NameTable::Item, ResourceLocation::read(AIR).ok()?)
}
