use glam::DVec3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MobState {
    pub position: DVec3,
    pub velocity: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub bounced: bool,
    pub health: f32,
    pub dying: bool,
}
