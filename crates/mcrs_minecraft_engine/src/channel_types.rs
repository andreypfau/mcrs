use crate::bus::{InboundPlayerDespawn, InboundPlayerSpawn};
use crate::command_handoff::CommandHandoff;
use crate::departure::PlayerTransfer;
use crate::player_order::OrderedPlayer;

#[derive(Clone, Debug)]
pub enum ToDim {
    Spawn(InboundPlayerSpawn),
    Despawn(InboundPlayerDespawn),
    Transfer(Box<PlayerTransfer>),
    Order(OrderedPlayer),
    Command(Box<CommandHandoff>),
}
