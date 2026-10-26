use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{In, Res, Resource, With};
use bevy_math::DVec3;
use bytes::Bytes;
use mcrs_command::{Arg, BuildError, IntegerArgument};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_engine::bus::{
    InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerPacket, PacketPayload, PacketTarget,
    PlayerTransferSnapshot,
};
use mcrs_minecraft_engine::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_engine::dim::{DimSubAppHandle, pump_channels};
use mcrs_minecraft_engine::session::SessionBundle;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{Place, PlayerSession, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::metrics::PacketDispatcher;
use mcrs_minecraft_protocol::command::CommandNodeKind;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundChatCommand, ServerboundCommandSuggestion,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Encode, GameMode, Packet, VarInt};
use mcrs_minecraft_server::command::vanilla::VanillaCommandsPlugin;
use mcrs_minecraft_server::command::{
    AppCommands, CommandFeedback, CommandPlugin, CommandRegistrar, CommandResult, Invocation,
};
use mcrs_minecraft_server::ops::{DefaultOpLevel, OpList, OpListEntry};
use mcrs_minecraft_server::world::entity::player::HostAnchor;
use mcrs_minecraft_server::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};

use crate::host_app;

#[derive(Resource, Clone, Copy)]
struct Echo {
    times: Arg<IntegerArgument>,
}

fn echo(r: &mut CommandRegistrar<'_>) -> Result<Echo, BuildError> {
    let echo = r.literal(
        CommandRegistrar::ROOT,
        "echo",
        PermissionCheck::Require(CommandLevel::Gamemasters),
    )?;
    let times = r.argument(
        echo,
        "times",
        IntegerArgument {
            min: 1,
            ..IntegerArgument::ANY
        },
        PermissionCheck::AlwaysPass,
    )?;
    r.executes(times.node())?;
    Ok(Echo { times })
}

fn run_echo(
    In(invocation): In<Invocation>,
    command: Res<Echo>,
    context: mcrs_minecraft_server::command::CommandContext,
    mut feedback: CommandFeedback,
) -> CommandResult {
    let lookup = context.lookup();
    let source = invocation.source(&lookup);
    let times = invocation
        .get(&context.tree, command.times, &source)
        .unwrap()?;
    feedback.success(&invocation, format!("echo {times}").into_text(), false);
    Ok(times)
}

struct Server {
    app: App,
    dim: Entity,
}

const OPERATOR: Uuid = Uuid::from_u128(1);
const GUEST: Uuid = Uuid::from_u128(2);

impl Server {
    fn start() -> Self {
        let mut app = host_app::make_host_app();
        app.add_plugins(CommandPlugin);
        app.add_plugins(VanillaCommandsPlugin);
        app.add_command(echo, run_echo);
        app.finish();
        app.init_resource::<PlayerSessionCounter>();
        app.init_resource::<mcrs_minecraft_level::world::in_flight::InFlightMoves>();
        app.add_message::<InboundPlayerSpawn>();
        app.insert_resource(OpList::new([OpListEntry {
            uuid: OPERATOR,
            name: "op".into(),
            level: 2,
            bypasses_player_limit: false,
        }]));
        app.insert_resource(DefaultOpLevel(PlayerOpLevel(0)));
        host_app::drive_to_playing(&mut app);
        host_app::materialise_sub_apps(&mut app, &[("minecraft:overworld", "minecraft:overworld")]);
        let dim = app
            .world_mut()
            .query_filtered::<Entity, With<DimSubAppHandle>>()
            .single(app.world())
            .unwrap();
        Self { app, dim }
    }

    fn ticks(&mut self, anchor: Entity, n: usize) -> Vec<PacketPayload> {
        let mut packets = Vec::new();
        for _ in 0..n {
            self.app.update();
            pump_channels(&mut self.app);
            packets.extend(
                self.app
                    .world_mut()
                    .resource_mut::<Messages<OutboundPlayerPacket>>()
                    .drain()
                    .filter(|packet| {
                        matches!(packet.target, PacketTarget::SinglePlayer(target) if target == anchor)
                    })
                    .map(|packet| packet.data),
            );
        }
        packets
    }

    fn submit(&self, message: ToDim) {
        let channels = self
            .app
            .world()
            .resource::<DimChannelsResource>()
            .get(self.dim)
            .unwrap();
        let sender = match message {
            ToDim::Serverbound(..) => &channels.serverbound_sender,
            _ => &channels.control_sender,
        };
        sender.try_send(message).unwrap();
    }

    fn join(&mut self, uuid: Uuid, name: &str) -> (Entity, Vec<PacketPayload>) {
        let session = self
            .app
            .world_mut()
            .resource_mut::<PlayerSessionCounter>()
            .next();
        let anchor = self
            .app
            .world_mut()
            .spawn(SessionBundle::placed(
                session,
                SessionPlacement::new(Place::Joining(self.dim), 0),
            ))
            .id();
        self.submit(ToDim::Spawn(InboundPlayerSpawn {
            host_anchor: anchor,
            session: PlayerSession(0),
            snapshot: PlayerTransferSnapshot {
                uuid,
                username: name.into(),
                position: DVec3::new(8.5, 64.0, 8.5),
                rotation: bevy_math::Vec2::ZERO,
                view_distance: 2,
            },
            dimensions: Vec::new().into(),
        }));
        let packets = self.ticks(anchor, 3);
        (anchor, packets)
    }

    fn queue<P: Packet + Encode>(&self, anchor: Entity, packet: &P) {
        let mut data = Vec::new();
        packet.encode(&mut data).unwrap();
        self.submit(ToDim::Serverbound(InboundPlayerPacket {
            player: anchor,
            id: P::ID,
            trigger: PacketDispatcher::serverbound()
                .dispatch(ConnectionState::Game, P::ID, &data)
                .unwrap(),
            data: Bytes::from(data),
            timestamp: std::time::Instant::now(),
        }));
    }

