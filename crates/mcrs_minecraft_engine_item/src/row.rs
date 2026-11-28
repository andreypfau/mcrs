use core::hash::Hash;
use core::hash::Hasher;

use bevy_ecs::component::Component;
use bevy_ecs::component::ComponentId;
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::CustomComponent;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_table::Prototype;
use rustc_hash::FxHasher;

use crate::cells::Cells;
use crate::column::Column;
use crate::undecoded::Undecoded;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowKey {
    Builtin(BuiltinKey),
    Undecoded,
    Custom(&'static str),
}

#[derive(Clone, Copy)]
pub(crate) struct Row {
    id: ComponentId,
    key: RowKey,
    neutral: bool,
    equal: fn(&Row, &Cells, &Cells) -> bool,
    hash: fn(&Row, &Cells, usize, &mut FxHasher),
    diff: fn(&Row, &Cells, Option<&'static Prototype>, ItemStack) -> ItemStack,
    gives: fn(&Row, &Cells, Option<&'static Prototype>, Option<&PatchEntry>) -> bool,
    insert_entry: fn(&mut EntityWorldMut, &PatchEntry) -> bool,
    insert_static: fn(&mut EntityWorldMut, &'static Builtin),
}

impl Row {
    pub(crate) fn builtin<T: ItemComponent + Component>(
        column: Column<T>,
        key: BuiltinKey,
    ) -> Self {
        Self {
            id: column.id(),
            key: RowKey::Builtin(key),
            neutral: T::neutral().is_some(),
            equal: equal::<T>,
            hash: hash::<T>,
            diff: diff::<T>,
            gives: gives::<T>,
            insert_entry: insert_builtin::<T>,
            insert_static: insert_static::<T>,
        }
    }

    pub(crate) fn custom<C: CustomComponent + Component>(column: Column<C>) -> Self {
        Self {
            id: column.id(),
            key: RowKey::Custom(C::KEY),
            neutral: false,
            equal: equal::<C>,
            hash: hash::<C>,
            diff: diff::<C>,
            gives: |_, _, _, _| false,
            insert_entry: insert_custom::<C>,
            insert_static: |_, _| {},
        }
    }

    pub(crate) fn undecoded(column: Column<Undecoded>) -> Self {
        Self {
            id: column.id(),
            key: RowKey::Undecoded,
            neutral: false,
            equal: |row, left, right| left.get(undecoded(row)) == right.get(undecoded(row)),
            hash: |row, cells, index, hasher| {
                if let Some(entries) = cells.get(undecoded(row)) {
                    hasher.write_usize(index);
                    entries.hash(hasher);
                }
            },
            diff: |row, cells, _, stack| match cells.get(undecoded(row)) {
                Some(entries) => entries
                    .0
                    .iter()
                    .fold(stack, |stack, entry| stack.with_entry(entry.clone())),
                None => stack,
            },
            gives: |_, _, _, _| false,
            insert_entry: |_, _| false,
            insert_static: |_, _| {},
        }
    }

    pub(crate) fn id(&self) -> ComponentId {
        self.id
    }

    pub(crate) fn key(&self) -> RowKey {
        self.key
    }

    pub(crate) fn is_neutral(&self) -> bool {
        self.neutral
    }

    pub(crate) fn equal(&self, left: &Cells, right: &Cells) -> bool {
        (self.equal)(self, left, right)
    }

    pub(crate) fn hash(&self, cells: &Cells, index: usize, hasher: &mut FxHasher) {
        (self.hash)(self, cells, index, hasher);
    }

    pub(crate) fn diff(
        &self,
        cells: &Cells,
        prototype: Option<&'static Prototype>,
        stack: ItemStack,
    ) -> ItemStack {
        (self.diff)(self, cells, prototype, stack)
    }

    pub(crate) fn gives(
        &self,
        cells: &Cells,
        prototype: Option<&'static Prototype>,
        entry: Option<&PatchEntry>,
    ) -> bool {
        (self.gives)(self, cells, prototype, entry)
    }

    pub(crate) fn insert_entry(&self, entity: &mut EntityWorldMut, entry: &PatchEntry) -> bool {
        (self.insert_entry)(entity, entry)
    }

    pub(crate) fn insert_static(&self, entity: &mut EntityWorldMut, value: &'static Builtin) {
        (self.insert_static)(entity, value);
    }
}

pub(crate) fn row_mask(rows: &[Row], has: impl Fn(ComponentId) -> bool) -> u128 {
    (rows.iter().enumerate())
        .filter(|(_, row)| has(row.id()))
        .fold(0, |mask, (index, _)| mask | bit(index))
}

pub(crate) fn bit(index: usize) -> u128 {
    u32::try_from(index)
        .ok()
        .and_then(|index| 1u128.checked_shl(index))
        .unwrap_or(0)
}

pub(crate) fn bits(mut mask: u128) -> impl Iterator<Item = usize> {
    core::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let index = mask.trailing_zeros();
        mask &= mask - 1;
        usize::try_from(index).ok()
    })
}

fn column<T>(row: &Row) -> Column<T> {
    // SAFETY: a row takes its id from a `Column<T>` and pairs it only with the operations for
    // that `T`, which are the only callers of this function.
    unsafe { Column::assume(row.id) }
}

fn undecoded(row: &Row) -> Column<Undecoded> {
    column(row)
}

fn equal<T: ItemComponent + Component>(row: &Row, left: &Cells, right: &Cells) -> bool {
    let column = column::<T>(row);
    left.effective(column) == right.effective(column)
}

fn hash<T: ItemComponent + Component>(
    row: &Row,
    cells: &Cells,
    index: usize,
    hasher: &mut FxHasher,
) {
    let value = cells.effective(column::<T>(row));
    if value.is_none() || value != T::neutral() {
        hasher.write_usize(index);
        value.hash(hasher);
    }
}

enum Needed<'value, T> {
    Nothing,
    Value(&'value T),
    Removal,
}

fn needed<'cells, T: ItemComponent + Component>(
    row: &Row,
    cells: &Cells<'cells>,
    prototype: Option<&'static Prototype>,
) -> Needed<'cells, T> {
    let value = cells.effective(column::<T>(row));
    match (value, prototype.and_then(Prototype::get::<T>)) {
        (None, None) => Needed::Nothing,
        (Some(value), None) if Some(value) == T::neutral() => Needed::Nothing,
        (Some(value), Some(base)) if value == base => Needed::Nothing,
        (Some(value), _) => Needed::Value(value),
        (None, Some(_)) => Needed::Removal,
    }
}

