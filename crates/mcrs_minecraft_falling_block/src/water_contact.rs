use glam::DVec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaterContact {
    Dry,
    Wet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaterSweep {
    Cell,
    Segment,
}

impl WaterSweep {
    #[must_use]
    pub fn of(velocity: DVec3) -> Self {
        if velocity.length_squared() > 1.0 {
            Self::Segment
        } else {
            Self::Cell
        }
    }
}
