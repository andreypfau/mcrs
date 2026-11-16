use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine_item::Denial;
use mcrs_minecraft_engine_item::Refusal;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestCheck;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_item_stack::ItemStack;

use crate::fixture::PLAIN;
use crate::fixture::app;
use crate::fixture::count;
use crate::request::ID;
use crate::request::place;
use crate::request::request;
use crate::request::snapshot;

const FIRST: Denial = Denial {
    source: RequestId(1),
    code: 4,
};

const SECOND: Denial = Denial {
    source: RequestId(2),
    code: 9,
};

#[derive(Resource, Default)]
struct Checked(Vec<(Entity, Request)>);

fn deny_twice(mut check: On<RequestCheck>, mut checked: ResMut<Checked>) {
    checked.0.push((check.actor, check.request().clone()));
    check.deny(FIRST);
    check.deny(SECOND);
}

#[test]
fn a_denied_action_changes_nothing_and_keeps_the_first_denial() {
    let mut app = app();
    app.init_resource::<Checked>().add_observer(deny_twice);
    let world = app.world_mut();
    let actor = world.spawn(Slots::new(2)).id();
    place(world, ItemStack::new(PLAIN, count(10)), actor, 0);
    let from = SlotRef {
        holder: actor,
        index: 0,
    };
    let to = SlotRef { index: 1, ..from };
    let action = Request::new(ID).split(from, to, 2).drop(from, 3);
    let before = snapshot(world);

    let results = request(world, actor, action.clone());

    assert_eq!(snapshot(world), before);
    let denied = RequestResult {
        holder: actor,
        id: ID,
        outcome: Err(Refusal::Denied(FIRST)),
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
    app.init_resource::<Checked>().add_observer(deny_twice);
    let world = app.world_mut();
    let actor = world.spawn(Slots::new(2)).id();
    let stack = place(world, ItemStack::new(PLAIN, count(10)), actor, 0);
    let from = SlotRef {
        holder: actor,
        index: 0,
    };
    let to = SlotRef { index: 1, ..from };

    let results = request(world, actor, Request::close(ID).swap(from, to));

    assert_eq!(results[0].outcome, Ok(()));
    assert_eq!(world.get::<Slots>(actor).unwrap().get(1), Some(stack));
    assert!(world.resource::<Checked>().0.is_empty());
}
