use bevy_ecs::system::Commands;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::world::EntityWorldMut;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;

use crate::spawn_stack::spawn_stack;

pub trait ItemCommandsExt {
    fn spawn_item(&mut self, kind: Kind) -> EntityCommands<'_>;

    fn spawn_stack(&mut self, value: ItemStack) -> EntityCommands<'_>;
}

impl ItemCommandsExt for Commands<'_, '_> {
    fn spawn_item(&mut self, kind: Kind) -> EntityCommands<'_> {
        let mut entity = self.spawn_empty();
        if let Some(one) = Count::new(1) {
            entity.queue_silenced(move |entity: EntityWorldMut| {
                spawn_stack(entity, ItemStack::new(kind, one));
            });
        }
        entity
    }

    fn spawn_stack(&mut self, value: ItemStack) -> EntityCommands<'_> {
        let mut entity = self.spawn_empty();
        entity.queue_silenced(move |entity: EntityWorldMut| spawn_stack(entity, value));
        entity
    }
}
