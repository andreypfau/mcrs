use std::ops::Range;

use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::prelude::Component;
use bevy_ecs::world::DeferredWorld;

use crate::dropped::DroppedItem;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[relationship(relationship_target = Holds)]
#[component(on_insert = Held::mirror_insert, on_discard = Held::mirror_discard)]
pub struct Held {
    #[relationship]
    pub holder: Entity,
    pub index: u16,
}

impl Held {
    fn mirror_insert(mut world: DeferredWorld, ctx: HookContext) {
        let held = *world.get::<Held>(ctx.entity).unwrap();
        debug_assert!(
            !world.entity(ctx.entity).contains::<DroppedItem>(),
            "{:?} is held and dropped at once",
            ctx.entity
        );
        let Some(mut table) = world.get_mut::<SlotTable>(held.holder) else {
            return;
        };
        table.place(held.index, ctx.entity);
    }

    fn mirror_discard(mut world: DeferredWorld, ctx: HookContext) {
        let held = *world.get::<Held>(ctx.entity).unwrap();
        // The holder is already gone while its subtree cascades out of a despawn.
        let Some(mut table) = world.get_mut::<SlotTable>(held.holder) else {
            return;
        };
        if let Some(cell) = table.cells.get_mut(held.index as usize)
            && *cell == Some(ctx.entity)
        {
            *cell = None;
        }
        while table.cells.last() == Some(&None) {
            table.cells.pop();
        }
    }
}

#[derive(Component, Debug)]
#[relationship_target(relationship = Held, linked_spawn)]
pub struct Holds(Vec<Entity>);

impl Holds {
    pub fn entities(&self) -> &[Entity] {
        &self.0
    }
}

/// The cells of a holder, mirrored from `Held` by its hooks; `cells` ends at
/// the last occupied one.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(on_insert = SlotTable::mirror_holds)]
pub struct SlotTable {
    cells: Vec<Option<Entity>>,
    bound: Option<usize>,
}

impl SlotTable {
    fn mirror_holds(mut world: DeferredWorld, ctx: HookContext) {
        let Some(holds) = world.get::<Holds>(ctx.entity) else {
            return;
        };
        let held: Vec<(u16, Entity)> = holds
            .entities()
            .iter()
            .map(|&stack| (world.get::<Held>(stack).unwrap().index, stack))
            .collect();
        let mut table = world.get_mut::<SlotTable>(ctx.entity).unwrap();
        for (index, stack) in held {
            table.place(index, stack);
        }
    }

    fn place(&mut self, index: u16, stack: Entity) {
        assert!(self.accepts(index), "cell {index} is outside the table holding {stack:?}");
        let index = index as usize;
        if index >= self.cells.len() {
            self.cells.resize(index + 1, None);
        }
        let cell = &mut self.cells[index];
        debug_assert!(cell.is_none_or(|by| by == stack));
        *cell = Some(stack);
    }

    pub fn fixed(len: usize) -> Self {
        Self {
            cells: Vec::new(),
            bound: Some(len),
        }
    }

    pub fn list() -> Self {
        Self {
            cells: Vec::new(),
            bound: None,
        }
    }

    pub fn is_growable(&self) -> bool {
        self.bound.is_none()
    }

    pub fn get(&self, index: u16) -> Option<Entity> {
        self.cells.get(index as usize).copied().flatten()
    }

    pub fn len(&self) -> usize {
        self.bound.unwrap_or(self.cells.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (u16, Entity)> + '_ {
        self.cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| cell.map(|entity| (index as u16, entity)))
    }

    pub fn first_free(&self, range: Range<u16>) -> Option<u16> {
        range
            .into_iter()
            .find(|index| self.accepts(*index) && self.get(*index).is_none())
    }

    pub(crate) fn accepts(&self, index: u16) -> bool {
        let index = index as usize;
        match self.bound {
            Some(bound) => index < bound,
            None => index <= self.cells.len(),
        }
    }
}
