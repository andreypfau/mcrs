use glam::DVec3;

/// One point of a stepped path: the position that the entity reaches after `ticks` more ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathStep {
    pub position: DVec3,
    pub ticks: u32,
}

/// The movement that a packet gives, already decoded to absolute positions.
#[derive(Clone, Debug, PartialEq)]
pub enum PacketPath {
    Linear(DVec3),
    /// With `start_immediately`, the path replaces the rest of the current path instead of
    /// following it.
    Stepped {
        steps: Vec<PathStep>,
        start_immediately: bool,
    },
}

impl PacketPath {
    /// The last position of the path, or `None` for a stepped path with no step.
    #[must_use]
    pub fn end(&self) -> Option<DVec3> {
        match self {
            Self::Linear(position) => Some(*position),
            Self::Stepped { steps, .. } => steps.last().map(|step| step.position),
        }
    }
}
