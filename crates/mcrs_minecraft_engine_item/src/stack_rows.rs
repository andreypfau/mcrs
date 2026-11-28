use core::hash::Hasher;

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_table::Prototype;
use rustc_hash::FxHasher;

use crate::ComponentState;
use crate::cells::Cells;
use crate::item_columns::ItemColumns;
use crate::row::Row;
use crate::row::RowKey;
use crate::row::bit;
use crate::row::bits;

#[derive(SystemParam)]
pub struct StackRows<'w, 's> {
    columns: ItemColumns<'w, 's>,
    kinds: Query<'w, 's, &'static Kind>,
}

impl<'w, 's> StackRows<'w, 's> {
    #[must_use]
    pub fn kind(&self, stack: Entity) -> Option<Kind> {
        self.kinds.get(stack).ok().copied()
    }

    #[must_use]
    pub fn mergeable(&mut self, left: Entity, right: Entity) -> bool {
        if self.kind(left).is_none() || self.kind(left) != self.kind(right) {
            return false;
        }
        match (self.columns.cells(left), self.columns.cells(right)) {
            (Some(left), Some(right)) => equal(self.columns.rows(), &left, &right),
            _ => false,
        }
    }

    #[must_use]
    pub fn row_hash(&mut self, stack: Entity) -> Option<u64> {
        let kind = self.kind(stack)?;
        let cells = self.columns.cells(stack)?;
        Some(row_hash(self.columns.rows(), kind, &cells))
    }

    pub(crate) fn get<T: ItemComponent + Component>(&mut self, stack: Entity) -> Option<&'w T> {
        self.kind(stack)?;
        let column = self.columns.column::<T>()?;
        self.columns.cells(stack)?.effective(column)
    }

    pub(crate) fn state<T: ItemComponent + Component>(
        &mut self,
        stack: Entity,
        prototype: Option<&'static Prototype>,
    ) -> ComponentState<'w, T> {
        let (Some(column), Some(cells)) = (self.columns.column::<T>(), self.columns.cells(stack))
        else {
            return ComponentState::Absent;
        };
        ComponentState::of(cells.effective(column), cells.removed(), prototype)
    }

    pub(crate) fn value(
        &mut self,
        stack: Entity,
        prototype: Option<&'static Prototype>,
        kind: Kind,
        count: Count,
    ) -> Option<ItemStack> {
        let cells = self.columns.cells(stack)?;
        Some(value_of(
            self.columns.rows(),
            &cells,
            prototype,
            kind,
            count,
        ))
    }

    pub(crate) fn equals_value(
        &mut self,
        stack: Entity,
        value: &ItemStack,
        prototype: Option<&'static Prototype>,
    ) -> bool {
        let Some(cells) = self.columns.cells(stack) else {
            return false;
        };
        let rows = self.columns.rows();
        let mut candidates = cells.present();
        for component in prototype.map_or(&[][..], Prototype::components) {
            candidates |= bit(component.key().index());
        }
        let mut in_place = true;
        for entry in value.patch() {
            match entry {
                PatchEntry::Value(component) => candidates |= bit(component.key().index()),
                PatchEntry::Removal(key) => candidates |= bit(key.index()),
                _ => in_place = false,
            }
        }
        let builtin = |index: usize| match rows.get(index).map(|row| (row, row.key())) {
            Some((row, RowKey::Builtin(key))) => Some((row, key)),
            _ => None,
        };
        if in_place && bits(candidates).all(|index| builtin(index).is_some()) {
            return bits(candidates).all(|index| {
                builtin(index).is_some_and(|(row, key)| {
                    let entry =
                        (value.patch().iter()).find(|entry| entry.builtin_key() == Some(key));
                    row.gives(&cells, prototype, entry)
                })
            });
        }
        value_of(rows, &cells, prototype, value.kind, value.count) == *value
    }
}

pub(crate) fn value_of(
    rows: &[Row],
    cells: &Cells,
    prototype: Option<&'static Prototype>,
    kind: Kind,
    count: Count,
) -> ItemStack {
    (rows.iter()).fold(ItemStack::new(kind, count), |value, row| {
        row.diff(cells, prototype, value)
    })
}

fn equal(rows: &[Row], left: &Cells, right: &Cells) -> bool {
    bits(left.present() | right.present())
        .all(|index| rows.get(index).is_some_and(|row| row.equal(left, right)))
}

fn row_hash(rows: &[Row], kind: Kind, cells: &Cells) -> u64 {
    let mut hasher = FxHasher::default();
    hasher.write_u16(kind.0);
    for index in bits(cells.present()) {
        if let Some(row) = rows.get(index) {
            row.hash(cells, index, &mut hasher);
        }
    }
    hasher.finish()
}
