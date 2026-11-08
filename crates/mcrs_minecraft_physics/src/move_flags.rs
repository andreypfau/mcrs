/// The five result flags of a move.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MoveFlags {
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub below_collision: bool,
    /// A horizontal collision at a small angle to the intended direction; only the player on its
    /// own client sets it.
    pub minor_collision: bool,
}
