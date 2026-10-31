use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{In, ResMut, Resource, With};
use bevy_math::{BVec3, DVec3};
use mcrs_command::{
    ArgumentHandle, BuildError, ErrorKind, IntegerArgument, LibraryError, Limits, NodeId,
};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_engine::{
    AppCommandsExt, Call, CommandEvent, CommandLimits, CommandOutput, CommandPlugin,
    CommandRegistrar, CommandResult, CommandTargets, EffectQueue, Feedback, Worlds,
};
use mcrs_minecraft_command_tree::CommandNodeKind;
use mcrs_minecraft_command_vanilla::RelativeRotation;
use mcrs_minecraft_command_vanilla::{
    Teleport, TeleportPlayer, VanillaCommands, VanillaCommandsPlugin, VanillaContext,
};
use mcrs_minecraft_core::LookAnchor;
use mcrs_minecraft_engine::bus::{PacketPayload, PacketTarget};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, VarInt};
use mcrs_minecraft_server::operator_broadcast::broadcast_to_operators;

use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_command_tree::{CommandNode, NumberRange, ParserProperties};
use mcrs_minecraft_core::{Id, ResourceLocation};
use mcrs_minecraft_engine::clock::{GameTime, WorldClockHostPlugin};
use mcrs_minecraft_environment::world_clock::{AdvanceTime, WorldClockPlugin};
use mcrs_minecraft_environment::world_clock::{ClockState, WorldClock, WorldClocks};
use mcrs_minecraft_level::world::storage::block_entity::BlockEntity;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundCommands, ClientboundSetTime, ClockUpdate, LookAtEntity,
};
use mcrs_minecraft_protocol::text::{TextContent, TranslateArg};
use mcrs_minecraft_protocol::{Decode, Packet, VarLong};
use mcrs_minecraft_registries::CurrentRegistries;

use crate::host_app::{self, PlayServer};

#[derive(Clone, Copy)]
struct Echo {
    times: ArgumentHandle<IntegerArgument>,
}

fn echo_at(
    name: &'static str,
) -> impl FnOnce(&mut CommandRegistrar<'_, VanillaCommands, ()>) -> Result<Echo, BuildError> {
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
    context: VanillaContext,
    mut feedback: CommandOutput,
) -> CommandResult {
    let command = call.command;
    let times = context.arg(&call.invocation, command.times)?;
    feedback.success(
        &call.invocation,
        format!("echo {times}").into_text(),
        Feedback::Source,
    );
    Ok(times)
}

const OPERATOR: Uuid = Uuid::from_u128(1);
const GUEST: Uuid = Uuid::from_u128(2);

fn start() -> PlayServer {
    with_limits(Limits::default())
}

fn with_limits(limits: Limits) -> PlayServer {
    start_in(&[("minecraft:overworld", "minecraft:overworld")], limits)
}

fn start_in(dimensions: &[(&str, &str)], limits: Limits) -> PlayServer {
    let mut app = host_app::make_host_app();
    app.insert_resource(CommandLimits(limits));
    app.add_plugins(WorldClockPlugin);
    app.insert_resource(AdvanceTime(false));
    app.add_plugins(WorldClockHostPlugin);
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_observer(broadcast_to_operators);
    app.add_plugins(VanillaCommandsPlugin);
    app.add_command(echo_at("echo"), run_echo).unwrap();
    app.add_command(echo_at("shout"), run_echo).unwrap();
    PlayServer::start_in(app, dimensions, &[(OPERATOR, 2)], 0)
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
    let dim = server.dim;
    transform_in(server, dim, anchor)
}

fn transform_in(server: &mut PlayServer, dim: Entity, anchor: Entity) -> Transform {
    let player = server.player_in(dim, anchor).unwrap();
    *server.world_in(dim).get::<Transform>(player).unwrap()
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

/// A block entity in the section of the stone column that holds `pos`.
fn place_block_entity(server: &mut PlayServer, entry: GeneratedBlockEntity) {
    use mcrs_minecraft_level::world::storage::block_entity::{BlockEntityPos, InSection};
    use mcrs_minecraft_level::world::storage::column::{ColumnSections, SectionLookup};
    let pos = entry.position();
    let world = server.world();
    let section = world
        .query::<&ColumnSections>()
        .single(world)
        .map(|column| column.lookup(pos.y >> 4))
        .unwrap();
    let SectionLookup::Loaded(section) = section else {
        panic!("the section of {pos:?} is loaded");
    };
    world.spawn((
        BlockEntityPos(mcrs_minecraft_core::BlockPos::new(pos.x, pos.y, pos.z)),
        InSection(section),
        BlockEntity(entry),
    ));
}

/// The tree the player receives on joining, decoded from the frame the server encoded once.
fn sent_tree(packets: &[PacketPayload]) -> ClientboundCommands<'_> {
    let bytes = packets
        .iter()
        .find_map(|packet| match packet {
            PacketPayload::Commands(bytes) => Some(bytes),
            _ => None,
        })
        .expect("the player receives its tree on joining");
    let mut slice = bytes.as_ref();
    assert_eq!(
        VarInt::decode(&mut slice).unwrap().0,
        ClientboundCommands::ID
    );
    let tree = ClientboundCommands::decode(&mut slice).unwrap();
    assert!(slice.is_empty(), "the decode ends at the last byte");
    tree
}

fn root_literals(packets: &[PacketPayload]) -> Vec<String> {
    let tree = sent_tree(packets);
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
    let mut literals = root_literals(&packets);
    literals.sort();
    assert_eq!(
        literals,
        [
            "echo", "execute", "gamemode", "locate", "shout", "teleport", "time", "tp"
        ]
    );
    let (guest, packets) = server.join(GUEST, "Guest");
    assert_eq!(root_literals(&packets), Vec::<String>::new());

    assert_eq!(suggest(&mut server, op, "/ec"), ["echo"]);
    assert!(suggest(&mut server, guest, "/ec").is_empty());

    for (line, reply) in [("echo 3", "echo 3"), ("shout 5", "echo 5")] {
        assert_eq!(
            plain(&chats(&command(&mut server, op, line))),
            [reply],
            "commands that share a handler type each read their own arguments: {line}"
        );
    }
    assert_eq!(
        chats(&command(&mut server, guest, "echo 3")),
        [
            red(translated("command.unknown.command", &[])),
            error_context("", "echo 3", "echo 3")
        ]
    );
    let [too_low, context] = chats(&command(&mut server, op, "echo 0"))
        .try_into()
        .unwrap();
    assert_eq!(
        too_low,
        Text::translate_args(
            "argument.integer.low",
            vec![
                TranslateArg::Number(NbtTag::Int(1)),
                TranslateArg::Number(NbtTag::Int(0))
            ],
        )
        .color(mcrs_minecraft_protocol::text::Color::RED),
        "the minimum, then the value, as numbers"
    );
    assert_eq!(context, error_context("echo ", "0", "echo 0"));
}

