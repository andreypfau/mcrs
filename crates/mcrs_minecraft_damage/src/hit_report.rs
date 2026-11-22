use crate::HitOutcome;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitReport {
    pub outcome: HitOutcome,
    pub armor: f32,
    pub absorption: f32,
    pub health: f32,
    pub died: bool,
}

impl HitReport {
    pub(crate) const REJECTED: Self = Self {
        outcome: HitOutcome::Rejected,
        armor: 0.0,
        absorption: 0.0,
        health: 0.0,
        died: false,
    };
}
