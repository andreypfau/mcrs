//! Lines that a handler must refuse with an answer, while the server keeps ticking.

use bevy_ecs::entity::Entity;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_command_world::WorldCommandsPlugin;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::text::{Color, TextContent};
use mcrs_minecraft_server::VanillaEntityPhysicsPlugin;

use crate::commands::{chats, load_stone_column};
use crate::host_app::{self, PlayServer};

const OPERATOR: Uuid = Uuid::from_u128(1);
const PIG: &str = "00000009-0000-0009-0000-000900000009";

fn start() -> PlayServer {
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins(WorldCommandsPlugin);
    app.add_plugins(VanillaEntityPhysicsPlugin);
    PlayServer::start(app, &[(OPERATOR, 4)], 0)
}

/// The translation key of the last answer to `line`, and whether the answer is red.
fn answer(server: &mut PlayServer, operator: Entity, line: &str) -> (String, bool) {
    let packets = server.send(
        operator,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    );
    let reply = chats(&packets)
        .pop()
        .unwrap_or_else(|| panic!("{line} gets an answer"));
    let TextContent::Translate { translate, .. } = &reply.content else {
        panic!("{line}: a translated answer: {reply:?}");
    };
    (translate.to_string(), reply.style.color == Some(Color::RED))
}

#[test]
fn hostile_lines_fail_with_an_answer_and_the_server_keeps_ticking() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    server.ticks(operator, 2);
    let summon = "summon minecraft:pig 4 64 4 {UUID:[I;9,9,9,9]}";
    let damage = format!("damage {PIG} 1");
    assert_eq!(
        answer(&mut server, operator, summon),
        ("commands.summon.success".to_owned(), false)
    );
    server.ticks(operator, 2);
    assert_eq!(
        answer(&mut server, operator, &format!("kill {PIG}")),
        ("commands.kill.success.single".to_owned(), false)
    );

    let cases = [
        (
            "summon minecraft:pig 30000000 64 0",
            "commands.summon.invalidPosition",
        ),
        (summon, "commands.summon.failed.uuid"),
        (
            "summon minecraft:pig 4 64 4 {Motion:[0d,0d,0d]}",
            "commands.summon.failed",
        ),
        (damage.as_str(), "commands.damage.invulnerable"),
        (
            "kill @e[type=minecraft:cow]",
            "argument.entity.notfound.entity",
        ),
        (
            "fill 0 -64 0 15 319 15 minecraft:stone",
            "commands.fill.toobig",
        ),
        (
            "fill 0 64 0 29999999 64 0 minecraft:stone",
            "argument.pos.unloaded",
        ),
        (
            "fill 0 -2000 0 0 64 0 minecraft:stone",
            "argument.pos.outofworld",
        ),
        ("forceload add -30000001 0", "argument.pos.outofworld"),
        ("forceload add 0 0 1000 1000", "commands.forceload.toobig"),
        (
            "forceload remove 29999999 29999999",
            "commands.forceload.removed.failure",
        ),
        (
            "forceload query 29999999 29999999",
            "commands.forceload.query.failure",
        ),
        (
            "gamerule max_entity_cramming 24",
            "commands.gamerule.not_set",
        ),
    ];
    for (line, key) in cases {
        assert_eq!(
            answer(&mut server, operator, line),
            (key.to_owned(), true),
            "{line}"
        );
    }

    server.ticks(operator, 25);
    assert_eq!(
        answer(&mut server, operator, summon),
        ("commands.summon.success".to_owned(), false),
        "the pig left the world after its death time, so its UUID is free"
    );
}
