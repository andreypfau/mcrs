use mcrs_minecraft_item_stack::ItemStack;

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerItems {
    pub slots: [Option<ItemStack>; 41],
    pub cursor: Option<ItemStack>,
    pub craft_grid: [Option<ItemStack>; 4],
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
