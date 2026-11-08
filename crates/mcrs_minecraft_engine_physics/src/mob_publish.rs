use std::collections::BTreeMap;

use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_engine_muxer::EntityEvent;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::MobSpawn;
use mcrs_minecraft_replication::MobState;
use mcrs_minecraft_replication::PlayerData;
use uuid::Uuid;

use crate::entity_pass::mob_state;
use crate::entity_rows::EntityRows;
use crate::mob_life::MobChanges;
use crate::mobs::Mobs;
use crate::tile_at::tile_at;

/// Records the tile events of the mobs of the tick: a spawn with the state of the start of its
/// tracker, the signals, a move for each row whose visible state changed, a change of tile, and
/// the removal at the end of a death. A mob that spawned in this tick spawns in the tile it ends
/// the tick in.
pub(crate) fn publish_mobs(
    rows: Res<EntityRows>,
    mut mobs: ResMut<Mobs>,
    mut changes: ResMut<MobChanges>,
    mut events: ResMut<TileEvents>,
) {
    let mut fresh: BTreeMap<EntityId, MobState> = changes
        .spawned
        .iter()
        .map(|spawned| (spawned.id, spawned.state))
        .collect();
    let spawn = |events: &mut TileEvents, id: EntityId, start: MobState, now: MobState| {
        let Some(mob) = mobs.0.get(&id) else {
            return;
        };
        events.push(
            tile_at(now.position),
            id,
            EntityEvent::Spawn(PlayerData::MobSpawn(MobSpawn {
                kind: mob.kind,
                uuid: Uuid::from_u128(mob.uuid),
                state: start,
                looks: mob.looks,
            })),
        );
    };
    for crossing in &rows.crossed {
        if fresh.contains_key(&crossing.id) || !mobs.0.contains_key(&crossing.id) {
            continue;
        }
        events.push(
            crossing.from,
            crossing.id,
            EntityEvent::Leave { to: crossing.to },
        );
        events.push(
            crossing.to,
            crossing.id,
            EntityEvent::Enter {
                from: crossing.from,
            },
        );
    }
    for signaled in &changes.signals {
        events.push(
            signaled.at.tile,
            signaled.at.id,
            EntityEvent::Data(PlayerData::MobSignal(signaled.signal)),
        );
    }
    let mut published: Vec<EntityId> = Vec::with_capacity(rows.moved.len());
    for moved in &rows.moved {
        if let Some(start) = fresh.remove(&moved.id) {
            spawn(&mut events, moved.id, start, moved.state);
        }
        if mobs.0.contains_key(&moved.id) {
            events.push(
                tile_at(moved.state.position),
                moved.id,
                EntityEvent::Move(PlayerData::MobMove(moved.state)),
            );
            published.push(moved.id);
        }
    }
    for (id, start) in fresh {
        spawn(&mut events, id, start, start);
    }
    published.sort_unstable();
    for touched in &changes.touched {
        if published.binary_search(&touched.id).is_ok() {
            continue;
        }
        if let Some(state) = rows.state_of(*touched).map(mob_state) {
            events.push(
                tile_at(state.position),
                touched.id,
                EntityEvent::Move(PlayerData::MobMove(state)),
            );
        }
    }
    for removed in &rows.removed {
        if mobs.0.remove(&removed.id).is_none() {
            continue;
        }
        events.push(
            removed.tile,
            removed.id,
            EntityEvent::Data(PlayerData::MobSignal(MobSignal::Removed)),
        );
        events.push(removed.tile, removed.id, EntityEvent::Despawn);
    }
    changes.spawned.clear();
    changes.touched.clear();
    changes.signals.clear();
}
