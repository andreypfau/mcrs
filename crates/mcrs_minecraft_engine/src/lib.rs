#![recursion_limit = "256"]

use bevy_app::{App, Plugin};

use crate::coordinator::coordinator_plugin::CoordinatorPlugin;
use crate::muxer::muxer_plugin::MuxerPlugin;

pub mod ability;
mod advance_digs;
mod advance_production;
mod assign_global_entity_id;
mod autosave;
mod background_writer;
pub mod banlist;
pub mod between_ticks;
pub mod block_action_ack;
mod block_break;
pub mod block_entity_packet;
mod block_mail;
pub mod block_simulation;
mod block_update;
mod break_session;
pub mod bus;
mod carried_attributes;
mod cell_crossing;
mod cell_mail;
mod cell_volume;
pub mod channel_types;
pub mod client_info;
pub mod client_on_ground;
mod client_view_distance;
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
mod column_store_mail;
pub mod column_streams;
mod column_tick_holder;
pub mod column_ticks;
pub mod column_version;
pub mod command_handoff;
mod consume_inbound_player_spawn;
pub mod coordinator;
mod departure;
pub mod despawn_inbound_player;
mod dig_rate;
mod diggers;
mod dim_inbox_drain;
pub mod dim_mail;
mod dim_player_plugin;
pub mod dim_tick;
pub mod dimension_change;
pub mod dimension_generators;
pub mod drain_player_despawn;
mod enter_level_packets;
pub mod foreign_views;
mod game_mode;
pub mod game_profile;
pub mod heightmap;
mod held_joins;
mod held_light;
mod hot_ticket;
mod json_list;
mod light_codec;
pub mod loaded_client;
mod max_health;
pub mod minecraft_entity;
mod movement;
pub mod muxer;
pub mod network_id;
mod op_level;
pub mod persisted_registry;
mod persistence;
mod player_bundle;
mod player_dimension_change;
mod player_dimensions;
pub mod player_order;
pub mod player_roster;
pub mod player_signal;
pub mod player_sprinting;
pub mod player_state;
mod player_tracker;
mod player_view;
mod pose_systems;
mod publish_player_tiles;
mod publish_pose;
mod random_tick_speed;
mod replicated;
mod respawn_packet;
mod saved_spawn;
mod send_own_attributes;
pub mod server_edit;
mod shift_key;
mod spawn_override;
pub mod stored_column;
mod stored_columns_landing;
pub mod swing;
pub mod teleport;
pub mod tile_events;
pub mod to_coordinator;
pub mod to_muxer;
pub mod use_item_on_request;
pub mod view_radius;
mod wanted_column;
pub mod whitelist;
pub mod world_kinds;
mod world_state;

#[cfg(feature = "world-access")]
pub use cell_volume::CellVolume;

pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<persistence::PlayerPersistencePlugin>() {
            app.add_plugins(persistence::PlayerPersistencePlugin);
        }
        app.add_plugins((CoordinatorPlugin, MuxerPlugin));
    }
}

pub use autosave::{AutosaveCadence, AutosavePlugin};
pub use block_break::{BlockBreakSet, eye_position, in_reach, reach};
pub use column_tick_holder::{ColumnTickHolder, CopiedTicks, NoColumnTicks, TakenTicks};
pub use consume_inbound_player_spawn::InboundPlayerSpawnSet;
pub use dim_inbox_drain::DimInboxDrain;
pub use held_joins::HeldJoins;
pub use max_health::max_health;
pub use persistence::{
    PlayerPersistencePlugin, UnreadablePlayerDat, apply_join_snapshot, load_player, save_player,
    write_player,
};
pub use player_dimension_change::{check_departure, is_own_dimension};
pub use player_dimensions::{PlayerBody, PlayerBodyItem, player_dimensions, player_scale};
pub use pose_systems::PoseSystems;
pub use publish_player_tiles::HiddenPlayer;
pub use respawn_packet::{KeptPlayerData, respawn_packet};
pub use saved_spawn::SavedSpawn;
pub use send_own_attributes::OwnAttributesSet;
pub use shift_key::ShiftKey;
pub use spawn_override::SpawnOverride;
pub use world_state::{WorldStatePlugin, WorldStateSources, WorldStateWrite, WorldStateWriter};
