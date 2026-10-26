use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{In, With};
use bevy_math::DVec3;
use mcrs_command::{Arg, BuildError, IntegerArgument, Limits, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_engine::bus::{PacketPayload, PacketTarget};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_protocol::command::CommandNodeKind;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, GameMode, VarInt};
use mcrs_minecraft_server::command::vanilla::VanillaCommandsPlugin;
use mcrs_minecraft_server::command::{
    AppCommands, Call, CommandFeedback, CommandLimits, CommandPlugin, CommandRegistrar,
    CommandResult,
};

use crate::host_app::{self, PlayServer};

#[derive(Clone, Copy)]
struct Echo {
    times: Arg<IntegerArgument>,
}

fn echo_at(
    name: &'static str,
) -> impl FnOnce(&mut CommandRegistrar<'_, ()>) -> Result<Echo, BuildError> {
    move |r| {
        let echo = r
            .literal(NodeId::ROOT, name)
            .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
            .id()?;
        let times = r
            .arg(
                echo,
                "times",
                IntegerArgument {
                    min: 1,
                    ..IntegerArgument::ANY
                },
            )
            .runs(())
            .id()?;
        Ok(Echo { times })
    }
}

fn run_echo(
    In(call): In<Call<(), Echo>>,
    context: mcrs_minecraft_server::command::CommandContext,
    mut feedback: CommandFeedback,
) -> CommandResult {
    let command = call.command;
    let times = context.arg(&call, command.times)?;
    feedback.success(&call, format!("echo {times}").into_text(), false);
    Ok(times)
}

const OPERATOR: Uuid = Uuid::from_u128(1);
const GUEST: Uuid = Uuid::from_u128(2);

fn start() -> PlayServer {
    with_limits(Limits::default())
}

fn with_limits(limits: Limits) -> PlayServer {
    let mut app = host_app::make_host_app();
    app.insert_resource(CommandLimits(limits));
    app.add_plugins(CommandPlugin);
    app.add_plugins(VanillaCommandsPlugin);
    app.add_command(echo_at("echo"), run_echo).unwrap();
    PlayServer::start(app, &[(OPERATOR, 2)], 0)
}

fn command(server: &mut PlayServer, anchor: Entity, line: &str) -> Vec<PacketPayload> {
    server.send(
        anchor,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    )
}

fn suggest(server: &mut PlayServer, anchor: Entity, text: &str) -> Vec<String> {
    let packets = server.send(
        anchor,
        &ServerboundCommandSuggestion {
            id: VarInt(3),
            text: Bounded(text),
        },
    );
    let mut replies = replies(&packets);
    replies.pop().expect("every request gets a reply").1
}

fn transform(server: &mut PlayServer, anchor: Entity) -> Transform {
    let player = server.player(anchor);
    *server.world().get::<Transform>(player).unwrap()
}

/// Loads the column at chunk `0 0` with stone from the bottom up to y 63 and air above.
fn load_stone_column(server: &mut PlayServer) {
    use mcrs_minecraft_heightmap::{
        MotionHeightmap, NoLeavesHeightmap, SolidHeightmap, SurfaceHeightmap,
    };
    use mcrs_minecraft_level::palette::{ChunkBlocks, SectionStates};
    use mcrs_minecraft_level::world::dimension::{Dimension, DimensionTypeConfig};
    use mcrs_minecraft_level::world::lifecycle::stage::SectionStage;
    use mcrs_minecraft_level::world::storage::column::{
        ColumnIndex, ColumnPos, ColumnSections, ColumnSlot,
    };
    let stone = crate::support::standalone_corpus()
        .0
        .default_state("minecraft:stone")
        .unwrap();
    let world = server.world();
    let (dimension, config) = world
        .query_filtered::<(Entity, &DimensionTypeConfig), With<Dimension>>()
        .single(world)
        .map(|(entity, config)| (entity, *config))
        .unwrap();
    let min_section = config.min_y >> 4;
    let mut sections = ColumnSections::new(min_section, config.section_count as usize);
    for chunk_y in min_section..min_section + config.section_count as i32 {
        let state = if chunk_y < 4 {
            stone
        } else {
            mcrs_minecraft_core::BlockStateId::AIR
        };
        let section = world
            .spawn((
                ChunkBlocks::new(SectionStates::homogeneous(state)),
                SectionStage::Loaded,
            ))
            .id();
        sections.set_loaded(chunk_y, section);
    }
    let mut surface = mcrs_minecraft_chunk::ColumnHeights::new(config.height, config.min_y);
    for x in 0..16 {
        for z in 0..16 {
            surface.set(x, z, 64);
        }
    }
    let column = world
        .spawn((
            sections,
            SurfaceHeightmap(surface.clone()),
            SolidHeightmap(surface.clone()),
            MotionHeightmap(surface.clone()),
            NoLeavesHeightmap(surface),
        ))
        .id();
    world.get_mut::<ColumnIndex>(dimension).unwrap().insert(
        ColumnPos::new(0, 0),
        ColumnSlot {
            entity: column,
            section_count: config.section_count,
        },
    );
}

