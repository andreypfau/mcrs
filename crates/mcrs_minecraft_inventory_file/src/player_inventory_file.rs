use mcrs_minecraft_nbt::compound::NbtCompound;

use crate::PlayerItems;

/// The part of the player file that holds the stacks of a player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerInventoryFile {
    pub items: PlayerItems,
    /// The keys of `equipment` that hold no slot of the player inventory, as the file gave them.
    /// A write puts them after the keys of the slots.
    pub kept_equipment: NbtCompound,
}
