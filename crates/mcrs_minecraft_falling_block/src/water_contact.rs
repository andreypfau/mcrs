use glam::DVec3;

/// Whether a falling block of a concrete powder touches water in this tick (FBK-TCK-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaterContact {
    Dry,
    Wet,
}

/// Where the host looks for the water of a concrete powder (FBK-TCK-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaterSweep {
    /// Only the cell of the falling block.
    Cell,
    /// The cell, and the segment from the position at the start of the tick to the new position.
    Segment,
}

impl WaterSweep {
    /// The sweep for the velocity after the move: the segment when its square length is more
    /// than 1.0.
    #[must_use]
    pub fn of(velocity: DVec3) -> Self {
        if velocity.length_squared() > 1.0 {
            Self::Segment
        } else {
            Self::Cell
        }
    }
}
