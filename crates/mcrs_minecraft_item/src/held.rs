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
        if let Some(slot) = table.slots.get_mut(held.index as usize)
            && *slot == Some(ctx.entity)
        {
            *slot = None;
        }
        while table.slots.last() == Some(&None) {
            table.slots.pop();
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

/// The slots of a holder, mirrored from `Held` by its hooks; `slots` ends at
/// the last occupied one.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
#[component(on_insert = SlotTable::mirror_holds)]
pub struct SlotTable {
    slots: Vec<Option<Entity>>,
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
        assert!(
            self.accepts(index),
            "slot {index} is outside the table holding {stack:?}"
        );
        let index = index as usize;
        if index >= self.slots.len() {
            self.slots.resize(index + 1, None);
        }
        let slot = &mut self.slots[index];
        debug_assert!(slot.is_none_or(|by| by == stack));
        *slot = Some(stack);
    }

    pub fn fixed(len: usize) -> Self {
        Self {
            slots: Vec::new(),
            bound: Some(len),
        }
    }

    pub fn list() -> Self {
        Self {
            slots: Vec::new(),
            bound: None,
        }
    }

    pub fn get(&self, index: u16) -> Option<Entity> {
        self.slots.get(index as usize).copied().flatten()
    }

    pub fn len(&self) -> usize {
        self.bound.unwrap_or(self.slots.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (u16, Entity)> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.map(|entity| (index as u16, entity)))
    }

    pub fn first_free(&self, range: Range<u16>) -> Option<u16> {
        range
            .into_iter()
            .find(|index| self.accepts(*index) && self.get(*index).is_none())
    }

    pub fn accepts(&self, index: u16) -> bool {
        let index = index as usize;
        match self.bound {
            Some(bound) => index < bound,
            None => index <= self.slots.len(),
        }
    }
}
