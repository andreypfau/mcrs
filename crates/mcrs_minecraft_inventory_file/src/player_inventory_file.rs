use mcrs_minecraft_nbt::compound::NbtCompound;

use crate::PlayerItems;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerInventoryFile {
    pub items: PlayerItems,
    pub kept_equipment: NbtCompound,
}
