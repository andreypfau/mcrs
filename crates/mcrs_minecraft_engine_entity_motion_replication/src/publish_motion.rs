use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_entity_motion::Movers;
use mcrs_minecraft_engine_entity_motion::StepOutcome;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::TickOutcome;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::MobSpawn;
use mcrs_minecraft_replication::MobState;
use mcrs_minecraft_replication::SpawnLook;

use crate::Appearance;
use crate::MotionId;
use crate::PublishedTile;
use crate::assign_ids::Assigned;
use crate::departures::DepartureKind;
use crate::departures::Departures;
use crate::mob_news::MobNews;
use crate::mob_news::Strike;
use crate::subject::Subject;

type Entities<'world, 'state> = Query<
    'world,
    'state,
    (
        &'static MotionState,
        &'static StepOutcome,
        Option<&'static Appearance>,
        &'static mut PublishedTile,
    ),
>;

#[expect(
    clippy::too_many_arguments,
    reason = "a system takes each resource that it reads"
)]
pub(crate) fn publish_motion(
    mut entities: Entities,
    ids: Query<&MotionId>,
    mobs: Query<&Mob>,
    mut assigned: ResMut<Assigned>,
    mut news: ResMut<MobNews>,
    movers: Res<Movers>,
    clock: Res<BirthClock>,
    mut moved: Local<Vec<Subject>>,
    mut departures: ResMut<Departures>,
    mut events: ResMut<TileEvents>,
) {
    publish_releases(&mut assigned, &mut entities, &mut events);
    publish_mob_spawns(&mut news, &mobs, &mut entities, &mut events);
    moved.clear();
    for &entity in movers.entities() {
        if let Ok(id) = ids.get(entity) {
            moved.push(Subject { id: id.0, entity });
        }
    }
    moved.sort_unstable();
    for &subject in moved.iter() {
        let Ok((state, outcome, _, mut published)) = entities.get_mut(subject.entity) else {
            continue;
        };
        let Some(before) = published.0 else {
            continue;
        };
        let kernel = if outcome.is_of(clock.0) {
            outcome.kernel
        } else {
            TickOutcome::default()
        };
        let tile = TileKey::at(state.0.body.position);
        if before != tile {
            published.0 = Some(tile);
            events.push(before, subject.id, EntityEvent::Leave { to: tile });
            events.push(tile, subject.id, EntityEvent::Enter { from: before });
        }
        push_strikes(news.strikes_of(subject), &mut events);
        push_move(&mut events, tile, subject.id, viewed(&state.0, &kernel));
    }
    for strikes in news
        .strikes
        .chunk_by(|left, right| left.subject == right.subject)
    {
        let Some(first) = strikes.first() else {
            continue;
        };
        if moved.binary_search(&first.subject).is_ok() {
            continue;
        }
        push_strikes(strikes, &mut events);
        if let Ok((state, _, _, published)) = entities.get(first.subject.entity)
            && let Some(tile) = published.tile()
        {
            let view = viewed(&state.0, &TickOutcome::default());
            push_move(&mut events, tile, first.subject.id, view);
        }
    }
    news.strikes.clear();
    let departures = &mut departures.0;
    departures.sort_unstable();
    for departure in departures.drain(..) {
        if departure.kind == DepartureKind::Mob {
            push_signal(
                &mut events,
                departure.tile,
                departure.id,
                MobSignal::Removed,
            );
        }
        events.push(departure.tile, departure.id, EntityEvent::Despawn);
    }
}

fn publish_releases(assigned: &mut Assigned, entities: &mut Entities, events: &mut TileEvents) {
    let assigned = &mut assigned.0;
    assigned.sort_unstable_by_key(|release| release.subject);
    for release in assigned.drain(..) {
        let subject = release.subject;
        let Ok((state, _, appearance, mut published)) = entities.get_mut(subject.entity) else {
            continue;
        };
        let Some(appearance) = appearance else {
            tracing::warn!(entity = ?subject.entity, "a moving entity has no appearance; it spawns for no viewer");
            continue;
        };
        let tile = TileKey::at(release.state.body.position);
        published.0 = Some(tile);
        let spawn = MobSpawn {
            kind: appearance.kind,
            uuid: state.0.key.0,
            state: viewed(&release.state, &TickOutcome::default()),
            look: appearance.look.clone(),
        };
        events.push(
            tile,
            subject.id,
            EntityEvent::Spawn(MobReport::Spawn(spawn).into()),
        );
    }
}

fn publish_mob_spawns(
    news: &mut MobNews,
    mobs: &Query<&Mob>,
    entities: &mut Entities,
    events: &mut TileEvents,
) {
    for spawned in news.spawned.drain(..) {
        let Ok(mob) = mobs.get(spawned.entity) else {
            continue;
        };
        let Ok((state, _, _, mut published)) = entities.get_mut(spawned.entity) else {
            continue;
        };
        let tile = TileKey::at(state.0.body.position);
        published.0 = Some(tile);
        let spawn = MobSpawn {
            kind: mob.kind,
            uuid: spawned.start.key.0,
            state: viewed(&spawned.start, &TickOutcome::default()),
            look: SpawnLook::Mob(mob.looks.clone()),
        };
        events.push(
            tile,
            spawned.id,
            EntityEvent::Spawn(MobReport::Spawn(spawn).into()),
        );
    }
}

fn push_strikes(strikes: &[Strike], events: &mut TileEvents) {
    for strike in strikes {
        push_signal(events, strike.tile, strike.subject.id, strike.signal);
    }
}

fn push_signal(events: &mut TileEvents, tile: TileKey, id: EntityId, signal: MobSignal) {
    events.push(
        tile,
        id,
        EntityEvent::Data(MobReport::Signal(signal).into()),
    );
}

fn push_move(events: &mut TileEvents, tile: TileKey, id: EntityId, state: MobState) {
    events.push(tile, id, EntityEvent::Move(MobReport::Move(state).into()));
}

fn viewed(state: &EntityState, outcome: &TickOutcome) -> MobState {
    let body = &state.body;
    let mut view = MobState {
        position: body.position,
        velocity: body.velocity,
        on_ground: body.flags.on_ground,
        horizontal_collision: body.flags.horizontal_collision,
        vertical_collision: body.flags.vertical_collision,
        bounced: outcome.bounced,
        ..MobState::default()
    };
    if let Some(living) = &state.living {
        view.yaw = living.orientation.yaw;
        view.pitch = living.orientation.pitch;
        view.head_yaw = living.orientation.head_yaw;
        view.health = living.health;
        view.dying = !living.is_alive();
    }
    view
}
