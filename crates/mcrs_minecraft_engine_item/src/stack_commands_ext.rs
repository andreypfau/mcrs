use bevy_ecs::component::Component;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_item_stack::ItemComponent;

use crate::consume::consume;
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

    /// Takes `count` items from the stack. The stack despawns when none is left.
    fn consume(&mut self, count: u8) -> &mut Self;
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
}
