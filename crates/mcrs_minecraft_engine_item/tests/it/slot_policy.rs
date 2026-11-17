use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::Gate;
use mcrs_minecraft_engine_item::GateId;
use mcrs_minecraft_engine_item::GateQuery;
use mcrs_minecraft_engine_item::ItemAppExt;
use mcrs_minecraft_engine_item::Reason;
use mcrs_minecraft_engine_item::Refusal;
use mcrs_minecraft_engine_item::RegisterError;
use mcrs_minecraft_engine_item::Rejected;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::SlotAccess;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_transaction::Filter;
use mcrs_minecraft_item_stack::ItemStack;

use crate::fixture::PLAIN;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::request_id;
use crate::request::place;
use crate::request::request;
use crate::request::snapshot;

const OPEN: SlotAccess = SlotAccess {
    limit: 99,
    take: Gate::Permits,
    place: Gate::Permits,
};

const SEALED: SlotAccess = SlotAccess {
    limit: 99,
    take: Gate::Refuses,
    place: Gate::Refuses,
};

fn at(holder: Entity, index: u16) -> SlotRef {
    SlotRef { holder, index }
}

fn with_place(place: Gate) -> SlotAccess {
    SlotAccess { place, ..OPEN }
}

fn with_limit(limit: u8) -> SlotAccess {
    SlotAccess { limit, ..OPEN }
}

fn policy(entries: &[SlotAccess]) -> SlotPolicy {
    entries.iter().copied().collect()
}

fn uniform_with(index: u16, access: SlotAccess) -> SlotPolicy {
    let mut policy = SlotPolicy::uniform(OPEN);
    policy.set(index, access);
    policy
}

fn short_with_set_past_its_end() -> SlotPolicy {
    let mut policy = policy(&[SEALED]);
    policy.set(2, SEALED);
    policy
}

struct Case {
    name: &'static str,
    policy: fn() -> SlotPolicy,
    request: fn(RequestId, Entity) -> Request,
    outcome: Result<(), Reason>,
}

fn split(id: RequestId, holder: Entity, from: u16, to: u16, items: u8) -> Request {
    Request::new(id).split(at(holder, from), at(holder, to), items)
}

#[test]
fn the_policy_of_a_holder_gives_the_limits_and_the_filters_of_its_slots() {
    let cases = [
        Case {
            name: "a locked slot refuses a take",
            policy: SlotPolicy::locked,
            request: |id, holder| split(id, holder, 0, 2, 1),
            outcome: Err(Reason::TakeDenied),
        },
        Case {
            name: "a locked slot refuses a place",
            policy: || policy(&[OPEN, OPEN, SEALED]),
            request: |id, holder| split(id, holder, 0, 2, 1),
            outcome: Err(Reason::PlaceDenied),
        },
        Case {
            name: "set changes one entry of a uniform policy",
            policy: || uniform_with(2, with_place(Gate::Refuses)),
            request: |id, holder| Request::new(id).merge(at(holder, 0), at(holder, 2)),
            outcome: Err(Reason::PlaceDenied),
        },
        Case {
            name: "the limit of the slot bounds a split",
            policy: || uniform_with(2, with_limit(4)),
            request: |id, holder| split(id, holder, 0, 2, 5),
            outcome: Err(Reason::NoRoom),
        },
        Case {
            name: "a limit of 0 clamps to 1",
            policy: || uniform_with(2, with_limit(0)),
            request: |id, holder| split(id, holder, 0, 2, 1),
            outcome: Ok(()),
        },
        Case {
            name: "the slots past the end of a short policy permit",
            policy: short_with_set_past_its_end,
            request: |id, holder| split(id, holder, 1, 2, 1),
            outcome: Ok(()),
        },
        Case {
            name: "set passes a locked slot",
            policy: SlotPolicy::locked,
            request: |id, holder| Request::new(id).set(at(holder, 0), None, 0),
            outcome: Ok(()),
        },
    ];
    for case in cases {
        let mut app = app();
        let world = app.world_mut();
        let id = request_id(world);
        let holder = world.spawn((Slots::new(3), (case.policy)())).id();
        place(world, ItemStack::new(PLAIN, count(10)), holder, 0);
        place(world, ItemStack::new(PLAIN, count(10)), holder, 1);
        let before = snapshot(world);

        let results = request(world, holder, (case.request)(id, holder));

        let outcome: Vec<Result<(), Reason>> = results
            .iter()
            .map(|result| match result.outcome {
                Ok(()) => Ok(()),
                Err(Refusal::Rejected(Rejected { op: 0, reason })) => Err(reason),
                Err(refusal) => panic!("{}: {refusal:?}", case.name),
            })
            .collect();
        assert_eq!(outcome, [case.outcome], "{}", case.name);
        if case.outcome.is_err() {
            assert_eq!(snapshot(world), before, "{}", case.name);
        }
    }
}

