/// The keys of a player in one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Keys {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
    /// The jump key of this tick comes from auto-jump; such a press does not toggle the flight
    /// state.
    pub auto_jump: bool,
}
