use glam::DVec3;

/// The state of a mob that its viewers see, at the end of a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MobState {
    pub position: DVec3,
    pub velocity: DVec3,
    /// The yaw of the body in degrees.
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub on_ground: bool,
    /// The horizontal collision flag of the last move (PHY-FLG-001).
    pub horizontal_collision: bool,
    /// The vertical collision flag of the last move (PHY-FLG-001).
    pub vertical_collision: bool,
    /// The entity bounced on a block in this tick (PHY-RST-004).
    pub bounced: bool,
    pub health: f32,
    /// Whether the mob is in the dying state, whose pose is `dying`.
    pub dying: bool,
}
