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
use mcrs_minecraft_item_table::ItemTable;

use crate::ComponentState;
use crate::StackRows;
use crate::cells::Cells;
use crate::item_components::ItemComponents;
use crate::row::row_mask;
use crate::stack_rows::value_of;

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
        self.rows.get(stack)
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
        self.rows.state(stack, self.table.prototype(kind))
    }

    #[must_use]
    pub fn to_value(&mut self, stack: Entity) -> Option<ItemStack> {
        let kind = self.kind(stack)?;
        let count = self.count(stack)?;
        self.rows
            .value(stack, self.table.prototype(kind), kind, count)
    }

    #[must_use]
    pub fn value_in(world: &World, stack: Entity) -> Option<ItemStack> {
        let kind = *world.get::<Kind>(stack)?;
        let count = *world.get::<Count>(stack)?;
        let prototype = world.get_resource::<ItemTable>()?.prototype(kind);
        let rows = world.get_resource::<ItemComponents>()?.rows();
        let cells = Cells::in_world(world, stack, |table| {
            row_mask(rows, |id| table.has_column(id))
        })?;
        Some(value_of(rows, &cells, prototype, kind, count))
    }

    #[must_use]
    pub fn equals_value(&mut self, stack: Entity, value: &ItemStack) -> bool {
        if self.kind(stack) != Some(value.kind) || self.count(stack) != Some(value.count) {
            return false;
        }
        self.rows
            .equals_value(stack, value, self.table.prototype(value.kind))
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
