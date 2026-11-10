//! The rules of `docs/spec/teleport-confirmation.md`, driven by a scripted client.

use bevy_app::Last;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::ResMut;
use bevy_math::DVec3;
use bevy_time::TimeUpdateStrategy;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_engine::coordinator::clock::WorldClockHostPlugin;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_environment::{AdvanceTime, WorldClockPlugin};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos,
};
use mcrs_minecraft_protocol_game::{MoveFlags, PositionFlags};

use crate::host_app::{self, PlayServer};

#[derive(Resource, Default)]
struct Captured(Vec<OutboundPlayerPacket>);

fn capture(mut packets: MessageReader<OutboundPlayerPacket>, mut captured: ResMut<Captured>) {
    captured.0.extend(packets.read().cloned());
}

/// A server with one joined player, whose login teleport is pending.
struct Client {
    server: PlayServer,
    anchor: Entity,
    login: ClientboundPlayerPosition,
}

impl Client {
    fn join() -> Self {
        let mut app = host_app::make_host_app();
        app.add_plugins(WorldClockPlugin);
        app.insert_resource(AdvanceTime(false));
        app.add_plugins(WorldClockHostPlugin);
        app.insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
        let mut server = PlayServer::start(app, &[], 0);
        let dim = server.dim;
        let sub_app = host_app::sub_app(&mut server.app, dim);
        sub_app.init_resource::<Captured>();
        sub_app.add_systems(Last, capture);
        let (anchor, _) = server.join(Uuid::from_u128(1), "Client");
        let mut client = Self {
            server,
            anchor,
            login: ClientboundPlayerPosition {
                teleport_id: VarInt(0),
                position: DVec3::ZERO,
                velocity: DVec3::ZERO,
                look: Look::default(),
                flags: PositionFlags::default(),
            },
        };
        let [login] = client.positions().try_into().unwrap();
        client.login = login;
        client
    }

    /// The packets the dimension sent to the player since the last call.
    fn packets(&mut self) -> Vec<PacketPayload> {
        let session = self.server.session_of(self.anchor).unwrap();
        std::mem::take(&mut self.server.world().resource_mut::<Captured>().0)
            .into_iter()
            .filter(|packet| matches!(packet.target, PacketTarget::SinglePlayer(s) if s == session))
            .map(|packet| packet.data)
            .collect()
    }

    fn positions(&mut self) -> Vec<ClientboundPlayerPosition> {
        self.packets()
            .into_iter()
            .filter_map(|packet| match packet {
                PacketPayload::PlayerPosition(position) => Some(position),
                _ => None,
            })
            .collect()
    }

    /// The reason of a disconnect the dimension sent since the last call, as debug text.
    fn disconnect(&mut self) -> Option<String> {
        self.packets().into_iter().find_map(|packet| match packet {
            PacketPayload::Disconnect(reason) => Some(format!("{reason:?}")),
            _ => None,
        })
    }

    fn position(&mut self) -> DVec3 {
        let player = self.server.player(self.anchor);
        self.server
            .world()
            .get::<Transform>(player)
            .unwrap()
            .translation
    }

    fn accept(&mut self, id: i32, at: DVec3) {
        self.server.send(
            self.anchor,
            &ServerboundAcceptTeleportation {
                teleport_id: VarInt(id),
                position: at.into(),
                look: Look::default(),
            },
        );
    }

    fn step(&mut self, to: DVec3) {
        self.server.send(
            self.anchor,
            &ServerboundMovePlayerPos {
                position: to.into(),
                flags: MoveFlags::new().with_on_ground(true),
            },
        );
    }

    /// Sends a move and answers whether it moved the player there.
    fn step_applies(&mut self) -> bool {
        let to = self.position() + DVec3::X;
        self.step(to);
        self.position() == to
    }
}

#[test]
fn an_accept_of_another_id_confirms_nothing_and_keeps_the_client() {
    let mut client = Client::join();
    let destination = client.login.position;
    assert_eq!(client.login.teleport_id.0, 1, "TP-CONFIRM-ID-001");

    client.accept(1001, destination);
    assert!(
        !client.step_applies(),
        "TP-CONFIRM-ACC-002: an id never sent"
    );
    assert_eq!(client.position(), destination, "TP-CONFIRM-MOV-001");

    client.server.ticks(client.anchor, 21);
    client.step(destination + DVec3::X);
    let [resend] = client.positions().try_into().unwrap();
    assert_eq!(resend.teleport_id.0, 2, "TP-CONFIRM-ID-002");
    client.accept(1, destination);
    assert!(
        !client.step_applies(),
        "TP-CONFIRM-ACC-002: an id that a resend replaced"
    );

    client.accept(2, destination);
    assert!(
        client.step_applies(),
        "TP-CONFIRM-ACC-001, TP-CONFIRM-MOV-002"
    );
    assert_eq!(client.disconnect(), None, "TP-CONFIRM-ACC-002");
}