/// The context line of an error at the end of `before`, with `rest` after it.
fn error_context(before: &str, rest: &str, line: &str) -> Text {
    use mcrs_minecraft_protocol::text::Color;
    let mut context = before.to_owned().into_text();
    if !rest.is_empty() {
        context = context.add_child(rest.to_owned().color(Color::RED).underlined());
    }
    context
        .add_child(
            Text::translate("command.context.here", vec![])
                .color(Color::RED)
                .italic(),
        )
        .color(Color::GRAY)
        .on_click_suggest_command(format!("/{line}"))
}

#[test]
fn teleport_sends_the_relative_axes_and_reports_the_resolved_position() {
    use mcrs_minecraft_protocol::{PositionFlag as F, PositionFlags};
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
    assert_eq!(
        position.flags,
        PositionFlags::from_iter([F::DeltaX, F::DeltaY, F::X, F::XRot, F::Y, F::YRot])
    );
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
    use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerLookAt;
    use mcrs_minecraft_protocol::{PositionFlag as F, PositionFlags};
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
    assert_eq!(position.flags, PositionFlags::from_iter([F::XRot, F::YRot]));
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
        let mut packets = command(&mut server, op, &format!("gamemode {mode}"));
        packets.extend(server.ticks(op, 1));
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
fn an_admin_message_reaches_only_the_other_listed_operators_of_every_dimension() {
    const WATCHER: Uuid = Uuid::from_u128(3);
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_observer(broadcast_to_operators);
    app.add_plugins(VanillaCommandsPlugin);
    let mut server = PlayServer::start_in(
        app,
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_nether", "minecraft:the_nether"),
        ],
        &[(OPERATOR, 2), (WATCHER, 2)],
        2,
    );
    let (op, _) = server.join(OPERATOR, "Op");
    let nether = host_app::label_of(&server.app, "minecraft:the_nether");
    let (watcher, _) = server.join_in(nether, WATCHER, "Watcher");
    let (guest, _) = server.join(GUEST, "Guest");
    server.queue(
        op,
        &ServerboundChatCommand {
            command: Bounded("gamemode spectator"),
        },
    );
    let packets = server.ticks_all(2);
    let admin = Text::translate(
        "chat.type.admin",
        vec![
            "Op".into_text(),
            Text::translate(
                "commands.gamemode.success.self",
                vec![Text::translate("gameMode.spectator", vec![])],
            ),
        ],
    )
    .color(mcrs_minecraft_protocol::text::Color::GRAY)
    .italic();
    let admin_to = |anchor: Entity| -> Vec<&Text> {
        packets
            .iter()
            .filter(|packet| {
                matches!(&packet.target, PacketTarget::PlayerSet(set) if set.contains(&anchor))
            })
            .filter_map(|packet| match &packet.data {
                PacketPayload::SystemChat(chat) => Some(&chat.content),
                _ => None,
            })
            .collect()
    };
    assert_eq!(admin_to(watcher), [&admin]);
    assert!(admin_to(guest).is_empty());
    assert!(admin_to(op).is_empty());
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
fn a_request_longer_than_the_chat_screen_gets_a_reply_only_from_a_command_block_editor() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let (guest, _) = server.join(GUEST, "Guest");
    let cases = [
        ("guest at the chat limit", guest, 256, true),
        ("guest past the chat limit", guest, 257, false),
        ("operator at the command block limit", op, 32_500, true),
    ];
    for (case, player, units, answered) in cases {
        let text = format!("/{}", "a".repeat(units - 1));
        let packets = server.send(
            player,
            &ServerboundCommandSuggestion {
                id: VarInt(1),
                text: Bounded(&text),
            },
        );
        assert_eq!(!replies(&packets).is_empty(), answered, "{case}");
    }
}

fn clashing(r: &mut CommandRegistrar<'_, VanillaCommands, ()>) -> Result<Echo, BuildError> {
    let partial = r.literal(NodeId::ROOT, "partial").runs(()).id()?;
    r.literal(NodeId::ROOT, "echo").runs(()).id()?;
    Ok(Echo {
        times: r.arg(partial, "times", IntegerArgument::ANY).id()?,
    })
}

