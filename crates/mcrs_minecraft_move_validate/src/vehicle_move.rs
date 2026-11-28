use glam::DVec3;

use crate::Look;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct VehicleMove {
    pub position: DVec3,
    pub look: Look,
    pub on_ground: bool,
}
