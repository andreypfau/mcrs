/// A `level_event` that a landing sends to the players of BRK-BRK-006.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LandingEvent {
    /// The type 2001 with the network id of the carried state, at the cell of the centre of the box
    /// (FBK-LND-005).
    BlockBreak,
    /// The type 1031 with the data 0 at the target cell (FBK-LND-007).
    AnvilLand,
    /// The type 1029 with the data 0 at the target cell (FBK-LND-007).
    AnvilDestroy,
}

impl LandingEvent {
    /// The type of the event on the wire.
    #[must_use]
    pub fn id(self) -> i32 {
        match self {
            Self::BlockBreak => 2001,
            Self::AnvilLand => 1031,
            Self::AnvilDestroy => 1029,
        }
    }
}
