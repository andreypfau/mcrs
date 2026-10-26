use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundEntityEvent;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundGameEvent;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundChangeGameMode;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{GameEventKind, GameMode};
use mcrs_minecraft_server::world::entity::player::ability::PlayerOpLevel;

use crate::host_app::{self, PlayServer};

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
