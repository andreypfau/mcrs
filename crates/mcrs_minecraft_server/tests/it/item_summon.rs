//! `summon minecraft:item` puts a stack into the world as an item entity at rest (IEN-SPN-001).

use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_minecraft_command_engine::CommandPlugin;
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_table::{DefinitionFile, ItemTable, bake};
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::text::TextContent;
use mcrs_minecraft_server::{VanillaEntityPhysicsPlugin, VanillaItemEntityPlugin};

use crate::commands::{chats, load_stone_column};
use crate::host_app::{self, PlayServer};

const OPERATOR: Uuid = Uuid::from_u128(1);

/// The items `air` and `stone`, at their wire ids 0 and 1.
fn table() -> ItemTable {
    let files: Vec<String> = ["air", "stone"]
        .iter()
        .zip(0..)
        .map(|(name, id)| {
            format!(
                r#"{{"minecraft:item":{{"description":{{"identifier":"minecraft:{name}","protocol_id":{id}}},"components":{{"minecraft:max_stack_size":64}}}}}}"#
            )
        })
        .collect();
    bake(
        files.iter().map(|bytes| DefinitionFile {
            name: "test",
            bytes: bytes.as_bytes(),
        }),
        &ItemTable::leak([]),
    )
    .expect("the test items bake")
    .table
}

fn start() -> PlayServer {
    let mut app = host_app::make_host_app();
    app.insert_resource(table());
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins(VanillaEntityPhysicsPlugin);
    app.add_plugins(VanillaItemEntityPlugin);
    PlayServer::start(app, &[(OPERATOR, 2)], 0)
}

/// The translation key of the reply to `line`.
fn run(server: &mut PlayServer, operator: Entity, line: &str) -> String {
    let replies = server.send(
        operator,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    );
    let reply = chats(&replies)
        .pop()
        .unwrap_or_else(|| panic!("{line} replies"));
    let TextContent::Translate { translate, .. } = &reply.content else {
        panic!("a translated reply: {reply:?}");
    };
    translate.to_string()
}

fn items(server: &mut PlayServer) -> Vec<(DVec3, u8)> {
    let world = server.world();
    world
        .query::<(&Dropped, &Count)>()
        .iter(world)
        .map(|(dropped, count)| (dropped.pos, count.get()))
        .collect()
}

#[test]
fn ien_spn_001_a_summoned_item_falls_onto_the_floor_with_its_stack() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    server.ticks(operator, 2);

    let reply = run(
        &mut server,
        operator,
        r#"summon minecraft:item 4 70 4 {Item:{id:"minecraft:stone",count:10},PickupDelay:10s}"#,
    );
    server.ticks(operator, 60);

    assert_eq!(reply, "commands.summon.success");
    assert_eq!(
        items(&mut server),
        [(DVec3::new(4.5, 64.0, 4.5), 10)],
        "R8: no start velocity; the stack falls and lies on the stone"
    );
}

#[test]
fn a_summoned_item_without_its_stack_or_with_an_unknown_key_fails() {
    let mut server = start();
    load_stone_column(&mut server);
    let (operator, _) = server.join(OPERATOR, "op");
    server.ticks(operator, 2);

    for line in [
        "summon minecraft:item 4 70 4",
        r#"summon minecraft:item 4 70 4 {Age:5s}"#,
        r#"summon minecraft:item 4 70 4 {Item:{id:"minecraft:stone",count:1},Glowing:1b}"#,
    ] {
        assert_eq!(
            run(&mut server, operator, line),
            "commands.summon.failed",
            "{line}"
        );
    }
    server.ticks(operator, 2);
    assert!(items(&mut server).is_empty());
}
