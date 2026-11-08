use glam::DVec3;

use crate::Look;

/// The confirmation of a teleport, with the position and the rotation the client reports.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Accept {
    pub teleport_id: i32,
    pub position: DVec3,
    pub look: Look,
}