fn root_literals(packets: &[PacketPayload]) -> Vec<String> {
    let tree = packets
        .iter()
        .find_map(|packet| match packet {
            PacketPayload::Commands(tree) => Some(tree),
            _ => None,
        })
        .expect("the player receives its tree on joining");
    let root = &tree.nodes[tree.root.0 as usize];
    root.children
        .iter()
        .filter_map(|child| match &tree.nodes[child.0 as usize].kind {
            CommandNodeKind::Literal(name) => Some(name.to_string()),
            _ => None,
        })
        .collect()
}

fn chats(packets: &[PacketPayload]) -> Vec<Text> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::SystemChat(chat) => Some(chat.content.clone()),
            _ => None,
        })
        .collect()
}

fn replies(packets: &[PacketPayload]) -> Vec<(i32, Vec<String>)> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::CommandSuggestions(reply) => Some((
                reply.id.0,
                reply
                    .suggestions
                    .iter()
                    .map(|suggestion| suggestion.text.to_string())
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

fn plain(messages: &[Text]) -> Vec<String> {
    messages.iter().map(Text::to_legacy_lossy).collect()
}

#[test]
fn an_operator_sees_completes_and_runs_a_plugin_command_that_a_guest_cannot() {
    let mut server = start();
    let (op, packets) = server.join(OPERATOR, "Op");
    assert_eq!(
        root_literals(&packets),
        ["execute", "gamemode", "teleport", "tp", "locate", "echo"]
    );
    let (guest, packets) = server.join(GUEST, "Guest");
    assert_eq!(root_literals(&packets), Vec::<String>::new());

    assert_eq!(suggest(&mut server, op, "/ec"), ["echo"]);
    assert!(suggest(&mut server, guest, "/ec").is_empty());

    assert_eq!(
        plain(&chats(&command(&mut server, op, "echo 3"))),
        ["echo 3"]
    );
    let refused = chats(&command(&mut server, guest, "echo 3"));
    assert_eq!(refused.len(), 2, "{refused:?}");
    assert_eq!(
        refused[0],
        mcrs_minecraft_protocol::text::IntoText::color(
            Text::translate("command.unknown.command", vec![]),
            mcrs_minecraft_protocol::text::Color::RED
        )
    );
    let too_low = plain(&chats(&command(&mut server, op, "echo 0")));
    assert_eq!(too_low.len(), 2, "{too_low:?}");
}

#[test]
fn gamemode_changes_the_targets_it_selects() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let (guest, _) = server.join(GUEST, "Guest");
    assert_ne!(server.game_mode(guest), GameMode::Spectator);
    command(&mut server, op, "gamemode spectator Guest");
    assert_eq!(server.game_mode(guest), GameMode::Spectator);
    command(&mut server, op, "gamemode spectator @a");
    assert_eq!(server.game_mode(op), GameMode::Spectator);
}

#[test]
fn teleport_sends_the_relative_axes_and_reports_the_resolved_position() {
    use mcrs_minecraft_protocol::PositionFlag as F;
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let packets = command(&mut server, op, "tp ~1 ~ 3");
    let position = packets
        .iter()
        .find_map(|packet| match packet {
            PacketPayload::PlayerPosition(position) => Some(position),
            _ => None,
        })
        .expect("the player is moved");
    assert_eq!(position.position, DVec3::new(1.0, 0.0, 3.5));
    let mut flags = position.flags.0.clone();
    flags.sort_by_key(|flag| format!("{flag:?}"));
    assert_eq!(flags, [F::DeltaX, F::DeltaY, F::X, F::XRot, F::Y, F::YRot]);
    assert_eq!(
        chats(&packets),
        [Text::translate(
            "commands.teleport.success.location.single",
            vec![
                "Op".into_text(),
                "9.500000".into_text(),
                "64.000000".into_text(),
                "3.500000".into_text()
            ]
        )]
    );
}

#[test]
fn teleport_facing_keeps_the_rotation_then_looks_at_the_target() {
    use mcrs_minecraft_protocol::PositionFlag as F;
    use mcrs_minecraft_protocol::packets::game::clientbound::{
        ClientboundPlayerLookAt, LookAnchor,
    };
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let packets = command(&mut server, op, "tp @s 0.5 64 0.5 facing 0.5 64 10.5");
    let moves: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::PlayerPosition(position) => Some(position),
            _ => None,
        })
        .collect();
    let [position] = moves[..] else {
        panic!("{packets:?}");
    };
    assert_eq!((position.look.yaw, position.look.pitch), (0.0, 0.0));
    let mut flags = position.flags.0.clone();
    flags.sort_by_key(|flag| format!("{flag:?}"));
    assert_eq!(flags, [F::XRot, F::YRot]);
    let looks: Vec<_> = packets
        .iter()
        .skip_while(|packet| !matches!(packet, PacketPayload::PlayerPosition(_)))
        .filter_map(|packet| match packet {
            PacketPayload::PlayerLookAt(look_at) => Some(look_at),
            _ => None,
        })
        .collect();
    assert_eq!(
        looks,
        [&ClientboundPlayerLookAt {
            from: LookAnchor::Feet,
            target: DVec3::new(0.5, 64.0, 10.5),
            entity: None,
        }]
    );
}