#[test]
fn a_command_whose_nodes_clash_is_not_registered_at_all() {
    let mut app = host_app::make_host_app();
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_observer(broadcast_to_operators);
    app.add_command(echo_at("echo"), run_echo).unwrap();
    let clash = app.add_command(clashing, run_echo);
    assert!(
        matches!(
            clash,
            Err(mcrs_minecraft_command_engine::RegistrationError::Tree(
                mcrs_command::BuildError::Conflict { .. }
            ))
        ),
        "{:?}",
        clash.err()
    );
    let server = PlayServer::start(app, &[], 0);
    let tree = server
        .app
        .world()
        .resource::<mcrs_minecraft_command_engine::CommandTree<VanillaCommands>>()
        .graph()
        .unwrap();
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
    let one = translated("commands.execute.conditional.pass_count", &["1"]);
    let cases = [
        (
            "execute if entity @a",
            vec![translated(
                "commands.execute.conditional.pass_count",
                &["2"],
            )],
        ),
        (
            "execute unless entity @a",
            vec![red(translated(
                "commands.execute.conditional.fail_count",
                &["2"],
            ))],
        ),
        ("execute unless entity @a[name=Nobody]", vec![pass.clone()]),
        ("execute if entity @a[name=Nobody]", vec![fail]),
        ("execute as @a if entity @s", vec![one.clone(), one]),
        ("execute if dimension minecraft:overworld", vec![pass]),
        (
            "execute if dimension minecraft:the_nether",
            vec![red(translated(
                "argument.dimension.invalid",
                &["minecraft:the_nether"],
            ))],
        ),
    ];
    for (line, expected) in cases {
        assert_eq!(chats(&command(&mut server, op, line)), expected, "{line}");
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
    assert_eq!(
        chats(&packets),
        [translated(
            "commands.teleport.success.location.single",
            &["Op", "0.500000", "64.000000", "0.500000"]
        )]
    );
    let rotation = transform(&mut server, op).rotation;
    let looks_down = 1.62f32.atan2(10.0).to_degrees();
    assert!(
        rotation.yaw().abs() < 1e-3 && (rotation.pitch() - looks_down).abs() < 1e-3,
        "the player looks from its eyes 1.62 above the target 10 blocks south: {rotation:?}"
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
    let cases = [
        (
            "execute positioned ~ ~ ~ run tp @s 0 25000000 0",
            vec![invalid],
        ),
        ("execute as @s run tp @s 0 25000000 0", vec![]),
    ];
    for (line, expected) in cases {
        assert_eq!(chats(&command(&mut server, op, line)), expected, "{line}");
    }
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

    let mut server = with_limits(Limits {
        sequence: 2,
        ..Limits::default()
    });
    let (op, _) = server.join(OPERATOR, "Op");
    let (guest, _) = server.join(GUEST, "Guest");
    let told = chats(&command(&mut server, op, "execute as @a run tp @s 1 64 1"));
    let at = |server: &mut PlayServer, player| transform(server, player).translation;
    assert_eq!(
        (told, at(&mut server, op), at(&mut server, guest)),
        (
            vec![translated(
                "commands.teleport.success.location.single",
                &["Op", "1.500000", "64.000000", "1.500000"]
            )],
            DVec3::new(1.5, 64.0, 1.5),
            DVec3::new(8.5, 64.0, 8.5)
        ),
        "one stage and the run of the first player fit in a sequence of 2"
    );
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
        ("execute if block ~ ~ ~ stone", fail.clone()),
        (
            "execute if block ~ ~-1 ~ #minecraft:base_stone_overworld",
            pass.clone(),
        ),
        (
            "execute if block ~ ~ ~ #minecraft:base_stone_overworld",
            fail.clone(),
        ),
        (
            "execute unless block ~ ~-1 ~ #minecraft:base_stone_overworld",
            fail,
        ),
        (
            "execute unless block ~ ~ ~ #minecraft:base_stone_overworld",
            pass.clone(),
        ),
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
fn execute_tests_the_saved_data_of_a_block_entity() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    load_stone_column(&mut server);
    place_block_entity(
        &mut server,
        GeneratedBlockEntity::chest(
            mcrs_minecraft_core::BlockPos::new(8, 63, 8),
            "minecraft:chests/simple_dungeon".to_owned(),
            5,
        ),
    );
    place_block_entity(
        &mut server,
        GeneratedBlockEntity::Chest(mcrs_minecraft_block_entity::ContainerData {
            x: 9,
            y: 63,
            z: 8,
            loot_table: None,
            loot_table_seed: 0,
            items: Vec::new(),
            custom_name: None,
            components: None,
        }),
    );
    let pass = translated("commands.execute.conditional.pass", &[]);
    let fail = red(translated("commands.execute.conditional.fail", &[]));
    let cases = [
        ("execute if block ~1 ~-1 ~ stone{Items:[]}", pass.clone()),
        ("execute if block ~ ~-1 ~ stone{Items:[]}", fail.clone()),
        ("execute if block ~ ~-1 ~ stone{}", pass.clone()),
        (
            "execute if block ~ ~-1 ~ stone{x:8,y:63,z:8,id:\"minecraft:chest\",components:{}}",
            pass.clone(),
        ),
        (
            "execute if block ~ ~-1 ~ stone{LootTable:\"minecraft:chests/simple_dungeon\",LootTableSeed:5L}",
            pass,
        ),
        ("execute if block ~ ~-2 ~ stone{}", fail),
    ];
    for (line, expected) in cases {
        assert_eq!(chats(&command(&mut server, op, line)), [expected], "{line}");
    }
}

const THIRD: Uuid = Uuid::from_u128(3);

/// The operator and a guest in the Overworld, and the player `Nether` in the Nether.
fn two_dimensions(limits: Limits) -> (PlayServer, Entity, Entity) {
    let mut server = start_in(
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_nether", "minecraft:the_nether"),
        ],
        limits,
    );
    let (op, _) = server.join(OPERATOR, "Op");
    server.join(GUEST, "Guest");
    let nether = host_app::label_of(&server.app, "minecraft:the_nether");
    let (player, _) = server.join_in(nether, THIRD, "Nether");
    (server, op, player)
}

#[test]
fn execute_in_runs_the_rest_of_the_line_in_the_named_dimension() {
    let (mut server, op, player) = two_dimensions(Limits::default());
    let nether = host_app::label_of(&server.app, "minecraft:the_nether");
    let pass = translated("commands.execute.conditional.pass", &[]);
    let cases = [
        (
            "execute in minecraft:the_nether if dimension minecraft:the_nether",
            vec![pass.clone()],
        ),
        (
            "execute in minecraft:overworld if dimension minecraft:overworld",
            vec![pass],
        ),
        (
            "execute in minecraft:the_nether if entity @a",
            vec![translated(
                "commands.execute.conditional.pass_count",
                &["3"],
            )],
        ),
        (
            "execute in minecraft:the_nether if entity @a[distance=0..]",
            vec![translated(
                "commands.execute.conditional.pass_count",
                &["1"],
            )],
        ),
        (
            "execute in minecraft:the_nether in minecraft:overworld if entity @a[distance=0..]",
            vec![translated(
                "commands.execute.conditional.pass_count",
                &["2"],
            )],
        ),
        (
            "execute in minecraft:nowhere run tp Nether 0 64 0",
            vec![red(translated(
                "argument.dimension.invalid",
                &["minecraft:nowhere"],
            ))],
        ),
        (
            "execute as @s in minecraft:nowhere run tp Nether 0 64 0",
            vec![],
        ),
        (
            "execute in minecraft:the_nether run tp Nobody 0 64 0",
            vec![red(translated("argument.entity.notfound.entity", &[]))],
        ),
    ];
    for (line, expected) in cases {
        let mut packets = command(&mut server, op, line);
        packets.extend(server.ticks(op, 1));
        assert_eq!(chats(&packets), expected, "{line}");
    }

    let packets = command(
        &mut server,
        op,
        "execute positioned 800.0 64 -160.0 in minecraft:the_nether run tp Nether ~ ~ ~",
    );
    assert_eq!(
        chats(&packets),
        [translated(
            "commands.teleport.success.location.single",
            &["Nether", "100.000000", "64.000000", "-20.000000"]
        )],
        "the operator hears from the Nether"
    );
    assert_eq!(
        transform_in(&mut server, nether, player).translation,
        DVec3::new(100.0, 64.0, -20.0)
    );
}

#[test]
fn suggestions_name_the_players_and_the_dimensions_that_run_anywhere() {
    let (mut server, op, _) = two_dimensions(Limits::default());
    let cases: [(&str, &[&str]); 2] = [
        ("/tp N", &["Nether"]),
        (
            "/execute in ",
            &["minecraft:overworld", "minecraft:the_nether"],
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(suggest(&mut server, op, text), expected, "{text}");
    }
}

#[test]
fn a_forked_chain_keeps_its_fork_and_its_cost_in_the_dimension_it_moves_to() {
    let (mut server, op, _) = two_dimensions(Limits::default());
    let line = "execute as @a in minecraft:the_nether run tp Nether 0 64 0";
    let moved = translated(
        "commands.teleport.success.location.single",
        &["Nether", "0.500000", "64.000000", "0.500000"],
    );
    assert_eq!(
        chats(&command(&mut server, op, line)),
        [moved.clone(), moved.clone(), moved.clone()]
    );
    assert!(
        chats(&command(
            &mut server,
            op,
            "execute as @a in minecraft:the_nether run tp Nobody 0 64 0"
        ))
        .is_empty()
    );

    let (mut server, op, _) = two_dimensions(Limits {
        sequence: 3,
        ..Limits::default()
    });
    assert_eq!(
        chats(&command(&mut server, op, line)),
        [moved],
        "two stages and one run fit in a sequence of 3"
    );
}

type MessageArg = mcrs_minecraft_protocol::text::TranslateArg<
    mcrs_minecraft_item::Template<mcrs_minecraft_registries::Named>,
>;

fn said(key: &'static str, with: Vec<MessageArg>) -> Text {
    Text::translate_args(key, with)
}

fn name(id: &str) -> MessageArg {
    id.to_owned().into()
}

fn set_times(packets: &[PacketPayload]) -> Vec<ClientboundSetTime> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::SetTime(set_time) => Some(set_time.clone()),
            _ => None,
        })
        .collect()
}

