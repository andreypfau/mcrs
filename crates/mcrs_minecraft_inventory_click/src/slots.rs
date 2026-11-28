use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_inventory_transaction::Loaded;
use mcrs_minecraft_inventory_transaction::Planned;

pub trait Slots {
    type Slot: Copy + PartialEq;
    type Stack: Copy + PartialEq;

    fn cursor(&self) -> Self::Slot;

    fn inventory(&self, index: u16) -> Option<Self::Slot>;

    fn load(&self, slot: Self::Slot) -> Option<Loaded<Self::Stack>>;

    fn place(&self, slot: Self::Slot, stack: Self::Stack) -> Filter;

    fn same_holder(&self, left: Self::Slot, right: Self::Slot) -> bool;

    fn equal(&self, left: Self::Stack, right: Self::Stack) -> bool;

    fn can_stack(&self, stack: Planned<Self::Stack>) -> bool;
}
