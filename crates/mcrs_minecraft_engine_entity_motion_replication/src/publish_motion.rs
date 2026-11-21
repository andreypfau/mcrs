use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::Movers;
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::TickOutcome;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSpawn;
use mcrs_minecraft_replication::MobState;

use crate::Appearance;
use crate::MotionId;
use crate::PublishedTile;
use crate::assign_ids::Assigned;
use crate::departures::Departures;
use crate::subject::Subject;

#[expect(
    clippy::too_many_arguments,
    reason = "a system takes each resource that it reads"
)]
pub(crate) fn publish_motion(
    mut entities: Query<(
        &MotionState,
        &StepOutcome,
        Option<&Appearance>,
        &mut PublishedTile,
    )>,
    ids: Query<&MotionId>,
    mut assigned: ResMut<Assigned>,
    movers: Res<Movers>,
    clock: Res<BirthClock>,
    mut moved: Local<Vec<Subject>>,
    mut departures: ResMut<Departures>,
    mut events: ResMut<TileEvents>,
) {
    let assigned = &mut assigned.0;
    assigned.sort_unstable_by_key(|release| release.subject);
    for release in assigned.iter() {
        let subject = release.subject;
        let Ok((state, _, appearance, mut published)) = entities.get_mut(subject.entity) else {
            continue;
        };
        let Some(appearance) = appearance else {
            tracing::warn!(entity = ?subject.entity, "a moving entity has no appearance; it spawns for no viewer");
            continue;
        };
        let tile = TileKey::at(release.body.position);
        published.0 = Some(tile);
        let spawn = MobSpawn {
            kind: appearance.kind,
            uuid: state.0.key.0,
            state: viewed(&release.body, &TickOutcome::default()),
            look: appearance.look.clone(),
        };
        events.push(
            tile,
            subject.id,
            EntityEvent::Spawn(MobReport::Spawn(spawn).into()),
        );
    }
    moved.clear();
    for &entity in movers.entities() {
        if let Ok(id) = ids.get(entity) {
            moved.push(Subject { id: id.0, entity });
        }
    }
    moved.sort_unstable();
    for subject in moved.iter() {
        let Ok((state, outcome, _, mut published)) = entities.get_mut(subject.entity) else {
            continue;
        };
        let Some(before) = published.0 else {
            continue;
        };
        let body = &state.0.body;
        let kernel = if outcome.is_of(clock.0) {
            outcome.kernel
        } else {
            TickOutcome::default()
        };
        let tile = TileKey::at(body.position);
        if before != tile {
            published.0 = Some(tile);
            events.push(before, subject.id, EntityEvent::Leave { to: tile });
            events.push(tile, subject.id, EntityEvent::Enter { from: before });
        }
        events.push(
            tile,
            subject.id,
            EntityEvent::Move(MobReport::Move(viewed(body, &kernel)).into()),
        );
    }
    let departures = &mut departures.0;
    departures.sort_unstable();
    for departure in departures.drain(..) {
        events.push(departure.tile, departure.id, EntityEvent::Despawn);
    }
    assigned.clear();
}

fn viewed(body: &Body, outcome: &TickOutcome) -> MobState {
    MobState {
        position: body.position,
        velocity: body.velocity,
        on_ground: body.flags.on_ground,
        horizontal_collision: body.flags.horizontal_collision,
        vertical_collision: body.flags.vertical_collision,
        bounced: outcome.bounced,
        ..MobState::default()
    }
}
