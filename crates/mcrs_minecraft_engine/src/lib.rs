use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_minecraft_level::world::sub_app::{DimDespawnQueue, DimSpawnQueue};

pub mod ability;
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
pub mod consume_inbound_player_spawn;
pub mod default_game_mode;
pub mod despawn_inbound_player;
pub mod dim;
pub mod dim_player_plugin;
pub mod dimension_change;
pub mod dimension_order;
pub mod disconnect;
pub mod drain_player_despawn;
pub mod enter_level_packets;
pub mod game_mode;
pub mod heightmap;
pub mod light_codec;
pub mod minecraft_entity;
pub mod movement;
pub mod network_add;
pub mod network_id;
pub mod op_level;
pub mod ops;
pub mod persistence;
pub mod player_bundle;
pub mod player_dimension_change;
pub mod player_tracker;
pub mod session;
pub mod sub_app_builder;
pub mod teleport;
pub mod tracked_by;
pub mod update_tracked_by;
pub mod world_plugin;

pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimSpawnQueue>();
        app.init_resource::<DimDespawnQueue>();

        // The bridge systems and the host side of the dimension channels read these messages
        // with `resource_mut`, which panics on a message that was never added.
        app.init_resource::<mcrs_minecraft_level::session::PlayerSessionCounter>();
        app.init_resource::<channel_types::DimChannelsResource>();
        app.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();
        // The pre-game loop and the bridge share one dispatcher, whichever plugin builds first.
        app.world_mut().get_resource_or_insert_with(
            mcrs_minecraft_network::metrics::PacketDispatcher::serverbound,
        );
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

        app.add_schedule(Schedule::new(bridge::OutboundFlush));
        app.add_systems(
            bridge::OutboundFlush,
            (bridge::bridge_outbound, bridge::dispatch_encode).chain(),
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
