use bevy::math::DVec3;
use bevy::prelude::*;
#[cfg(not(target_family = "wasm"))]
use bytes::BytesMut;
use mcrs_minecraft_chunk::PalettedContainer;
use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
use mcrs_minecraft_engine_world::entity::physics::Transform as PhysicsTransform;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::ConnectionState;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_network::client::{
    ClientConnection, ClientNetworkPlugin, JoinRequest, finish_configuration,
};
use mcrs_minecraft_network::metrics::PacketDispatcher;
use mcrs_minecraft_physics::{PlayerState, Tables, TestBlocks};
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::Bounded;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::decode::{PacketDecoder, PacketFrame};
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundFinishConfiguration;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundClientInformation;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::packets::login::clientbound::{
    ClientboundLoginFinishedWithOptions, OptionFlags,
};
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::profile::GameProfile;
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Encode, Packet, VarInt};
#[cfg(not(target_family = "wasm"))]
use mcrs_minecraft_protocol::{PacketEncoder, WritePacket};
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::SectionPos;
#[cfg(not(target_family = "wasm"))]
use std::io::{Read, Write};
#[cfg(not(target_family = "wasm"))]
use std::net::{TcpListener, TcpStream};
#[cfg(not(target_family = "wasm"))]
use std::thread;
#[cfg(not(target_family = "wasm"))]
use std::time::Duration;

use crate::collision_tables::ClientPhysicsTables;
use crate::columns::{AIR, Column, ColumnStore, Extent, Section};
use crate::game_mode::GameModePlugin;
use crate::game_packets::GAME_CLIENTBOUND;
use crate::health_packets::{NEW_PLAYER_FOOD, NEW_PLAYER_VITALS};
use crate::joined_game::JoinedGame;
use crate::local_motion::LocalMotion;
use crate::local_player::LocalPlayerPlugin;
use crate::movement_input::MovementInput;
use crate::player::player_at;

pub(crate) const FLOOR_TOP: f64 = 16.0;
pub(crate) const OWN_PLAYER: i32 = 7;

pub(crate) fn stone_floor(blocks: &TestBlocks) -> ColumnStore {
    let extent = Extent {
        min_section_y: 0,
        sections: 2,
    };
    let ground = Section {
        blocks: Box::new([blocks.stone; SectionPos::VOLUME]),
        biomes: PalettedContainer::Homogeneous(0),
        states: vec![AIR, blocks.stone],
    };
    let mut columns = ColumnStore::default();
    columns.enter(extent);
    columns.insert(
        ColumnPos::new(0, 0),
        Column::unlit(extent.min_section_y, vec![Some(ground), None]),
    );
    columns
}

pub(crate) fn receive<P: Packet + Encode>(app: &mut App, connection: Entity, packet: P) {
    let mut data = Vec::new();
    packet.encode(&mut data).unwrap();
    let trigger = PacketDispatcher::clientbound()
        .with_play(GAME_CLIENTBOUND)
        .dispatch(P::STATE, P::ID, &data)
        .unwrap()
        .unwrap();
    trigger.raise(app.world_mut().commands(), connection, data.into());
    app.world_mut().flush();
}

pub(crate) fn spawn_info(game_mode: GameMode) -> PlayerSpawnInfo {
    PlayerSpawnInfo::new(Id::checked(0, 1).unwrap(), OVERWORLD.into(), game_mode)
}

pub(crate) fn login(game_mode: GameMode) -> ClientboundLogin {
    ClientboundLogin {
        player_id: OWN_PLAYER,
        hardcore: false,
        dimensions: Vec::new(),
        max_players: VarInt(20),
        chunk_radius: VarInt(8),
        simulation_distance: VarInt(8),
        reduced_debug_info: false,
        show_death_screen: true,
        do_limited_crafting: false,
        player_spawn_info: spawn_info(game_mode),
        online_mode: false,
        enforces_secure_chat: false,
    }
}

pub(crate) struct TestSession {
    pub(crate) app: App,
    pub(crate) player: Entity,
    pub(crate) connection: Entity,
}

