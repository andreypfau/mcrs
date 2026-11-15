use mcrs_minecraft_inventory_player::HOTBAR;
use mcrs_minecraft_inventory_player::MAIN;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_item_stack::read_saved_with_slot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::FileError;
use crate::PlayerInventoryFile;
use crate::PlayerItems;
use crate::equipment_key::EQUIPMENT;
use crate::equipment_key::EQUIPMENT_KEY;
use crate::equipment_key::EquipmentKey;
use crate::equipment_key::INVENTORY_KEY;
use crate::equipment_key::SELECTED_KEY;
use crate::equipment_key::is_slot_key;

/// The stacks that `player` gives, with no filter and no slot limit (INV-SAV-001, INV-SAV-002).
/// The cursor and the craft grid are empty. A stack of `Inventory` with a slot outside the main
/// inventory is skipped. A stack that does not load fails the read, so the next write does not
/// lose it.
pub fn read<L: NameLookup>(
    player: &NbtCompound,
    names: &L,
) -> Result<PlayerInventoryFile, FileError> {
    let mut items = PlayerItems::default();
    for entry in inventory_entries(player)? {
        let NbtTag::Compound(saved) = entry else {
            return Err(FileError::BadShape(INVENTORY_KEY));
        };
        let slotted = read_saved_with_slot(saved, names).map_err(|cause| FileError::Stack {
            key: INVENTORY_KEY,
            cause,
        })?;
        if !MAIN.contains(&u16::from(slotted.slot)) {
            continue;
        }
        if let Some(slot) = items.slots.get_mut(usize::from(slotted.slot)) {
            *slot = Some(slotted.stack);
        }
    }
    let empty = NbtCompound::new();
    let equipment = match player.get(EQUIPMENT_KEY) {
        None => &empty,
        Some(NbtTag::Compound(equipment)) => equipment,
        Some(_) => return Err(FileError::BadShape(EQUIPMENT_KEY)),
    };
    for EquipmentKey { key, index } in EQUIPMENT {
        if let Some(saved) = equipment.get_compound(key)
            && let Some(slot) = items.slots.get_mut(usize::from(index))
        {
            *slot = Some(read_saved(saved, names).map_err(|cause| FileError::Stack {
                key: EQUIPMENT_KEY,
                cause,
            })?);
        }
    }
    let selected = match player.get(SELECTED_KEY) {
        None => 0,
        Some(NbtTag::Int(selected)) => *selected,
        Some(_) => return Err(FileError::BadShape(SELECTED_KEY)),
    };
    items.selected = u8::try_from(selected)
        .ok()
        .filter(|selected| HOTBAR.contains(&u16::from(*selected)))
        .unwrap_or(0);
    let kept_equipment: NbtCompound = equipment
        .child_tags
        .iter()
        .filter(|(key, _)| !is_slot_key(key))
        .cloned()
        .collect();
    Ok(PlayerInventoryFile {
        items,
        kept_equipment,
    })
}

fn inventory_entries(player: &NbtCompound) -> Result<&[NbtTag], FileError> {
    match player.get(INVENTORY_KEY) {
        None => Ok(&[]),
        Some(NbtTag::List(entries)) => Ok(entries),
        Some(_) => Err(FileError::BadShape(INVENTORY_KEY)),
    }
}
