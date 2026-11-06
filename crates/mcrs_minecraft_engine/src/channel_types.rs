use crate::bus::{InboundPlayerDespawn, InboundPlayerSpawn};
use crate::command_handoff::CommandHandoff;
use crate::departure::PlayerTransfer;
use crate::player_order::OrderedPlayer;

/// A lifecycle message that the host hands a dimension world for its next tick. It is never
/// shed, unlike a serverbound packet.
///
/// A player is named by its `PlayerSession`, never by an entity of either world, so the two
/// sides could run in different processes.
#[derive(Clone, Debug)]
pub enum ToDim {
    /// Spawn a player into this dim (default-reset components; no faithful snapshot).
    Spawn(InboundPlayerSpawn),
    /// The host signals that a player disconnected. The dim should despawn the
    /// player's in-dim entity, which it finds through its `DimPlayerIndex`.
    Despawn(InboundPlayerDespawn),
    /// A player that changes dimension arrives in this dim, or comes back to it.
    Transfer(Box<PlayerTransfer>),
    /// A command of another dimension changes a player of this dim.
    Order(OrderedPlayer),
    /// The rest of a command run that reached a stage which reads this dim.
    Command(Box<CommandHandoff>),
}
