use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::TickOutcome;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSpawn;
use mcrs_minecraft_replication::MobState;
use uuid::Uuid;

use crate::Appearance;
use crate::MotionState;
use crate::PublishedTile;
use crate::StepOutcome;
use crate::assign_ids::Assigned;
use crate::departures::Departures;
use crate::step_motion::Movers;
use crate::subject::Subject;

/// Records the tile events of the tick in the order of the ids: the spawn of each entity that got
/// its id, the change of tile and the move of each entity whose body changed, and the removal of
/// each entity that lost its motion. An entity that got its id in this tick sends no move, and
/// one that left before its spawn sends nothing. A change of tile and a removal start from the
/// [`PublishedTile`], so that they reach the viewers that know the entity.
pub(crate) fn publish_motion(
    mut entities: Query<(
        &MotionState,
        &StepOutcome,
        Option<&Appearance>,
        &mut PublishedTile,
    )>,
    mut assigned: ResMut<Assigned>,
    mut movers: ResMut<Movers>,
    mut moved: Local<Vec<Subject>>,
    mut departures: ResMut<Departures>,
    mut events: ResMut<TileEvents>,
) {
    let assigned = &mut assigned.0;
    assigned.sort_unstable();
    let spawned = |id| {
        assigned
            .binary_search_by_key(&id, |subject: &Subject| subject.id)
            .is_ok()
    };
    for subject in assigned.iter() {
        let Ok((state, outcome, appearance, mut published)) = entities.get_mut(subject.entity)
        else {
            continue;
        };
        let Some(appearance) = appearance else {
            tracing::warn!(entity = ?subject.entity, "a moving entity has no appearance; it spawns for no viewer");
            continue;
        };
        let state = &state.0;
        let tile = TileKey::at(state.body.position);
        published.0 = Some(tile);
        let spawn = MobSpawn {
            kind: appearance.kind,
            uuid: Uuid::from_u128(state.key.0),
            state: viewed(state, &outcome.kernel),
            look: appearance.look.clone(),
        };
        events.push(
            tile,
            subject.id,
            EntityEvent::Spawn(MobReport::Spawn(spawn).into()),
        );
    }
    moved.clear();
    movers.0.drain_into(&mut moved);
    moved.sort_unstable();
    for subject in moved.iter() {
        if spawned(subject.id) {
            continue;
        }
        let Ok((state, outcome, _, mut published)) = entities.get_mut(subject.entity) else {
            continue;
        };
        let Some(before) = published.0 else {
            continue;
        };
        let state = &state.0;
        let tile = TileKey::at(state.body.position);
        if before != tile {
            published.0 = Some(tile);
            events.push(before, subject.id, EntityEvent::Leave { to: tile });
            events.push(tile, subject.id, EntityEvent::Enter { from: before });
        }
        events.push(
            tile,
            subject.id,
            EntityEvent::Move(MobReport::Move(viewed(state, &outcome.kernel)).into()),
        );
    }
    let departures = &mut departures.0;
    departures.sort_unstable();
    for departure in departures.drain(..) {
        events.push(departure.tile, departure.id, EntityEvent::Despawn);
    }
    assigned.clear();
}

fn viewed(state: &EntityState, outcome: &TickOutcome) -> MobState {
    MobState {
        position: state.body.position,
        velocity: state.body.velocity,
        on_ground: state.body.flags.on_ground,
        horizontal_collision: state.body.flags.horizontal_collision,
        vertical_collision: state.body.flags.vertical_collision,
        bounced: outcome.bounced,
        ..MobState::default()
    }
}
