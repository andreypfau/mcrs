use bevy_app::{App, Plugin};

use crate::coordinator::coordinator_plugin::CoordinatorPlugin;
use crate::muxer::muxer_plugin::MuxerPlugin;

pub mod ability;
pub mod between_ticks;
pub mod block_entity_packet;
pub mod block_simulation;
pub mod block_update;
pub mod bus;
pub mod cell_crossing;
pub mod cell_mail;
mod cell_volume;
pub mod channel_types;
pub mod client_info;
pub mod cold_columns;
pub mod column_asks;
pub mod column_change;
pub mod column_generation;
mod column_packet;
pub mod column_production;
pub mod column_request;
mod column_snapshot;
pub mod column_store;
pub mod column_streams;
pub mod column_ticks;
pub mod column_version;
pub mod command_handoff;
pub mod consume_inbound_player_spawn;
pub mod coordinator;
pub mod departure;
pub mod despawn_inbound_player;
pub mod dim_mail;
pub mod dim_player_plugin;
pub mod dim_tick;
pub mod dimension_change;
pub mod dimension_generators;
pub mod drain_player_despawn;
pub mod enter_level_packets;
pub mod entity_ids;
mod foreign_views;
pub mod game_mode;
pub mod game_profile;
pub mod heightmap;
pub mod hot_ticket;
pub mod light_codec;
pub mod minecraft_entity;
pub mod movement;
pub mod muxer;
pub mod network_id;
pub mod op_level;
pub mod persistence;
pub mod player_bundle;
pub mod player_dimension_change;
pub mod player_order;
pub mod player_roster;
pub mod player_tiles;
pub mod player_tracker;
pub mod player_view;
pub mod publish_player_tiles;
pub mod replicated;
pub mod stored_column;
pub mod stored_columns_landing;
pub mod teleport;
pub mod tile_events;
pub mod to_coordinator;
pub mod to_muxer;
pub mod world_kinds;
pub mod world_seed;

/// The host: the coordinator and the muxer, in one world.
pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((CoordinatorPlugin, MuxerPlugin));
    }
}
