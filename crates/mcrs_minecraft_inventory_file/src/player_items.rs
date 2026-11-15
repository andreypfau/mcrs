use mcrs_minecraft_item_stack::ItemStack;

/// The stacks of a player as values, and its selected slot. The craft result has no part: no
/// player file and no carry holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerItems {
    /// One value for each slot of the player inventory, in the order of INV-SLT-001.
    pub slots: [Option<ItemStack>; 41],
    pub cursor: Option<ItemStack>,
    pub craft_grid: [Option<ItemStack>; 4],
    /// The selected slot of the hotbar.
    pub selected: u8,
}

impl Default for PlayerItems {
    fn default() -> Self {
        Self {
            slots: [const { None }; 41],
            cursor: None,
            craft_grid: Default::default(),
            selected: 0,
        }
    }
}
