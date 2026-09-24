use mcrs_minecraft_level::session::{MoveId, PlayerSession};
use mcrs_minecraft_level::world::channels::DimChannels;

use crate::world::bus::{
    ArrivalCause, InboundConfirmMove, InboundEntitySpawn, InboundPlayerDespawn,
    InboundPlayerPacket, InboundPlayerSpawn, InboundRollbackMove, MovePayload,
    OutboundPlayerPacket,
};

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
    Serverbound(InboundPlayerPacket),
    /// Spawn a player into this dim (default-reset components; no faithful snapshot).
    Spawn(InboundPlayerSpawn),
    /// The host signals that a player is leaving this dim (disconnected or
    /// transferred). The dim should despawn the player's in-dim entity.
    /// `session` carries the real `PlayerSession` so the dim can evict the
    /// matching `DimPlayerIndex` entry; the entity despawn itself keys off
    /// `host_anchor`.
    Despawn(InboundPlayerDespawn),
    /// Confirmed-move spawn command to the target dim.  Non-sheddable.
    SpawnEntity(InboundEntitySpawn),
    /// Confirm that the move completed; source dim may despawn the hidden entity.
    ConfirmMove(InboundConfirmMove),
    /// Immediate rollback signal to the source dim (Disconnected or tick-timeout).
    RollbackMove(InboundRollbackMove),
}

/// Dim→host message channel type.
///
/// Carries all traffic from a dimension world back to the host (MainWorld).
#[derive(Clone, Debug)]
pub enum FromDim {
    /// A packet to be sent to one or more clients. The `session` and `epoch`
    /// fields are stamped at the outbound boundary before the packet reaches
    /// `bridge_outbound`.
    Clientbound(OutboundPlayerPacket),
    /// Confirmed-move initiation: source dim requests transfer to another dim by name.
    /// The host resolves the name, inserts into InFlightMoves, and sends SpawnEntity.
    MoveEntity {
        move_id: MoveId,
        /// Destination dimension by name (resolved host-side).
        target: String,
        cause: ArrivalCause,
        payload: MovePayload,
        player: Option<PlayerSession>,
    },
    /// Ack from target dim: the entity has been spawned and arrival resolved.
    Spawned { move_id: MoveId },
}

/// Convenience alias for the concrete channel registry parameterized by this
/// crate's message types.
pub type DimChannelsResource = DimChannels<ToDim, FromDim>;
