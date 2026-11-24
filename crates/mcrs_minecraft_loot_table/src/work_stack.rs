use std::borrow::Cow;

use mcrs_minecraft_item_stack::{
    Builtin, Count, ItemComponent, ItemStack, Kind, MaxStackSize, PatchEntry,
};
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_predicate::{BakedItemPredicate, ItemSubject};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WorkStack<'a> {
    item: Option<Cow<'a, ItemStack>>,
    pub(crate) count: i32,
}

impl<'a> WorkStack<'a> {
    pub(crate) fn one(kind: Kind) -> Self {
        Self {
            item: carrier(kind),
            count: 1,
        }
    }

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

    pub(crate) fn count(&self) -> i32 {
        match self.read() {
            Some(_) => self.count,
            None => 0,
        }
    }

    pub(crate) fn read(&self) -> Option<&ItemStack> {
        self.item.as_deref().filter(|_| self.count > 0)
    }

    pub(crate) fn kind(&self) -> Option<Kind> {
        self.read().map(|stack| stack.kind)
    }

    pub(crate) fn passes(&self, filter: &BakedItemPredicate, items: &ItemTable) -> bool {
        match self.read() {
            Some(stack) => filter.matches_count(stack, self.count, items),
            None => filter.matches(ItemSubject::Empty, items),
        }
    }

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

    pub(crate) fn replace(&mut self, kind: Kind, count: i32) {
        self.item = carrier(kind);
        self.count = count;
    }

    pub(crate) fn discard(&mut self) {
        self.item = None;
        self.count = 0;
    }

    fn max_stack_size(&self, items: &ItemTable) -> u8 {
        self.item
            .as_deref()
            .and_then(|stack| items.effective::<MaxStackSize>(stack))
            .map_or(1, |size| size.0.clamp(1, i32::from(Count::MAX)))
            .try_into()
            .unwrap_or(1)
    }

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

fn has_effect(entry: &PatchEntry, prototype: &[Builtin]) -> bool {
    match entry {
        PatchEntry::Value(value) => !prototype.contains(value),
        PatchEntry::Removal(key) => prototype.iter().any(|component| component.key() == *key),
        _ => true,
    }
}

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

    #[test]
    fn stk_cnt_007_divide_places_every_item_for_a_maximum_stack_size_above_99() {
        let items = ItemTable::leak([Vec::new()]);
        let one = Count::new(1).expect("a count above 0");
        for (size, total, expected) in [
            (200, 150, vec![99, 51]),
            (255, 99, vec![99]),
            (16, 40, vec![16, 16, 8]),
            (0, 3, vec![1, 1, 1]),
        ] {
            let stack = ItemStack::new(Kind(0), one).set(MaxStackSize(size));
            let mut work = WorkStack::copy(Some(&stack));
            work.count = total;
            let mut stacks = Vec::new();
            work.divide(&items, &mut stacks);
            let counts: Vec<u8> = stacks
                .iter()
                .flatten()
                .map(|stack| stack.count.get())
                .collect();
            assert_eq!(counts, expected, "size {size}");
        }
    }
}
