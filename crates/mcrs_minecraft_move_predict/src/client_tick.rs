use mcrs_minecraft_move_validate::Move;
use mcrs_minecraft_physics::PlayerInput;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientTick {
    pub input: PlayerInput,
    pub reported: Move,
    pub sprint: Option<bool>,
    pub flight: Option<bool>,
    pub item_use: Option<bool>,
    pub changes: ChangeStates,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChangeStates {
    pub applied: u64,
    pub seen: u64,
}