fn diff<T: ItemComponent + Component>(
    row: &Row,
    cells: &Cells,
    prototype: Option<&'static Prototype>,
    stack: ItemStack,
) -> ItemStack {
    match needed::<T>(row, cells, prototype) {
        Needed::Nothing => stack,
        Needed::Value(value) => stack.set(value.clone()),
        Needed::Removal => stack.unset::<T>(),
    }
}

fn gives<T: ItemComponent + Component>(
    row: &Row,
    cells: &Cells,
    prototype: Option<&'static Prototype>,
    entry: Option<&PatchEntry>,
) -> bool {
    match needed::<T>(row, cells, prototype) {
        Needed::Nothing => entry.is_none(),
        Needed::Value(value) => matches!(
            entry,
            Some(PatchEntry::Value(builtin)) if T::from_builtin(builtin) == Some(value)
        ),
        Needed::Removal => matches!(
            entry,
            Some(PatchEntry::Removal(key)) if T::builtin_key() == Some(*key)
        ),
    }
}

fn insert_builtin<T: ItemComponent + Component>(
    entity: &mut EntityWorldMut,
    entry: &PatchEntry,
) -> bool {
    let PatchEntry::Value(value) = entry else {
        return false;
    };
    let Some(value) = T::from_builtin(value) else {
        return false;
    };
    if Some(value) != T::neutral() {
        entity.insert(value.clone());
    }
    true
}

fn insert_custom<C: CustomComponent + Component>(
    entity: &mut EntityWorldMut,
    entry: &PatchEntry,
) -> bool {
    let PatchEntry::Custom(value) = entry else {
        return false;
    };
    let Some(value) = (value.key == C::KEY)
        .then(|| C::decode(&value.bytes))
        .flatten()
    else {
        return false;
    };
    entity.insert(value);
    true
}

fn insert_static<T: ItemComponent + Component>(
    entity: &mut EntityWorldMut,
    value: &'static Builtin,
) {
    if let Some(value) = T::from_builtin(value)
        && Some(value) != T::neutral()
    {
        entity.insert(T::borrow_static(value));
    }
}
