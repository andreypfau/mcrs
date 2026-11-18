use mcrs_minecraft_item_stack::ItemStack;

#[derive(Clone, Debug, PartialEq)]
pub enum CreativeArea {
    Grid(Option<ItemStack>),
    Hotbar,
    Inventory,
    Destroy,
}
