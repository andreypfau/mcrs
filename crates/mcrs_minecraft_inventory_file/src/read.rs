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
use crate::equipment_key::FILE_KEYS;
use crate::equipment_key::INVENTORY_KEY;
use crate::equipment_key::is_slot_key;
use crate::player_file::PlayerFile;

pub fn read<L: NameLookup>(
    player: &NbtCompound,
    names: &L,
) -> Result<PlayerInventoryFile, FileError> {
    let file: PlayerFile = mcrs_minecraft_nbt::from_tag(NbtTag::Compound(file_keys(player)))
        .map_err(|_| FileError::BadShape(bad_key(player)))?;
    let mut items = PlayerItems::default();
    for saved in &file.inventory {
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
    for EquipmentKey { key, index } in EQUIPMENT {
        if let Some(saved) = file.equipment.get_compound(key)
            && let Some(slot) = items.slots.get_mut(usize::from(index))
        {
            *slot = Some(read_saved(saved, names).map_err(|cause| FileError::Stack {
                key: EQUIPMENT_KEY,
                cause,
            })?);
        }
    }
    items.selected = u8::try_from(file.selected_item_slot)
        .ok()
        .filter(|selected| HOTBAR.contains(&u16::from(*selected)))
        .unwrap_or(0);
    let kept_equipment: NbtCompound = file
        .equipment
        .child_tags
        .into_iter()
        .filter(|(key, _)| !is_slot_key(key))
        .collect();
    Ok(PlayerInventoryFile {
        items,
        kept_equipment,
    })
}

fn file_keys(player: &NbtCompound) -> NbtCompound {
    player
        .child_tags
        .iter()
        .filter(|(key, _)| FILE_KEYS.contains(&key.as_str()))
        .cloned()
        .collect()
}

fn bad_key(player: &NbtCompound) -> &'static str {
    FILE_KEYS
        .into_iter()
        .find(|key| {
            let Some(tag) = player.get(key) else {
                return false;
            };
            let mut alone = NbtCompound::new();
            alone.put(key, tag.clone());
            mcrs_minecraft_nbt::from_tag::<PlayerFile>(NbtTag::Compound(alone)).is_err()
        })
        .unwrap_or(INVENTORY_KEY)
}
