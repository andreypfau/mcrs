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
use crate::placement::Placement;
use crate::placement::put_at;
use crate::placement::swap;
use crate::store::Target;
use crate::store::store;

/// The write commands of a stack. Each does nothing to an entity that is not a stack or no longer
/// exists.
pub trait StackCommandsExt {
    /// Gives the stack the component `value`.
    fn set<T: ItemComponent + Component>(&mut self, value: T) -> &mut Self;

    /// Removes the component of type `T` from the stack, also when its prototype has one.
    fn unset<T: ItemComponent + Component>(&mut self) -> &mut Self;

    /// Gives the stack the component of type `T` of its prototype.
    fn reset<T: ItemComponent + Component>(&mut self) -> &mut Self;

    /// Takes `count` items from the stack. The stack despawns when none is left, also when `count`
    /// is larger than its count.
    fn consume(&mut self, count: u8) -> &mut Self;

    /// Merges the stack into the stacks of `holder`, then puts the rest in the first free slot.
    /// The rest that finds no room keeps the placement that it had. The stack despawns when it
    /// gives all its items.
    fn put_in(&mut self, holder: Entity) -> &mut Self;

    /// Puts the stack in the slot `index` of `holder`. A stack in that slot loses its placement.
    /// It does nothing when the slot does not exist.
    fn put_at(&mut self, holder: Entity, index: u16) -> &mut Self;

    /// Exchanges the placements of the stack and `other`.
    fn swap_with(&mut self, other: Entity) -> &mut Self;

    /// Puts the stack in the world at `pos`.
    fn drop_at(&mut self, pos: DVec3) -> &mut Self;
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
}
