/// What one tick did besides its state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TickOutcome {
    /// A falling block on the ground tries to become a block.
    pub landed: bool,
    /// An item inside a collision shape, which the server pushes to a free side.
    pub stuck_in_block: bool,
}
