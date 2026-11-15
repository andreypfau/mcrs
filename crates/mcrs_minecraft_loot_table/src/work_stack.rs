use std::borrow::Cow;

use mcrs_minecraft_item_stack::{
    Builtin, Count, ItemComponent, ItemStack, Kind, MaxStackSize, PatchEntry,
};
use mcrs_minecraft_item_table::ItemTable;

/// A stack while the evaluation makes it: its count can be 0, negative or above the maximum
/// stack size until the division (LOOT-EVL-011, LOOT-OUT-001). The item and the component patch
/// stay while the count is 0 or less (LOOT-MOD-011).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorkStack<'a> {
    /// The item and the component patch; `None` is the item `minecraft:air`. The count of the
    /// value has no meaning. A stack of the context stays borrowed until the first write.
    item: Option<Cow<'a, ItemStack>>,
    pub(crate) count: i32,
}

impl<'a> WorkStack<'a> {
    /// One `kind` with no component patch.
    pub(crate) fn one(kind: Kind) -> Self {
        Self {
            item: carrier(kind),
            count: 1,
        }
    }

    /// `stack` as a work stack; `None` is the empty stack, which has the item `minecraft:air`.
    pub(crate) fn copy(stack: Option<&'a ItemStack>) -> Self {
        match stack {
            Some(stack) => Self {
                item: Some(Cow::Borrowed(stack)),
                count: i32::from(stack.count.get()),
            },
            None => Self {
                item: None,
                count: 0,
            },
        }
    }

    /// LOOT-MOD-011: the count, 0 for an empty work stack.
    pub(crate) fn count(&self) -> i32 {
        match self.read() {
            Some(_) => self.count,
            None => 0,
        }
    }

    /// The stack that a modifier reads: `None` for an empty work stack, which reads as
    /// `minecraft:air` with no component (LOOT-MOD-011).
    pub(crate) fn read(&self) -> Option<&ItemStack> {
        self.item.as_deref().filter(|_| self.count > 0)
    }

    pub(crate) fn kind(&self) -> Option<Kind> {
        self.read().map(|stack| stack.kind)
    }

    /// Writes `value` to the component patch, also of an empty work stack, and keeps the patch in
    /// normal form (STK-PAT-006): a value equal to the component of the prototype leaves no entry.
    pub(crate) fn set<T: ItemComponent + 'static>(&mut self, value: T, items: &ItemTable) {
        let Some(stack) = self.item.take() else {
            return;
        };
        let prototype = items
            .prototype(stack.kind)
            .and_then(|prototype| prototype.get());
        self.item = Some(if prototype == Some(&value) {
            without::<T>(stack)
        } else {
            Cow::Owned(stack.into_owned().set(value))
        });
    }

    /// Replaces the item; the count and the component patch stay, in normal form against the
    /// prototype of `kind` (STK-PAT-004).
    pub(crate) fn set_kind(&mut self, kind: Kind, items: &ItemTable) {
        let Some(stack) = self.item.take() else {
            return;
        };
        let prototype = items
            .prototype(kind)
            .map(|prototype| prototype.components())
            .unwrap_or_default();
        let kept = stack
            .patch()
            .iter()
            .filter(|entry| has_effect(entry, prototype));
        let empty = ItemStack::new(kind, stack.count);
        self.item = Some(Cow::Owned(kept.cloned().fold(empty, ItemStack::with_entry)));
    }

    /// Replaces the work stack with `kind`, `count` and an empty component patch.
    pub(crate) fn replace(&mut self, kind: Kind, count: i32) {
        self.item = carrier(kind);
        self.count = count;
    }

    /// The empty stack, with no item and no component patch (LOOT-MOD-014).
    pub(crate) fn discard(&mut self) {
        self.item = None;
        self.count = 0;
    }

    /// The effective maximum stack size (STK-CNT-002); no component gives 1.
    fn max_stack_size(&self, items: &ItemTable) -> u8 {
        self.item
            .as_deref()
            .and_then(|stack| items.effective::<MaxStackSize>(stack))
            .and_then(|size| u8::try_from(size.0).ok())
            .unwrap_or(1)
    }

    /// Adds the stacks of the division (LOOT-OUT-001) to `stacks`: `None` for the item
    /// `minecraft:air` and for a count of 0 or less, else stacks of the maximum stack size, at
    /// least 1, in order and then the rest, each with the component patch.
    pub(crate) fn divide(&self, items: &ItemTable, stacks: &mut Vec<Option<ItemStack>>) {
        let Some(source) = self.read() else {
            stacks.push(None);
            return;
        };
        let max = i32::from(self.max_stack_size(items).max(1));
        let mut rest = self.count;
        while rest > 0 {
            let part = rest.min(max);
            let Some(count) = u8::try_from(part).ok().and_then(Count::new) else {
                return;
            };
            let mut stack = source.clone();
            stack.count = count;
            stacks.push(Some(stack));
            rest -= part;
        }
    }
}

fn carrier<'a>(kind: Kind) -> Option<Cow<'a, ItemStack>> {
    Count::new(1).map(|one| Cow::Owned(ItemStack::new(kind, one)))
}

/// STK-PAT-004: false for a value equal to the component of `prototype` and for a removal of a
/// type that `prototype` does not have.
fn has_effect(entry: &PatchEntry, prototype: &[Builtin]) -> bool {
    match entry {
        PatchEntry::Value(value) => !prototype.contains(value),
        PatchEntry::Removal(key) => prototype.iter().any(|component| component.key() == *key),
        _ => true,
    }
}

/// `stack` without the patch entry of `T`.
fn without<T: ItemComponent>(stack: Cow<'_, ItemStack>) -> Cow<'_, ItemStack> {
    if !stack.patch().iter().any(T::is_entry_of) {
        return stack;
    }
    let others = stack.patch().iter().filter(|entry| !T::is_entry_of(entry));
    let empty = ItemStack::new(stack.kind, stack.count);
    Cow::Owned(others.cloned().fold(empty, ItemStack::with_entry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_item_stack::{Damage, RepairCost};

    /// STK-PAT-004, STK-PAT-006: a change of the item drops a value equal to the component of the
    /// new prototype and a removal of a type that it does not have; other entries stay.
    #[test]
    fn stk_pat_004_set_kind_normalizes_the_patch_against_the_new_prototype() {
        let items = ItemTable::leak([Vec::new(), vec![Builtin::MaxStackSize(MaxStackSize(16))]]);
        let one = Count::new(1).expect("a count above 0");
        let stack = ItemStack::new(Kind(0), one)
            .set(MaxStackSize(16))
            .unset::<Damage>()
            .set(RepairCost(2));
        let mut work = WorkStack::copy(Some(&stack));
        work.set_kind(Kind(1), &items);
        let expected = ItemStack::new(Kind(1), one).set(RepairCost(2));
        assert_eq!(work.read(), Some(&expected));
    }
}
