use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_world::world::sub_app::{DimDespawnQueue, DimSpawnQueue};

pub mod ability;
pub mod between_ticks;
pub mod block_entity_packet;
pub mod block_update;
pub mod bridge;
pub mod bridge_queue;
pub mod bus;
pub mod channel_types;
pub mod client_info;
pub mod clock;
pub mod column_held;
pub mod column_view;
mod column_waiters;
pub mod consume_inbound_player_spawn;
pub mod default_game_mode;
pub mod departure;
pub mod despawn_inbound_player;
pub mod dim;
pub mod dim_cells;
pub mod dim_inputs;
pub mod dim_player_plugin;
pub mod dim_tick;
pub mod dimension_change;
pub mod dimension_generators;
pub mod dimension_order;
pub mod disconnect;
pub mod drain_player_despawn;
pub mod enter_level_packets;
pub mod entity_replication;
pub mod game_mode;
pub mod game_packets;
pub mod heightmap;
pub mod latency;
pub mod light_codec;
pub mod minecraft_entity;
pub mod movement;
pub mod muxer_placement;
pub mod network_id;
pub mod online_count;
pub mod op_level;
pub mod ops;
pub mod persistence;
pub mod player_bundle;
pub mod player_dimension_change;
pub mod player_list_policy;
pub mod player_tiles;
pub mod player_tracker;
pub mod publish_player_tiles;
pub mod replicated;
pub mod session;
pub mod sub_app_builder;
pub mod teleport;
pub mod tile_delta_log;
pub mod tile_events;
pub mod world_plugin;
pub mod world_seed;

pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimSpawnQueue>();
        app.init_resource::<DimDespawnQueue>();

        // The bridge systems and the host side of the dimension channels read these messages
        // with `resource_mut`, which panics on a message that was never added.
        app.init_resource::<mcrs_minecraft_engine_world::session::PlayerSessionCounter>();
        app.init_resource::<mcrs_minecraft_engine_world::session::SessionAnchors>();
        app.add_plugins(dim_cells::DimCellsPlugin);
        app.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();
        // The pre-game loop and the bridge share one dispatcher, whichever plugin builds first.
        app.world_mut()
            .get_resource_or_insert_with(
                mcrs_minecraft_network::metrics::PacketDispatcher::serverbound,
            )
            .set_play(game_packets::GAME_SERVERBOUND);
        app.add_message::<bus::OutboundPlayerPacket>();
        app.add_message::<bus::InboundPlayerPacket>();
        app.add_message::<bus::OutboundPlayerAttached>();
        app.add_message::<bus::OutboundPlayerDisconnect>();
        // The commands of `attach_outbound_queue` apply at the end of FixedPreUpdate, so every
        // connection that `SpawnConnections` spawned carries an `OutboundQueue` by
        // FixedPostUpdate.
        app.add_systems(
            FixedPreUpdate,
            bridge::attach_outbound_queue
                .after(mcrs_minecraft_network::NetworkSet::SpawnConnections),
        );

        // One chain: threads would only add the cost of handing its systems out, on a flush
        // that runs after every drain.
        app.init_resource::<bridge::PendingConnections>();
        app.init_resource::<bridge::FlushScope>();
        let mut flush = Schedule::new(bridge::OutboundFlush);
        flush.set_executor(bevy_ecs::schedule::SingleThreadedExecutor::new());
        app.add_schedule(flush);
        app.add_systems(
            bridge::OutboundFlush,
            (
                bridge::bridge_outbound,
                bridge::dispatch_encode.run_if(bridge::flushes_every_connection),
                bridge::dispatch_pending.run_if(bridge::flushes_pending_connections),
            )
                .chain(),
        );
        app.add_systems(
            FixedPostUpdate,
            (
                bridge::run_outbound_flush.after(bevy_ecs::message::signal_message_update_system),
                bridge::bridge_inbound,
            )
                .chain(),
        );

        app.add_systems(
            Update,
            (
                bridge::bridge_inbound_to_channel,
                bridge::bridge_player_attach,
                bridge::forward_pending_inbound,
                online_count::send_online_count,
                latency::forward_latency,
            )
                .chain(),
        );

        app.add_observer(
            |trigger: On<Remove, dim::DimSubAppHandle>, mut queue: ResMut<DimDespawnQueue>| {
                queue.0.push(trigger.event().entity);
            },
        );
        app.add_plugins(disconnect::DisconnectProtocolPlugin);
    }
}
