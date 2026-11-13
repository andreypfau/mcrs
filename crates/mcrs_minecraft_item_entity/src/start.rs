use glam::DVec3;

/// The position and the velocity that an item entity starts with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Start {
    pub position: DVec3,
    pub velocity: DVec3,
}
