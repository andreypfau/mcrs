use glam::DVec3;

use crate::Look;

/// One of the four move packets of the player. A packet without a position or a rotation has
/// `None` there.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Move {
    pub position: Option<DVec3>,
    pub look: Option<Look>,
    pub on_ground: bool,
    pub horizontal_collision: bool,
}
