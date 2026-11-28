use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::message::Messages;
use bevy_ecs::world::DeferredWorld;
use bevy_ecs::world::EntityWorldMut;
use bevy_ecs::world::World;

use crate::Dropped;
use crate::SlotChanged;
use crate::SlotRef;
use crate::Slots;
use crate::slot_changed::write_slot_changed;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[component(immutable, on_insert = occupy, on_discard = vacate)]
pub struct InSlot {
    holder: Entity,
    index: u16,
}

impl InSlot {
    pub(crate) fn new(holder: Entity, index: u16) -> Self {
        Self { holder, index }
    }

    #[must_use]
    pub fn holder(&self) -> Entity {
        self.holder
    }

    #[must_use]
    pub fn index(&self) -> u16 {
        self.index
    }

    pub(crate) fn slot_ref(self) -> SlotRef {
        SlotRef {
            holder: self.holder,
            index: self.index,
        }
    }
}

fn occupy(mut world: DeferredWorld, context: HookContext) {
    let stack = context.entity;
    let Some(slot) = world.get::<InSlot>(stack).copied() else {
        return;
    };
    let previous = (world.get_mut::<Slots>(slot.holder))
        .and_then(|mut slots| slots.replace(slot.index, Some(stack)));
    if previous.is_some_and(|evicted| evicted != Some(stack)) {
        write_slot_changed(
            world.get_resource_mut::<Messages<SlotChanged>>(),
            slot.slot_ref(),
        );
    }
    let detached = match previous {
        None => Some(stack),
        Some(evicted) => evicted.filter(|evicted| *evicted != stack),
    };
    let dropped = world.entity(stack).contains::<Dropped>();
    let mut commands = world.commands();
    if let Some(detached) = detached {
        commands.queue(move |world: &mut World| detach(world, detached, slot));
    }
    if dropped {
        commands.queue(move |world: &mut World| leave_world(world, stack, slot));
    }
}

fn vacate(mut world: DeferredWorld, context: HookContext) {
    let stack = context.entity;
    let Some(slot) = world.get::<InSlot>(stack).copied() else {
        return;
    };
    let Some(mut slots) = world.get_mut::<Slots>(slot.holder) else {
        return;
    };
    if slots.get(usize::from(slot.index)) == Some(stack) {
        slots.replace(slot.index, None);
        write_slot_changed(
            world.get_resource_mut::<Messages<SlotChanged>>(),
            slot.slot_ref(),
        );
    }
}

fn detach(world: &mut World, stack: Entity, slot: InSlot) {
    let holds = (world.get::<Slots>(slot.holder))
        .and_then(|slots| slots.get(usize::from(slot.index)))
        == Some(stack);
    if holds {
        return;
    }
    if let Ok(mut entity) = world.get_entity_mut(stack)
        && entity.get::<InSlot>() == Some(&slot)
    {
        entity.remove::<InSlot>();
    }
}

fn leave_world(world: &mut World, stack: Entity, slot: InSlot) {
    if let Ok(mut entity) = world.get_entity_mut(stack)
        && entity.get::<InSlot>() == Some(&slot)
    {
        entity.remove::<Dropped>();
    }
}

pub(crate) fn touch(stack: &mut EntityWorldMut) {
    let Some(slot) = stack.get::<InSlot>().copied() else {
        return;
    };
    stack.world_scope(|world| {
        write_slot_changed(
            world.get_resource_mut::<Messages<SlotChanged>>(),
            slot.slot_ref(),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_in_slot_that_names_a_missing_slot_is_removed_again() {
        let mut world = World::new();
        let holder = world.spawn(Slots::new(2)).id();
        let without_slots = world.spawn_empty().id();
        for slot in [InSlot::new(without_slots, 0), InSlot::new(holder, 2)] {
            let stack = world.spawn(slot).id();
            world.flush();
            assert!(!world.entity(stack).contains::<InSlot>(), "{slot:?}");
        }
        let slots: Option<Vec<Option<Entity>>> =
            (world.get::<Slots>(holder)).map(|slots| slots.iter().collect());
        assert_eq!(slots, Some(vec![None, None]));
    }
}
