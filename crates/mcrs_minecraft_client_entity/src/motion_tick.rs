use bevy_ecs::prelude::*;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MotionTick(pub(crate) u64);

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "the motion clock advances on every fixed tick, also when no entity steps"
    )
)]
pub(crate) fn advance_motion_tick(mut tick: ResMut<MotionTick>) {
    let next = MotionTick(tick.0.wrapping_add(1));
    tick.set_if_neq(next);
}
