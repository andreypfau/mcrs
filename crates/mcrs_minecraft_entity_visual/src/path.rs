use glam::DVec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathStep {
    pub position: DVec3,
    pub ticks: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PacketPath {
    Linear(DVec3),
    Stepped {
        steps: Vec<PathStep>,
        start_immediately: bool,
    },
}

impl PacketPath {
    #[must_use]
    pub fn end(&self) -> Option<DVec3> {
        match self {
            Self::Linear(position) => Some(*position),
            Self::Stepped { steps, .. } => steps.last().map(|step| step.position),
        }
    }
}
