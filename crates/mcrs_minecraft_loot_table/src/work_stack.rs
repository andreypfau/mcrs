use mcrs_minecraft_item_stack::{Count, ItemStack, Kind};

/// A stack while the evaluation makes it: its count can be 0, negative or above the maximum
/// stack size until the division (LOOT-EVL-011, LOOT-OUT-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkStack {
    pub(crate) kind: Kind,
    pub(crate) count: i32,
}

impl WorkStack {
    /// Adds the stacks of the division (LOOT-OUT-001) to `stacks`: `None` for a count of 0 or
    /// less, else stacks of `max`, at least 1, in order and then the rest.
    pub(crate) fn divide(&self, max: u8, stacks: &mut Vec<Option<ItemStack>>) {
        if self.count <= 0 {
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
            stacks.push(Some(ItemStack::new(self.kind, count)));
            rest -= part;
        }
    }
}
