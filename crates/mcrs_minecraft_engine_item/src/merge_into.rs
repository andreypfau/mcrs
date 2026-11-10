use bevy_ecs::entity::Entity;
use bevy_ecs::system::In;
use bevy_ecs::system::ParamSet;
use bevy_ecs::system::Query;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::MaxStackSize;

use crate::Slots;
use crate::Stacks;

/// The slot limit of a slot whose holder gives none (ITX-LIM-001).
pub(crate) const SLOT_LIMIT: u8 = 99;

pub(crate) struct Offer {
    pub(crate) holder: Entity,
    pub(crate) stack: Entity,
}

pub(crate) struct Merged {
    /// The items of the offered stack that no stack of the holder took.
    pub(crate) rest: u8,
    pub(crate) first: Option<Entity>,
}

/// Moves items of the offered stack into the mergeable stacks of the holder, in the order of their
/// slots, up to the capacity of each. The offered stack keeps its count.
pub(crate) fn merge_into(
    In(offer): In<Offer>,
    holders: Query<&Slots>,
    mut stacks: ParamSet<(Stacks, Query<&mut Count>)>,
) -> Option<Merged> {
    let slots = holders.get(offer.holder).ok()?;
    let mut reader = stacks.p0();
    let mut rest = reader.count(offer.stack)?.get();
    let capacity = capacity(&mut reader, offer.stack);
    let mut first = None;
    for target in slots.occupied() {
        if rest == 0 {
            break;
        }
        if target == offer.stack {
            continue;
        }
        let mut reader = stacks.p0();
        let Some(count) = reader.count(target) else {
            continue;
        };
        let room = capacity.saturating_sub(count.get());
        if room == 0 || !reader.mergeable(offer.stack, target) {
            continue;
        }
        let moved = rest.min(room);
        let Some(next) = count.get().checked_add(moved).and_then(Count::new) else {
            continue;
        };
        let mut counts = stacks.p1();
        let Ok(mut count) = counts.get_mut(target) else {
            continue;
        };
        *count = next;
        rest = rest.saturating_sub(moved);
        first.get_or_insert(target);
    }
    Some(Merged { rest, first })
}

/// The smaller of the slot limit and the maximum stack size, which is 1 without the component
/// (ITX-LIM-002, STK-CMP-001).
pub(crate) fn capacity(stacks: &mut Stacks, stack: Entity) -> u8 {
    let max = stacks.get::<MaxStackSize>(stack).map_or(1, |max| max.0);
    u8::try_from(max.clamp(1, i32::from(SLOT_LIMIT))).unwrap_or(1)
}
