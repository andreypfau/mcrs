use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Neutral;
use mcrs_minecraft_item_table::ItemTable;

use crate::ComponentState;
use crate::StackRows;

/// Reads everything of a stack: its kind, its count and its effective components.
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

    /// The effective component of type `T`, or `None` when `T` is not an item component type of
    /// the world.
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

    /// The value form of the stack. Spawned with [`spawn_stack`], it gives an equal stack.
    ///
    /// [`spawn_stack`]: crate::ItemCommandsExt::spawn_stack
    #[must_use]
    pub fn to_value(&mut self, stack: Entity) -> Option<ItemStack> {
        let kind = self.kind(stack)?;
        let count = self.count(stack)?;
        let prototype = self.table.prototype(kind);
        let columns = self.rows.columns();
        let cells = columns.cells(stack)?;
        let value = (columns.rows().iter()).fold(ItemStack::new(kind, count), |value, row| {
            row.diff(&cells, prototype, value)
        });
        Some(value)
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