fn clock_id(server: &PlayServer, clock: &str) -> Id<WorldClock> {
    server
        .app
        .world()
        .resource::<CurrentRegistries>()
        .registry::<WorldClock>()
        .unwrap()
        .iter()
        .find(|(_, name, _)| name.as_str() == clock)
        .map(|(id, _, _)| id)
        .unwrap()
}

fn clock(server: &PlayServer, clock: &str) -> ClockState {
    *server
        .app
        .world()
        .resource::<WorldClocks>()
        .get(clock_id(server, clock))
        .unwrap()
}

/// The `set_time` of a change to the overworld clock that leaves it at `total_ticks`.
fn overworld_update(server: &PlayServer, total_ticks: i64) -> ClientboundSetTime {
    ClientboundSetTime {
        game_time: 0,
        clocks: vec![ClockUpdate {
            clock: VarInt(i32::from(clock_id(server, OVERWORLD_CLOCK).number())),
            total_ticks: VarLong(total_ticks),
            partial_tick: 0.0,
            rate: 0.0,
        }],
    }
}

const OVERWORLD_CLOCK: &str = "minecraft:overworld";

#[test]
fn a_clock_change_reaches_the_next_stage_in_another_dimension() {
    let (mut server, op, _) = two_dimensions(Limits::default());
    let ow = || name(OVERWORLD_CLOCK);
    let packets = command(
        &mut server,
        op,
        "execute at @a run time of minecraft:overworld add 10",
    );
    let set = |total: i64| said("commands.time.set.absolute", vec![ow(), total.into()]);
    assert_eq!(chats(&packets), [set(10), set(20), set(30)]);
    assert_eq!(clock(&server, OVERWORLD_CLOCK).total_ticks, 30);
}

fn without_game_time(mut set_times: Vec<ClientboundSetTime>) -> Vec<ClientboundSetTime> {
    for set_time in &mut set_times {
        set_time.game_time = 0;
    }
    set_times
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the table holds one row for each example of the specification"
)]
fn time_changes_and_reads_the_overworld_clock() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let ow = || name(OVERWORLD_CLOCK);

    server.queue(
        op,
        &ServerboundChatCommand {
            command: Bounded("time add 10"),
        },
    );
    let packets = command(&mut server, op, "time add 10");
    assert_eq!(
        chats(&packets),
        [
            said("commands.time.set.absolute", vec![ow(), 10i64.into()]),
            said("commands.time.set.absolute", vec![ow(), 20i64.into()]),
        ],
        "a second command of one tick sees the change of the first"
    );
    assert_eq!(clock(&server, OVERWORLD_CLOCK).total_ticks, 20);

    // (command line, the message, the total ticks after it, whether it sends `set_time`)
    let cases: Vec<(&str, Text, i64, bool)> = vec![
        (
            "time set 1000",
            said("commands.time.set.absolute", vec![ow(), 1000.into()]),
            1000,
            true,
        ),
        (
            "time set 1000",
            red(said(
                "commands.time.set.already_at_time",
                vec![ow(), 1000.into()],
            )),
            1000,
            false,
        ),
        (
            "time set 1d",
            said("commands.time.set.absolute", vec![ow(), 24_000.into()]),
            24_000,
            true,
        ),
        (
            "time set 5x",
            red(said(
                "commands.time.no_time_marker_found",
                vec![name("minecraft:5x"), ow()],
            )),
            24_000,
            false,
        ),
        (
            "time set -1",
            red(said(
                "commands.time.no_time_marker_found",
                vec![name("minecraft:-1"), ow()],
            )),
            24_000,
            false,
        ),
        (
            "time set day",
            said(
                "commands.time.set.time_marker",
                vec![ow(), name("minecraft:day")],
            ),
            25_000,
            true,
        ),
        (
            "time set day",
            red(said(
                "commands.time.set.already_at_time_marker",
                vec![ow(), name("minecraft:day")],
            )),
            25_000,
            false,
        ),
        (
            "time set minecraft:wake_up_from_sleep",
            said(
                "commands.time.set.time_marker",
                vec![ow(), name("minecraft:wake_up_from_sleep")],
            ),
            48_000,
            true,
        ),
        (
            "time set 50000",
            said("commands.time.set.absolute", vec![ow(), 50_000.into()]),
            50_000,
            true,
        ),
        (
            "time query day",
            said(
                "commands.time.query.timeline",
                vec![name("minecraft:day"), 2000i64.into()],
            ),
            50_000,
            false,
        ),
        (
            "time query day repetition",
            said(
                "commands.time.query.timeline.repetitions",
                vec![name("minecraft:day"), 2i64.into()],
            ),
            50_000,
            false,
        ),
        (
            "time query early_game",
            said(
                "commands.time.query.timeline",
                vec![name("minecraft:early_game"), 50_000i64.into()],
            ),
            50_000,
            false,
        ),
        (
            "time add -3d",
            said("commands.time.set.absolute", vec![ow(), 0i64.into()]),
            0,
            true,
        ),
        (
            "time add 0",
            said("commands.time.set.absolute", vec![ow(), 0i64.into()]),
            0,
            true,
        ),
        (
            "time pause",
            said("commands.time.pause", vec![ow()]),
            0,
            true,
        ),
        (
            "time pause",
            red(said("commands.time.pause.already_paused", vec![ow()])),
            0,
            false,
        ),
        (
            "time resume",
            said("commands.time.resume", vec![ow()]),
            0,
            true,
        ),
        (
            "time resume",
            red(said("commands.time.pause.already_running", vec![ow()])),
            0,
            false,
        ),
        (
            "time rate 1",
            red(said(
                "commands.time.rate.already_same",
                vec![ow(), 1.0f32.into()],
            )),
            0,
            false,
        ),
        (
            "time rate 2",
            said("commands.time.rate", vec![ow(), 2.0f32.into()]),
            0,
            true,
        ),
        (
            "time of minecraft:overworld query time",
            said("commands.time.query.absolute", vec![ow(), 0i64.into()]),
            0,
            false,
        ),
        (
            "time of minecraft:the_end query day",
            red(said(
                "commands.time.wrong_timeline_for_clock",
                vec![name("minecraft:day"), name("minecraft:the_end")],
            )),
            0,
            false,
        ),
        (
            "time of minecraft:the_end set day",
            red(said(
                "commands.time.no_time_marker_found",
                vec![name("minecraft:day"), name("minecraft:the_end")],
            )),
            0,
            false,
        ),
        (
            "time of minecraft:nope query time",
            red(said(
                "argument.resource.not_found",
                vec![name("minecraft:nope"), name("minecraft:world_clock")],
            )),
            0,
            false,
        ),
        (
            "time set 100000d",
            said("commands.time.set.absolute", vec![ow(), i32::MAX.into()]),
            i64::from(i32::MAX),
            true,
        ),
        (
            "time add 5",
            said(
                "commands.time.set.absolute",
                vec![ow(), (i64::from(i32::MAX) + 5).into()],
            ),
            i64::from(i32::MAX) + 5,
            true,
        ),
    ];
    for (line, message, total_ticks, sends) in cases {
        let packets = command(&mut server, op, line);
        assert_eq!(chats(&packets).first(), Some(&message), "{line}");
        assert_eq!(
            clock(&server, OVERWORLD_CLOCK).total_ticks,
            total_ticks,
            "{line}"
        );
        let expected = if sends {
            vec![overworld_update(&server, total_ticks)]
        } else {
            Vec::new()
        };
        assert_eq!(without_game_time(set_times(&packets)), expected, "{line}");
    }
    let state = clock(&server, OVERWORLD_CLOCK);
    assert_eq!((state.rate, state.paused), (2.0, false));
}