#[test]
fn a_game_mode_change_sends_the_abilities_after_the_game_event() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let mut flags = Vec::new();
    for mode in ["spectator", "creative", "survival", "creative"] {
        let packets = command(&mut server, op, &format!("gamemode {mode}"));
        let event = packets
            .iter()
            .position(|packet| matches!(packet, PacketPayload::GameEvent(_)));
        let abilities = packets
            .iter()
            .enumerate()
            .find_map(|(at, packet)| match packet {
                PacketPayload::PlayerAbilities(abilities) => Some((at, abilities)),
                _ => None,
            });
        let (Some(event), Some((at, abilities))) = (event, abilities) else {
            panic!("{mode}: {packets:?}");
        };
        assert!(event < at, "{mode}");
        flags.push([
            abilities.invulnerable,
            abilities.flying,
            abilities.may_fly,
            abilities.instant_build,
        ]);
    }
    assert_eq!(
        flags,
        [
            [true, true, true, false],
            [true, true, true, true],
            [false; 4],
            [true, false, true, true],
        ]
    );
}

#[test]
fn an_admin_message_reaches_only_the_other_listed_operators() {
    const WATCHER: Uuid = Uuid::from_u128(3);
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin);
    app.add_plugins(VanillaCommandsPlugin);
    let mut server = PlayServer::start(app, &[(OPERATOR, 2), (WATCHER, 2)], 2);
    let (op, _) = server.join(OPERATOR, "Op");
    let (watcher, _) = server.join(WATCHER, "Watcher");
    let (guest, _) = server.join(GUEST, "Guest");
    server.queue(
        op,
        &ServerboundChatCommand {
            command: Bounded("gamemode spectator"),
        },
    );
    let packets = server.ticks_all(2);
    let admin = |anchor: Entity| {
        packets.iter().any(|packet| {
            matches!(&packet.target, PacketTarget::PlayerSet(set) if set.contains(&anchor))
                && matches!(&packet.data, PacketPayload::SystemChat(chat)
                    if chat.content.to_string().contains("chat.type.admin"))
        })
    };
    assert!(admin(watcher));
    assert!(!admin(guest));
    assert!(!admin(op));
}