#[derive(Resource)]
struct Permitted(Vec<GateQuery>);

fn permit_listed(world: &World, query: &GateQuery) -> Filter {
    let listed = world.resource::<Permitted>().0.contains(query);
    if listed {
        Filter::Permits
    } else {
        Filter::Refuses
    }
}

#[test]
fn an_ask_gate_gets_the_slot_the_actor_and_the_stack_and_its_answer_decides() {
    #[derive(Clone, Copy)]
    enum Listed {
        Both,
        TakeOnly,
        PlaceOnly,
    }
    let cases = [
        (Listed::Both, Ok(())),
        (Listed::TakeOnly, Err(Reason::PlaceDenied)),
        (Listed::PlaceOnly, Err(Reason::TakeDenied)),
    ];
    for (listed, expected) in cases {
        let mut app = app();
        let gate = Gate::Ask(app.register_slot_gate(permit_listed).unwrap());
        let world = app.world_mut();
        let id = request_id(world);
        let actor = world.spawn(Slots::new(1)).id();
        let take = SlotAccess { take: gate, ..OPEN };
        let source = world.spawn((Slots::new(1), policy(&[take]))).id();
        let target = world
            .spawn((Slots::new(1), policy(&[with_place(gate)])))
            .id();
        let stack = place(world, ItemStack::new(PLAIN, count(3)), source, 0);
        let take_query = GateQuery {
            slot: at(source, 0),
            actor,
            stack,
        };
        let place_query = GateQuery {
            slot: at(target, 0),
            ..take_query
        };
        let permitted = match listed {
            Listed::Both => vec![take_query, place_query],
            Listed::TakeOnly => vec![take_query],
            Listed::PlaceOnly => vec![place_query],
        };
        world.insert_resource(Permitted(permitted));

        let move_one = Request::new(id).split(at(source, 0), at(target, 0), 1);
        let results = request(world, actor, move_one);

        let outcome = results[0].outcome.map_err(|refusal| match refusal {
            Refusal::Rejected(rejected) => rejected.reason,
            Refusal::Denied(denial) => panic!("{denial:?}"),
        });
        assert_eq!(outcome, expected);
    }
}

#[test]
fn a_gate_id_that_the_world_does_not_have_refuses() {
    let foreign: GateId = App::new()
        .register_slot_gate(|_, _| Filter::Permits)
        .unwrap();
    let mut app = app();
    let world = app.world_mut();
    let id = request_id(world);
    let source = world.spawn(Slots::new(1)).id();
    let target_policy = policy(&[with_place(Gate::Ask(foreign))]);
    let target = world.spawn((Slots::new(1), target_policy)).id();
    place(world, ItemStack::new(PLAIN, count(3)), source, 0);

    let move_one = Request::new(id).split(at(source, 0), at(target, 0), 1);
    let results = request(world, source, move_one);

    let rejected = Rejected {
        op: 0,
        reason: Reason::PlaceDenied,
    };
    assert_eq!(results[0].outcome, Err(Refusal::Rejected(rejected)));
}

#[test]
#[cfg_attr(miri, ignore = "65,535 registrations run past the timeout under Miri")]
fn the_gate_table_holds_65535_gates() {
    let mut app = App::new();
    let permits = |_: &World, _: &GateQuery| Filter::Permits;
    for _ in 0..u16::MAX {
        app.register_slot_gate(permits).unwrap();
    }

    assert_eq!(
        app.register_slot_gate(permits),
        Err(RegisterError::GateTableFull)
    );
}
