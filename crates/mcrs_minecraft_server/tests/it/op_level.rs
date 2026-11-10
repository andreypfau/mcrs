use bevy_ecs::entity::Entity;
use mcrs_minecraft_command_engine::{CommandLimits, CommandPlugin};
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaCommandsPlugin};
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::coordinator::ops::{OpList, OpListHostPlugin};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_item::ItemHoverMap;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::GameEventKind;
use mcrs_minecraft_protocol_game::clientbound::ClientboundEntityEvent;
use mcrs_minecraft_protocol_game::clientbound::ClientboundGameEvent;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChangeGameMode;
use mcrs_minecraft_protocol_game::serverbound::ServerboundChatCommand;
use mcrs_minecraft_protocol_game::text::{Color, IntoText, Text};

use crate::host_app::{self, PlayServer, sent_to};

const ADMIN: Uuid = Uuid::from_u128(1);

fn start(ops: &[(Uuid, u8)], default_level: u8) -> PlayServer {
    PlayServer::start(host_app::make_host_app(), ops, default_level)
}

fn op_statuses(packets: &[PacketPayload]) -> Vec<i8> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::OpLevelEntityEvent(ClientboundEntityEvent { entity_status, .. }) => {
                Some(*entity_status)
            }
            _ => None,
        })
        .collect()
}

fn game_mode_changes(packets: &[PacketPayload]) -> Vec<GameMode> {
    packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::GameEvent(ClientboundGameEvent {
                game_event: GameEventKind::ChangeGameMode(mode),
            }) => Some(*mode),
            _ => None,
        })
        .collect()
}

fn other_mode(mode: GameMode) -> GameMode {
    if mode == GameMode::Survival {
        GameMode::Creative
    } else {
        GameMode::Survival
    }
}

#[test]
fn a_listed_op_receives_its_level_after_login_and_may_use_it() {
    let uuid = Uuid::new_v4();
    let mut server = start(&[(uuid, 2)], 0);
    let (anchor, packets) = server.join(uuid, "op_level");

    assert_eq!(op_statuses(&packets), vec![26]);
    let login = packets
        .iter()
        .position(|packet| matches!(packet, PacketPayload::PlayerLogin(_)))
        .expect("the player logs in");
    let status = packets
        .iter()
        .position(|packet| matches!(packet, PacketPayload::OpLevelEntityEvent(_)))
        .unwrap();
    assert!(
        login < status,
        "the client applies the status to the player the login packet creates"
    );

    let target = other_mode(server.game_mode(anchor));
    let packets = server.send(anchor, &ServerboundChangeGameMode { mode: target });
    assert_eq!(game_mode_changes(&packets), vec![target]);
    assert_eq!(server.game_mode(anchor), target);
}

#[test]
fn an_unlisted_player_receives_the_default_level_and_every_change_to_it() {
    let mut server = start(&[(Uuid::new_v4(), 4)], 1);
    let (anchor, packets) = server.join(Uuid::new_v4(), "op_level");
    assert_eq!(op_statuses(&packets), vec![25]);
    assert!(op_statuses(&server.ticks(anchor, 2)).is_empty());

    let player = server.player(anchor);
    server.world().get_mut::<PlayerOpLevel>(player).unwrap().0 = 3;

    assert_eq!(op_statuses(&server.ticks(anchor, 2)), vec![27]);
}

fn start_with_commands() -> PlayServer {
    let mut app = host_app::make_host_app();
    app.insert_resource(CommandLimits(Default::default()));
    app.add_plugins(CommandPlugin::<VanillaCommands>::new());
    app.add_plugins(VanillaCommandsPlugin);
    app.add_plugins(OpListHostPlugin);
    PlayServer::start(app, &[(ADMIN, 3)], 0)
}

struct Answer {
    source: Vec<Text>,
    target: Vec<PacketPayload>,
}

fn run(server: &mut PlayServer, source: Entity, target: Entity, line: &str) -> Answer {
    server.queue(
        source,
        &ServerboundChatCommand {
            command: Bounded(line),
        },
    );
    let packets = server.ticks_all(3);
    let session = |anchor| server.session_of(anchor).unwrap();
    let source = sent_to(session(source), packets.clone())
        .into_iter()
        .filter_map(|packet| match packet {
            PacketPayload::SystemChat(chat) => {
                Some(chat.content.to_text().unwrap().map(&ItemHoverMap).unwrap())
            }
            _ => None,
        })
        .collect();
    Answer {
        source,
        target: sent_to(session(target), packets),
    }
}

fn named(key: &'static str, name: &str) -> Text {
    Text::translate(key, vec![name.to_owned().into_text()])
}

fn failed(key: &'static str) -> Text {
    Text::translate(key, Vec::<Text>::new()).color(Color::RED)
}

#[test]
fn op_and_deop_change_the_level_of_an_online_player_at_once_and_fail_without_a_change() {
    let mut server = start_with_commands();
    let (admin, _) = server.join(ADMIN, "Admin");
    let target_id = Uuid::new_v4();
    let (target, _) = server.join(target_id, "Target");

    let op = run(&mut server, admin, target, "op target");
    assert_eq!(op.source, [named("commands.op.success", "Target")]);
    assert_eq!(op_statuses(&op.target), [28]);
    assert!(
        op.target
            .iter()
            .any(|packet| matches!(packet, PacketPayload::Commands(_))),
        "the client gets the command tree of its new level"
    );
    let host_level = |server: &mut PlayServer| {
        let ops = server.app.world().resource::<OpList>();
        ops.get(&target_id).map(|entry| entry.level)
    };
    assert_eq!(host_level(&mut server), Some(4));
    assert_eq!(
        run(&mut server, admin, target, "op Target").source,
        [failed("commands.op.failed")]
    );

    let deop = run(&mut server, admin, target, "deop Target");
    assert_eq!(deop.source, [named("commands.deop.success", "Target")]);
    assert_eq!(op_statuses(&deop.target), [24]);
    assert_eq!(host_level(&mut server), None);
    assert_eq!(
        run(&mut server, admin, target, "deop Target").source,
        [failed("commands.deop.failed")]
    );
}
