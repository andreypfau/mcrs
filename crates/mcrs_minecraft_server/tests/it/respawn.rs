use bevy_math::DVec3;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::game_rules::GameRules;
use mcrs_minecraft_game_rule::GameRule;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::serverbound::{
    ClientCommandAction, ServerboundChatCommand, ServerboundClientCommand,
};
use mcrs_minecraft_server::{
    VanillaEntityAttributesPlugin, VanillaEntityPhysicsPlugin, VanillaRespawnPlugin,
};

use crate::commands::load_stone_column;
use crate::host_app::{self, PlayServer, sent_to};

const PLAYER: Uuid = Uuid::from_u128(1);

struct Case {
    name: &'static str,
    killed: bool,
}

#[test]
fn perform_respawn_respawns_a_dead_player_at_the_world_spawn_and_ignores_a_living_one() {
    for case in [
        Case {
            name: "dead",
            killed: true,
        },
        Case {
            name: "alive",
            killed: false,
        },
    ] {
        let mut app = host_app::make_host_app();
        app.add_plugins(CommandPlugin::<VanillaCommands>::new());
        app.add_plugins(VanillaCommandsPlugin);
        app.add_plugins((
            VanillaEntityPhysicsPlugin,
            VanillaEntityAttributesPlugin,
            VanillaRespawnPlugin,
        ));
        let mut server = PlayServer::start(app, &[(PLAYER, 2)], 0);
        load_stone_column(&mut server);
        let (anchor, _) = server.join(PLAYER, "player");
        let _ = server.ticks_all(5);
        let session = server.session_of(anchor).unwrap();
        let player = server.player(anchor);
        if case.killed {
            server.queue(
                anchor,
                &ServerboundChatCommand {
                    command: Bounded("kill @s"),
                },
            );
            let _ = server.ticks_all(2);
        }
        let before = *server.world().get::<Vitals>(player).unwrap();
        let from = server.world().get::<Transform>(player).unwrap().translation;

        server.queue(
            anchor,
            &ServerboundClientCommand {
                action: ClientCommandAction::PerformRespawn,
            },
        );
        let sent = sent_to(session, server.ticks_all(2));
        let respawns = sent
            .iter()
            .filter(|packet| matches!(packet, PacketPayload::Respawn(_)))
            .count();
        let vitals = *server.world().get::<Vitals>(player).unwrap();
        let at = server.world().get::<Transform>(player).unwrap().translation;

        if case.killed {
            assert_eq!(before.health, 0.0, "{}: the player is dead", case.name);
            assert_eq!(respawns, 1, "{}: PLD-NET-001 sends one respawn", case.name);
            assert_eq!(vitals, Vitals::new(20.0), "{}: alive again", case.name);
            let offset = at - DVec3::new(0.5, 64.0, 0.5);
            assert!(
                offset.x.abs() <= 10.0 && offset.z.abs() <= 10.0 && offset.y == 0.0,
                "{}: PLD-RSP-004 puts the player on the stone inside the respawn radius around the world spawn, got {at}",
                case.name
            );
        } else {
            assert_eq!(
                (respawns, vitals, at),
                (0, before, from),
                "{}: PLD-RSP-001 ignores the request of a living player",
                case.name
            );
        }
    }
}

#[test]
fn perform_respawn_in_another_dimension_places_the_player_on_the_ground_of_the_world_spawn() {
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins((
        VanillaEntityPhysicsPlugin,
        VanillaEntityAttributesPlugin,
        VanillaRespawnPlugin,
    ));
    let mut server = PlayServer::start_in(
        app,
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_end", "minecraft:the_end"),
        ],
        &[(PLAYER, 2)],
        0,
    );
    load_stone_column(&mut server);
    let overworld = server.dim;
    let end = host_app::label_of(&server.app, "minecraft:the_end");
    server
        .world_in(overworld)
        .resource_mut::<GameRules>()
        .set(GameRule::RespawnRadius, 0);
    let (anchor, _) = server.join_in(end, PLAYER, "player");
    let session = server.session_of(anchor).unwrap();
    server.queue(
        anchor,
        &ServerboundChatCommand {
            command: Bounded("kill @s"),
        },
    );
    let _ = server.ticks_all(2);

    server.queue(
        anchor,
        &ServerboundClientCommand {
            action: ClientCommandAction::PerformRespawn,
        },
    );
    let sent = sent_to(session, server.ticks_all(6));

    let player = server
        .player_in(overworld, anchor)
        .expect("PLD-RSP-002 moves the player to the dimension of the world spawn");
    let at = server
        .world_in(overworld)
        .get::<Transform>(player)
        .unwrap()
        .translation;
    let told = sent.iter().rev().find_map(|packet| match packet {
        PacketPayload::PlayerPosition(position) => Some(position.position),
        _ => None,
    });
    let spawn = DVec3::new(0.5, 64.0, 0.5);
    assert_eq!(
        (at, told),
        (spawn, Some(spawn)),
        "PLD-RSP-004 puts the player on the stone of the target dimension and tells the client"
    );
}
