use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin, Update};
use bevy_ecs::prelude::*;
use mcrs_minecraft_level::world::sub_app::{DimDespawnQueue, DimSpawnQueue};

pub mod bridge;
pub mod bridge_queue;
pub mod bus;
pub mod channel_types;
pub mod dim;
pub mod disconnect;
pub mod heightmap;
pub mod session;

pub struct EnginePlugin;

impl Plugin for EnginePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimSpawnQueue>();
        app.init_resource::<DimDespawnQueue>();

        // The bridge systems and the host side of the dimension channels read these messages
        // with `resource_mut`, which panics on a message that was never added.
        app.init_resource::<mcrs_minecraft_level::session::PlayerSessionCounter>();
        app.init_resource::<channel_types::DimChannelsResource>();
        app.init_resource::<mcrs_minecraft_level::world::in_flight::InFlightMoves>();
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
