use bevy_app::Update;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use mcrs_minecraft_engine_inventory_menu::MenuClosed;
use mcrs_minecraft_engine_inventory_menu::MenuOpened;
use mcrs_minecraft_engine_item::Slots;

use crate::fixture::Logged;
use crate::fixture::app;
use crate::fixture::close;
use crate::fixture::log;
use crate::fixture::menus;
use crate::fixture::open;

#[test]
fn a_second_open_closes_the_menu_above_the_first() {
    let mut app = app();
    let viewer = app.world_mut().spawn_empty().id();
    let first = open(&mut app, viewer, &[]);
    let above = open(&mut app, viewer, &[]);
    let next = open(&mut app, viewer, &[]);

    assert_eq!(
        log(&app),
        [
            Logged::Opened(first),
            Logged::Opened(above),
            Logged::Closed(above),
            Logged::Opened(next),
        ]
    );
    assert_eq!(menus(&app, viewer), [first, next]);
    assert!(app.world().get_entity(above).is_err());
    let world = app.world();
    let opened: Vec<MenuOpened> = world
        .resource::<Messages<MenuOpened>>()
        .iter_current_update_messages()
        .copied()
        .collect();
    let closed: Vec<MenuClosed> = world
        .resource::<Messages<MenuClosed>>()
        .iter_current_update_messages()
        .copied()
        .collect();
    assert_eq!(
        opened,
        [first, above, next].map(|menu| MenuOpened { viewer, menu })
    );
    assert_eq!(
        closed,
        [MenuClosed {
            viewer,
            menu: above
        }]
    );
}

#[test]
fn close_despawns_the_menu_above_the_first_and_keeps_the_first() {
    let mut app = app();
    let viewer = app.world_mut().spawn_empty().id();
    let first = open(&mut app, viewer, &[]);
    let above = open(&mut app, viewer, &[]);
    close(&mut app, viewer);
    close(&mut app, viewer);

    assert_eq!(
        log(&app),
        [
            Logged::Opened(first),
            Logged::Opened(above),
            Logged::Closed(above),
            Logged::Closed(first),
        ]
    );
    assert_eq!(menus(&app, viewer), [first]);
    assert!(app.world().get_entity(above).is_err());
}

#[derive(Resource)]
struct Doomed(Entity);

#[test]
fn a_despawned_part_holder_closes_the_open_menu_in_the_same_update() {
    let mut app = app();
    let holder = app.world_mut().spawn(Slots::new(3)).id();
    let viewer = app.world_mut().spawn(Slots::new(3)).id();
    let first = open(&mut app, viewer, &[viewer, holder]);
    let above = open(&mut app, viewer, &[viewer, holder]);
    app.insert_resource(Doomed(holder)).add_systems(
        Update,
        |doomed: Res<Doomed>, mut commands: Commands| {
            commands.entity(doomed.0).try_despawn();
        },
    );
    app.update();

    assert_eq!(log(&app).last(), Some(&Logged::Closed(above)));
    assert_eq!(menus(&app, viewer), [first]);
    assert!(app.world().get_entity(above).is_err());
}
