use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::world::EntityWorldMut;
use bevy_math::DVec3;
use mcrs_minecraft_item_stack::ItemComponent;

use crate::Dropped;
use crate::consume::consume;
use crate::give::Overflow;
use crate::give::put_in;
use crate::in_slot::touch;
use crate::placement::Placement;
use crate::placement::put_at;
use crate::placement::swap;
use crate::store::Target;
use crate::store::store;

pub trait StackCommandsExt {
    fn set<T: ItemComponent + Component>(&mut self, value: T) -> &mut Self;

    fn unset<T: ItemComponent + Component>(&mut self) -> &mut Self;

    fn reset<T: ItemComponent + Component>(&mut self) -> &mut Self;

    fn consume(&mut self, count: u8) -> &mut Self;

    fn put_in(&mut self, holder: Entity) -> &mut Self;

    fn put_at(&mut self, holder: Entity, index: u16) -> &mut Self;

    fn swap_with(&mut self, other: Entity) -> &mut Self;

    fn drop_at(&mut self, pos: DVec3) -> &mut Self;

    fn touch(&mut self) -> &mut Self;
}

impl StackCommandsExt for EntityCommands<'_> {
    fn set<T: ItemComponent + Component>(&mut self, value: T) -> &mut Self {
        self.queue_silenced(move |entity: EntityWorldMut| store(entity, Target::Value(value)))
    }

    fn unset<T: ItemComponent + Component>(&mut self) -> &mut Self {
        self.queue_silenced(|entity: EntityWorldMut| store::<T>(entity, Target::Removed))
    }

    fn reset<T: ItemComponent + Component>(&mut self) -> &mut Self {
        self.queue_silenced(|entity: EntityWorldMut| store::<T>(entity, Target::Prototype))
    }

    fn consume(&mut self, count: u8) -> &mut Self {
        self.queue_silenced(move |entity: EntityWorldMut| consume(entity, count))
    }

    fn put_in(&mut self, holder: Entity) -> &mut Self {
        self.queue_silenced(move |entity: EntityWorldMut| {
            let stack = entity.id();
            put_in(entity.into_world_mut(), holder, stack, Overflow::Keep);
        })
    }

    fn put_at(&mut self, holder: Entity, index: u16) -> &mut Self {
        self.queue_silenced(move |entity: EntityWorldMut| {
            let stack = entity.id();
            put_at(entity.into_world_mut(), stack, holder, index);
        })
    }

    fn swap_with(&mut self, other: Entity) -> &mut Self {
        self.queue_silenced(move |entity: EntityWorldMut| {
            let stack = entity.id();
            swap(entity.into_world_mut(), stack, other);
        })
    }

    fn drop_at(&mut self, pos: DVec3) -> &mut Self {
        self.queue_silenced(move |entity: EntityWorldMut| {
            let stack = entity.id();
            Placement::World(Dropped { pos }).apply(entity.into_world_mut(), stack);
        })
    }

    fn touch(&mut self) -> &mut Self {
        self.queue_silenced(|mut entity: EntityWorldMut| touch(&mut entity))
    }
}
