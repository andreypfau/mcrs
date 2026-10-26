use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::In;
use bevy_math::DVec3;
use mcrs_command::{Arg, BuildError, IntegerArgument, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_engine::bus::{PacketPayload, PacketTarget};
use mcrs_minecraft_protocol::command::CommandNodeKind;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, GameMode, VarInt};
use mcrs_minecraft_server::command::vanilla::VanillaCommandsPlugin;
use mcrs_minecraft_server::command::{
    AppCommands, Call, CommandFeedback, CommandPlugin, CommandRegistrar, CommandResult,
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
    let mut app = host_app::make_host_app();
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
        ["gamemode", "teleport", "tp", "locate", "echo"]
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
