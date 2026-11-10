//! The muxer: the gateway for the clients. It owns the connections and the bridge, the sessions
//! and their placement, the input that it routes to the owner dimension of each player, and the
//! host side of entity replication, and the view of each player.
//!
//! The rule of the boundary: a module of this tree never names a module of `coordinator`, and no
//! system here reads a resource or a component that the coordinator owns. The muxer learns what
//! the coordinator decides from the messages of `to_muxer`, and tells the coordinator what it
//! needs through the messages of `to_coordinator`.

pub mod backlog;
mod batch_pacing;
pub mod bridge;
pub mod bridge_queue;
pub mod column_views;
pub mod departure_routing;
pub mod dim_inputs;
pub mod dim_lifecycle;
pub mod dim_outputs;
pub mod disconnect;
pub mod entity_replication;
pub mod game_packets;
pub mod join;
pub mod latency;
pub mod muxer_link;
pub mod muxer_placement;
pub mod muxer_plugin;
pub mod muxer_stage;
pub mod online_count;
pub mod probes;
pub mod session;
pub mod tick_wake;
#[cfg(feature = "world-access")]
pub mod tile_delta_log;
