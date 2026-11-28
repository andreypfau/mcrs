use glam::DVec3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MovePath {
    pub start: DVec3,
    pub end: DVec3,
}

impl MovePath {
    #[must_use]
    pub const fn at(position: DVec3) -> Self {
        Self {
            start: position,
            end: position,
        }
    }
}
