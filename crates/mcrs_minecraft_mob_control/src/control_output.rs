/// What the controls set for one tick. The fields map one to one to the `MobIntent` of the entity
/// pass: a field that is `None` keeps the value of the row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControlOutput {
    pub steering: Option<ControlSteering>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub head_yaw: Option<f32>,
    pub body_yaw: Option<f32>,
    /// The path ended in this tick: the host sends the fact `PathEnded` to the brain.
    pub path_ended: Option<PathEnd>,
}

/// The `Steering` of the walker row: the input vector, the jump input and the mob speed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControlSteering {
    pub sideways: f32,
    pub vertical: f32,
    pub forward: f32,
    pub jump: bool,
    pub speed: f32,
}

/// Why a path ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathEnd {
    /// The mob reached the last node.
    Done,
    /// The stuck check dropped the path (PATH-FOL-003).
    Stuck,
    /// The timeout dropped the path (PATH-FOL-004).
    TimedOut,
}
