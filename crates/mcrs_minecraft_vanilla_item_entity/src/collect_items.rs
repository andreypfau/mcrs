use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::Local;
use bevy_ecs::system::ParamSet;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Slot;
use mcrs_minecraft_item_entity::can_pick_up;
use mcrs_minecraft_item_entity::insert;
use mcrs_minecraft_item_entity::pickup_box;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_physics::Aabb;

use crate::Collect;
use crate::ItemCollector;
use crate::Owner;
use crate::PickedUp;
use crate::item_row::ItemRow;
use crate::max_count::max_count;
use crate::merge_items::ITEM_SIZE;
use crate::step_items::Stepped;

/// The item entities that a collector won in this tick: no other collector takes from them.
#[derive(Resource, Debug, Default)]
pub(crate) struct Claimed(Vec<Entity>);

/// Queues the pickup of each item entity in reach of a collector, in the order of the item
/// UUIDs, then of the collector UUIDs. The first collector that takes an item wins the item
/// entity (IEN-PCK-001, IEN-PCK-002, IEN-PCK-007).
pub(crate) fn collect_items(
    mut commands: Commands,
    stepped: Res<Stepped>,
    items: Query<(&ItemEntity, Option<&Owner>)>,
    collectors: Query<(Entity, &ItemCollector)>,
    mut claimed: ResMut<Claimed>,
    mut sorted: Local<Vec<(u128, Entity, Aabb)>>,
) {
    claimed.0.clear();
    sorted.clear();
    sorted.extend(collectors.iter().map(|(entity, collector)| {
        (
            collector.uuid,
            entity,
            pickup_box(collector.bounds, collector.vehicle),
        )
    }));
    sorted.sort_unstable_by_key(|(uuid, entity, _)| (*uuid, *entity));
    for stepped in &stepped.0 {
        let Ok((item, owner)) = items.get(stepped.stack) else {
            continue;
        };
        let bounds = Aabb::of_body(stepped.position, ITEM_SIZE, ITEM_SIZE);
        let owner = owner.map(|owner| owner.0);
        for &(uuid, collector, reach) in sorted.iter() {
            if can_pick_up(item.pickup_delay, owner, uuid) && reach.intersects(&bounds) {
                let offer = Offer {
                    stack: stepped.stack,
                    collector,
                };
                commands.queue(move |world: &mut World| {
                    if let Err(error) = world.run_system_cached_with(pick_up, offer) {
                        tracing::error!(%error, "the pickup of an item entity did not run");
                    }
                });
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Offer {
    stack: Entity,
    collector: Entity,
}

/// Inserts the items of a dropped stack into the slots of a collector (IEN-PCK-003 to
/// IEN-PCK-006).
fn pick_up(
    In(offer): In<Offer>,
    mut commands: Commands,
    collectors: Query<(&Slots, Option<&Selected>, &ItemCollector)>,
    dropped: Query<&ItemRow, With<Dropped>>,
    mut stacks: ParamSet<(Stacks, Query<&mut Count>)>,
    mut picked: MessageWriter<PickedUp>,
    mut claimed: ResMut<Claimed>,
) {
    if claimed.0.contains(&offer.stack) {
        return;
    }
    let (Ok((slots, selected, collector)), Ok(row)) =
        (collectors.get(offer.collector), dropped.get(offer.stack))
    else {
        return;
    };
    let mut reader = stacks.p0();
    let Some(count) = reader.count(offer.stack).map(Count::get) else {
        return;
    };
    let max = max_count(&mut reader, offer.stack);
    let damaged = reader
        .get::<Damage>(offer.stack)
        .is_some_and(|damage| damage.0 > 0);
    let slot = |index: u16| match slots.get(usize::from(index)) {
        None if usize::from(index) < slots.len() => Slot::Empty,
        Some(held) if reader.mergeable(offer.stack, held) => {
            let held_count = reader.count(held).map_or(max, Count::get);
            Slot::Alike {
                room: max.saturating_sub(held_count),
            }
        }
        _ => Slot::Other,
    };
    let mut merges = Vec::new();
    let end = insert(
        count,
        max,
        damaged,
        selected.map_or(0, |selected| selected.0),
        slot,
        |index, moved| merges.push((index, moved)),
    );
    let stays = match collector.mode {
        Collect::Survival => end.rest,
        Collect::Creative => 0,
    };
    if stays == count {
        return;
    }
    claimed.0.push(offer.stack);
    let mut counts = stacks.p1();
    for (index, moved) in merges {
        if let Some(held) = slots.get(usize::from(index))
            && let Ok(mut held_count) = counts.get_mut(held)
            && let Some(next) = held_count.get().checked_add(moved).and_then(Count::new)
        {
            *held_count = next;
        }
    }
    let filled = end.empty.zip(Count::new(end.filled));
    match (filled, Count::new(end.rest)) {
        (Some((index, filled)), None) => {
            if let Ok(mut own) = counts.get_mut(offer.stack) {
                *own = filled;
            }
            commands.entity(offer.stack).put_at(offer.collector, index);
        }
        (Some((index, filled)), Some(rest)) => {
            if let Some(mut value) = stacks.p0().to_value(offer.stack) {
                value.count = filled;
                commands.spawn_stack(value).put_at(offer.collector, index);
            }
            keep_or_destroy(
                &mut commands,
                &mut stacks.p1(),
                offer.stack,
                rest,
                collector.mode,
            );
        }
        (None, Some(rest)) => {
            keep_or_destroy(
                &mut commands,
                &mut counts,
                offer.stack,
                rest,
                collector.mode,
            );
        }
        (None, None) => {
            commands.entity(offer.stack).despawn();
        }
    }
    picked.write(PickedUp {
        item: row.id,
        collector: offer.collector,
        count: count - stays,
    });
}

/// Leaves the `rest` that found no room in the item entity, or destroys it for a creative
/// collector (IEN-PCK-005, IEN-PCK-006).
fn keep_or_destroy(
    commands: &mut Commands,
    counts: &mut Query<&mut Count>,
    stack: Entity,
    rest: Count,
    mode: Collect,
) {
    if mode == Collect::Creative {
        commands.entity(stack).despawn();
    } else if let Ok(mut count) = counts.get_mut(stack) {
        *count = rest;
    }
}
