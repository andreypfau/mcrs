use std::collections::BTreeMap;

use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_tasks::ComputeTaskPool;
use mcrs_engine_muxer::EntityEvent;
use mcrs_engine_muxer::SubjectEvent;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_physics::MobChanges;
use mcrs_minecraft_engine_physics::Mobs;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_entity_rows::TileRows;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::MobSpawn;
use mcrs_minecraft_replication::MobState;
use mcrs_minecraft_replication::ReplicationData;
use rustc_hash::FxHashSet;
use uuid::Uuid;

use crate::mob_state::mob_state;

/// Below this count of moves a task of [`publish_mobs`] costs more than it saves.
const MOVES_PER_TASK: usize = 1024;

/// Records the tile events of the mobs of the tick: a spawn with the state of the start of its
/// tracker, the signals, a move for each row whose visible state changed, a change of tile, and
/// the removal at the end of a death. A mob that spawned in this tick spawns in the tile it ends
/// the tick in.
pub(crate) fn publish_mobs<M: Send + Sync + 'static>(
    rows: Res<RowStore<M>>,
    mobs: Res<Mobs>,
    changes: Res<MobChanges>,
    mut events: ResMut<TileEvents>,
) {
    let mut fresh: BTreeMap<EntityId, MobState> = changes
        .spawned()
        .iter()
        .map(|spawned| (spawned.id, mob_state(&spawned.start)))
        .collect();
    for crossing in rows.changes().crossed() {
        if !mobs.0.contains_key(&crossing.id) || fresh.contains_key(&crossing.id) {
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
            TileKey::at(now.position),
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
    for struck in changes.strikes() {
        let mut signal = |signal: MobSignal| {
            events.push(
                struck.at.tile,
                struck.at.id,
                EntityEvent::Data(MobReport::Signal(signal).into()),
            );
        };
        if struck.outcome == HitOutcome::Full {
            signal(MobSignal::Hurt {
                damage_type: struck.damage_type,
            });
        }
        if struck.died {
            signal(MobSignal::Died);
        }
    }
    let mut published: Vec<EntityId> = Vec::new();
    if fresh.is_empty() {
        let touched: FxHashSet<EntityId> = changes.touched().iter().map(|at| at.id).collect();
        let pool = ComputeTaskPool::get();
        let per_task = rows
            .moved()
            .len()
            .div_ceil(pool.thread_num().saturating_mul(4).max(1))
            .max(MOVES_PER_TASK);
        let rows = &*rows;
        let mobs = &*mobs;
        let touched = &touched;
        let batches: Vec<MoveBatches> = pool.scope(|scope| {
            for chunk in rows.moved().chunks(per_task) {
                scope.spawn(async move { move_batches(rows, chunk, mobs, touched) });
            }
        });
        for batch in batches {
            for (tile, moves) in batch.tiles {
                events.append(tile, moves);
            }
            published.extend(batch.touched);
        }
    } else {
        let mut states = MovedStates::new(&rows);
        for &at in rows.moved() {
            let Some(state) = states.state(at).map(mob_state) else {
                continue;
            };
            if let Some(start) = fresh.remove(&at.id) {
                spawn(&mut events, at.id, start, state);
            }
            if mobs.0.contains_key(&at.id) {
                events.push(
                    TileKey::at(state.position),
                    at.id,
                    EntityEvent::Move(MobReport::Move(state).into()),
                );
                published.push(at.id);
            }
        }
        for (id, start) in fresh {
            spawn(&mut events, id, start, start);
        }
    }
    published.sort_unstable();
    for touched in changes.touched() {
        if published.binary_search(&touched.id).is_ok() {
            continue;
        }
        if let Some(state) = rows.state_of(*touched).map(mob_state) {
            events.push(
                TileKey::at(state.position),
                touched.id,
                EntityEvent::Move(MobReport::Move(state).into()),
            );
        }
    }
    for removed in rows.changes().removed() {
        if !mobs.0.contains_key(&removed.id) {
            continue;
        }
        events.push(
            removed.tile,
            removed.id,
            EntityEvent::Data(MobReport::Signal(MobSignal::Removed).into()),
        );
        events.push(removed.tile, removed.id, EntityEvent::Despawn);
    }
}

/// The move events of a run of moved rows, by tile in the order of the run, and the ids of the
/// moved rows that a writer also touched.
struct MoveBatches {
    tiles: Vec<(TileKey, Vec<SubjectEvent<ReplicationData>>)>,
    touched: Vec<EntityId>,
}

fn move_batches<M>(
    rows: &RowStore<M>,
    moved: &[Placed],
    mobs: &Mobs,
    touched: &FxHashSet<EntityId>,
) -> MoveBatches {
    let mut out = MoveBatches {
        tiles: Vec::new(),
        touched: Vec::new(),
    };
    let mut states = MovedStates::new(rows);
    for &at in moved {
        if !mobs.0.contains_key(&at.id) {
            continue;
        }
        let Some(state) = states.state(at).map(mob_state) else {
            continue;
        };
        let tile = TileKey::at(state.position);
        let event = SubjectEvent {
            subject: at.id,
            event: EntityEvent::Move(MobReport::Move(state).into()),
        };
        match out.tiles.last_mut() {
            Some((last, moves)) if *last == tile => moves.push(event),
            _ => out.tiles.push((tile, vec![event])),
        }
        if touched.contains(&at.id) {
            out.touched.push(at.id);
        }
    }
    out
}

/// The states of the moved rows after the tile moves. The moved rows come in tile order, so it
/// keeps the last tile of the pass that it looked up.
struct MovedStates<'rows, M> {
    rows: &'rows RowStore<M>,
    last: Option<LastTile<'rows, M>>,
}

struct LastTile<'rows, M> {
    key: TileKey,
    rows: Option<&'rows TileRows<M>>,
}

