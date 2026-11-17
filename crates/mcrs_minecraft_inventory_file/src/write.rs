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
use crate::player_file::PlayerFile;

pub fn write<L: NameLookup>(
    file: &PlayerInventoryFile,
    names: &L,
    player: &mut NbtCompound,
) -> Result<(), FileError> {
    let items = &file.items;
    let mut saved_file = PlayerFile {
        selected_item_slot: i32::from(items.selected),
        ..PlayerFile::default()
    };
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
        saved_file.inventory.push(saved);
    }
    for EquipmentKey { key, index } in EQUIPMENT {
        if let Some(Some(stack)) = items.slots.get(usize::from(index)) {
            let saved = write_saved(stack, names).map_err(|cause| FileError::Stack {
                key: EQUIPMENT_KEY,
                cause,
            })?;
            saved_file.equipment.put(key, NbtTag::Compound(saved));
        }
    }
    for (key, saved) in &file.kept_equipment.child_tags {
        saved_file.equipment.put(key, saved.clone());
    }
    let Ok(NbtTag::Compound(written)) = mcrs_minecraft_nbt::to_nbt_tag(&saved_file) else {
        return Err(FileError::NotACompound);
    };
    if written.get(EQUIPMENT_KEY).is_none() {
        player.child_tags.retain(|(key, _)| key != EQUIPMENT_KEY);
    }
    for (key, tag) in written.child_tags {
        player.put(&key, tag);
    }
    Ok(())
}
