use bevy_ecs::message::Messages;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_damage::Died;
use mcrs_minecraft_entity_key::WriterKey;

use crate::scene;
use crate::scene::Executor;

#[test]
fn a_lethal_hit_dies_once_and_the_counters_run_down_on_the_hit_mob_only() {
    let mut app = scene::app(Executor::Single);
    let doomed = scene::living(&mut app, 1, 10.0);
    let idle = scene::living(&mut app, 2, 10.0);
    let idle_changed = || {
        app.world()
            .entity(idle)
            .get_change_ticks::<Vitals>()
            .expect("a living entity")
            .changed
    };
    let idle_before = idle_changed();
    scene::push(
        &mut app,
        scene::request(doomed, 100.0, 0, WriterKey::Command { sequence: 0 }),
    );
    for _ in 0..6 {
        scene::tick(&mut app);
    }
    let deaths: Vec<_> = app
        .world()
        .resource::<Messages<Died>>()
        .iter_current_update_messages()
        .map(|died| died.target)
        .collect();
    assert_eq!(deaths, [doomed], "one death for one lethal hit");
    let vitals = app.world().get::<Vitals>(doomed).copied();
    assert_eq!(
        vitals.map(|vitals| (vitals.hurt_time, vitals.damage_cooldown, vitals.death_time)),
        Some((5, 15, Some(5))),
        "five ticks after the hit tick"
    );
    let idle_after = app
        .world()
        .entity(idle)
        .get_change_ticks::<Vitals>()
        .expect("a living entity")
        .changed;
    assert_eq!(idle_after, idle_before, "an untouched mob is never written");
}

#[test]
fn the_counters_of_inserted_vitals_run_down() {
    let mut app = scene::app(Executor::Single);
    let mob = scene::living(&mut app, 1, 10.0);
    let hurt = Vitals {
        hurt_time: 3,
        ..Vitals::new(10.0)
    };
    app.world_mut().entity_mut(mob).insert(hurt);
    scene::tick(&mut app);
    let hurt_time = app
        .world()
        .get::<Vitals>(mob)
        .map(|vitals| vitals.hurt_time);
    assert_eq!(hurt_time, Some(2), "one tick runs the counter down once");
}