    fn send<P: Packet + Encode>(&mut self, anchor: Entity, packet: &P) -> Vec<PacketPayload> {
        self.queue(anchor, packet);
        self.ticks(anchor, 2)
    }

    fn command(&mut self, anchor: Entity, line: &str) -> Vec<Text> {
        self.send(
            anchor,
            &ServerboundChatCommand {
                command: Bounded(line),
            },
        )
        .into_iter()
        .filter_map(|packet| match packet {
            PacketPayload::SystemChat(chat) => Some(chat.content),
            _ => None,
        })
        .collect()
    }

    fn suggest(&mut self, anchor: Entity, text: &str) -> Vec<String> {
        self.send(
            anchor,
            &ServerboundCommandSuggestion {
                id: VarInt(3),
                text: Bounded(text),
            },
        )
        .into_iter()
        .find_map(|packet| match packet {
            PacketPayload::CommandSuggestions(reply) => Some(
                reply
                    .suggestions
                    .into_iter()
                    .map(|suggestion| suggestion.text.into_owned())
                    .collect(),
            ),
            _ => None,
        })
        .expect("every request gets a reply")
    }

    fn game_mode(&mut self, anchor: Entity) -> GameMode {
        let world = self
            .app
            .sub_app_mut(mcrs_minecraft_level::world::sub_app::DimAppLabel(self.dim))
            .world_mut();
        world
            .query_filtered::<(&HostAnchor, &PlayerGameMode), With<Player>>()
            .iter(world)
            .find(|(host, _)| host.0 == anchor)
            .map(|(_, mode)| mode.0)
            .unwrap()
    }
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

fn plain(messages: &[Text]) -> Vec<String> {
    messages.iter().map(Text::to_legacy_lossy).collect()
}

#[test]
fn an_operator_sees_completes_and_runs_a_plugin_command_that_a_guest_cannot() {
    let mut server = Server::start();
    let (op, packets) = server.join(OPERATOR, "Op");
    assert_eq!(
        root_literals(&packets),
        ["gamemode", "teleport", "tp", "locate", "echo"]
    );
    let (guest, packets) = server.join(GUEST, "Guest");
    assert_eq!(root_literals(&packets), Vec::<String>::new());

    assert_eq!(server.suggest(op, "/ec"), ["echo"]);
    assert!(server.suggest(guest, "/ec").is_empty());

    assert_eq!(plain(&server.command(op, "echo 3")), ["echo 3"]);
    let refused = server.command(guest, "echo 3");
    assert_eq!(refused.len(), 2, "{refused:?}");
    assert_eq!(
        refused[0],
        mcrs_minecraft_protocol::text::IntoText::color(
            Text::translate("command.unknown.command", vec![]),
            mcrs_minecraft_protocol::text::Color::RED
        )
    );
    let too_low = plain(&server.command(op, "echo 0"));
    assert_eq!(too_low.len(), 2, "{too_low:?}");
}

#[test]
fn gamemode_changes_the_targets_it_selects() {
    let mut server = Server::start();
    let (op, _) = server.join(OPERATOR, "Op");
    let (guest, _) = server.join(GUEST, "Guest");
    let before = server.game_mode(guest);
    let other = if before == GameMode::Spectator {
        GameMode::Survival
    } else {
        GameMode::Spectator
    };
    let name = match other {
        GameMode::Survival => "survival",
        _ => "spectator",
    };
    server.command(op, &format!("gamemode {name} Guest"));
    assert_eq!(server.game_mode(guest), other);
    server.command(guest, "gamemode creative");
    assert_eq!(
        server.game_mode(guest),
        other,
        "a guest cannot run gamemode"
    );
    server.command(op, &format!("gamemode {name} @a"));
    assert_eq!(server.game_mode(op), other);
}

#[test]
fn teleport_sends_the_relative_axes_and_reports_the_resolved_position() {
    use mcrs_minecraft_protocol::PositionFlag as F;
    let mut server = Server::start();
    let (op, _) = server.join(OPERATOR, "Op");
    let packets = server.send(
        op,
        &ServerboundChatCommand {
            command: Bounded("tp ~1 ~ 3"),
        },
    );
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
    let message = packets.iter().find_map(|packet| match packet {
        PacketPayload::SystemChat(chat) => Some(chat.content.clone()),
        _ => None,
    });
    assert_eq!(
        message,
        Some(Text::translate(
            "commands.teleport.success.location.single",
            vec![
                "Op".into_text(),
                "9.500000".into_text(),
                "64.000000".into_text(),
                "3.500000".into_text()
            ]
        ))
    );
}

fn suggestion_replies(packets: &[PacketPayload]) -> Vec<i32> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::CommandSuggestions(reply) => Some(reply.id.0),
            _ => None,
        })
        .collect()
}

#[test]
fn a_tick_answers_only_the_latest_request_of_a_player() {
    let mut server = Server::start();
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
    assert_eq!(suggestion_replies(&packets), [3]);
    let reply = packets.iter().find_map(|packet| match packet {
        PacketPayload::CommandSuggestions(reply) => Some(reply),
        _ => None,
    });
    assert_eq!(reply.unwrap().suggestions[0].text, "echo");
}

#[test]
fn a_long_request_from_a_player_who_cannot_edit_command_blocks_gets_no_reply() {
    let mut server = Server::start();
    let (guest, _) = server.join(GUEST, "Guest");
    let text = format!("/{}", "a".repeat(32_499));
    let packets = server.send(
        guest,
        &ServerboundCommandSuggestion {
            id: VarInt(1),
            text: Bounded(&text),
        },
    );
    assert!(suggestion_replies(&packets).is_empty());
}
