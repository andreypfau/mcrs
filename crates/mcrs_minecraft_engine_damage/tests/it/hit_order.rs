use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::WriterKey;

use crate::scene;
use crate::scene::Executor;

const SMALL: u32 = 3;
const LARGE: u32 = 4;
const FIRST_COMMAND: u32 = 5;
const SECOND_COMMAND: u32 = 6;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Seen {
    target: EntityUuid,
    damage_type: u32,
    outcome: HitOutcome,
    health: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Order {
    Forward,
    Reverse,
}

fn run(order: Order, executor: Executor) -> (Vec<(EntityUuid, Vitals)>, Vec<Seen>) {
    let mut app = scene::app(executor);
    let mut spawn = [(9, 20.0), (5, 20.0)];
    if order == Order::Reverse {
        spawn.reverse();
    }
    let targets: Vec<_> = spawn
        .into_iter()
        .map(|(uuid, health)| scene::living(&mut app, uuid, health))
        .collect();
    let by_uuid = |uuid: u128| {
        targets
            .iter()
            .copied()
            .find(|&entity| scene::uuid(&app, entity) == EntityUuid::from_u128(uuid))
            .expect("the target is spawned")
    };
    let [struck, commanded] = [5, 9].map(by_uuid);
    let entity = |key: u128| WriterKey::Entity {
        key: EntityUuid::from_u128(key),
    };
    let command = WriterKey::Command { sequence: 0 };
    let mut requests = vec![
        scene::request(struck, 6.0, LARGE, entity(20)),
        scene::request(struck, 2.0, SMALL, entity(3)),
        {
            let mut second = scene::request(commanded, 5.0, SECOND_COMMAND, command);
            second.sequence = 1;
            second
        },
        scene::request(commanded, 1.0, FIRST_COMMAND, command),
    ];
    if order == Order::Reverse {
        requests.reverse();
    }
    for request in requests {
        scene::push(&mut app, request);
    }
    scene::tick(&mut app);
    let mut vitals: Vec<(EntityUuid, Vitals)> = targets
        .iter()
        .map(|&entity| {
            let vitals = app.world().get::<Vitals>(entity).copied();
            (scene::uuid(&app, entity), vitals.expect("a living entity"))
        })
        .collect();
    vitals.sort_by_key(|(uuid, _)| *uuid);
    let seen = scene::damaged(&app)
        .into_iter()
        .map(|damaged| Seen {
            target: scene::uuid(&app, damaged.target),
            damage_type: damaged.source.damage_type,
            outcome: damaged.report.outcome,
            health: damaged.report.health,
        })
        .collect();
    (vitals, seen)
}

#[test]
fn the_hit_order_depends_on_neither_the_spawn_order_nor_the_thread_count() {
    let (vitals, seen) = run(Order::Forward, Executor::Single);
    let expected = [
        (5, SMALL, HitOutcome::Full, 2.0),
        (5, LARGE, HitOutcome::Partial, 4.0),
        (9, FIRST_COMMAND, HitOutcome::Full, 1.0),
        (9, SECOND_COMMAND, HitOutcome::Partial, 4.0),
    ]
    .map(|(uuid, damage_type, outcome, health)| Seen {
        target: EntityUuid::from_u128(uuid),
        damage_type,
        outcome,
        health,
    });
    assert_eq!(
        seen, expected,
        "the lower writer key and then the lower sequence give the full hit, targets in UUID order"
    );
    for (order, executor) in [
        (Order::Reverse, Executor::Single),
        (Order::Forward, Executor::Multi),
        (Order::Reverse, Executor::Multi),
    ] {
        assert_eq!(
            run(order, executor),
            (vitals.clone(), seen.clone()),
            "{order:?} at {executor:?}"
        );
    }
}
