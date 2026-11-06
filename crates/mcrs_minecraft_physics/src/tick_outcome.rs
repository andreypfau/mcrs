/// What one tick did besides its state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TickOutcome {
    /// The tick left the motion state as it was: the next tick can be skipped.
    pub fixed_point: bool,
    /// The tick skipped its physics part.
    pub skipped: bool,
    /// A falling block on the ground tries to become a block.
    pub landed: bool,
    /// An item inside a collision shape, which the server pushes to a free side.
    pub stuck_in_block: bool,
}
