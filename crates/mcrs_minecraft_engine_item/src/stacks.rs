use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::SystemParam;
use bevy_ecs::world::World;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Neutral;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::Prototype;

use crate::ComponentState;
use crate::StackRows;
use crate::cells::Cells;
use crate::item_components::ItemComponents;
use crate::row::Row;
use crate::row::RowKey;
use crate::row::bit;
use crate::row::bits;

#[derive(SystemParam)]
pub struct Stacks<'w, 's> {
    rows: StackRows<'w, 's>,
    counts: Query<'w, 's, &'static Count>,
    table: Res<'w, ItemTable>,
}

impl<'w> Stacks<'w, '_> {
    #[must_use]
    pub fn kind(&self, stack: Entity) -> Option<Kind> {
        self.rows.kind(stack)
    }

    #[must_use]
    pub fn count(&self, stack: Entity) -> Option<Count> {
        self.counts.get(stack).ok().copied()
    }

    #[must_use]
    pub fn get<T: ItemComponent + Component>(&mut self, stack: Entity) -> Option<&'w T> {
        self.kind(stack)?;
        let columns = self.rows.columns();
        let column = columns.column::<T>()?;
        columns.cells(stack)?.effective(column)
    }

    #[must_use]
    pub fn get_or_neutral<T: Neutral + Component>(&mut self, stack: Entity) -> &'w T {
        self.get(stack).unwrap_or(T::NEUTRAL)
    }

    #[must_use]
    pub fn state<T: ItemComponent + Component>(&mut self, stack: Entity) -> ComponentState<'w, T> {
        let Some(kind) = self.kind(stack) else {
            return ComponentState::Absent;
        };
        let prototype = self.table.prototype(kind);
        let columns = self.rows.columns();
        let (Some(column), Some(cells)) = (columns.column::<T>(), columns.cells(stack)) else {
            return ComponentState::Absent;
        };
        ComponentState::of(cells.effective(column), cells.removed(), prototype)
    }

    #[must_use]
    pub fn to_value(&mut self, stack: Entity) -> Option<ItemStack> {
        let kind = self.kind(stack)?;
        let count = self.count(stack)?;
        let prototype = self.table.prototype(kind);
        let columns = self.rows.columns();
        let cells = columns.cells(stack)?;
        Some(value_of(columns.rows(), &cells, prototype, kind, count))
    }

    #[must_use]
    pub fn value_in(world: &World, stack: Entity) -> Option<ItemStack> {
        let kind = *world.get::<Kind>(stack)?;
        let count = *world.get::<Count>(stack)?;
        let prototype = world.get_resource::<ItemTable>()?.prototype(kind);
        let rows = world.get_resource::<ItemComponents>()?.rows();
        let cells = Cells::in_world(world, stack, rows)?;
        Some(value_of(rows, &cells, prototype, kind, count))
    }

    #[must_use]
    pub fn equals_value(&mut self, stack: Entity, value: &ItemStack) -> bool {
        if self.kind(stack) != Some(value.kind) || self.count(stack) != Some(value.count) {
            return false;
        }
        let prototype = self.table.prototype(value.kind);
        let columns = self.rows.columns();
        let Some(cells) = columns.cells(stack) else {
            return false;
        };
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
        let rows = columns.rows();
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
        self.to_value(stack).as_ref() == Some(value)
    }

    #[must_use]
    pub fn mergeable(&mut self, left: Entity, right: Entity) -> bool {
        self.rows.mergeable(left, right)
    }

    #[must_use]
    pub fn row_hash(&mut self, stack: Entity) -> Option<u64> {
        self.rows.row_hash(stack)
    }
}

fn value_of(
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
