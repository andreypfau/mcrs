/// The attribute values of a living entity that its motion reads, as f64 values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionAttributes {
    pub gravity: f64,
    pub step_height: f64,
    pub movement_speed: f64,
    pub jump_strength: f64,
    pub scale: f64,
    pub sneaking_speed: f64,
    pub movement_efficiency: f64,
    pub bounciness: f64,
    pub friction_modifier: f64,
    pub air_drag_modifier: f64,
}

impl Default for MotionAttributes {
    /// The defaults of the attributes; `movement_speed` is the base of a player.
    fn default() -> Self {
        Self {
            gravity: 0.08,
            step_height: 0.6,
            movement_speed: f64::from(0.1_f32),
            jump_strength: f64::from(0.42_f32),
            scale: 1.0,
            sneaking_speed: 0.3,
            movement_efficiency: 0.0,
            bounciness: 0.0,
            friction_modifier: 1.0,
            air_drag_modifier: 1.0,
        }
    }
}
