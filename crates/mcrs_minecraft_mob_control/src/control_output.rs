#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControlOutput {
    pub steering: Option<ControlSteering>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub head_yaw: Option<f32>,
    pub body_yaw: Option<f32>,
    pub path_ended: Option<PathEnd>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ControlSteering {
    pub sideways: f32,
    pub vertical: f32,
    pub forward: f32,
    pub jump: bool,
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathEnd {
    Done,
    Stuck,
    TimedOut,
}
