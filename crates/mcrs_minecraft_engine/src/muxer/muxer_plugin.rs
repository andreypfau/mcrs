use bevy_app::{App, FixedPostUpdate, Plugin, Update};
use bevy_ecs::message::signal_message_update_system;
use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule, SingleThreadedExecutor};
use mcrs_minecraft_engine_world::session::{PlayerSessionCounter, SessionAnchors};
use mcrs_minecraft_network::metrics::{BridgeTelemetry, PacketDispatcher};

use crate::bus::{InboundPlayerPacket, OutboundPlayerAttached, OutboundPlayerPacket};
use crate::muxer::backlog;
use crate::muxer::bridge;
use crate::muxer::dim_inputs::DimInputsPlugin;
use crate::muxer::disconnect::DisconnectProtocolPlugin;
use crate::muxer::game_packets;
use crate::muxer::latency;
use crate::muxer::muxer_link;
use crate::muxer::online_count;

/// The muxer: the connections and the bridge, the sessions, and the input that it routes to
/// each dimension.
pub struct MuxerPlugin;

impl Plugin for MuxerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSessionCounter>();
        app.init_resource::<SessionAnchors>();
        app.init_resource::<BridgeTelemetry>();
        app.add_observer(muxer_link::acknowledge_batch);
        // The pre-game loop and the bridge share one dispatcher, whichever plugin builds first.
        app.world_mut()
            .get_resource_or_insert_with(PacketDispatcher::serverbound)
            .set_play(game_packets::GAME_SERVERBOUND);
        // The bridge systems and the host side of the dimension channels read these messages
        // with `resource_mut`, which panics on a message that was never added.
        app.add_message::<OutboundPlayerPacket>();
        app.add_message::<InboundPlayerPacket>();
        app.add_message::<OutboundPlayerAttached>();
        app.add_observer(bridge::attach_outbound_queue);

        // One chain: threads would only add the cost of handing its two systems out.
        app.init_resource::<bridge::PendingConnections>();
        let mut flush = Schedule::new(bridge::OutboundFlush);
        flush.set_executor(SingleThreadedExecutor::new());
        app.add_schedule(flush);
        app.add_systems(
            bridge::OutboundFlush,
            (
                bridge::bridge_outbound,
                bridge::dispatch_encode,
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
                bridge::bridge_inbound_to_channel,
                bridge::bridge_player_attach,
                bridge::forward_pending_inbound,
                online_count::send_online_count,
                latency::forward_latency,
            )
                .chain(),
        );

        app.add_plugins(DisconnectProtocolPlugin);
        app.add_plugins(DimInputsPlugin);
    }
}
