use bevy_ecs::prelude::*;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity_visual::Motion;

pub(crate) fn project_simulated_positions(
    mut entities: Populated<(&MotionState, &mut Motion), Changed<MotionState>>,
) {
    for (MotionState(state), mut motion) in &mut entities {
        let position = state.body.position;
        let on_ground = state.body.flags.on_ground;
        if motion.current.position != position || motion.on_ground != on_ground {
            motion.current.position = position;
            motion.on_ground = on_ground;
        }
    }
}
