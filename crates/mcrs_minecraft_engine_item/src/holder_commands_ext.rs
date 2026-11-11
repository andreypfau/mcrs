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

/// The commands of a holder: an entity with [`Slots`](crate::Slots).
///
/// The commands need [`ItemPlugin`](crate::ItemPlugin) and do nothing without it.
pub trait HolderCommandsExt {
    /// Merges the gift into the stacks of the holder, then puts the rest in the first free slot.
    /// The rest that finds no room keeps the placement that it had; a given value becomes a stack
    /// without placement. Writes [`Given`](crate::Given). A stack that gives all its items
    /// despawns.
    fn give(&mut self, gift: impl Into<Gift>) -> &mut Self;

    /// As [`give`](Self::give), but the rest that finds no room is dropped at `pos`.
    fn give_or_drop(&mut self, gift: impl Into<Gift>, pos: DVec3) -> &mut Self;

    /// Applies every operation of the request, or none. Writes and triggers
    /// [`RequestResult`](crate::RequestResult) for the holder. A split to an empty slot spawns one
    /// new stack; a stack that gives all its items to another stack despawns; every other stack
    /// keeps its entity.
    fn request(&mut self, request: Request) -> &mut Self;

    /// Gives the holder one slot for each value, with a new stack in each slot whose value is
    /// `Some`. The stacks that the holder had despawn.
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
            commit(entity.into_world_mut(), holder, &request);
        })
    }

    fn load_inventory(&mut self, values: Vec<Option<ItemStack>>) -> &mut Self {
        self.queue_silenced(move |entity: EntityWorldMut| {
            let holder = entity.id();
            load_inventory(entity.into_world_mut(), holder, values);
        })
    }
}
