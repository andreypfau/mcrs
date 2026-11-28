use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::In;
use bevy_ecs::system::ParamSet;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_item_stack::Count;

use crate::InSlot;
use crate::SlotChanged;
use crate::Slots;
use crate::Stacks;
use crate::capacity::capacity;

pub(crate) struct Offer {
    pub(crate) holder: Entity,
    pub(crate) stack: Entity,
}

pub(crate) struct Merged {
    pub(crate) rest: u8,
    pub(crate) first: Option<Entity>,
}

pub(crate) fn merge_into(
    In(offer): In<Offer>,
    holders: Query<&Slots>,
    mut stacks: ParamSet<(Stacks, Query<(&mut Count, Option<&InSlot>)>)>,
    mut changes: Option<ResMut<Messages<SlotChanged>>>,
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
        let Ok((mut count, slot)) = counts.get_mut(target) else {
            continue;
        };
        count.set_if_neq(next);
        if let (Some(changes), Some(slot)) = (changes.as_mut(), slot) {
            changes.write(SlotChanged {
                slot: slot.slot_ref(),
            });
        }
        rest = rest.saturating_sub(moved);
        first.get_or_insert(target);
    }
    Some(Merged { rest, first })
}
