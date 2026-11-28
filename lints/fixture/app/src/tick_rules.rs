use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Update;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemCondition;
use bevy_time::Fixed;
use bevy_time::Real;
use bevy_time::Time;
use fixture_sets::Enabled;
use fixture_sets::enabled_changed;

#[derive(Component, PartialEq)]
pub struct Score(pub u8);

#[derive(Component)]
pub struct Held;

#[derive(Resource)]
pub struct Counter(pub u8);

#[derive(Resource, PartialEq)]
pub struct Switch {
    pub on: bool,
    pub ticks: u32,
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
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
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
    let units: Vec<()> = scores.iter().map(|_| ()).collect();
    drop(units);
    drop(async { load() });
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn grows_local(scores: Query<&Score>) {
    let mut values = Vec::new();
    for score in &scores {
        values.push(score.0);
    }
    drop(values);
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn grows_a_deque(scores: Query<&Score>) {
    let mut values = std::collections::VecDeque::new();
    for score in &scores {
        values.push_back(score.0);
    }
    drop(values);
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn fills_through_helper(scores: Query<&Score>) {
    let mut values = Vec::new();
    fill(&mut values, &scores);
    for value in &values {
        assert!(*value < 10, "fixture scores are small");
    }
}

fn fill(buffer: &mut Vec<u8>, scores: &Query<&Score>) {
    buffer.extend(scores.iter().map(|score| score.0));
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn formats_a_label(scores: Query<&Score>) {
    for score in &scores {
        drop(format!("score {}", score.0));
    }
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, expect(mcrs::alloc_site_count, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn states_a_wrong_count(scores: Query<&Score>) {
    let first: Vec<u8> = scores.iter().map(|score| score.0).collect();
    let second: Vec<u8> = scores.iter().map(|score| score.0 + 1).collect();
    drop((first, second));
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, expect(mcrs::alloc_site_count, reason = "fixture"))]
fn states_no_count(scores: Query<&Score>) {
    let values: Vec<u8> = scores.iter().map(|score| score.0).collect();
    drop(values);
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(2))]
fn states_the_right_count(scores: Query<&Score>) {
    let first: Vec<u8> = scores.iter().map(|score| score.0).collect();
    let second: Vec<u8> = scores.iter().map(|score| score.0 + 1).collect();
    drop((first, second));
}

pub struct Holder {
    pub items: Vec<u8>,
}

fn moves_a_new_vec(scores: Query<&Score>) {
    let items = Vec::new();
    let holder = Holder { items };
    let unused: Vec<u8> = Vec::new();
    drop((holder, unused, scores));
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

fn allocates_per_message(mut notes: MessageReader<Note>) {
    for _ in notes.read() {
        let copy: Vec<u8> = std::iter::once(1).collect();
        drop(copy);
    }
}

fn allocates_per_change(scores: Query<&Score, Changed<Score>>) {
    for score in &scores {
        let copy: Vec<u8> = std::iter::once(score.0).collect();
        drop(copy);
    }
}

fn toggles_switch(mut switch: ResMut<Switch>) {
    switch.on = !switch.on;
    switch.ticks = switch.ticks.wrapping_add(1);
    let was = std::mem::take(&mut switch.on);
    switch.on = was;
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

#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn score_arrived(scores: Query<(), Added<Score>>) -> bool {
    !scores.is_empty()
}

fn score_waiting(scores: Query<&Score>) -> bool {
    scores.iter().len() > 3
}

#[cfg_attr(mcrs_lint, expect(mcrs::invalid_event_condition, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::event_condition)]
pub fn counts_scores(scores: Query<&Score>) -> u8 {
    u8::try_from(scores.iter().len()).unwrap_or(u8::MAX)
}

#[cfg_attr(mcrs_lint, expect(mcrs::invalid_event_condition, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::event_condition)]
fn never_gates(scores: Query<&Score>) -> bool {
    scores.is_empty()
}

pub fn checks_directly(world: &mut World) -> bool {
    world.run_system_cached(never_gates).unwrap_or(false)
}

fn collect_scores(scores: &Query<&Score>) {
    let values: Vec<u8> = scores.iter().map(|score| score.0).collect();
    drop(values);
}

fn allocates_when_marked(scores: Query<&Score>) {
    collect_scores(&scores);
}

fn allocates_when_marked_and(scores: Query<&Score>) {
    collect_scores(&scores);
}

fn allocates_when_enabled_changes(scores: Query<&Score>) {
    collect_scores(&scores);
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn allocates_when_unmarked(scores: Query<&Score>) {
    collect_scores(&scores);
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn allocates_when_marked_or(scores: Query<&Score>) {
    collect_scores(&scores);
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn allocates_when_negated(scores: Query<&Score>) {
    collect_scores(&scores);
}

fn allocates_on_changed_resource(counter: Res<Counter>) {
    if counter.is_changed() {
        let copy: Vec<u8> = std::iter::once(counter.0).collect();
        drop(copy);
    }
}

fn allocates_after_unchanged_exit(counter: Res<Counter>) {
    if !counter.is_changed() {
        return;
    }
    let copy: Vec<u8> = std::iter::once(counter.0).collect();
    drop(copy);
}

fn allocates_on_changed_ref(scores: Query<Ref<Score>>) {
    for score in &scores {
        if Ref::is_changed(&score) {
            let copy: Vec<u8> = std::iter::once(score.0).collect();
            drop(copy);
        }
    }
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn allocates_when_unchanged(counter: Res<Counter>) {
    if !counter.is_changed() {
        let copy: Vec<u8> = std::iter::once(counter.0).collect();
        drop(copy);
    }
}

#[cfg_attr(mcrs_lint, expect(mcrs::alloc_in_tick, reason = "fixture"))]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
fn allocates_after_changed_exit(counter: Res<Counter>) {
    if counter.is_changed() {
        return;
    }
    let copy: Vec<u8> = std::iter::once(counter.0).collect();
    drop(copy);
}

pub fn build_tick_rules(app: &mut App) {
    let gate = resource_exists::<Enabled>;
    app.add_systems(
        FixedUpdate,
        (
            reads_clock,
            reads_file_through_helper,
            allocates,
            grows_local,
            grows_a_deque,
            fills_through_helper,
            formats_a_label,
            states_a_wrong_count,
            states_no_count,
            states_the_right_count,
            moves_a_new_vec,
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
            allocates_when_marked.run_if(score_arrived),
            allocates_when_marked_and.run_if(score_arrived.and_then(score_waiting)),
            allocates_when_enabled_changes.run_if(enabled_changed),
            allocates_when_unmarked.run_if(score_waiting),
            allocates_when_marked_or.run_if(score_arrived.or_else(score_waiting)),
            allocates_when_negated.run_if(not(score_arrived)),
            inserts_until_present.run_if(not(resource_exists::<Counter>)),
            (
                allocates_per_message,
                allocates_per_change,
                toggles_switch,
                joins_finished,
                inserts_after_return,
                inserts_on_change,
                inserts_in_observer,
                allocates_on_changed_resource,
                allocates_after_unchanged_exit,
                allocates_on_changed_ref,
                allocates_when_unchanged,
                allocates_after_changed_exit,
            )
                .run_if(gate),
        ),
    )
    .add_observer(exclusive_observer)
    .add_observer(observer);
    messages(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn writes_in_test(mut scores: Query<&mut Score>) {
        for mut score in &mut scores {
            score.0 = 9;
        }
    }

    #[test]
    fn registers_without_gates() {
        let mut app = App::new();
        app.add_systems(Update, (writes_in_test, writes_on_change, reuses));
    }
}
