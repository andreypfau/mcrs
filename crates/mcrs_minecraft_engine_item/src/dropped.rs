use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::world::DeferredWorld;
use bevy_ecs::world::World;
use bevy_math::DVec3;

use crate::InSlot;

/// The stack lies in the world at `pos`. An insert removes [`InSlot`].
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[component(on_insert = leave_slot)]
pub struct Dropped {
    pub pos: DVec3,
}

fn leave_slot(mut world: DeferredWorld, context: HookContext) {
    let stack = context.entity;
    if world.entity(stack).contains::<InSlot>() {
        world
            .commands()
            .queue(move |world: &mut World| take_slot(world, stack));
    }
}

fn take_slot(world: &mut World, stack: Entity) {
    if let Ok(mut entity) = world.get_entity_mut(stack)
        && entity.contains::<Dropped>()
    {
        entity.remove::<InSlot>();
    }
}
