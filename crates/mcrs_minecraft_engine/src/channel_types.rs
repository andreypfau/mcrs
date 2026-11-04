use crate::bus::{InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn};
use crate::departure::PlayerTransfer;

/// What the host hands a dimension world for its next tick.
///
/// A player is named by its `PlayerSession`, never by an entity of either world, so the two
/// sides could run in different processes. `Serverbound` is the sole sheddable class; the
/// control variants are never shed.
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
    /// A player that changes dimension arrives in this dim, or comes back to it.
    Transfer(Box<PlayerTransfer>),
}
