//! The AI of summoned pigs in a dimension world: a pig near a player strolls, and a hit by a
//! cactus makes a pig panic and run.

use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine_brain::MobAi;
use mcrs_minecraft_engine_physics::EntityRows;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_server::{VanillaEntityPhysicsPlugin, VanillaMobBrainPlugin};

use crate::commands::load_stone_column;
use crate::host_app::{self, PlayServer};

const OPERATOR: Uuid = Uuid::from_u128(1);
/// The goal of the panic in the goal list of the pig.
const PANIC: usize = 0;

fn start() -> (PlayServer, Entity) {
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins(VanillaEntityPhysicsPlugin);
    app.add_plugins(VanillaMobBrainPlugin);
    let mut server = PlayServer::start(app, &[(OPERATOR, 2)], 0);
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    (server, operator)
}

fn run(server: &mut PlayServer, operator: Entity, line: &str) {
    server.send(
        operator,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    );
}

fn pig_positions(server: &mut PlayServer) -> Vec<DVec3> {
    server
        .world()
        .resource_mut::<EntityRows>()
        .living_rows()
        .map(|(_, state)| state.body.position)
        .collect()
}

fn horizontal_distance(from: DVec3, to: DVec3) -> f64 {
    (to - from).with_y(0.0).length()
}

/// GOAL-STR-001, AIT-VAN-020: with a player that stands near them, summoned pigs on a stone floor
/// start a random stroll within a minute of game time.
#[test]
fn pigs_near_a_player_stroll_on_the_floor() {
    let (mut server, operator) = start();
    for x in [3, 6, 9, 12] {
        run(
            &mut server,
            operator,
            &format!("summon minecraft:pig {x} 64 4"),
        );
    }
    server.ticks(operator, 40);
    let rested = pig_positions(&mut server);
    assert_eq!(rested.len(), 4, "four pigs stand on the floor");

    server.ticks(operator, 1200);

    let walked = pig_positions(&mut server);
    let farthest = rested
        .iter()
        .zip(&walked)
        .map(|(from, to)| horizontal_distance(*from, *to))
        .fold(0.0, f64::max);
    assert!(
        farthest > 1.5,
        "a pig strolls away from where it landed: {rested:?} to {walked:?}"
    );
}

/// GOAL-PNC-001, GOAL-PNC-002: a pig with no player near it stands still; a hit of 0 by a cactus
/// starts its panic, and it runs.
#[test]
fn a_cactus_hit_makes_a_resting_pig_panic_and_run() {
    let (mut server, operator) = start();
    run(&mut server, operator, "gamemode spectator");
    run(&mut server, operator, "summon minecraft:pig 8 64 8");
    server.ticks(operator, 300);
    let rested = pig_positions(&mut server);
    server.ticks(operator, 40);
    assert_eq!(
        pig_positions(&mut server),
        rested,
        "a pig that no player watches stops its strolls"
    );

    run(
        &mut server,
        operator,
        "damage @e[type=minecraft:pig,limit=1] 0 minecraft:cactus",
    );
    let mut panicked = false;
    for _ in 0..10 {
        server.ticks(operator, 1);
        let ai = server.world().resource::<MobAi<PigBrain>>();
        panicked |= ai.stage().tiles().any(|(_, brains)| {
            brains
                .iter()
                .any(|entry| entry.brain.selector().is_running(PANIC))
        });
    }
    assert!(panicked, "the hit starts the panic within 10 ticks");
    server.ticks(operator, 40);
    let ran = pig_positions(&mut server);
    assert!(
        horizontal_distance(rested[0], ran[0]) > 0.5,
        "the panicking pig runs: {rested:?} to {ran:?}"
    );
}

/// GOAL-STR-004, GOAL-WAKE-020, TRK-SUM-007: a pig of `{PersistenceRequired:1b}` stops its
/// strolls under a spectator, and a hit of 0 by drown each 80 ticks makes it stroll again without
/// a panic and without a push.
#[test]
fn a_drown_hit_each_80_ticks_keeps_a_pig_strolling_without_a_panic() {
    const HIT: &str = "execute as @e[type=minecraft:pig] run damage @s 0 minecraft:drown";
    let (mut server, operator) = start();
    run(&mut server, operator, "gamemode spectator");
    run(
        &mut server,
        operator,
        "summon minecraft:pig 8 64 8 {PersistenceRequired:1b}",
    );
    server.ticks(operator, 300);
    let rested = pig_positions(&mut server);
    assert_eq!(rested.len(), 1, "the summon accepts PersistenceRequired");
    server.ticks(operator, 40);
    assert_eq!(
        pig_positions(&mut server),
        rested,
        "the spectator resets no idle time"
    );
    let velocity = |server: &mut PlayServer| {
        server
            .world()
            .resource_mut::<EntityRows>()
            .living_rows()
            .map(|(_, state)| state.body.velocity)
            .collect::<Vec<DVec3>>()
    };
    let resting = velocity(&mut server);
    run(&mut server, operator, HIT);
    server.ticks(operator, 1);
    assert_eq!(velocity(&mut server), resting, "a drown hit gives no push");

    let mut farthest: f64 = 0.0;
    for round in 0..10 {
        if round > 0 {
            run(&mut server, operator, HIT);
        }
        for _ in 0..80 {
            server.ticks(operator, 1);
            let ai = server.world().resource::<MobAi<PigBrain>>();
            let panics = ai.stage().tiles().any(|(_, brains)| {
                brains
                    .iter()
                    .any(|entry| entry.brain.selector().is_running(PANIC))
            });
            assert!(!panics, "drown is not a panic cause");
        }
        let now = pig_positions(&mut server);
        farthest = farthest.max(horizontal_distance(rested[0], now[0]));
    }
    assert!(farthest > 0.5, "the hits make the pig stroll again");
}
