use bevy_ecs::prelude::*;

/// The number of the current client tick, which the step gives to each
/// [`StepOutcome`](mcrs_minecraft_engine_entity_motion::StepOutcome) it writes.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MotionTick(pub(crate) u64);

pub(crate) fn advance_motion_tick(mut tick: ResMut<MotionTick>) {
    tick.0 = tick.0.wrapping_add(1);
}
