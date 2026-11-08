use glam::DVec3;

/// The positions at the start and at the end of the moves of one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MovePath {
    pub start: DVec3,
    pub end: DVec3,
}

impl MovePath {
    /// The path of no length at `position`.
    #[must_use]
    pub const fn at(position: DVec3) -> Self {
        Self {
            start: position,
            end: position,
        }
    }
}
