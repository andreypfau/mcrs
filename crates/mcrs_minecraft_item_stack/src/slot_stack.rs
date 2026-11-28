use crate::ItemStack;

#[derive(Clone, Debug, PartialEq, Hash)]
pub struct SlotStack {
    pub slot: u8,
    pub stack: ItemStack,
}
