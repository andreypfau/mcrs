use mcrs_minecraft_item_stack::ItemStack;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemSubject<'a> {
    Empty,
    Stack(&'a ItemStack),
}