fn child<'p, 'a>(
    tree: &'p ClientboundCommands<'a>,
    node: &'p CommandNode<'a>,
    name: &str,
) -> &'p CommandNode<'a> {
    node.children
        .iter()
        .map(|child| &tree.nodes[child.0 as usize])
        .find(|child| match &child.kind {
            CommandNodeKind::Literal(literal) => literal == name,
            CommandNodeKind::Argument { name: argument, .. } => argument == name,
            CommandNodeKind::Root => false,
        })
        .unwrap_or_else(|| panic!("no child {name}"))
}

fn child_names(tree: &ClientboundCommands<'_>, node: &CommandNode<'_>) -> Vec<String> {
    node.children
        .iter()
        .map(|child| match &tree.nodes[child.0 as usize].kind {
            CommandNodeKind::Literal(name) | CommandNodeKind::Argument { name, .. } => {
                name.to_string()
            }
            CommandNodeKind::Root => String::new(),
        })
        .collect()
}

/// The properties and the suggestion type of an argument node.
fn argument_wire<'a>(node: &CommandNode<'a>) -> (ParserProperties<'a>, Option<String>) {
    let CommandNodeKind::Argument {
        parser,
        suggestions,
        ..
    } = &node.kind
    else {
        panic!("{node:?}");
    };
    (
        parser.properties.clone(),
        suggestions.as_ref().map(ToString::to_string),
    )
}

#[test]
fn the_time_tree_and_its_server_suggestions_follow_the_clock() {
    let mut server = start();
    let (op, packets) = server.join(OPERATOR, "Op");
    let tree = sent_tree(&packets);
    let root = &tree.nodes[tree.root.0 as usize];
    let time = child(&tree, root, "time");
    let sorted = |node| {
        let mut names = child_names(&tree, node);
        names.sort();
        names
    };
    assert_eq!(
        sorted(time),
        ["add", "of", "pause", "query", "rate", "resume", "set"]
    );
    assert_eq!(
        sorted(child(&tree, time, "query")),
        ["gametime", "time", "timeline"]
    );
    let clock = child(&tree, child(&tree, time, "of"), "clock");
    assert_eq!(sorted(child(&tree, clock, "query")), ["time", "timeline"]);
    let set = child(&tree, time, "set");
    assert_eq!(
        child_names(&tree, set),
        ["time", "timemarker"],
        "a time is tried before a marker"
    );
    let ask_server = Some("minecraft:ask_server".to_owned());
    let registry =
        |id: &str| ParserProperties::Registry(ResourceLocation::read(id).unwrap().into());
    let cases = [
        (child(&tree, set, "time"), (ParserProperties::Time(0), None)),
        (
            child(&tree, set, "timemarker"),
            (ParserProperties::None, ask_server.clone()),
        ),
        (
            child(&tree, child(&tree, time, "add"), "time"),
            (ParserProperties::Time(i32::MIN), None),
        ),
        (
            child(&tree, child(&tree, time, "rate"), "rate"),
            (
                ParserProperties::Float(NumberRange {
                    min: Some(1.0e-5),
                    max: Some(1000.0),
                }),
                None,
            ),
        ),
        (clock, (registry("minecraft:world_clock"), None)),
        (
            child(&tree, child(&tree, time, "query"), "timeline"),
            (registry("minecraft:timeline"), ask_server.clone()),
        ),
    ];
    for (node, expected) in cases {
        assert_eq!(argument_wire(node), expected, "{node:?}");
    }

    let markers = [
        "minecraft:day",
        "minecraft:midnight",
        "minecraft:night",
        "minecraft:noon",
    ];
    let suggestions: [(&str, &[&str]); 6] = [
        ("/time set ", &markers),
        ("/time of minecraft:overworld set ", &markers),
        ("/time set ni", &["minecraft:night"]),
        ("/time set 1", &["d", "s", "t"]),
        (
            "/time of minecraft:overworld query ",
            &[
                "minecraft:day",
                "minecraft:early_game",
                "minecraft:moon",
                "minecraft:villager_schedule",
                "time",
            ],
        ),
        ("/time of minecraft:the_end set ", &[]),
    ];
    for (text, expected) in suggestions {
        assert_eq!(suggest(&mut server, op, text), expected, "{text}");
    }
}

fn seen_by(
    packets: &[mcrs_minecraft_engine::bus::OutboundPlayerPacket],
    anchor: Entity,
) -> Vec<PacketPayload> {
    packets
        .iter()
        .filter(|packet| match &packet.target {
            PacketTarget::SinglePlayer(target) => *target == anchor,
            PacketTarget::PlayerSet(set) => set.contains(&anchor),
            PacketTarget::AllPlayers => true,
            _ => false,
        })
        .map(|packet| packet.data.clone())
        .collect()
}

#[test]
fn a_clock_change_in_one_dimension_reaches_every_player() {
    let mut server = start_in(
        &[
            ("minecraft:the_nether", "minecraft:the_nether"),
            ("minecraft:overworld", "minecraft:overworld"),
        ],
        Limits::default(),
    );
    let (op, _) = server.join(OPERATOR, "Op");
    let overworld = host_app::label_of(&server.app, "minecraft:overworld");
    let (guest, _) = server.join_in(overworld, GUEST, "Guest");

    assert_eq!(
        chats(&command(&mut server, op, "time set day")),
        [red(said(
            "commands.time.no_default_clock",
            vec![name("minecraft:the_nether")]
        ))]
    );

    let before = server.app.world().resource::<GameTime>().0;
    let packets = command(&mut server, op, "time query gametime");
    let after = server.app.world().resource::<GameTime>().0;
    let [message] = chats(&packets).try_into().unwrap();
    let TextContent::Translate {
        translate, with, ..
    } = &message.content
    else {
        panic!("{message:?}");
    };
    assert_eq!(translate, "commands.time.query.gametime");
    let [TranslateArg::Number(NbtTag::Long(game_time))] = with.as_slice() else {
        panic!("{with:?}");
    };
    assert!((before..=after).contains(game_time), "{game_time}");

    let total = clock(&server, OVERWORLD_CLOCK).total_ticks;
    let cases = [
        (
            "execute in minecraft:overworld run time add 10",
            said(
                "commands.time.set.absolute",
                vec![name(OVERWORLD_CLOCK), (total + 10).into()],
            ),
            total + 10,
        ),
        (
            "time of minecraft:overworld set night",
            said(
                "commands.time.set.time_marker",
                vec![name(OVERWORLD_CLOCK), name("minecraft:night")],
            ),
            13_000,
        ),
    ];
    for (line, message, total_ticks) in cases {
        server.queue(
            op,
            &ServerboundChatCommand {
                command: Bounded(line),
            },
        );
        let packets = server.ticks_all(3);
        assert_eq!(chats(&seen_by(&packets, op)), [message], "{line}");
        assert_eq!(clock(&server, OVERWORLD_CLOCK).total_ticks, total_ticks);
        for player in [op, guest] {
            assert_eq!(
                without_game_time(set_times(&seen_by(&packets, player))),
                [overworld_update(&server, total_ticks)],
                "{line}"
            );
        }
    }
    assert_eq!(
        chats(&command(
            &mut server,
            op,
            "time of minecraft:overworld query time"
        )),
        [said(
            "commands.time.query.absolute",
            vec![name(OVERWORLD_CLOCK), 13_000i64.into()]
        )]
    );
}

