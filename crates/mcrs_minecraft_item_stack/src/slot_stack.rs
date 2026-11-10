use crate::ItemStack;

/// A stack with the number of the container slot that holds it.
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct SlotStack {
    pub slot: u8,
    pub stack: ItemStack,
}
