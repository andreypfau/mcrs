use mcrs_minecraft_item_stack::{
    Builtin, BuiltinKey, Count, ItemStack, Kind, MaxStackSize, PatchEntry,
};

/// A stack while the evaluation makes it: its count can be 0, negative or above the maximum
/// stack size until the division (LOOT-EVL-011, LOOT-OUT-001).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WorkStack<'s> {
    item: WorkItem<'s>,
    pub(crate) count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum WorkItem<'s> {
    /// An item without a component patch.
    Kind(Kind),
    /// The item and the patch of a stack of the context.
    Copy(&'s ItemStack),
    /// The empty stack of the context.
    Empty,
}

impl<'s> WorkStack<'s> {
    /// One `kind` with no component patch.
    pub(crate) fn one(kind: Kind) -> Self {
        Self {
            item: WorkItem::Kind(kind),
            count: 1,
        }
    }

    /// A copy of `stack`; `None` is the empty stack.
    pub(crate) fn copy(stack: Option<&'s ItemStack>) -> Self {
        match stack {
            Some(stack) => Self {
                item: WorkItem::Copy(stack),
                count: i32::from(stack.count.get()),
            },
            None => Self {
                item: WorkItem::Empty,
                count: 0,
            },
        }
    }

    /// The effective maximum stack size (STK-CNT-002): the entry of the patch of a copied stack,
    /// else the size of the item in `defaults`, by item id. No component gives 1.
    pub(crate) fn max_stack_size(&self, defaults: &[u8]) -> u8 {
        let default = |kind: Kind| defaults.get(usize::from(kind.0)).copied().unwrap_or(1);
        match self.item {
            WorkItem::Kind(kind) => default(kind),
            WorkItem::Copy(stack) => {
                for entry in stack.patch() {
                    match entry {
                        PatchEntry::Value(Builtin::MaxStackSize(MaxStackSize(size))) => {
                            return u8::try_from(*size).unwrap_or(1);
                        }
                        PatchEntry::Removal(BuiltinKey::MaxStackSize) => return 1,
                        _ => {}
                    }
                }
                default(stack.kind)
            }
            WorkItem::Empty => 1,
        }
    }

    /// Adds the stacks of the division (LOOT-OUT-001) to `stacks`: `None` for the empty stack
    /// and for a count of 0 or less, else stacks of `max`, at least 1, in order and then the
    /// rest.
    pub(crate) fn divide(&self, max: u8, stacks: &mut Vec<Option<ItemStack>>) {
        if self.count <= 0 || self.item == WorkItem::Empty {
            stacks.push(None);
            return;
        }
        let max = i32::from(max.max(1));
        let mut rest = self.count;
        while rest > 0 {
            let part = rest.min(max);
            let Some(count) = u8::try_from(part).ok().and_then(Count::new) else {
                return;
            };
            let stack = match self.item {
                WorkItem::Kind(kind) => ItemStack::new(kind, count),
                WorkItem::Copy(source) => {
                    let mut stack = source.clone();
                    stack.count = count;
                    stack
                }
                WorkItem::Empty => return,
            };
            stacks.push(Some(stack));
            rest -= part;
        }
    }
}