#[test]
fn a_game_mode_set_before_a_change_of_dimension_in_the_same_pass_reaches_every_player_list() {
    let mut server = start_in(
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_end", "minecraft:the_end"),
        ],
        Limits::default(),
    );
    let (op, _) = server.join(OPERATOR, "Op");
    let (guest, _) = server.join(GUEST, "Guest");
    server.queue(
        op,
        &ServerboundChatCommand {
            command: Bounded("gamemode spectator"),
        },
    );
    server.queue(
        op,
        &ServerboundChatCommand {
            command: Bounded("execute in minecraft:the_end run tp @s 0 100 0"),
        },
    );

    let seen = server.ticks(guest, 3);

    let modes: Vec<GameMode> = seen
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::PlayerInfoUpdate { entries } => Some(entries),
            _ => None,
        })
        .flatten()
        .filter(|entry| entry.player_uuid == OPERATOR)
        .map(|entry| entry.game_mode)
        .collect();
    assert_eq!(modes, [GameMode::Spectator]);
}

#[test]
fn execute_in_moves_the_source_player_to_another_dimension() {
    use mcrs_minecraft_protocol::{PositionFlag as F, PositionFlags};
    let mut server = start_in(
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_end", "minecraft:the_end"),
        ],
        Limits::default(),
    );
    let (op, _) = server.join(OPERATOR, "Op");
    let overworld = server.dim;
    let end = host_app::label_of(&server.app, "minecraft:the_end");

    let packets = command(
        &mut server,
        op,
        "execute in minecraft:the_end run tp @s 0 100 0",
    );

    assert!(server.player_in(overworld, op).is_none());
    assert_eq!(
        transform_in(&mut server, end, op).translation,
        DVec3::new(0.5, 100.0, 0.5)
    );
    let flags: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::PlayerPosition(position) => Some(position.flags),
            _ => None,
        })
        .collect();
    assert_eq!(
        flags,
        [PositionFlags::from_iter([F::XRot, F::YRot])],
        "the position is absolute"
    );
    assert_eq!(
        chats(&packets),
        [translated(
            "commands.teleport.success.location.single",
            &["Op", "0.500000", "100.000000", "0.500000"]
        )]
    );
    assert_eq!(
        chats(&command(
            &mut server,
            op,
            "execute in minecraft:the_end if entity @a[distance=..5]"
        )),
        [translated(
            "commands.execute.conditional.pass_count",
            &["1"]
        )],
        "the next command finds the player where it went"
    );
}

fn anchor_of(server: &mut PlayServer, uuid: Uuid) -> Entity {
    let world = server.app.world_mut();
    world
        .query::<(Entity, &mcrs_minecraft_engine::session::GameProfile)>()
        .iter(world)
        .find(|(_, profile)| profile.id == uuid)
        .map(|(anchor, _)| anchor)
        .unwrap()
}

fn mode_in(server: &mut PlayServer, dim: Entity, anchor: Entity) -> GameMode {
    use mcrs_minecraft_engine::ability::PlayerGameMode;
    let player = server.player_in(dim, anchor).unwrap();
    server
        .world_in(dim)
        .get::<PlayerGameMode>(player)
        .unwrap()
        .0
}

