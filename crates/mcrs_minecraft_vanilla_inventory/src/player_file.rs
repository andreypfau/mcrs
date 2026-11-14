use mcrs_minecraft_nbt::compound::NbtCompound;
use serde::Deserialize;
use serde::Serialize;

pub(crate) const INVENTORY_KEY: &str = "Inventory";
pub(crate) const EQUIPMENT_KEY: &str = "equipment";
pub(crate) const FILE_KEYS: &[&str] = &[INVENTORY_KEY, "SelectedItemSlot", EQUIPMENT_KEY];

/// The keys of the player file that hold the stacks of a player and its selected slot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct PlayerFile {
    #[serde(rename = "Inventory", default)]
    pub(crate) inventory: Vec<NbtCompound>,
    #[serde(rename = "SelectedItemSlot", default)]
    pub(crate) selected_item_slot: i32,
    #[serde(default, skip_serializing_if = "NbtCompound::is_empty")]
    pub(crate) equipment: NbtCompound,
}
