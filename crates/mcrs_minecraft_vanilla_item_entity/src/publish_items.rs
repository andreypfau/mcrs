use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::query::Added;
use bevy_ecs::query::Changed;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Local;
use bevy_ecs::system::Populated;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::Ref;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_entity_motion_replication::Appearance;
use mcrs_minecraft_engine_entity_motion_replication::MotionId;
use mcrs_minecraft_engine_entity_motion_replication::PublishedTile;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::SpawnLook;

use crate::ItemCollector;
use crate::PickedUp;
use crate::wire_stack::wire_stack_of;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Release {
    pub(crate) id: EntityId,
    pub(crate) tile: TileKey,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct Released(pub(crate) Vec<Release>);

#[derive(Resource, Debug, Default)]
pub(crate) struct Takes(Vec<Take>);

#[derive(Clone, Copy, Debug)]
struct Take {
    item: EntityId,
    signal: MobSignal,
}

impl Takes {
    fn of(&self, item: EntityId) -> Option<MobSignal> {
        let at = self.0.binary_search_by_key(&item, |take| take.item).ok()?;
        self.0.get(at).map(|take| take.signal)
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "clears the takes of the last tick and corrects the count of the item entities spawned in this tick"
    )
)]
pub(crate) fn publish_takes(
    mut picked: MessageReader<PickedUp>,
    collectors: Query<&ItemCollector>,
    mut spawning: Query<(&Count, &mut Appearance), Added<MotionId>>,
    mut takes: ResMut<Takes>,
    mut released: ResMut<Released>,
    mut events: ResMut<TileEvents>,
) {
    takes.0.clear();
    takes.0.extend(picked.read().filter_map(|pickup| {
        let collector = collectors.get(pickup.collector).ok()?;
        Some(Take {
            item: pickup.item,
            signal: MobSignal::Taken {
                collector: collector.id,
                amount: i32::from(pickup.count),
            },
        })
    }));
    takes.0.sort_unstable_by_key(|take| take.item);
    for release in released.0.drain(..) {
        if let Some(take) = takes.of(release.id) {
            events.push(
                release.tile,
                release.id,
                EntityEvent::Data(MobReport::Signal(take).into()),
            );
        }
    }
    for (count, mut appearance) in &mut spawning {
        let count = i32::from(count.get());
        if let SpawnLook::Item(stack) = &mut appearance.bypass_change_detection().look
            && stack.count != count
        {
            stack.count = count;
            appearance.set_changed();
        }
    }
}

pub(crate) fn publish_stacks(
    items: Populated<(Entity, Ref<MotionId>, &PublishedTile), Changed<Count>>,
    mut stacks: Stacks,
    mut bytes: Local<Vec<u8>>,
    table: Res<ItemTable>,
    takes: Res<Takes>,
    mut events: ResMut<TileEvents>,
) {
    for (stack, id, published) in &items {
        if id.is_added() {
            continue;
        }
        let Some(tile) = published.tile() else {
            continue;
        };
        let Some(wire) = wire_stack_of(&mut stacks, &table, stack, &mut bytes) else {
            continue;
        };
        if let Some(take) = takes.of(id.0) {
            events.push(
                tile,
                id.0,
                EntityEvent::Data(MobReport::Signal(take).into()),
            );
        }
        events.push(tile, id.0, EntityEvent::Data(MobReport::Stack(wire).into()));
    }
}
