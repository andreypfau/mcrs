use glam::DVec3;

use crate::Look;

/// The motion of a vehicle that the client controls.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct VehicleMove {
    pub position: DVec3,
    pub look: Look,
    pub on_ground: bool,
}
