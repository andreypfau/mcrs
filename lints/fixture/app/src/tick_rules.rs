use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Update;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemCondition;
use bevy_time::Fixed;
use bevy_time::Real;
use bevy_time::Time;
use fixture_sets::Enabled;

#[derive(Component, PartialEq)]
pub struct Score(pub u8);

#[derive(Component)]
pub struct Held;

#[derive(Resource)]
pub struct Counter(pub u8);

#[derive(Resource, PartialEq)]
pub struct Switch {
    pub on: bool,
}

#[derive(Resource)]
pub struct Worker(pub Option<std::thread::JoinHandle<()>>);

#[derive(Event)]
pub struct Ping;

#[derive(EntityEvent)]
pub struct Poke {
    pub entity: Entity,
}

#[derive(Message)]
pub struct Note;

#[cfg_attr(mcrs_lint, expect(mcrs::blocking_in_system, reason = "fixture"))]
fn reads_clock() {
    let _clock = std::time::Instant::now();
}

#[cfg_attr(mcrs_lint, expect(mcrs::blocking_in_system, reason = "fixture"))]
fn reads_file_through_helper() {
    load();
}

fn load() {
    drop(std::fs::read("fixture"));
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
fn allocates(scores: Query<&Score>) {
    let values: Vec<u8> = scores.iter().map(|score| score.0).collect();
    drop(values);
}

fn reuses(mut buffer: Local<Vec<u8>>, scores: Query<&Score>) {
    buffer.clear();
    buffer.extend(scores.iter().map(|score| score.0));
    let taken = std::mem::take(&mut *buffer);
    *buffer = taken;
    drop(Vec::<u8>::new());
    drop(async { load() });
}

#[cfg_attr(mcrs_lint, expect(mcrs::per_frame_insert_resource, reason = "fixture"))]
fn inserts(mut commands: Commands) {
    commands.insert_resource(Counter(1));
}

fn inserts_once(mut commands: Commands, counter: Option<Res<Counter>>) {
    if counter.is_none() {
        commands.insert_resource(Counter(0));
    }
}

#[cfg_attr(mcrs_lint, expect(mcrs::marker_toggle, reason = "fixture"))]
fn toggles(mut commands: Commands, entities: Query<(Entity, Has<Held>)>) {
    for (entity, held) in &entities {
        if held {
            commands.entity(entity).remove::<Held>();
        } else {
            commands.entity(entity).insert(Held);
        }
    }
}

#[cfg_attr(mcrs_lint, expect(mcrs::time_clock_mismatch, reason = "fixture"))]
fn wall_clock_in_fixed(_: Res<Time<Real>>) {}

fn fixed_clock_in_update(_: Res<Time<Fixed>>) {}

fn schedule_clock(_: Res<Time>) {}

#[cfg_attr(mcrs_lint, expect(mcrs::missed_set_if_neq, reason = "fixture"))]
fn writes(mut scores: Query<&mut Score>) {
    for mut score in &mut scores {
        score.0 = 1;
    }
}

fn writes_on_change(mut scores: Query<&mut Score>) {
    for mut score in &mut scores {
        if score.0 != 2 {
            score.0 = 2;
        }
        score.set_if_neq(Score(3));
        if score.0 > 4 {
            score.0 = 4;
        }
        if score.0 == 5 {
            continue;
        }
        score.0 = 5;
    }
}

#[cfg_attr(mcrs_lint, expect(mcrs::exclusive_observer, reason = "fixture"))]
fn exclusive_observer(_: On<Ping>, _: &mut World) {}

fn observer(_: On<Ping>, _: Commands) {}

#[cfg_attr(mcrs_lint, expect(mcrs::insert_message_resource, reason = "fixture"))]
fn messages(app: &mut App) {
    app.init_resource::<Messages<Note>>();
}

fn allocates_on_message(mut notes: MessageReader<Note>) {
    let count: Vec<usize> = notes.read().map(|_| 1).collect();
    drop(count);
}

fn inserts_until_present(mut commands: Commands) {
    commands.insert_resource(Counter(0));
}

fn toggles_switch(mut switch: ResMut<Switch>) {
    switch.on = !switch.on;
}

fn joins_finished(mut worker: ResMut<Worker>) {
    if worker
        .0
        .as_ref()
        .is_some_and(std::thread::JoinHandle::is_finished)
        && let Some(handle) = worker.0.take()
    {
        drop(handle.join());
    }
}

fn inserts_after_return(mut commands: Commands, counter: Option<Res<Counter>>) {
    if counter.is_some() {
        return;
    }
    commands.insert_resource(Counter(0));
}

fn inserts_on_change(mut commands: Commands, score: Single<&Score, Changed<Score>>) {
    commands.insert_resource(Counter(score.0));
}

fn inserts_in_observer(mut commands: Commands) {
    commands
        .spawn_empty()
        .observe(|_: On<Poke>, mut commands: Commands| {
            commands.insert_resource(Counter(2));
        });
}

pub fn build_tick_rules(app: &mut App) {
    let gate = resource_exists::<Enabled>;
    app.add_systems(
        FixedUpdate,
        (
            reads_clock,
            reads_file_through_helper,
            allocates,
            reuses,
            inserts,
            inserts_once,
            toggles,
            wall_clock_in_fixed,
            schedule_clock,
            writes,
            writes_on_change,
        )
            .run_if(gate),
    )
    .add_systems(Update, (fixed_clock_in_update, schedule_clock).run_if(gate))
    .add_systems(
        Update,
        (
            allocates_on_message.run_if(on_message::<Note>.and_then(gate)),
            inserts_until_present.run_if(not(resource_exists::<Counter>)),
            (
                toggles_switch,
                joins_finished,
                inserts_after_return,
                inserts_on_change,
                inserts_in_observer,
            )
                .run_if(gate),
        ),
    )
    .add_observer(exclusive_observer)
    .add_observer(observer);
    messages(app);
}