#[test]
fn a_tick_answers_only_the_latest_request_of_a_player() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    for (id, text) in [(1, "/g"), (2, "/ga"), (3, "/ec")] {
        server.queue(
            op,
            &ServerboundCommandSuggestion {
                id: VarInt(id),
                text: Bounded(text),
            },
        );
    }
    let packets = server.ticks(op, 2);
    assert_eq!(replies(&packets), [(3, vec!["echo".to_owned()])]);
}

#[test]
fn a_long_request_from_a_player_who_cannot_edit_command_blocks_gets_no_reply() {
    let mut server = start();
    let (guest, _) = server.join(GUEST, "Guest");
    let text = format!("/{}", "a".repeat(32_499));
    let packets = server.send(
        guest,
        &ServerboundCommandSuggestion {
            id: VarInt(1),
            text: Bounded(&text),
        },
    );
    assert!(replies(&packets).is_empty());
}

#[test]
fn commands_that_share_a_handles_type_each_read_their_own_arguments() {
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin);
    app.add_command(echo_at("echo"), run_echo).unwrap();
    app.add_command(echo_at("shout"), run_echo).unwrap();
    let mut server = PlayServer::start(app, &[(OPERATOR, 2)], 0);
    let (op, _) = server.join(OPERATOR, "Op");
    for (line, reply) in [("echo 3", "echo 3"), ("shout 5", "echo 5")] {
        assert_eq!(
            plain(&chats(&command(&mut server, op, line))),
            [reply],
            "{line}"
        );
    }
}

fn clashing(r: &mut CommandRegistrar<'_, ()>) -> Result<Echo, BuildError> {
    let partial = r.literal(NodeId::ROOT, "partial").runs(()).id()?;
    r.literal(NodeId::ROOT, "echo").runs(()).id()?;
    Ok(Echo {
        times: r.arg(partial, "times", IntegerArgument::ANY).id()?,
    })
}

#[test]
fn a_command_whose_nodes_clash_is_not_registered_at_all() {
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin);
    app.add_command(echo_at("echo"), run_echo).unwrap();
    let clash = app.add_command(clashing, run_echo);
    assert!(
        matches!(
            clash,
            Err(mcrs_minecraft_server::command::RegistrationError::Tree(
                mcrs_command::BuildError::Conflict { .. }
            ))
        ),
        "{:?}",
        clash.err()
    );
    let mut server = PlayServer::start(app, &[], 0);
    let tree = &server
        .world()
        .resource::<mcrs_minecraft_server::command::CommandTree>()
        .0;
    let names: Vec<&str> = tree
        .node(NodeId::ROOT)
        .unwrap()
        .children()
        .iter()
        .map(|&child| tree.node(child).unwrap().kind().name())
        .collect();
    assert_eq!(names, ["echo"]);
}

fn translated(key: &'static str, with: &[&str]) -> Text {
    Text::translate(
        key,
        with.iter()
            .map(|&text| text.to_owned().into_text())
            .collect::<Vec<Text>>(),
    )
}

fn red(text: Text) -> Text {
    mcrs_minecraft_protocol::text::IntoText::color(text, mcrs_minecraft_protocol::text::Color::RED)
}

#[test]
fn execute_conditions_report_their_result_as_the_run_stage() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    server.join(GUEST, "Guest");
    let pass = translated("commands.execute.conditional.pass", &[]);
    let fail = red(translated("commands.execute.conditional.fail", &[]));
    let cases = [
        (
            "execute if entity @a",
            translated("commands.execute.conditional.pass_count", &["2"]),
        ),
        (
            "execute unless entity @a",
            red(translated(
                "commands.execute.conditional.fail_count",
                &["2"],
            )),
        ),
        ("execute unless entity @a[name=Nobody]", pass.clone()),
        ("execute if entity @a[name=Nobody]", fail.clone()),
        (
            "execute as @a if entity @s",
            translated("commands.execute.conditional.pass_count", &["1"]),
        ),
        ("execute if dimension minecraft:overworld", pass),
        ("execute if dimension minecraft:the_nether", fail),
    ];
    for (line, expected) in cases {
        let messages = chats(&command(&mut server, op, line));
        let expected = if line.starts_with("execute as @a") {
            vec![expected.clone(), expected]
        } else {
            vec![expected]
        };
        assert_eq!(messages, expected, "{line}");
    }
}

