use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_engine_world::world::channels::DimChannels;

use crate::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerPacket,
};

/// Host→dim message channel type.
///
/// Carries all traffic from the host (MainWorld) into a dimension world. A player is named by
/// its `PlayerSession`, never by an entity of either world, so the two sides could run in
/// different processes.
/// `Serverbound` is the sole sheddable class; the control variants `Spawn` and
/// `Despawn` are never shed.
#[derive(Clone, Debug)]
pub enum ToDim {
    /// A packet arriving from the network, routed to this dim. Sheddable: loss is recoverable
    /// like ordinary network packet loss.
    Serverbound(InboundPlayerPacket),
    /// Spawn a player into this dim (default-reset components; no faithful snapshot).
    Spawn(InboundPlayerSpawn),
    /// The host signals that a player disconnected. The dim should despawn the
    /// player's in-dim entity, which it finds through its `DimPlayerIndex`.
    Despawn(InboundPlayerDespawn),
}

/// Dim→host message channel type.
///
/// Carries all traffic from a dimension world back to the host (MainWorld).
#[derive(Clone, Debug)]
pub enum FromDim {
    /// The packets of one drain, in the order the dimension wrote them, each to one or more
    /// clients. The `session` and `epoch` fields are stamped at the outbound boundary before
    /// the packets reach `bridge_outbound`.
    Clientbound(Vec<OutboundPlayerPacket>),
    /// The dim has handled a `Despawn` for the session: whatever player it held is saved and gone.
    Released { session: PlayerSession },
}

/// Convenience alias for the concrete channel registry parameterized by this
/// crate's message types.
pub type DimChannelsResource = DimChannels<ToDim, FromDim>;
