use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use bytes::Bytes;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::session::{Place, PlayerSession, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::DimAppLabel;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundEntityEvent;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundGameEvent;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundChangeGameMode;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Encode, GameEventKind, GameMode, Packet};
use mcrs_minecraft_server::dim::pump_channels;
use mcrs_minecraft_server::ops::{DefaultOpLevel, OpList, OpListEntry};
use mcrs_minecraft_server::world::bus::InboundPlayerPacket;
use mcrs_minecraft_server::world::bus::{
    InboundPlayerSpawn, OutboundPlayerPacket, PacketPayload, PacketTarget, PlayerTransferSnapshot,
};
use mcrs_minecraft_server::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_server::world::entity::player::HostAnchor;
use mcrs_minecraft_server::world::entity::player::ability::{PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_server::world::session::SessionBundle;
use mcrs_minecraft_server::world::sub_app_builder::DimSubAppHandle;

use crate::host_app;

struct Server {
    app: App,
    dim: Entity,
    host_anchor: Entity,
}

impl Server {
    fn start(ops: &[(Uuid, u8)], default_level: u8) -> Self {
        let mut app = host_app::make_host_app();
        app.init_resource::<PlayerSessionCounter>();
        app.init_resource::<mcrs_minecraft_level::world::in_flight::InFlightMoves>();
        app.add_message::<InboundPlayerSpawn>();
        app.insert_resource(OpList::new(ops.iter().map(|&(uuid, level)| OpListEntry {
            uuid,
            name: "op".into(),
            level,
            bypasses_player_limit: false,
        })));
        app.insert_resource(DefaultOpLevel(PlayerOpLevel(default_level)));
        host_app::drive_to_playing(&mut app);
        host_app::materialise_sub_apps(&mut app, &[("test:overworld", true)]);
        let dim = app
            .world_mut()
            .query_filtered::<Entity, With<DimSubAppHandle>>()
            .single(app.world())
            .unwrap();
        let host_anchor = app.world_mut().spawn_empty().id();
        Self {
            app,
            dim,
            host_anchor,
        }
    }

    fn world(&mut self) -> &mut World {
        self.app.sub_app_mut(DimAppLabel(self.dim)).world_mut()
    }

    fn ticks(&mut self, n: usize) -> Vec<OutboundPlayerPacket> {
        let anchor = self.host_anchor;
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
                    }),
            );
        }
        packets
    }

    fn join(&mut self, uuid: Uuid) -> Vec<OutboundPlayerPacket> {
        let session = self
            .app
            .world_mut()
            .resource_mut::<PlayerSessionCounter>()
            .next();
        self.app
            .world_mut()
            .entity_mut(self.host_anchor)
            .insert(SessionBundle::placed(
                session,
                SessionPlacement::new(Place::Joining(self.dim), 0),
            ));
        self.submit(ToDim::Spawn(InboundPlayerSpawn {
            host_anchor: self.host_anchor,
            session: PlayerSession(0),
            snapshot: PlayerTransferSnapshot {
                uuid,
                username: "op_level".into(),
                position: DVec3::new(8.5, 64.0, 8.5),
                rotation: bevy_math::Vec2::ZERO,
                view_distance: 2,
            },
            dimensions: Vec::new(),
        }));
        self.ticks(2)
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

    fn send<P: Packet + Encode>(&self, packet: &P) {
        let mut data = Vec::new();
        packet.encode(&mut data).unwrap();
        self.submit(ToDim::Serverbound(InboundPlayerPacket {
            player: self.host_anchor,
            id: P::ID,
            data: Bytes::from(data),
            timestamp: std::time::Instant::now(),
        }));
    }

    fn player(&mut self) -> Entity {
        let anchor = self.host_anchor;
        self.world()
            .query_filtered::<(Entity, &HostAnchor), With<Player>>()
            .iter(self.world())
            .find(|(_, host)| host.0 == anchor)
            .map(|(player, _)| player)
            .expect("the player is in the dimension")
    }

    fn game_mode(&mut self) -> GameMode {
        let player = self.player();
        self.world().get::<PlayerGameMode>(player).unwrap().0
    }
}

fn op_statuses(packets: &[OutboundPlayerPacket]) -> Vec<i8> {
    packets
        .iter()
        .filter_map(|packet| match packet.data {
            PacketPayload::OpLevelEntityEvent(ClientboundEntityEvent { entity_status, .. }) => {
                Some(entity_status)
            }
            _ => None,
        })
        .collect()
}

fn game_mode_changes(packets: &[OutboundPlayerPacket]) -> Vec<GameMode> {
    packets
        .iter()
        .filter_map(|packet| match &packet.data {
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
fn listed_op_receives_its_level_after_login() {
    let uuid = Uuid::new_v4();
    let mut server = Server::start(&[(uuid, 4)], 0);
    let packets = server.join(uuid);

    assert_eq!(op_statuses(&packets), vec![28]);
    let login = packets
        .iter()
        .position(|packet| matches!(packet.data, PacketPayload::PlayerLogin(_)))
        .expect("the player logs in");
    let status = packets
        .iter()
        .position(|packet| matches!(packet.data, PacketPayload::OpLevelEntityEvent(_)))
        .unwrap();
    assert!(
        login < status,
        "the client applies the status to the player the login packet creates"
    );
}

#[test]
fn unlisted_player_receives_the_default_level() {
    let mut server = Server::start(&[(Uuid::new_v4(), 4)], 1);
    let packets = server.join(Uuid::new_v4());

    assert_eq!(op_statuses(&packets), vec![25]);
}

#[test]
fn a_changed_level_is_sent_again() {
    let mut server = Server::start(&[], 0);
    assert_eq!(op_statuses(&server.join(Uuid::new_v4())), vec![24]);
    assert!(op_statuses(&server.ticks(2)).is_empty());

    let player = server.player();
    server.world().get_mut::<PlayerOpLevel>(player).unwrap().0 = 3;

    assert_eq!(op_statuses(&server.ticks(2)), vec![27]);
}

#[test]
fn level_two_may_switch_game_mode() {
    let uuid = Uuid::new_v4();
    let mut server = Server::start(&[(uuid, 2)], 0);
    server.join(uuid);
    let target = other_mode(server.game_mode());

    server.send(&ServerboundChangeGameMode { mode: target });
    let packets = server.ticks(2);

    assert_eq!(game_mode_changes(&packets), vec![target]);
    assert_eq!(server.game_mode(), target);
}

#[test]
fn level_one_may_not_switch_game_mode() {
    let uuid = Uuid::new_v4();
    let mut server = Server::start(&[(uuid, 1)], 0);
    server.join(uuid);
    let before = server.game_mode();

    server.send(&ServerboundChangeGameMode {
        mode: other_mode(before),
    });
    let packets = server.ticks(2);

    assert!(game_mode_changes(&packets).is_empty());
    assert_eq!(server.game_mode(), before);
}
