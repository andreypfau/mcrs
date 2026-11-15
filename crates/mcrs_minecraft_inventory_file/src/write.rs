use mcrs_minecraft_inventory_player::MAIN;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::SlotStack;
use mcrs_minecraft_item_stack::write_saved;
use mcrs_minecraft_item_stack::write_saved_with_slot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::FileError;
use crate::PlayerInventoryFile;
use crate::equipment_key::EQUIPMENT;
use crate::equipment_key::EQUIPMENT_KEY;
use crate::equipment_key::EquipmentKey;
use crate::equipment_key::INVENTORY_KEY;
use crate::equipment_key::SELECTED_KEY;

/// Writes `Inventory`, `SelectedItemSlot` and `equipment` of `file` into `player`
/// (INV-SAV-001). It writes `equipment` only when it has a key, and no cursor or craft grid
/// stack.
pub fn write<L: NameLookup>(
    file: &PlayerInventoryFile,
    names: &L,
    player: &mut NbtCompound,
) -> Result<(), FileError> {
    let items = &file.items;
    let mut inventory = Vec::new();
    let main = items.slots.iter().take(usize::from(MAIN.end));
    for (slot, stack) in (0..=u8::MAX).zip(main) {
        let Some(stack) = stack else {
            continue;
        };
        let slotted = SlotStack {
            slot,
            stack: stack.clone(),
        };
        let saved = write_saved_with_slot(&slotted, names).map_err(|cause| FileError::Stack {
            key: INVENTORY_KEY,
            cause,
        })?;
        inventory.push(NbtTag::Compound(saved));
    }
    let mut equipment = NbtCompound::new();
    for EquipmentKey { key, index } in EQUIPMENT {
        if let Some(Some(stack)) = items.slots.get(usize::from(index)) {
            let saved = write_saved(stack, names).map_err(|cause| FileError::Stack {
                key: EQUIPMENT_KEY,
                cause,
            })?;
            equipment.put(key, NbtTag::Compound(saved));
        }
    }
    for (key, saved) in &file.kept_equipment.child_tags {
        equipment.put(key, saved.clone());
    }
    player.put_list(INVENTORY_KEY, inventory);
    player.put(SELECTED_KEY, NbtTag::Int(i32::from(items.selected)));
    if equipment.is_empty() {
        player.child_tags.retain(|(key, _)| key != EQUIPMENT_KEY);
    } else {
        player.put(EQUIPMENT_KEY, NbtTag::Compound(equipment));
    }
    Ok(())
}
