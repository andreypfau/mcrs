use bevy_app::{App, Plugin};

use crate::coordinator::coordinator_plugin::CoordinatorPlugin;
use crate::muxer::muxer_plugin::MuxerPlugin;

pub mod ability;
mod advance_production;
pub mod between_ticks;
pub mod block_action_ack;
mod block_break;
pub mod block_drops;
pub mod block_entity_packet;
pub mod block_simulation;
mod block_update;
pub mod bus;
mod cell_crossing;
mod cell_mail;
mod cell_volume;
pub mod channel_types;
pub mod client_info;
mod cold_columns;
pub mod cold_light_edit;
mod column_asks;
pub mod column_change;
pub mod column_generation;
pub mod column_light;
mod column_packet;
pub mod column_production;
mod column_request;
mod column_snapshot;
pub mod column_store;
pub mod column_streams;
pub mod column_ticks;
pub mod column_version;
pub mod command_handoff;
mod consume_inbound_player_spawn;
pub mod coordinator;
mod departure;
pub mod despawn_inbound_player;
pub mod dim_mail;
mod dim_player_plugin;
pub mod dim_tick;
pub mod dimension_change;
pub mod dimension_generators;
pub mod drain_player_despawn;
mod enter_level_packets;
pub mod entity_ids;
pub mod foreign_views;
mod game_mode;
pub mod game_profile;
pub mod heightmap;
mod held_light;
mod hot_ticket;
mod light_codec;
pub mod minecraft_entity;
mod movement;
pub mod muxer;
pub mod network_id;
mod op_level;
mod persistence;
mod player_bundle;
mod player_dimension_change;
pub mod player_order;
pub mod player_roster;
mod player_tiles;
mod player_tracker;
mod player_view;
mod publish_player_tiles;
mod random_tick_speed;
mod replicated;
pub mod stored_column;
mod stored_columns_landing;
pub mod teleport;
mod tile_events;
pub mod to_coordinator;
pub mod to_muxer;
mod wanted_column;
pub mod world_kinds;
pub mod world_seed;

/// The host: the coordinator and the muxer, in one world.
pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((CoordinatorPlugin, MuxerPlugin));
    }
}