impl<'rows, M> MovedStates<'rows, M> {
    fn new(rows: &'rows RowStore<M>) -> Self {
        Self { rows, last: None }
    }

    /// The state of the moved row `at`; `None` when the row left the world.
    fn state(&mut self, at: Placed) -> Option<&'rows EntityState> {
        let tile = match &self.last {
            Some(last) if last.key == at.tile => last.rows,
            _ => {
                let rows = self.rows.tile(at.tile);
                self.last = Some(LastTile { key: at.tile, rows });
                rows
            }
        };
        if let Some(tile) = tile
            && let Some(row) = tile.row_of(at.id)
        {
            return tile.states().get(row);
        }
        let now = self.rows.tile_of(at.id)?;
        self.rows.state_of(Placed {
            tile: now,
            id: at.id,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::mem;

    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use bevy_tasks::TaskPool;
    use glam::DVec3;
    use mcrs_minecraft_engine_physics::EntityWrites;
    use mcrs_minecraft_engine_physics::FLOOR;
    use mcrs_minecraft_engine_physics::Mob;
    use mcrs_minecraft_engine_physics::TestScene;
    use mcrs_minecraft_engine_physics::mob_body;
    use mcrs_minecraft_engine_physics::mob_row;
    use mcrs_minecraft_entity::EntityTypeData;
    use mcrs_minecraft_entity_key::EntityKey;
    use mcrs_minecraft_protocol_game::entity::MobLooks;

    use super::*;

    #[test]
    fn a_mob_that_crosses_a_tile_moves_in_its_new_tile_with_its_state_after_the_pass() {
        let pool = ComputeTaskPool::get_or_init(TaskPool::default);
        let mut scene = TestScene::<()>::new(2, 0, 0, 1);
        let id = EntityId(2);
        let row = mob_row(&EntityTypeData::PIG);
        let mut state = mob_body(
            &EntityTypeData::PIG,
            &row,
            EntityKey(2),
            DVec3::new(31.9, f64::from(FLOOR), 5.5),
        );
        state.body.velocity = DVec3::new(0.5, 0.0, 0.0);
        scene.rows.spawn(id, state, row);
        scene.step(pool, 1, &mut EntityWrites::default());
        let to = TileKey::new(1, 0, 0);
        assert_eq!(
            scene.rows.tile_of(id),
            Some(to),
            "the push of 0.5 takes the pig over the edge of its tile"
        );

        let now = scene
            .rows
            .state_of(Placed { tile: to, id })
            .map(mob_state)
            .expect("the pig has a row in its new tile");
        let mut world = World::new();
        world.insert_resource(mem::take(&mut scene.rows));
        let mut mobs = Mobs::default();
        mobs.0.insert(
            id,
            Mob {
                kind: &EntityTypeData::PIG,
                uuid: 2,
                looks: MobLooks::default(),
                custom_name: None,
                tags: Vec::new(),
            },
        );
        world.insert_resource(mobs);
        world.insert_resource(MobChanges::default());
        world.insert_resource(TileEvents::default());
        world
            .run_system_once(publish_mobs::<()>)
            .expect("the publish runs");

        let deltas = world.resource_mut::<TileEvents>().take();
        let moves: Vec<(TileKey, MobState)> = deltas
            .tiles
            .iter()
            .flat_map(|delta| delta.events.iter().map(move |event| (delta.tile, event)))
            .filter_map(|(tile, event)| match &event.event {
                EntityEvent::Move(ReplicationData::Mob(MobReport::Move(state))) => {
                    Some((tile, *state))
                }
                _ => None,
            })
            .collect();
        assert_eq!(moves, [(to, now)], "one move, from the row after the pass");
    }
}