#[test]
fn without_a_move_nothing_is_resent_and_the_client_stays() {
    let mut client = Client::join();
    client.server.ticks(client.anchor, 100);
    assert!(client.positions().is_empty(), "TP-CONFIRM-RES-001");
    assert!(
        client
            .server
            .player_in(client.server.dim, client.anchor)
            .is_some(),
        "TP-CONFIRM-RES-005"
    );
}

#[test]
fn a_move_resends_the_pending_teleport_each_21_ticks() {
    let mut client = Client::join();
    let destination = client.login.position;
    let away = destination + DVec3::new(3.0, 0.0, 0.0);
    let mut resent = Vec::new();
    for tick in 0..80 {
        client.server.queue(
            client.anchor,
            &ServerboundMovePlayerPos {
                position: away.into(),
                flags: MoveFlags::new().with_on_ground(true),
            },
        );
        client.server.ticks(client.anchor, 1);
        resent.extend(client.positions().into_iter().map(|resend| (tick, resend)));
    }

    let ids: Vec<_> = resent
        .iter()
        .map(|(_, resend)| resend.teleport_id.0)
        .collect();
    assert_eq!(ids, [2, 3, 4], "TP-CONFIRM-RES-002, TP-CONFIRM-ID-002");
    let gaps: Vec<_> = resent
        .windows(2)
        .map(|pair| pair[1].0 - pair[0].0)
        .collect();
    assert_eq!(gaps, [21, 21], "TP-CONFIRM-RES-003, TP-CONFIRM-RES-006");
    for (_, resend) in resent {
        assert_eq!(resend.position, destination, "TP-CONFIRM-RES-004");
        assert_eq!(
            (resend.velocity, resend.flags),
            (DVec3::ZERO, PositionFlags::default()),
            "TP-CONFIRM-RES-006: zero velocity, no relative flags"
        );
    }
    assert_eq!(client.position(), destination, "TP-CONFIRM-MOV-001");
}

/// This server checks an accept against the destination (deviation D8 of `movement-checks.md`), so
/// an accept 5 blocks away gets a correction as one 1000 blocks away does.
#[test]
fn an_accept_away_from_the_destination_gets_a_correction_each_time() {
    let mut client = Client::join();
    let destination = client.login.position;

    client.accept(1, destination + DVec3::new(5.0, 0.0, 0.0));
    let [first] = client.positions().try_into().unwrap();
    assert_eq!(
        (first.teleport_id.0, first.position),
        (2, destination),
        "TP-CONFIRM-ACC-004, TP-CONFIRM-ID-002"
    );
    assert_eq!(client.position(), destination);
    assert!(!client.step_applies(), "TP-CONFIRM-ACC-007");

    client.accept(2, destination + DVec3::new(1000.0, 0.0, 0.0));
    let [second] = client.positions().try_into().unwrap();
    assert_eq!(
        (second.teleport_id.0, second.position),
        (3, destination),
        "TP-CONFIRM-ACC-008: no interval between corrections"
    );

    client.accept(3, destination);
    assert!(client.step_applies(), "TP-CONFIRM-ACC-007");
}

#[test]
fn an_accept_of_the_newest_id_with_nothing_pending_disconnects() {
    let mut client = Client::join();
    let destination = client.login.position;
    client.accept(1, destination);
    assert_eq!(client.disconnect(), None);

    client.accept(1, destination);
    let reason = client
        .disconnect()
        .expect("TP-CONFIRM-ACC-005: a disconnect");
    assert!(
        reason.contains("multiplayer.disconnect.invalid_player_movement"),
        "TP-CONFIRM-ACC-005: {reason}"
    );
}

#[test]
fn an_accept_of_an_id_never_sent_with_nothing_pending_keeps_the_client() {
    let mut client = Client::join();
    let destination = client.login.position;
    client.accept(1, destination);
    client.accept(1001, destination);
    assert_eq!(client.disconnect(), None, "TP-CONFIRM-ACC-006");
    assert!(client.step_applies());
}
