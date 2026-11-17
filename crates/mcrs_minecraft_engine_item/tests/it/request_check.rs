use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_item::Denial;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::Refusal;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestCheck;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_item_stack::ItemStack;

use crate::fixture::PLAIN;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::request_id;
use crate::request::place;
use crate::request::request;
use crate::request::snapshot;

#[derive(Resource, Clone, Copy)]
struct Denials {
    first: Denial,
    second: Denial,
}

fn deny_with_two_sources(app: &mut App) -> Denials {
    let denials = Denials {
        first: Denial {
            source: app.allocate_request_source().unwrap(),
            code: 4,
        },
        second: Denial {
            source: app.allocate_request_source().unwrap(),
            code: 9,
        },
    };
    app.insert_resource(denials)
        .init_resource::<Checked>()
        .add_observer(deny_twice);
    denials
}

#[derive(Resource, Default)]
struct Checked(Vec<(Entity, Request)>);

fn deny_twice(mut check: On<RequestCheck>, denials: Res<Denials>, mut checked: ResMut<Checked>) {
    checked.0.push((check.actor, check.request().clone()));
    check.deny(denials.first);
    check.deny(denials.second);
}

#[test]
fn a_denied_action_changes_nothing_and_keeps_the_first_denial() {
    let mut app = app();
    let denials = deny_with_two_sources(&mut app);
    let world = app.world_mut();
    let id = request_id(world);
    let actor = world.spawn(Slots::new(2)).id();
    place(world, ItemStack::new(PLAIN, count(10)), actor, 0);
    let from = SlotRef {
        holder: actor,
        index: 0,
    };
    let to = SlotRef { index: 1, ..from };
    let action = Request::new(id).split(from, to, 2).drop(from, 3);
    let before = snapshot(world);

    let results = request(world, actor, action.clone());

    assert_eq!(snapshot(world), before);
    let denied = RequestResult {
        holder: actor,
        id,
        outcome: Err(Refusal::Denied(denials.first)),
    };
    assert_eq!(results, [denied]);
    let drops = world
        .resource_mut::<Messages<RequestDrop>>()
        .drain()
        .count();
    assert_eq!(drops, 0);
    assert_eq!(world.resource::<Checked>().0, [(actor, action)]);
}

#[test]
fn a_close_never_reaches_the_check() {
    let mut app = app();
    deny_with_two_sources(&mut app);
    let world = app.world_mut();
    let id = request_id(world);
    let actor = world.spawn(Slots::new(2)).id();
    let stack = place(world, ItemStack::new(PLAIN, count(10)), actor, 0);
    let from = SlotRef {
        holder: actor,
        index: 0,
    };
    let to = SlotRef { index: 1, ..from };

    let results = request(world, actor, Request::close(id).swap(from, to));

    assert_eq!(results[0].outcome, Ok(()));
    assert_eq!(world.get::<Slots>(actor).unwrap().get(1), Some(stack));
    assert!(world.resource::<Checked>().0.is_empty());
}

#[test]
fn each_allocation_in_one_app_gives_a_new_source() {
    let mut app = app();
    let denials = deny_with_two_sources(&mut app);
    let fixture = request_id(app.world()).source;

    assert_ne!(denials.first.source, denials.second.source);
    assert_ne!(denials.first.source, fixture);
    assert_ne!(denials.second.source, fixture);
}
