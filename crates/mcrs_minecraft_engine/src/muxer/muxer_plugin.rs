use bevy_app::{App, FixedPostUpdate, Plugin, Update};
use bevy_ecs::message::signal_message_update_system;
use bevy_ecs::query::{Changed, With};
use bevy_ecs::schedule::common_conditions::{any_component_removed, any_match_filter};
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, SingleThreadedExecutor, SystemCondition};
use mcrs_minecraft_engine_world::session::{
    PlayerSessionCounter, SessionAnchors, SessionPlacement,
};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_network::metrics::{BridgeTelemetry, PacketDispatcher};

use crate::bus::{InboundPlayerPacket, OutboundPlayerAttached, OutboundPlayerPacket};
use crate::coordinator::server_state_host_plugin;
use crate::muxer::backlog;
use crate::muxer::bridge;
use crate::muxer::bridge_queue::OutboundQueue;
use crate::muxer::dim_inputs::DimInputsPlugin;
use crate::muxer::disconnect::DisconnectProtocolPlugin;
use crate::muxer::game_packets;
use crate::muxer::latency;
use crate::muxer::muxer_link;
use crate::muxer::online_count;

pub struct MuxerPlugin;

impl Plugin for MuxerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSessionCounter>();
        app.init_resource::<SessionAnchors>();
        app.init_resource::<BridgeTelemetry>();
        app.add_observer(muxer_link::acknowledge_batch);
        app.world_mut()
            .get_resource_or_insert_with(PacketDispatcher::serverbound)
            .set_play(game_packets::GAME_SERVERBOUND);
        app.add_message::<OutboundPlayerPacket>();
        app.add_message::<InboundPlayerPacket>();
        app.add_message::<OutboundPlayerAttached>();
        app.add_observer(bridge::attach_outbound_queue);

        app.init_resource::<bridge::PendingConnections>();
        let mut flush = Schedule::new(bridge::OutboundFlush);
        flush.set_executor(SingleThreadedExecutor::new());
        app.add_schedule(flush);
        app.add_systems(
            bridge::OutboundFlush,
            (
                bridge::bridge_outbound.run_if(bridge::messages_pending::<OutboundPlayerPacket>),
                bridge::dispatch_encode
                    .run_if(any_match_filter::<(With<OutboundQueue>, With<ServerSideConnection>)>),
                backlog::forward_backlog,
            )
                .chain(),
        );
        app.add_systems(
            FixedPostUpdate,
            (
                bridge::run_outbound_flush.after(signal_message_update_system),
                bridge::bridge_inbound,
            )
                .chain(),
        );

        app.add_systems(
            Update,
            (
                bridge::bridge_inbound_to_channel
                    .run_if(bridge::messages_pending::<InboundPlayerPacket>),
                bridge::bridge_player_attach
                    .run_if(bridge::messages_pending::<OutboundPlayerAttached>),
                bridge::forward_pending_inbound.run_if(bridge::inbound_held),
                online_count::send_online_count
                    .run_if(
                        any_match_filter::<Changed<SessionPlacement>>
                            .or_eager(any_component_removed::<SessionPlacement>),
                    )
                    .before(server_state_host_plugin::announce_tick_state),
                latency::forward_latency,
            )
                .chain(),
        );

        app.add_plugins(DisconnectProtocolPlugin);
        app.add_plugins(DimInputsPlugin);
    }
}
