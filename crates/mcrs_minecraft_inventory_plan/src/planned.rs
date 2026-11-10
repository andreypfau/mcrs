/// A stack as the planner sees it: a handle of the caller and the numbers the rules read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Planned<T> {
    /// The handle of the caller for the item and its components.
    pub stack: T,
    /// The item kind. Two stacks with different kinds are never equal.
    pub kind: u16,
    /// The count. A stack with the count 0 is the empty stack.
    pub count: u8,
    /// The maximum stack size.
    pub max: u8,
}
