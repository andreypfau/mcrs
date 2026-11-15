use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_chunk::PalettedContainer;
use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::metrics::PacketDispatcher;
use mcrs_minecraft_physics::{PlayerState, Tables, TestBlocks};
use mcrs_minecraft_protocol::{Encode, Packet, VarInt};
use mcrs_minecraft_protocol_game::ColumnPos;
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;
use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::SectionPos;

use crate::collision_tables::ClientPhysicsTables;
use crate::columns::{AIR, Column, ColumnStore, Extent, Section};
use crate::game_packets::GAME_CLIENTBOUND;
use crate::joined_game::JoinedGame;
use crate::local_motion::LocalMotion;
use crate::local_player::LocalPlayerPlugin;
use crate::movement_input::MovementInput;
use crate::player::spawn_player;

pub(crate) const FLOOR_TOP: f64 = 16.0;
pub(crate) const OWN_PLAYER: i32 = 7;

/// A stone floor whose top is at [`FLOOR_TOP`], in the one column at the origin of a dimension of
/// two sections.
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

/// Raises the observers of `packet` as if `connection` received it in the play state.
///
/// # Panics
///
/// When `packet` does not encode or is not a clientbound play packet.
pub(crate) fn receive<P: Packet + Encode>(app: &mut App, connection: Entity, packet: P) {
    let mut data = Vec::new();
    packet.encode(&mut data).unwrap();
    let trigger = PacketDispatcher::clientbound()
        .with_play(GAME_CLIENTBOUND)
        .dispatch(P::STATE, P::ID, &data)
        .unwrap()
        .unwrap();
    trigger.raise(&mut app.world_mut().commands(), connection, data.into());
    app.world_mut().flush();
}

pub(crate) fn spawn_info(game_mode: GameMode) -> PlayerSpawnInfo {
    PlayerSpawnInfo::new(Id::checked(0, 1).unwrap(), OVERWORLD.into(), game_mode)
}

/// The `login` of [`OWN_PLAYER`] in `game_mode`.
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

/// An app with the [`LocalPlayerPlugin`], the local player and a connection in the play state.
pub(crate) struct TestSession {
    pub(crate) app: App,
    pub(crate) player: Entity,
    pub(crate) connection: Entity,
}

impl TestSession {
    /// A local player at `position` above the stone floor of [`stone_floor`], before its `login`.
    pub(crate) fn new(position: DVec3) -> Self {
        let mut app = App::new();
        app.add_plugins(LocalPlayerPlugin);
        let blocks = TestBlocks::new();
        app.insert_resource(stone_floor(&blocks))
            .insert_resource(ClientPhysicsTables(Tables::new(blocks.table)));
        let player = spawn_player(app.world_mut(), position, 0.0, 0.0);
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

    /// Runs `ticks` fixed ticks with `input` held.
    pub(crate) fn ticks(&mut self, input: MovementInput, ticks: usize) {
        self.app.insert_resource(input);
        for _ in 0..ticks {
            self.app.world_mut().run_schedule(FixedUpdate);
        }
    }

    /// The kernel state of the player.
    ///
    /// # Panics
    ///
    /// Before the `login`, which gives the player its kernel state.
    pub(crate) fn motion(&self) -> PlayerState {
        self.app.world().get::<LocalMotion>(self.player).unwrap().0
    }
}