fn told(packets: &[mcrs_minecraft_engine::bus::OutboundPlayerPacket], anchor: Entity) -> Vec<Text> {
    packets
        .iter()
        .filter(|packet| matches!(packet.target, PacketTarget::SinglePlayer(target) if target == anchor))
        .filter_map(|packet| match &packet.data {
            PacketPayload::SystemChat(chat) => Some(chat.content.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn gamemode_changes_players_of_every_dimension_unless_the_selector_is_world_limited() {
    let (mut server, op, nether_player) = two_dimensions(Limits::default());
    let nether = host_app::label_of(&server.app, "minecraft:the_nether");
    let overworld = server.dim;
    let guest = anchor_of(&mut server, GUEST);
    let mode = || Text::translate("gameMode.adventure", vec![]);

    command(&mut server, op, "gamemode survival @a[distance=..5]");
    assert_eq!(
        mode_in(&mut server, nether, nether_player),
        GameMode::Creative
    );
    assert_eq!(mode_in(&mut server, overworld, guest), GameMode::Survival);

    server.queue(
        op,
        &ServerboundChatCommand {
            command: Bounded("gamemode adventure @a"),
        },
    );
    let packets = server.ticks_all(3);
    assert_eq!(
        mode_in(&mut server, nether, nether_player),
        GameMode::Adventure
    );
    assert_eq!(
        told(&packets, op),
        [
            Text::translate("commands.gamemode.success.self", vec![mode()]),
            Text::translate(
                "commands.gamemode.success.other",
                vec!["Guest".into_text(), mode()]
            ),
            Text::translate(
                "commands.gamemode.success.other",
                vec!["Nether".into_text(), mode()]
            ),
        ]
    );
    assert_eq!(
        told(&packets, nether_player),
        [Text::translate("gameMode.changed", vec![mode()])]
    );
    assert!(
        packets.iter().any(|packet| matches!(packet.target, PacketTarget::SinglePlayer(target) if target == nether_player)
            && matches!(packet.data, PacketPayload::GameEvent(_))),
        "the player in the Nether hears of its new mode from its own dimension"
    );
}

#[test]
fn a_target_that_an_earlier_stage_changed_is_not_changed_again() {
    let (mut server, op, nether_player) = two_dimensions(Limits::default());
    let nether = host_app::label_of(&server.app, "minecraft:the_nether");
    command(&mut server, op, "gamemode survival @a");
    server.ticks_all(3);
    server.queue(
        op,
        &ServerboundChatCommand {
            command: Bounded("execute as @a run gamemode creative @a"),
        },
    );
    let packets = server.ticks_all(3);
    let mode = || Text::translate("gameMode.creative", vec![]);
    assert_eq!(
        told(&packets, op),
        [
            Text::translate("commands.gamemode.success.self", vec![mode()]),
            Text::translate(
                "commands.gamemode.success.other",
                vec!["Guest".into_text(), mode()]
            ),
            Text::translate(
                "commands.gamemode.success.other",
                vec!["Nether".into_text(), mode()]
            ),
        ]
    );
    assert_eq!(
        told(&packets, nether_player),
        [Text::translate("gameMode.changed", vec![mode()])]
    );
    assert_eq!(
        mode_in(&mut server, nether, nether_player),
        GameMode::Creative
    );
}

#[test]
fn nearest_compares_coordinates_across_dimensions_and_later_commands_see_the_moves() {
    let (mut server, op, nether_player) = two_dimensions(Limits::default());
    let nether = host_app::label_of(&server.app, "minecraft:the_nether");
    let overworld = server.dim;
    let guest = anchor_of(&mut server, GUEST);
    command(&mut server, op, "tp Guest 13.5 64 8.5");
    command(
        &mut server,
        op,
        "execute in minecraft:the_nether run tp Nether 9.5 64 8.5",
    );
    assert_eq!(
        transform_in(&mut server, nether, nether_player).translation,
        DVec3::new(9.5, 64.0, 8.5)
    );

    command(&mut server, op, "tp @p[name=!Op] ~ ~ ~");
    assert!(server.player_in(nether, nether_player).is_none());
    assert_eq!(
        transform_in(&mut server, overworld, nether_player).translation,
        DVec3::new(8.5, 64.0, 8.5),
        "the player in the Nether at distance 1 is nearer than the guest at distance 5"
    );
    assert_eq!(
        transform_in(&mut server, overworld, guest).translation,
        DVec3::new(13.5, 64.0, 8.5)
    );

    command(
        &mut server,
        op,
        "execute in minecraft:the_nether run tp Guest 0 64 0",
    );
    let moved = chats(&command(&mut server, op, "tp @a 0 64 0"));
    assert_eq!(
        moved,
        [translated(
            "commands.teleport.success.location.multiple",
            &["3", "0.500000", "64.000000", "0.500000"]
        )]
    );
    assert!(server.player_in(nether, guest).is_none());
    assert_eq!(
        chats(&command(
            &mut server,
            op,
            "execute if entity @a[distance=..2]"
        )),
        [translated(
            "commands.execute.conditional.pass_count",
            &["3"]
        )]
    );
}

#[derive(Clone, Copy)]
struct Mark(Entity);

#[derive(Resource, Default)]
struct Marks(Vec<(usize, DVec3)>);

fn mark(worlds: &mut Worlds, Mark(player): Mark) {
    let Some(entry) = worlds.targets().player(player) else {
        return;
    };
    let (dimension, entity) = (entry.dimension, entry.entity);
    let Some(&held) = worlds
        .world_mut(dimension)
        .and_then(|world| world.get::<Transform>(entity))
    else {
        return;
    };
    let (host, _) = worlds.host_and_worlds();
    host.resource_mut::<Marks>()
        .0
        .push((dimension, held.translation));
}

fn bounce(
    In(call): In<Call<(), ()>>,
    context: VanillaContext,
    mut output: CommandOutput,
    mut teleports: ResMut<EffectQueue<TeleportPlayer>>,
    mut marks: ResMut<EffectQueue<Mark>>,
) -> CommandResult {
    let player = call.invocation.source.entity.unwrap();
    let labels: Vec<Entity> = context
        .targets()
        .dimensions()
        .iter()
        .map(|d| d.label)
        .collect();
    for (label, x) in [(labels[1], 3.5), (labels[0], 7.5)] {
        let to = Teleport {
            dimension: label,
            position: DVec3::new(x, 64.0, 0.5),
            rotation: None,
            relative_position: BVec3::FALSE,
            relative_rotation: RelativeRotation {
                yaw: true,
                pitch: true,
            },
            facing: None,
            anchor: LookAnchor::Feet,
        };
        output.effect(&mut teleports, TeleportPlayer { player, to });
        output.effect(&mut marks, Mark(player));
    }
    Ok(1)
}

#[test]
fn the_effects_of_one_run_apply_in_the_order_of_writing_across_effect_types() {
    let mut app = host_app::make_host_app();
    app.add_plugins(WorldClockPlugin);
    app.insert_resource(AdvanceTime(false));
    app.add_plugins(WorldClockHostPlugin);
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_observer(broadcast_to_operators);
    app.add_plugins(VanillaCommandsPlugin);
    app.init_resource::<Marks>();
    app.add_command_effect(mark);
    app.add_command(
        |r: &mut CommandRegistrar<'_, VanillaCommands, ()>| {
            r.literal(NodeId::ROOT, "bounce").runs(()).id()?;
            Ok(())
        },
        bounce,
    )
    .unwrap();
    let mut server = PlayServer::start_in(
        app,
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_nether", "minecraft:the_nether"),
        ],
        &[(OPERATOR, 2)],
        0,
    );
    let (op, _) = server.join(OPERATOR, "Op");
    command(&mut server, op, "bounce");
    assert_eq!(
        server.app.world().resource::<Marks>().0,
        [
            (1, DVec3::new(3.5, 64.0, 0.5)),
            (0, DVec3::new(7.5, 64.0, 0.5))
        ]
    );
    let overworld = server.dim;
    assert_eq!(
        transform_in(&mut server, overworld, op).translation,
        DVec3::new(7.5, 64.0, 0.5)
    );
}

/// The tree the operator receives, through the wire and into the client tree.
fn client_tree_of(
    packets: &[PacketPayload],
) -> mcrs_minecraft_command_argument_vanilla::ClientGraph {
    let decoded = sent_tree(packets);
    let graph =
        mcrs_minecraft_command_argument_vanilla::client_tree(&decoded.nodes, decoded.root, &[])
            .unwrap();
    assert_eq!(graph.node_count(), decoded.nodes.len());
    graph
}

#[test]
fn the_client_parses_the_tree_it_receives_and_shows_the_usage_of_the_caret() {
    use mcrs_minecraft_chat_client::{ChatAbilities, CommandInput, Context, Hint};
    use mcrs_minecraft_command::{NoLookup, Source};
    let mut server = start();
    let (_, packets) = server.join(OPERATOR, "Op");
    let tree = client_tree_of(&packets);
    let source = Source::new(CommandLevel::Gamemasters, &NoLookup);
    let cx = Context {
        tree: &tree,
        source: &source,
        abilities: ChatAbilities::default(),
        auto_suggest: true,
        completions: &[],
    };
    let mut input = CommandInput::default();
    let mut lines = |line: &str| -> Vec<String> {
        assert_eq!(input.refresh(line, line.len(), &cx), None, "{line}");
        input
            .hints()
            .iter()
            .map(|hint| match hint {
                Hint::Usage(range) => input.usage_text(range.clone()).to_owned(),
                other => format!("{other:?}"),
            })
            .collect()
    };
    assert_eq!(
        lines("/execute in minecraft:the_end run tp "),
        [
            "<location>",
            "<destination>",
            "<targets> (<location>|<destination>)",
        ],
        "after `run` the parse continues at the root"
    );
    assert_eq!(lines("/gamemode creative "), ["[<target>]"]);
    let line = "/execute in minecraft:the_end run tp @s ";
    assert_eq!(input.refresh(line, line.len(), &cx), None);
    let [Hint::Error(error)] = input.hints() else {
        panic!("{:?}", input.hints());
    };
    assert_eq!(
        (error.kind, error.position),
        (ErrorKind::Library(LibraryError::UnknownArgument), 39),
        "a separator with nothing after it is trailing data, and the destination has no child"
    );
    let highlights: Vec<_> = input
        .highlights()
        .iter()
        .map(|&(span, _)| span.start..span.end)
        .collect();
    assert_eq!(
        highlights,
        [37..39, 39..40],
        "only the argument after `run` has a colour"
    );
}

#[test]
fn an_observer_that_cancels_a_command_reads_the_targets_of_the_pass() {
    fn cancel_for_known_players(
        mut event: bevy_ecs::prelude::On<CommandEvent>,
        targets: bevy_ecs::prelude::Res<CommandTargets>,
    ) {
        let event = event.event_mut();
        event.cancelled = targets
            .players()
            .iter()
            .any(|player| player.entity == event.player());
    }
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let dim = server.dim;
    server.world_in(dim).add_observer(cancel_for_known_players);

    assert_eq!(
        chats(&command(&mut server, op, "echo 3")),
        Vec::<Text>::new()
    );
}

fn teleport_ids(packets: &[PacketPayload]) -> Vec<i32> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::PlayerPosition(position) => Some(position.teleport_id.0),
            _ => None,
        })
        .collect()
}

