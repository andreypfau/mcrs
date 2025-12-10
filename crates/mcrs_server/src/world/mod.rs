use crate::client_info::ClientViewDistance;
use crate::world::chunk_observer::{ChunkObserverPlugin, PlayerChunkObserver};
use crate::world::movement::{MovementBundle, MovementPlugin};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;
use mcrs_network::{ConnectionState, ServerSideConnection};
use mcrs_protocol::entity::player::PlayerSpawnInfo;
use mcrs_protocol::math::DVec3;
use mcrs_protocol::packets::game::clientbound::{ClientboundGameEvent, ClientboundLogin};
use mcrs_protocol::{ident, GameEventKind, Position, VarInt, WritePacket};
use crate::world::block::minecraft::MinecraftBlockPlugin;
use crate::world::block_update::BlockUpdatePlugin;
use crate::world::dimension_time::DimensionTimePlugin;
use crate::world::entity::player::PlayerBundle;
use crate::world::entity::player_action::PlayerActionPlugin;

pub mod chunk;
pub mod chunk_observer;
mod chunk_tickets;
mod movement;
mod paletted_container;
mod player_chunk_loader;
mod format;
pub mod entity;
pub mod block;
mod block_update;
mod generate;
mod dimension_time;
mod pumpkin_palette;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DimensionTimePlugin);
        app.add_plugins(ChunkObserverPlugin);
        app.add_plugins(MovementPlugin);
        app.add_plugins(PlayerActionPlugin);
        app.add_plugins(BlockUpdatePlugin);
        app.add_plugins(MinecraftBlockPlugin);
        app.add_systems(FixedUpdate, spawn_player);
    }
}

fn spawn_player(
    mut query: Query<
        (Entity, &ClientViewDistance, &ConnectionState, &mut ServerSideConnection),
        Changed<ConnectionState>,
    >,
    mut commands: Commands,
) {
    query.iter_mut().for_each(|(entity, distance, con_state, mut con)| {
        if *con_state != ConnectionState::Game {
            return;
        }
        con.write_packet(&ClientboundLogin {
            player_id: entity.index() as i32,
            hardcore: false,
            dimensions: vec![ident!("overworld").into()],
            max_players: VarInt(100),
            chunk_radius: VarInt(12),
            simulation_distance: VarInt(12),
            reduced_debug_info: false,
            show_death_screen: false,
            do_limited_crafting: false,
            player_spawn_info: PlayerSpawnInfo::default(),
            enforces_secure_chat: false,
        });
        con.write_packet(&ClientboundGameEvent {
            game_event: GameEventKind::LevelChunksLoadStart,
        });
        let pos = Position::from(DVec3::new(0.0, 17.0, 0.0));
        commands.entity(entity).insert((
            PlayerChunkObserver {
                ..Default::default()
            },
            MovementBundle {
                position: Position::from(DVec3::new(0.0, 17.0, 0.0)),
                ..Default::default()
            },
            PlayerBundle {
                ..Default::default()
            },
        ));
    });
}
