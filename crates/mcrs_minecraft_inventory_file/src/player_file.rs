use mcrs_minecraft_nbt::compound::NbtCompound;
use serde::Deserialize;
use serde::Serialize;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct PlayerFile {
    #[serde(rename = "Inventory", default)]
    pub(crate) inventory: Vec<NbtCompound>,
    #[serde(rename = "SelectedItemSlot", default)]
    pub(crate) selected_item_slot: i32,
    #[serde(default, skip_serializing_if = "NbtCompound::is_empty")]
    pub(crate) equipment: NbtCompound,
}
