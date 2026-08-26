use bevy_ecs::prelude::Entity;
use bytes::Bytes;
use mcrs_voxel_world::session::{MoveId, PlayerSession};
use mcrs_voxel_world::world::channels::DimChannels;
use std::time::Instant;

use crate::world::bus::{
    ArrivalCause, MovePayload, PacketPayload, PacketPriority, PacketTarget, PlayerTransferSnapshot,
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
        /// Dimension resource-location strings sourced from the host's
        /// `LoadedWorldPreset`. The per-dim spawn consumer uses this list to
        /// fill `ClientboundLogin.dimensions` without touching the preset
        /// resource directly (which is not present in a DimWorld).
        dimensions: Vec<String>,
    },
    /// The host signals that a player is leaving this dim (disconnected or
    /// transferred). The dim should despawn the player's in-dim entity.
    /// `session` carries the real `PlayerSession` so the dim can evict the
    /// matching `DimPlayerIndex` entry; the entity despawn itself keys off
    /// `host_anchor`.
    Despawn {
        host_anchor: Entity,
        session: PlayerSession,
    },
    /// Confirmed-move spawn command to the target dim.  Non-sheddable.
    SpawnEntity {
        move_id: MoveId,
        /// The player's newly bumped epoch; ignored for non-player moves.
        epoch: u32,
        cause: ArrivalCause,
        payload: MovePayload,
        player: Option<PlayerSession>,
    },
    /// Confirm that the move completed; source dim may despawn the hidden entity.
    ConfirmMove { move_id: MoveId },
    /// Immediate rollback signal to the source dim (Disconnected or tick-timeout).
    RollbackMove { move_id: MoveId },
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
