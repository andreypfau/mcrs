use mcrs_minecraft_move_validate::Move;
use mcrs_minecraft_physics::PlayerInput;

/// What layer 1 hands on for one client tick of a player.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientTick {
    /// The keys of the last `player_input`, the rotation of the last move packet with one, and
    /// the facts of the player. The push is ignored: the prediction bounds it.
    pub input: PlayerInput,
    /// The move packet of the tick; without a position for a silent tick. The flags are the
    /// flags of the last move packet.
    pub reported: Move,
    /// The sprint state of a `player_command` with action 1 or 2 in the tick.
    pub sprint: Option<bool>,
    /// The flight state of a serverbound `player_abilities` in the tick.
    pub flight: Option<bool>,
    /// The item use after the tick packets, when a use starts or ends in the tick.
    pub item_use: Option<bool>,
    pub changes: ChangeStates,
}

/// The state of each change for one client tick. Changes are numbered from 0 in the order of
/// the writes, and a later change never has a later state than an earlier one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChangeStates {
    /// The changes below this number are applied.
    pub applied: u64,
    /// The changes from `applied` up to below this number are in their window.
    pub seen: u64,
}
