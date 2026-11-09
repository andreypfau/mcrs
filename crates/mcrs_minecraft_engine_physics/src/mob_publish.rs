use std::collections::BTreeMap;

use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_tasks::ComputeTaskPool;
use mcrs_engine_muxer::EntityEvent;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::SubjectEvent;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::MobSpawn;
use mcrs_minecraft_replication::MobState;
use mcrs_minecraft_replication::ReplicationData;
use rustc_hash::FxHashSet;
use uuid::Uuid;

use crate::entity_pass::mob_state;
use crate::entity_rows::EntityRows;
use crate::mob_life::MobChanges;
use crate::mob_moved::MobMoved;
use crate::mobs::MobUuids;
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
    mut uuids: ResMut<MobUuids>,
) {
    let mut fresh: BTreeMap<EntityId, MobState> = changes
        .spawned
        .iter()
        .map(|spawned| (spawned.id, spawned.state))
        .collect();
    for crossing in &rows.crossed {
        let Some(mob) = mobs.0.get_mut(&crossing.id) else {
            continue;
        };
        mob.tile = crossing.to;
        if fresh.contains_key(&crossing.id) {
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
    let spawn = |events: &mut TileEvents, id: EntityId, start: MobState, now: MobState| {
        let Some(mob) = mobs.0.get(&id) else {
            return;
        };
        events.push(
            tile_at(now.position),
            id,
            EntityEvent::Spawn(
                MobReport::Spawn(MobSpawn {
                    kind: mob.kind,
                    uuid: Uuid::from_u128(mob.uuid),
                    state: start,
                    looks: mob.looks.clone(),
                })
                .into(),
            ),
        );
    };
    for signaled in &changes.signals {
        events.push(
            signaled.at.tile,
            signaled.at.id,
            EntityEvent::Data(MobReport::Signal(signaled.signal).into()),
        );
    }
    let mut published: Vec<EntityId> = Vec::new();
    if fresh.is_empty() {
        let touched: FxHashSet<EntityId> = changes.touched.iter().map(|at| at.id).collect();
        let pool = ComputeTaskPool::get();
        let per_task = rows
            .moved
            .len()
            .div_ceil(pool.thread_num().saturating_mul(4).max(1))
            .max(MOVES_PER_TASK);
        let mobs = &*mobs;
        let touched = &touched;
        let batches: Vec<MoveBatches> = pool.scope(|scope| {
            for chunk in rows.moved.chunks(per_task) {
                scope.spawn(async move { move_batches(chunk, mobs, touched) });
            }
        });
        for batch in batches {
            for (tile, moves) in batch.tiles {
                events.append(tile, moves);
            }
            published.extend(batch.touched);
        }
    } else {
        for moved in &rows.moved {
            if let Some(start) = fresh.remove(&moved.id) {
                spawn(&mut events, moved.id, start, moved.state);
            }
            if mobs.0.contains_key(&moved.id) {
                events.push(
                    tile_at(moved.state.position),
                    moved.id,
                    EntityEvent::Move(MobReport::Move(moved.state).into()),
                );
                published.push(moved.id);
            }
        }
        for (id, start) in fresh {
            spawn(&mut events, id, start, start);
        }
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
                EntityEvent::Move(MobReport::Move(state).into()),
            );
        }
    }
    for removed in &rows.removed {
        let Some(mob) = mobs.0.remove(&removed.id) else {
            continue;
        };
        uuids.0.remove(&mob.uuid);
        events.push(
            removed.tile,
            removed.id,
            EntityEvent::Data(MobReport::Signal(MobSignal::Removed).into()),
        );
        events.push(removed.tile, removed.id, EntityEvent::Despawn);
    }
    changes.spawned.clear();
    changes.touched.clear();
    changes.signals.clear();
}

/// Below this count of moves a task of [`publish_mobs`] costs more than it saves.
const MOVES_PER_TASK: usize = 1024;

/// The move events of a run of moved rows, by tile in the order of the run, and the ids of the
/// moved rows that a writer also touched.
struct MoveBatches {
    tiles: Vec<(TileKey, Vec<SubjectEvent<ReplicationData>>)>,
    touched: Vec<EntityId>,
}

fn move_batches(moved: &[MobMoved], mobs: &Mobs, touched: &FxHashSet<EntityId>) -> MoveBatches {
    let mut out = MoveBatches {
        tiles: Vec::new(),
        touched: Vec::new(),
    };
    for moved in moved {
        if !mobs.0.contains_key(&moved.id) {
            continue;
        }
        let tile = tile_at(moved.state.position);
        let event = SubjectEvent {
            subject: moved.id,
            event: EntityEvent::Move(MobReport::Move(moved.state).into()),
        };
        match out.tiles.last_mut() {
            Some((last, moves)) if *last == tile => moves.push(event),
            _ => out.tiles.push((tile, vec![event])),
        }
        if touched.contains(&moved.id) {
            out.touched.push(moved.id);
        }
    }
    out
}
