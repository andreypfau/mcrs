use mcrs_minecraft_item_stack::ItemStack;

/// The stack that an item predicate tests. A caller without a stack, such as an entity without
/// the slot, has no subject: it fails the predicate without a match.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemSubject<'a> {
    /// The empty stack: the item air, the count 0 and no component.
    Empty,
    /// A stack with an item and a count above zero.
    Stack(&'a ItemStack),
}
