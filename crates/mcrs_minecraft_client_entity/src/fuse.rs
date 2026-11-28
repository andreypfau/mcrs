use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_entity_motion::StepOutcome;

use crate::motion_tick::MotionTick;

pub(crate) const FUSE_INDEX: u8 = 8;
const DEFAULT_FUSE: i32 = 80;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fuse(pub i32);

impl Default for Fuse {
    fn default() -> Self {
        Self(DEFAULT_FUSE)
    }
}

pub(crate) fn burn_fuses(tick: Res<MotionTick>, mut fuses: Populated<(&StepOutcome, &mut Fuse)>) {
    for (outcome, mut fuse) in &mut fuses {
        if outcome.is_of(tick.0) {
            fuse.set_if_neq(Fuse(fuse.0.saturating_sub(1)));
        }
    }
}
