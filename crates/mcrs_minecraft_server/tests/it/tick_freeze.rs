use bevy_ecs::entity::Entity;
use bevy_time::TimeUpdateStrategy;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_command_world::WorldCommandsPlugin;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::coordinator::clock::{GameTime, WorldClockHostPlugin};
use mcrs_minecraft_engine::server_edit::ServerStateHostPlugin;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_world::tick_pace::TickPace;
use mcrs_minecraft_environment::{AdvanceTime, WorldClockPlugin};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{ClientboundTickingState, ClientboundTickingStep};
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::text::TextContent;
use mcrs_minecraft_server::VanillaEntityPhysicsPlugin;

use crate::commands::{chats, load_stone_column};
use crate::host_app::{self, PlayServer};

const OPERATOR: Uuid = Uuid::from_u128(1);
const VIEWER: Uuid = Uuid::from_u128(2);

#[derive(Debug, PartialEq)]
enum Ticking {
    State(ClientboundTickingState),
    Step(ClientboundTickingStep),
}

fn start() -> PlayServer {
    let mut app = host_app::make_host_app();
    app.insert_resource(TickPace::new(20));
    app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    app.add_plugins(ServerStateHostPlugin);
    app.add_plugins(WorldClockPlugin);
    app.insert_resource(AdvanceTime(true));
    app.add_plugins(WorldClockHostPlugin);
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins(WorldCommandsPlugin);
    app.add_plugins(VanillaEntityPhysicsPlugin);
    PlayServer::start(app, &[(OPERATOR, 4)], 0)
}

fn ticks(server: &mut PlayServer, operator: Entity, count: usize) -> Vec<PacketPayload> {
    let mut packets = Vec::new();
    for _ in 0..count {
        server
            .app
            .world_mut()
            .resource_mut::<TickPace>()
            .begin_tick();
        packets.extend(server.ticks(operator, 1));
    }
    packets
}

fn run(server: &mut PlayServer, operator: Entity, line: &str) -> (Vec<String>, Vec<Ticking>) {
    server.queue(
        operator,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    );
    let packets = ticks(server, operator, 4);
    let keys = chats(&packets)
        .into_iter()
        .map(|reply| match &reply.content {
            TextContent::Translate { translate, .. } => translate.to_string(),
            other => panic!("{line}: a translated answer: {other:?}"),
        })
        .collect();
    (keys, ticking(packets))
}

fn ticking(packets: Vec<PacketPayload>) -> Vec<Ticking> {
    packets
        .into_iter()
        .filter_map(|packet| match packet {
            PacketPayload::TickingState(state) => Some(Ticking::State(state)),
            PacketPayload::TickingStep(step) => Some(Ticking::Step(step)),
            _ => None,
        })
        .collect()
}

fn snapshot(server: &mut PlayServer) -> (i64, f64) {
    let game_time = server.app.world().resource::<GameTime>().0;
    let world = server.world();
    let pig = world
        .query::<&MotionState>()
        .iter(world)
        .map(|state| state.0.body.position.y)
        .next()
        .expect("the pig exists");
    (game_time, pig)
}

#[test]
fn a_frozen_game_stops_time_and_mobs_until_a_step_runs_them() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    run(&mut server, operator, "summon minecraft:pig 4 100 4");
    ticks(&mut server, operator, 2);

    assert_eq!(
        run(&mut server, operator, "tick step"),
        (vec!["commands.tick.step.fail".to_owned()], vec![])
    );
    assert_eq!(
        run(&mut server, operator, "tick freeze"),
        (
            vec!["commands.tick.status.frozen".to_owned()],
            vec![Ticking::State(ClientboundTickingState {
                tick_rate: 20.0,
                frozen: true,
            })]
        )
    );
    let (_, joined) = server.join(VIEWER, "viewer");
    assert_eq!(
        ticking(joined),
        [
            Ticking::State(ClientboundTickingState {
                tick_rate: 20.0,
                frozen: true,
            }),
            Ticking::Step(ClientboundTickingStep { steps: VarInt(0) })
        ],
        "a join receives the frozen state and the steps"
    );
    let frozen = snapshot(&mut server);
    ticks(&mut server, operator, 5);
    assert_eq!(snapshot(&mut server), frozen, "nothing moves while frozen");

    let (keys, packets) = run(&mut server, operator, "tick step 3");
    assert_eq!(keys, ["commands.tick.step.success"]);
    assert_eq!(
        packets,
        [Ticking::Step(ClientboundTickingStep { steps: VarInt(3) })]
    );
    ticks(&mut server, operator, 4);
    let (game_time, pig) = snapshot(&mut server);
    assert_eq!(game_time, frozen.0 + 3, "a step of 3 runs 3 ticks");
    assert!(pig < frozen.1, "the pig falls during the step");

    assert_eq!(
        run(&mut server, operator, "tick step stop").0,
        ["commands.tick.step.stop.fail"]
    );
    assert_eq!(
        run(&mut server, operator, "tick rate 12.345"),
        (
            vec!["commands.tick.rate.success".to_owned()],
            vec![Ticking::State(ClientboundTickingState {
                tick_rate: 12.345,
                frozen: true,
            })]
        )
    );
    assert_eq!(
        run(&mut server, operator, "tick query").0,
        [
            "commands.tick.status.frozen",
            "commands.tick.query.rate.running",
            "commands.tick.query.percentiles"
        ]
    );
    run(&mut server, operator, "tick unfreeze");
    let before = snapshot(&mut server).0;
    ticks(&mut server, operator, 2);
    assert_eq!(snapshot(&mut server).0, before + 2, "time runs again");
}