#[test]
fn execute_modifies_the_source_of_each_later_stage() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let (guest, _) = server.join(GUEST, "Guest");
    let cases = [
        (
            "execute positioned 1 64 1 run tp @s ~ ~ ~",
            (1.5, 64.0, 1.5),
            None,
        ),
        (
            "execute positioned 1.7 64 -0.2 align xz run tp @s ~ ~ ~",
            (1.0, 64.0, -1.0),
            None,
        ),
        (
            "execute positioned 0.5 64 0.5 facing 10.5 64 0.5 run tp @s ~ ~ ~ ~ ~",
            (0.5, 64.0, 0.5),
            Some((-90.0, 0.0)),
        ),
        (
            "execute positioned 0.5 64 0.5 facing 0.5 64 10.5 run tp @s ~ ~ ~ ~ ~",
            (0.5, 64.0, 0.5),
            Some((0.0, 0.0)),
        ),
        (
            "execute rotated 30 0 rotated ~15 0 run tp @s ~ ~ ~ ~ ~",
            (0.5, 64.0, 0.5),
            Some((45.0, 0.0)),
        ),
        (
            "execute positioned 5 70 5 positioned as Op run tp @s ~ ~ ~",
            (0.5, 64.0, 0.5),
            None,
        ),
    ];
    for (line, (x, y, z), rotation) in cases {
        command(&mut server, op, line);
        let transform = transform(&mut server, op);
        assert_eq!(transform.translation, DVec3::new(x, y, z), "{line}");
        if let Some((yaw, pitch)) = rotation {
            assert_eq!(
                (transform.rotation.yaw(), transform.rotation.pitch()),
                (yaw, pitch),
                "{line}"
            );
        }
    }
    let packets = command(
        &mut server,
        op,
        "execute positioned 0.5 64 0.5 anchored eyes facing 0.5 64 10.5 run tp @s ~ ~ ~ ~ ~",
    );
    assert!(!chats(&packets).is_empty());
    assert!(
        transform(&mut server, op).rotation.pitch() > 0.0,
        "the player looks down from its eyes"
    );

    command(&mut server, op, "execute as Guest run tp @s 20 70 20");
    assert_eq!(
        transform(&mut server, guest).translation,
        DVec3::new(20.5, 70.0, 20.5)
    );
    command(&mut server, op, "execute at Guest run tp @s ~ ~ ~");
    assert_eq!(
        transform(&mut server, op).translation,
        DVec3::new(20.5, 70.0, 20.5)
    );
    command(&mut server, op, "execute as @a run tp @s 3 80 3");
    assert_eq!(
        transform(&mut server, guest).translation,
        DVec3::new(3.5, 80.0, 3.5)
    );
}

#[test]
fn execute_reports_errors_only_outside_a_fork() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let invalid = red(translated("commands.teleport.invalidPosition", &[]));
    assert_eq!(
        chats(&command(&mut server, op, "execute run tp @s 0 25000000 0")),
        [invalid]
    );
    assert!(
        chats(&command(
            &mut server,
            op,
            "execute as @s run tp @s 0 25000000 0"
        ))
        .is_empty()
    );
    assert!(
        chats(&command(
            &mut server,
            op,
            "execute as @a[name=Nobody] run tp @s 0 64 0"
        ))
        .is_empty()
    );
}

