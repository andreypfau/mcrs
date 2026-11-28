use glam::DVec3;

use crate::Look;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Move {
    pub position: Option<DVec3>,
    pub look: Option<Look>,
    pub on_ground: bool,
    pub horizontal_collision: bool,
}
