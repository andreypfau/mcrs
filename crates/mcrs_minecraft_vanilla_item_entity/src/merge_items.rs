use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::In;
use bevy_ecs::system::ParamSet;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_item_entity::ItemEntity;
use mcrs_minecraft_item_entity::Target;
use mcrs_minecraft_item_entity::merge_box;
use mcrs_minecraft_item_entity::merge_target;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_physics::Aabb;
use rustc_hash::FxHashSet;

use crate::Owner;
use crate::max_count::max_count;
use crate::step_items::Stepped;

pub(crate) const ITEM_SIZE: f32 = 0.25;

#[derive(Resource, Debug, Default)]
pub(crate) struct Merged(FxHashSet<Entity>);

pub(crate) fn merge_items(
    mut commands: Commands,
    stepped: Res<Stepped>,
    mut merged: ResMut<Merged>,
) {
    merged.0.clear();
    for item in stepped.0.iter().filter(|item| item.merges) {
        let reach = merge_box(Aabb::of_body(item.position, ITEM_SIZE, ITEM_SIZE));
        for other in &stepped.0 {
            if other.stack != item.stack
                && reach.intersects(&Aabb::of_body(other.position, ITEM_SIZE, ITEM_SIZE))
            {
                let pair = Pair {
                    first: item.stack,
                    second: other.stack,
                };
                commands.queue(move |world: &mut World| {
                    if let Err(error) = world.run_system_cached_with(merge_pair, pair) {
                        tracing::error!(%error, "the merge of two item entities did not run");
                    }
                });
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Pair {
    first: Entity,
    second: Entity,
}

fn merge_pair(
    In(pair): In<Pair>,
    mut commands: Commands,
    mut items: Query<(&mut ItemEntity, Option<&Owner>), With<Dropped>>,
    mut stacks: ParamSet<(Stacks, Query<&mut Count>)>,
    mut merged: ResMut<Merged>,
) {
    if merged.0.contains(&pair.first) || merged.0.contains(&pair.second) {
        return;
    }
    let Ok([(first, first_owner), (second, second_owner)]) =
        items.get_many([pair.first, pair.second])
    else {
        return;
    };
    let (first, second) = (*first, *second);
    let mut reader = stacks.p0();
    let (Some(first_count), Some(second_count)) =
        (reader.count(pair.first), reader.count(pair.second))
    else {
        return;
    };
    let (first_count, second_count) = (first_count.get(), second_count.get());
    let max = max_count(&mut reader, pair.first);
    let total = u16::from(first_count) + u16::from(second_count);
    let merges = first_owner == second_owner
        && first.can_merge(first_count, max)
        && second.can_merge(second_count, max)
        && total <= u16::from(max)
        && reader.mergeable(pair.first, pair.second);
    let Some(total) = u8::try_from(total).ok().and_then(Count::new) else {
        return;
    };
    if !merges {
        return;
    }
    let (target, source, absorbed) = match merge_target(first_count, second_count) {
        Target::First => (pair.first, pair.second, second),
        Target::Second => (pair.second, pair.first, first),
    };
    if let Ok(mut count) = stacks.p1().get_mut(target) {
        count.set_if_neq(total);
    }
    if let Ok((mut item, _)) = items.get_mut(target) {
        item.absorb(absorbed);
    }
    merged.0.extend([target, source]);
    commands.entity(source).despawn();
}
