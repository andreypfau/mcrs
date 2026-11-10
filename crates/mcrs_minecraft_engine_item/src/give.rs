use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Kind;

use crate::Dropped;
use crate::Gift;
use crate::Given;
use crate::InSlot;
use crate::Slots;
use crate::merge_into::Offer;
use crate::merge_into::merge_into;
use crate::placement::Placement;
use crate::spawn_stack::spawn_stack;

/// What happens to the items that find no room.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Overflow {
    Keep,
    Drop(DVec3),
}

/// Gives `gift` to `holder` and writes [`Given`]. It does nothing when `holder` has no [`Slots`]
/// or the gift is not a stack.
pub(crate) fn give(world: &mut World, holder: Entity, gift: Gift, overflow: Overflow) {
    if world.get::<Slots>(holder).is_none() {
        return;
    }
    let (stack, origin) = match gift {
        Gift::Stack(stack) => (stack, Origin::Existing),
        Gift::Value(value) => {
            let entity = world.spawn_empty();
            let stack = entity.id();
            spawn_stack(entity, value);
            (stack, Origin::Spawned)
        }
    };
    match (put_in(world, holder, stack, overflow), origin) {
        (Some(given), _) => {
            world.write_message(given);
        }
        (None, Origin::Spawned) => {
            world.despawn(stack);
        }
        (None, Origin::Existing) => {}
    }
}

enum Origin {
    Existing,
    Spawned,
}

/// Merges `stack` into the stacks of `holder`, then moves the rest to the first free slot. A rest
/// without a free slot keeps its placement, or is dropped. A stack that gives all its items
/// despawns.
pub(crate) fn put_in(
    world: &mut World,
    holder: Entity,
    stack: Entity,
    overflow: Overflow,
) -> Option<Given> {
    world.get::<Kind>(stack)?;
    let merged = (world
        .run_system_cached_with(merge_into, Offer { holder, stack })
        .ok())??;
    let mut given = Given {
        holder,
        stack: merged.first,
        remainder: None,
    };
    let Some(rest) = Count::new(merged.rest) else {
        world.despawn(stack);
        return Some(given);
    };
    let mut entity = world.get_entity_mut(stack).ok()?;
    if entity.get::<Count>() != Some(&rest) {
        entity.insert(rest);
    }
    let in_holder = entity
        .get::<InSlot>()
        .is_some_and(|slot| slot.holder() == holder);
    let free = world.get::<Slots>(holder).and_then(Slots::first_free);
    let placement = match (in_holder, free, overflow) {
        (true, _, _) => None,
        (false, Some(index), _) => Some(Placement::Slot(InSlot::new(holder, index))),
        (false, None, Overflow::Keep) => {
            given.remainder = Some(stack);
            None
        }
        (false, None, Overflow::Drop(pos)) => {
            given.remainder = Some(stack);
            Some(Placement::World(Dropped { pos }))
        }
    };
    if given.remainder.is_none() {
        given.stack.get_or_insert(stack);
    }
    if let Some(placement) = placement {
        placement.apply(world, stack);
    }
    Some(given)
}
