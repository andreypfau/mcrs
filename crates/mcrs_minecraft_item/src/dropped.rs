use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::prelude::Component;
use bevy_ecs::world::{DeferredWorld, World};
use mcrs_minecraft_protocol::item::RawStack;

use crate::held::Held;
use crate::mutate;

pub const LIFETIME: i16 = 6000;
pub const INFINITE_PICKUP_DELAY: i16 = 32767;
pub const INFINITE_LIFETIME: i16 = -32768;
pub const DEFAULT_HEALTH: i16 = 5;
pub const GRAVITY: f64 = 0.04;
pub const AIR_DRAG: f64 = 0.98;

/// The dropped item is the stack entity itself; it is never `Held`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[component(on_insert = DroppedItem::assert_unheld)]
pub struct DroppedItem {
    pub age: i16,
    pub pickup_delay: i16,
    pub health: i16,
}

impl DroppedItem {
    fn assert_unheld(world: DeferredWorld, ctx: HookContext) {
        debug_assert!(
            !world.entity(ctx.entity).contains::<Held>(),
            "{:?} is held and dropped at once",
            ctx.entity
        );
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thrower(pub Entity);

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct WireStack(pub RawStack);

/// Detaches the stack from its holder and turns it into a dropped item; the
/// caller adds its place in the world.
pub fn spawn_dropped(world: &mut World, stack: Entity, pickup_delay: i16, thrower: Option<Entity>) {
    mutate::detach(world, stack);
    let mut entity = world.entity_mut(stack);
    entity.insert(DroppedItem {
        age: 0,
        pickup_delay,
        health: DEFAULT_HEALTH,
    });
    if let Some(thrower) = thrower {
        entity.insert(Thrower(thrower));
    }
    mutate::bump(world, stack);
}
