use bevy_ecs::entity::Entity;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_command_world::WorldCommandsPlugin;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::text::TextContent;
use mcrs_minecraft_server::{VanillaEntityAttributesPlugin, VanillaEntityPhysicsPlugin};
use mcrs_minecraft_text::TranslateArg;

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
    app.add_plugins(VanillaEntityAttributesPlugin);
    PlayServer::start(app, &[(OPERATOR, 4)], 0)
}

fn answer(server: &mut PlayServer, operator: Entity, line: &str) -> (String, Option<f64>) {
    let packets = server.send(
        operator,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    );
    server.ticks(operator, 1);
    let reply = chats(&packets)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{line} gets an answer"));
    let TextContent::Translate {
        translate, with, ..
    } = &reply.content
    else {
        panic!("{line}: a translated answer: {reply:?}");
    };
    let number = with.iter().find_map(|argument| match argument {
        TranslateArg::Number(NbtTag::Double(number)) => Some(*number),
        _ => None,
    });
    (translate.to_string(), number)
}

#[test]
fn attribute_sets_reads_and_resets_the_base_of_a_mob() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    server.ticks(operator, 2);
    answer(
        &mut server,
        operator,
        "summon minecraft:pig 4 64 4 {UUID:[I;9,9,9,9]}",
    );
    server.ticks(operator, 2);
    let max_health = format!("attribute {PIG} minecraft:max_health");
    let cases = [
        ("get", "commands.attribute.value.get.success", Some(10.0)),
        (
            "base set 5000",
            "commands.attribute.base_value.set.success",
            Some(5000.0),
        ),
        ("get", "commands.attribute.value.get.success", Some(1024.0)),
        (
            "base get 0.5",
            "commands.attribute.base_value.get.success",
            Some(5000.0),
        ),
        (
            "base reset",
            "commands.attribute.base_value.reset.success",
            Some(10.0),
        ),
        (
            "get 2.5",
            "commands.attribute.value.get.success",
            Some(10.0),
        ),
    ];
    for (form, key, number) in cases {
        let line = format!("{max_health} {form}");
        assert_eq!(
            answer(&mut server, operator, &line),
            (key.to_owned(), number),
            "{line}"
        );
    }
    for (line, key) in [
        (
            format!("attribute {PIG} minecraft:attack_damage base set 1"),
            "commands.attribute.failed.no_attribute",
        ),
        (
            "attribute @e[type=minecraft:pig,limit=1] minecraft:nope get".to_owned(),
            "argument.resource.not_found",
        ),
    ] {
        assert_eq!(
            answer(&mut server, operator, &line),
            (key.to_owned(), None),
            "{line}"
        );
    }
}
