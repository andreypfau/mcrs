use bevy_ecs::prelude::Entity;
use bytes::Bytes;
use mcrs_engine::session::PlayerSession;
use mcrs_engine::world::channels::{DimChannels, DimSender};
use mcrs_engine::world::sub_app::DimDespawnQueue;
use std::time::Instant;
use tracing::warn;

use crate::world::bus::{PacketPayload, PacketPriority, PacketTarget, PlayerTransferSnapshot};

/// Host→dim message channel type.
///
/// Carries all traffic from the host (MainWorld) into a dimension world.
/// `Serverbound` is the sole sheddable class; the three control variants
/// (`Spawn`, `Despawn`, `Attach`) are never shed.
#[derive(Clone, Debug)]
pub enum ToDim {
    /// A packet arriving from the network, routed to this dim.
    /// `player` is the host-anchor `Entity`. Sheddable: loss is recoverable
    /// like ordinary network packet loss.
    Serverbound {
        player: Entity,
        id: i32,
        data: Bytes,
        timestamp: Instant,
    },
    /// Spawn a player into this dim (default-reset components; no faithful snapshot).
    Spawn {
        host_anchor: Entity,
        session: PlayerSession,
        snapshot: PlayerTransferSnapshot,
    },
    /// The host signals that a player is leaving this dim (disconnected or
    /// transferred). The dim should despawn the player's in-dim entity.
    Despawn {
        host_anchor: Entity,
    },
    /// Attach/ready signal sent after the player entity is prepared host-side.
    Attach {
        host_anchor: Entity,
        session: PlayerSession,
    },
}

impl ToDim {
    /// Returns `true` if this message may be shed when the channel is at
    /// capacity. Only `Serverbound` is sheddable; lifecycle/control messages
    /// are never shed.
    pub(crate) fn is_sheddable(&self) -> bool {
        matches!(self, ToDim::Serverbound { .. })
    }
}

/// Dim→host message channel type.
///
/// Carries all traffic from a dimension world back to the host (MainWorld).
#[derive(Clone, Debug)]
pub enum FromDim {
    /// A packet to be sent to one or more clients. The `session` and `epoch`
    /// fields are stamped at the outbound boundary before the packet reaches
    /// `bridge_outbound`.
    Clientbound {
        target: PacketTarget,
        priority: PacketPriority,
        data: PacketPayload,
        session: PlayerSession,
        epoch: u32,
    },
    /// The dim requests a (current, unconfirmed) player transfer to another dim.
    Transfer {
        host_anchor: Entity,
        dest_dim: Entity,
        snapshot: PlayerTransferSnapshot,
    },
    /// The dim requests a transfer by dimension name (host resolves the name).
    TransferRequest {
        host_anchor: Entity,
        dim_name: String,
        snapshot: PlayerTransferSnapshot,
    },
    /// The dim has spawned the player entity and reports back the in-dim entity.
    Attached {
        host_anchor: Entity,
        new_in_dim_entity: Entity,
    },
}

/// Convenience alias for the concrete channel registry parameterized by this
/// crate's message types.
pub type DimChannelsResource = DimChannels<ToDim, FromDim>;

/// Send a non-sheddable control message, escalating a saturated control channel
/// to whole-dim teardown.
///
/// The control channel holds the lifecycle reserve (`Spawn`/`Despawn`/`Attach`)
/// that the structural two-channel split guarantees is always available under
/// normal load. A `Full` here therefore means the dim has stopped draining its
/// inbox — hard overload — so its label `Entity` is enqueued into
/// `DimDespawnQueue` for the host runner to tear down rather than silently
/// dropping the lifecycle message. A `Disconnected` channel means the dim is
/// already gone, so there is nothing to tear down.
pub fn send_control_or_teardown(
    sender: &DimSender<ToDim>,
    dim_entity: Entity,
    msg: ToDim,
    despawn_queue: &mut DimDespawnQueue,
) {
    match sender.try_send(msg) {
        Ok(()) => {}
        Err(flume::TrySendError::Full(_)) => {
            warn!(
                dim = ?dim_entity,
                "control channel saturated (hard overload); enqueueing dim teardown"
            );
            if !despawn_queue.0.contains(&dim_entity) {
                despawn_queue.0.push(dim_entity);
            }
        }
        Err(flume::TrySendError::Disconnected(_)) => {}
    }
}
