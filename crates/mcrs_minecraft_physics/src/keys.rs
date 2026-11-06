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
    /// The jump key changed to down in this tick, not by auto-jump: the press that toggles the
    /// flight state.
    pub jump_pressed: bool,
}