#[test]
fn execute_stops_at_the_fork_and_sequence_limits() {
    let mut server = with_limits(Limits {
        forks: 1,
        ..Limits::default()
    });
    let (op, _) = server.join(OPERATOR, "Op");
    assert_eq!(
        chats(&command(
            &mut server,
            op,
            "execute positioned ~ ~ ~ run tp @s 1 64 1"
        )),
        [red(translated("command.forkLimit", &["1"]))]
    );
    assert_eq!(
        transform(&mut server, op).translation,
        DVec3::new(8.5, 64.0, 8.5)
    );

    let mut server = with_limits(Limits {
        sequence: 2,
        ..Limits::default()
    });
    let (op, _) = server.join(OPERATOR, "Op");
    let (guest, _) = server.join(GUEST, "Guest");
    command(&mut server, op, "execute as @a run tp @s 1 64 1");
    let moved = [op, guest]
        .into_iter()
        .filter(|&player| transform(&mut server, player).translation == DVec3::new(1.5, 64.0, 1.5))
        .count();
    assert_eq!(moved, 1, "one stage and one run fit in a sequence of 2");
}

#[test]
fn execute_reads_the_blocks_and_heights_of_loaded_chunks() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    load_stone_column(&mut server);
    let pass = translated("commands.execute.conditional.pass", &[]);
    let fail = red(translated("commands.execute.conditional.fail", &[]));
    let unloaded = red(translated("argument.pos.unloaded", &[]));
    let cases = [
        ("execute if loaded ~ ~ ~", pass.clone()),
        ("execute if loaded 100000 64 100000", fail.clone()),
        ("execute if loaded ~ 99999 ~", pass.clone()),
        ("execute if block ~ ~-1 ~ minecraft:stone", pass.clone()),
        ("execute if block ~ ~ ~ air", pass.clone()),
        ("execute unless block ~ ~-1 ~ stone", fail.clone()),
        ("execute if block ~ ~ ~ stone", fail),
        (
            "execute if block ~ 1000 ~ air",
            red(translated("argument.pos.outofworld", &[])),
        ),
        ("execute if block 100000 64 100000 stone", unloaded.clone()),
        (
            "execute positioned 100000 64 100000 positioned over world_surface run tp @s ~ ~ ~",
            unloaded,
        ),
    ];
    for (line, expected) in cases {
        assert_eq!(chats(&command(&mut server, op, line)), [expected], "{line}");
    }
    assert!(
        chats(&command(
            &mut server,
            op,
            "execute as @s if block 100000 64 100000 stone run tp @s 0 64 0"
        ))
        .is_empty()
    );
    command(
        &mut server,
        op,
        "execute positioned 3.5 100 4.5 positioned over world_surface run tp @s ~ ~ ~",
    );
    assert_eq!(
        transform(&mut server, op).translation,
        DVec3::new(3.5, 64.0, 4.5)
    );
}

#[test]
fn the_execute_tree_holds_only_the_branches_that_run() {
    let mut server = start();
    let tree = &server
        .world()
        .resource::<mcrs_minecraft_server::command::CommandTree>()
        .0;
    let child = |parent: NodeId, name: &str| {
        tree.node(parent)
            .unwrap()
            .children()
            .iter()
            .copied()
            .find(|&child| tree.node(child).unwrap().kind().name() == name)
            .unwrap()
    };
    let names = |parent: NodeId| -> Vec<&str> {
        tree.node(parent)
            .unwrap()
            .children()
            .iter()
            .map(|&child| tree.node(child).unwrap().kind().name())
            .collect()
    };
    let execute = child(NodeId::ROOT, "execute");
    assert_eq!(
        names(execute),
        [
            "run",
            "if",
            "unless",
            "as",
            "at",
            "positioned",
            "rotated",
            "facing",
            "align",
            "anchored"
        ]
    );
    for condition in ["if", "unless"] {
        assert_eq!(
            names(child(execute, condition)),
            ["block", "loaded", "dimension", "entity"]
        );
    }
    let run = tree.node(child(execute, "run")).unwrap();
    assert_eq!((run.redirect(), run.modifier()), (Some(NodeId::ROOT), None));
    let entity = child(child(child(execute, "if"), "entity"), "entities");
    let entity = tree.node(entity).unwrap();
    assert!(entity.command().is_some() && entity.forks());
    assert_eq!(entity.redirect(), Some(execute));
    let pos = tree
        .node(child(child(execute, "positioned"), "pos"))
        .unwrap();
    assert!(pos.modifier().is_some() && !pos.forks());
}
