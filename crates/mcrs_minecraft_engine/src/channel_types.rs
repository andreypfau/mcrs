use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::world::channels::DimChannels;

use crate::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerPacket,
};

/// Host→dim message channel type.
///
/// Carries all traffic from the host (MainWorld) into a dimension world.
/// `Serverbound` is the sole sheddable class; the control variants `Spawn` and
/// `Despawn` are never shed.
#[derive(Clone, Debug)]
pub enum ToDim {
    /// A packet arriving from the network, routed to this dim.
    /// `player` is the host-anchor `Entity`. Sheddable: loss is recoverable
    /// like ordinary network packet loss.
    Serverbound(InboundPlayerPacket),
    /// Spawn a player into this dim (default-reset components; no faithful snapshot).
    Spawn(InboundPlayerSpawn),
    /// The host signals that a player disconnected. The dim should despawn the
    /// player's in-dim entity.
    /// `session` carries the real `PlayerSession` so the dim can evict the
    /// matching `DimPlayerIndex` entry; the entity despawn itself keys off
    /// `host_anchor`.
    Despawn(InboundPlayerDespawn),
}

/// Dim→host message channel type.
///
/// Carries all traffic from a dimension world back to the host (MainWorld).
#[derive(Clone, Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "nearly every message is a packet, so a box would allocate once per packet"
)]
pub enum FromDim {
    /// A packet to be sent to one or more clients. The `session` and `epoch`
    /// fields are stamped at the outbound boundary before the packet reaches
    /// `bridge_outbound`.
    Clientbound(OutboundPlayerPacket),
    /// The dim has handled a `Despawn` for the session: whatever player it held is saved and gone.
    Released { session: PlayerSession },
}

/// Convenience alias for the concrete channel registry parameterized by this
/// crate's message types.
pub type DimChannelsResource = DimChannels<ToDim, FromDim>;
