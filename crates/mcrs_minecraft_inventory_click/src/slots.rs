use mcrs_minecraft_inventory_plan::Filter;
use mcrs_minecraft_inventory_plan::Loaded;
use mcrs_minecraft_inventory_plan::Planned;

/// The slots of the player that clicks and of the menu, read as the planner reads them.
pub trait Slots {
    /// A slot reference.
    type Slot: Copy + PartialEq;
    /// The handle of the caller for the item and the components of a stack.
    type Stack: Copy + PartialEq;

    /// The slot of the cursor.
    fn cursor(&self) -> Self::Slot;

    /// The slot `index` of the player inventory, or `None` when it has no such slot.
    fn inventory(&self, index: u16) -> Option<Self::Slot>;

    /// The inputs of `slot`, or `None` for a slot that does not exist.
    fn load(&self, slot: Self::Slot) -> Option<Loaded<Self::Stack>>;

    /// The place filter of `slot` for `stack`.
    fn place(&self, slot: Self::Slot, stack: Self::Stack) -> Filter;

    /// Whether two stacks of the same kind have the same components.
    fn equal(&self, left: Self::Stack, right: Self::Stack) -> bool;

    /// Whether `stack` can stack: its maximum stack size is larger than 1, and it cannot take
    /// damage or its damage value is 0.
    fn can_stack(&self, stack: Planned<Self::Stack>) -> bool;
}
