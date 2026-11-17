use bevy_ecs::prelude::*;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MotionTick(pub(crate) u64);

pub(crate) fn advance_motion_tick(mut tick: ResMut<MotionTick>) {
    tick.0 = tick.0.wrapping_add(1);
}