fn accept_teleports(server: &mut PlayServer, anchor: Entity, ids: &[i32]) {
    use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundAcceptTeleportation;
    for &id in ids {
        server.queue(
            anchor,
            &ServerboundAcceptTeleportation {
                teleport_id: VarInt(id),
                position: mcrs_minecraft_protocol::Position::default(),
                look: mcrs_minecraft_core::Look::default(),
            },
        );
    }
    server.ticks(anchor, 2);
}

#[test]
fn a_client_move_after_it_accepts_each_teleport_moves_the_player() {
    use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundMovePlayerPos;
    let mut server = start_in(
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_end", "minecraft:the_end"),
        ],
        Limits::default(),
    );
    let (op, joined) = server.join(OPERATOR, "Op");
    accept_teleports(&mut server, op, &teleport_ids(&joined));
    let overworld = server.dim;
    let end = host_app::label_of(&server.app, "minecraft:the_end");
    let cases = [
        ("tp @s 2 64 2", overworld, DVec3::new(3.0, 64.0, 2.0)),
        (
            "execute in minecraft:the_end run tp @s 0 100 0",
            end,
            DVec3::new(1.0, 100.0, 0.0),
        ),
    ];
    for (line, dim, step) in cases {
        let ids = teleport_ids(&command(&mut server, op, line));
        assert_eq!(ids.len(), 1, "{line}: one position per move");
        accept_teleports(&mut server, op, &ids);
        let packets = server.send(
            op,
            &ServerboundMovePlayerPos {
                position: step.into(),
                flags: mcrs_minecraft_protocol::MoveFlags::new().with_on_ground(true),
            },
        );
        assert_eq!(
            transform_in(&mut server, dim, op).translation,
            step,
            "{line}"
        );
        assert!(
            teleport_ids(&packets).is_empty(),
            "{line}: the server does not echo a client move"
        );
    }
}

#[test]
fn teleport_to_an_entity_takes_its_dimension_position_and_rotation() {
    use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerLookAt;
    let mut server = start_in(
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_nether", "minecraft:the_nether"),
        ],
        Limits::default(),
    );
    let (op, _) = server.join(OPERATOR, "Op");
    let (_, joined) = server.join(GUEST, "Guest");
    let guest_id = joined
        .iter()
        .find_map(|packet| match packet {
            PacketPayload::PlayerLogin(login) => Some(login.player_id),
            _ => None,
        })
        .unwrap();
    let overworld = server.dim;
    let nether = host_app::label_of(&server.app, "minecraft:the_nether");
    server.join_in(nether, THIRD, "Nether");
    command(&mut server, op, "tp Guest 13.5 64 8.5 -30 5");
    command(
        &mut server,
        op,
        "execute in minecraft:the_nether run tp Nether 9.5 64 8.5 90 10",
    );

    let cases = [
        (
            "tp @s Guest",
            overworld,
            (13.5, 64.0, 8.5),
            (-30.0, 5.0),
            "Guest",
        ),
        (
            "tp @s Nether",
            nether,
            (9.5, 64.0, 8.5),
            (90.0, 10.0),
            "Nether",
        ),
    ];
    for (line, dim, (x, y, z), (yaw, pitch), destination) in cases {
        assert_eq!(
            chats(&command(&mut server, op, line)),
            [translated(
                "commands.teleport.success.entity.single",
                &["Op", destination]
            )],
            "{line}"
        );
        let at = transform_in(&mut server, dim, op);
        assert_eq!(
            (at.translation, at.rotation.yaw(), at.rotation.pitch()),
            (DVec3::new(x, y, z), yaw, pitch),
            "{line}"
        );
    }

    command(
        &mut server,
        op,
        "execute in minecraft:overworld run tp @s 0.5 64 0.5",
    );
    let packets = command(&mut server, op, "tp @s 0.5 64 0.5 facing entity Guest");
    let looks: Vec<_> = packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::PlayerLookAt(look_at) => Some(look_at),
            _ => None,
        })
        .collect();
    assert_eq!(
        looks,
        [&ClientboundPlayerLookAt {
            from: LookAnchor::Feet,
            target: DVec3::new(13.5, 64.0, 8.5),
            entity: Some(LookAtEntity {
                id: VarInt(guest_id),
                anchor: LookAnchor::Feet,
            }),
        }]
    );
    let rotation = transform(&mut server, op).rotation;
    let yaw = 8.0f32.atan2(13.0).to_degrees() - 90.0;
    assert!(
        (rotation.yaw() - yaw).abs() < 1e-3 && rotation.pitch().abs() < 1e-3,
        "{rotation:?}"
    );
}

#[test]
fn locate_reports_what_it_cannot_find_and_values_that_do_not_exist() {
    let mut server = start();
    let (op, _) = server.join(OPERATOR, "Op");
    let text = |key: &'static str, value: &str| red(translated(key, &[value]));
    let biome = "minecraft:worldgen/biome";
    let cases = [
        (
            "locate structure minecraft:nope",
            text("commands.locate.structure.invalid", "minecraft:nope"),
        ),
        (
            "locate structure #minecraft:nope",
            text("commands.locate.structure.invalid", "#minecraft:nope"),
        ),
        (
            "locate structure minecraft:village_plains",
            text(
                "commands.locate.structure.not_found",
                "minecraft:village_plains",
            ),
        ),
        (
            "locate biome minecraft:nope",
            red(said(
                "argument.resource.not_found",
                vec![name("minecraft:nope"), name(biome)],
            )),
        ),
        (
            "locate biome #minecraft:nope",
            red(said(
                "argument.resource_tag.not_found",
                vec![name("minecraft:nope"), name(biome)],
            )),
        ),
        (
            "locate poi minecraft:home",
            text("commands.locate.poi.not_found", "minecraft:home"),
        ),
    ];
    for (line, expected) in cases {
        assert_eq!(
            chats(&command(&mut server, op, line)).first(),
            Some(&expected),
            "{line}"
        );
    }
}