impl TestSession {
    pub(crate) fn new(position: DVec3) -> Self {
        let mut app = App::new();
        app.add_plugins((GameModePlugin, LocalPlayerPlugin));
        let blocks = TestBlocks::new();
        app.insert_resource(stone_floor(&blocks))
            .insert_resource(ClientPhysicsTables(Tables::new(blocks.table)));
        let player = app
            .world_mut()
            .spawn((
                player_at(position, 0.0, 0.0),
                NEW_PLAYER_VITALS,
                NEW_PLAYER_FOOD,
            ))
            .id();
        let connection = app
            .world_mut()
            .spawn((
                ConnectionState::Game,
                JoinedGame {
                    player_id: OWN_PLAYER,
                    dimensions: Vec::new(),
                },
            ))
            .id();
        Self {
            app,
            player,
            connection,
        }
    }

    pub(crate) fn receive<P: Packet + Encode>(&mut self, packet: P) {
        receive(&mut self.app, self.connection, packet);
    }

    pub(crate) fn ticks(&mut self, input: MovementInput, ticks: usize) {
        self.app.insert_resource(input);
        for _ in 0..ticks {
            self.app.world_mut().run_schedule(FixedUpdate);
        }
    }

    pub(crate) fn position(&self) -> DVec3 {
        self.app
            .world()
            .get::<PhysicsTransform>(self.player)
            .unwrap()
            .translation
    }

    pub(crate) fn motion(&self) -> PlayerState {
        self.app.world().get::<LocalMotion>(self.player).unwrap().0
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) struct Server {
    stream: TcpStream,
    decoder: PacketDecoder,
}

#[cfg(not(target_family = "wasm"))]
impl Server {
    pub(crate) fn accept(listener: &TcpListener) -> Self {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Self {
            stream,
            decoder: PacketDecoder::new(),
        }
    }

    pub(crate) fn join(listener: &TcpListener) -> (Self, PacketFrame) {
        let mut server = Self::accept(listener);
        let _handshake = server.next_frame();
        let _hello = server.next_frame();
        server.send(&ClientboundLoginFinishedWithOptions {
            profile: GameProfile {
                id: Uuid::nil(),
                username: Bounded("Steve"),
                properties: Default::default(),
            },
            session_id: Uuid::nil(),
            options: OptionFlags::new(),
        });
        let _acknowledged = server.next_frame();
        let _brand = server.next_frame();
        let _mod_list = server.next_frame();
        let information = server.next_frame();
        assert_eq!(
            information.id,
            ServerboundClientInformation::ID,
            "the configuration starts with the client information"
        );
        server.send(&ClientboundFinishConfiguration);
        (server, information)
    }

    pub(crate) fn next_frame(&mut self) -> PacketFrame {
        loop {
            if let Some(frame) = self.decoder.try_next_packet().unwrap() {
                return frame;
            }
            let mut buffer = [0; 1024];
            let read = self.stream.read(&mut buffer).unwrap();
            assert_ne!(read, 0, "the client closed the connection");
            self.decoder.queue_bytes(BytesMut::from(&buffer[..read]));
        }
    }

    pub(crate) fn send(&mut self, packet: &(impl Encode + Packet)) {
        let mut encoder = PacketEncoder::new();
        encoder.write_packet(packet);
        self.stream.write_all(&encoder.take()).unwrap();
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) struct Joined {
    pub(crate) app: App,
    pub(crate) server: Server,
    pub(crate) connection: Entity,
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn joined(mut app: App) -> Joined {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    app.add_plugins(ClientNetworkPlugin {
        holds_vanilla_pack: false,
        play: None,
    });
    app.world_mut().write_message(JoinRequest {
        server: listener.local_addr().unwrap(),
        username: "Steve".to_owned(),
        profile_id: None,
        view_distance: 8,
    });
    app.update();
    let (mut server, _information) = Server::join(&listener);
    let connection = (0..1000)
        .find_map(|_| {
            app.update();
            thread::sleep(Duration::from_millis(10));
            app.world_mut()
                .query_filtered::<Entity, With<ClientConnection>>()
                .single(app.world())
                .ok()
        })
        .expect("the client joins");
    finish_configuration(&mut app.world_mut().commands(), connection);
    app.world_mut().flush();
    app.update();
    let _finished = server.next_frame();
    app.world_mut().entity_mut(connection).insert(JoinedGame {
        player_id: OWN_PLAYER,
        dimensions: Vec::new(),
    });
    Joined {
        app,
        server,
        connection,
    }
}
