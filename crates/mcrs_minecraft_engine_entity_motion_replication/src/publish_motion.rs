use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_damage::Vitals;
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
        Option<&'static Vitals>,
        &'static mut PublishedTile,
    ),
>;

pub(crate) fn has_motion_work(
    assigned: Res<Assigned>,
    news: Res<MobNews>,
    movers: Res<Movers>,
    departures: Res<Departures>,
) -> bool {
    !assigned.0.is_empty()
        || !news.spawned.is_empty()
        || !news.strikes.is_empty()
        || !movers.entities().is_empty()
        || !departures.0.is_empty()
}

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
    publish_releases(&mut assigned, entities.reborrow(), &mut events);
    publish_mob_spawns(&mut news, &mobs, entities.reborrow(), &mut events);
    moved.clear();
    for &entity in movers.entities() {
        if let Ok(id) = ids.get(entity) {
            moved.push(Subject { id: id.0, entity });
        }
    }
    moved.sort_unstable();
    for &subject in moved.iter() {
        let Ok((state, outcome, _, vitals, mut published)) = entities.get_mut(subject.entity)
        else {
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
        push_move(
            &mut events,
            tile,
            subject.id,
            viewed(&state.0, vitals, &kernel),
        );
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
        if let Ok((state, _, _, vitals, published)) = entities.get(first.subject.entity)
            && let Some(tile) = published.tile()
        {
            let view = viewed(&state.0, vitals, &TickOutcome::default());
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

fn publish_releases(
    assigned: &mut Assigned,
    mut entities: Entities<'_, '_>,
    events: &mut TileEvents,
) {
    let assigned = &mut assigned.0;
    assigned.sort_unstable_by_key(|release| release.subject);
    for release in assigned.drain(..) {
        let subject = release.subject;
        let Ok((state, _, appearance, vitals, mut published)) = entities.get_mut(subject.entity)
        else {
            continue;
        };
        let Some(appearance) = appearance else {
            tracing::warn!(entity = ?subject.entity, "a moving entity has no appearance; it spawns for no viewer");
            continue;
        };
        let tile = TileKey::at(release.state.body.position);
        published.set_if_neq(PublishedTile(Some(tile)));
        let spawn = MobSpawn {
            kind: appearance.kind,
            uuid: state.0.key.0,
            state: viewed(&release.state, vitals, &TickOutcome::default()),
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
    mut entities: Entities<'_, '_>,
    events: &mut TileEvents,
) {
    for spawned in news.spawned.drain(..) {
        let Ok(mob) = mobs.get(spawned.entity) else {
            continue;
        };
        let Ok((state, _, _, _, mut published)) = entities.get_mut(spawned.entity) else {
            continue;
        };
        let tile = TileKey::at(state.0.body.position);
        published.set_if_neq(PublishedTile(Some(tile)));
        let spawn = MobSpawn {
            kind: mob.kind,
            uuid: spawned.start.key.0,
            state: viewed(
                &spawned.start,
                Some(&spawned.vitals),
                &TickOutcome::default(),
            ),
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
        if let Some(signal) = strike.signal {
            push_signal(events, strike.tile, strike.subject.id, signal);
        }
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

fn viewed(state: &EntityState, vitals: Option<&Vitals>, outcome: &TickOutcome) -> MobState {
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
        if let Some(vitals) = vitals {
            view.health = vitals.health;
            view.dying = !vitals.is_alive();
        }
    }
    view
}
