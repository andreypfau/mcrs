use alloc::vec::Vec;

use bevy_ecs::system::EntityCommands;
use bevy_ecs::world::EntityWorldMut;
use bevy_math::DVec3;
use mcrs_minecraft_item_stack::ItemStack;

use crate::Gift;
use crate::Request;
use crate::commit::commit;
use crate::give::Overflow;
use crate::give::give;
use crate::load_inventory::load_inventory;

pub trait HolderCommandsExt {
    fn give(&mut self, gift: impl Into<Gift>) -> &mut Self;

    fn give_or_drop(&mut self, gift: impl Into<Gift>, pos: DVec3) -> &mut Self;

    fn request(&mut self, request: Request) -> &mut Self;

    fn load_inventory(&mut self, values: Vec<Option<ItemStack>>) -> &mut Self;
}

impl HolderCommandsExt for EntityCommands<'_> {
    fn give(&mut self, gift: impl Into<Gift>) -> &mut Self {
        let gift = gift.into();
        self.queue_silenced(move |entity: EntityWorldMut| {
            let holder = entity.id();
            give(entity.into_world_mut(), holder, gift, Overflow::Keep);
        })
    }

    fn give_or_drop(&mut self, gift: impl Into<Gift>, pos: DVec3) -> &mut Self {
        let gift = gift.into();
        self.queue_silenced(move |entity: EntityWorldMut| {
            let holder = entity.id();
            give(entity.into_world_mut(), holder, gift, Overflow::Drop(pos));
        })
    }

    fn request(&mut self, request: Request) -> &mut Self {
        self.queue_silenced(move |entity: EntityWorldMut| {
            let holder = entity.id();
            commit(entity.into_world_mut(), holder, request);
        })
    }

    fn load_inventory(&mut self, values: Vec<Option<ItemStack>>) -> &mut Self {
        self.queue_silenced(move |entity: EntityWorldMut| {
            let holder = entity.id();
            load_inventory(entity.into_world_mut(), holder, values);
        })
    }
}
