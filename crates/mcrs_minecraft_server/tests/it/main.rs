#![recursion_limit = "256"]
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]
#![expect(
    clippy::disallowed_macros,
    clippy::disallowed_types,
    reason = "test input is written as JSON"
)]

// Every file here is one module of a single test binary; a top-level
// file under tests/ would be a separate crate and a separate link.
mod anvil_block_states;
mod ban;
mod block_break;
mod block_entities;
mod block_loot_addressing;
mod block_place;
mod bridge_dispatch;
mod bridge_inbound;
mod bridge_outbound;
mod bus_e2e;
mod channel_overload;
mod chat;
mod client_join;
mod column_store;
mod commands;
mod configuration_registries;
mod configuration_start;
mod configuration_tags;
mod cross_player_isolation;
mod dimension_change;
mod dimension_list;
mod dimension_plugins;
mod dimension_type;
mod disconnect;
mod entity_ids;
mod entity_replication;
mod epoch_round_trip;
mod host_app;
mod host_subapp_handoff;
mod hostile_commands;
mod item_summon;
mod light_engine_e2e;
mod login_handshake_e2e;
mod login_profile;
mod mob_ai;
mod mob_replication;
#[path = "common/mock_connection.rs"]
mod mock_connection;
mod op_level;
mod persistence;
mod play_state_delivery;
mod player_file;
mod probe_bans;
mod run_server_loop_exit;
mod schedule_ambiguity;
mod session_lifecycle;
mod subapp_lifecycle;
mod support;
mod teleport_confirmation;
mod tick_determinism;
mod whitelist;
mod world_cells;
mod world_server;
mod worldgen_y_range_clamp;
